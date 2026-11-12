<script setup lang="ts">
import type { LyricsLine } from '~/api/types'

// Текст песни в правой панели и в листе на телефоне. Синхронный — текущая строка крупно и ярко,
// спетое приглушено, лента сама держит строку на трети высоты. Человек прокрутил сам — лента
// ждёт несколько секунд (и показывает «Вернуться к текущей строке»). Нажатие на строку — перемотка.
// Позиция — из плеера, поэтому работает и пульт (там она приходит снимками другого устройства)
const props = withDefaults(defineProps<{ variant?: 'panel' | 'sheet' }>(), { variant: 'panel' })

const player = usePlayerStore()
const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

/** Строку подсвечиваем чуть раньше: глаз успевает к началу пения */
const LEAD_MS = 250
/** Сколько лента ждёт после прокрутки рукой */
const PAUSE_MS = 5000

const trackId = computed(() => player.current?.track.id ?? null)
const { state, retry } = useTrackLyrics(trackId)
const data = computed(() => (state.value.status === 'ready' ? state.value.data : null))
const lines = computed<LyricsLine[] | null>(() => (data.value?.kind === 'synced' && data.value.lines.length ? data.value.lines : null))
const stanzas = computed(() => {
  if (data.value?.kind !== 'plain' || !data.value.plain) return []
  return data.value.plain.split(/\n\s*\n/).map(s => s.split('\n').map(l => l.trim()).filter(Boolean)).filter(s => s.length)
})

const clock = usePlaybackClock(computed(() => !!lines.value))
const current = computed(() => (lines.value ? lineAt(lines.value, clock.value + LEAD_MS) : -1))

// ---------- Лента ----------
const root = ref<HTMLElement | null>(null)
const lineEls = ref<HTMLElement[]>([])
let scroller: HTMLElement | null = null
const following = ref(true)
const pillBottom = ref(16)
/** Текущая строка выше видимой части: стрелка на кнопке — вверх */
const pillUp = ref(false)
let trackChanged = false
let resumeTimer: ReturnType<typeof setTimeout> | undefined
let settleTimer: ReturnType<typeof setTimeout> | undefined

function findScroller(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const o = getComputedStyle(p).overflowY
    if (o === 'auto' || o === 'scroll') return p
  }
  return null
}

/** Видимая часть ленты: лист на телефоне бывает открыт до половины, низ — за краем экрана */
function visibleBox(s: HTMLElement) {
  const r = s.getBoundingClientRect()
  const top = Math.max(r.top, 0)
  const bottom = Math.min(r.bottom, window.innerHeight)
  return { top, height: Math.max(0, bottom - top), hidden: Math.max(0, r.bottom - window.innerHeight) }
}

function scrollToCurrent(smooth: boolean) {
  const s = scroller
  if (!s || !lines.value) return
  const el = current.value >= 0 ? lineEls.value[current.value] : null
  if (!el) {
    s.scrollTo({ top: 0, behavior: smooth && !ui.reducedMotion ? 'smooth' : 'auto' })
    return
  }
  const box = visibleBox(s)
  const top = s.scrollTop + el.getBoundingClientRect().top - box.top - box.height / 3
  s.scrollTo({ top: Math.max(0, top), behavior: smooth && !ui.reducedMotion ? 'smooth' : 'auto' })
}

function pause() {
  if (!lines.value) return
  following.value = false
  if (scroller) pillBottom.value = 16 + visibleBox(scroller).hidden
  onScroll()
  clearTimeout(resumeTimer)
  resumeTimer = setTimeout(resume, PAUSE_MS)
}

/** Пока лента стоит, стрелка на кнопке смотрит туда, где текущая строка */
function onScroll() {
  if (following.value || !scroller) return
  const el = lineEls.value[current.value]
  pillUp.value = !!el && el.getBoundingClientRect().bottom < visibleBox(scroller).top
}

function resume() {
  clearTimeout(resumeTimer)
  following.value = true
  scrollToCurrent(true)
}

// Намерение человека — колесо, палец, клавиши; программная прокрутка эти события не шлёт
const onKey = (e: KeyboardEvent) => {
  if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(e.key)) pause()
}
function attach() {
  detach()
  if (!root.value) return
  scroller = findScroller(root.value)
  if (!scroller) return
  scroller.addEventListener('wheel', pause, { passive: true })
  scroller.addEventListener('touchmove', pause, { passive: true })
  scroller.addEventListener('keydown', onKey)
  scroller.addEventListener('scroll', onScroll, { passive: true })
}
function detach() {
  if (!scroller) return
  scroller.removeEventListener('wheel', pause)
  scroller.removeEventListener('touchmove', pause)
  scroller.removeEventListener('keydown', onKey)
  scroller.removeEventListener('scroll', onScroll)
  scroller = null
}

onMounted(attach)
onBeforeUnmount(() => {
  detach()
  clearTimeout(resumeTimer)
  clearTimeout(settleTimer)
})

watch(current, () => {
  if (following.value) scrollToCurrent(true)
})

// Новый текст — сразу на место, без проезда по всей песне
watch(lines, async (v, old) => {
  following.value = true
  clearTimeout(resumeTimer)
  if (!v) return
  await nextTick()
  if (!scroller) attach()
  if (old !== undefined && scroller && trackChanged) scroller.scrollTop = 0
  trackChanged = false
  scrollToCurrent(false)
  // Лист на телефоне ещё выезжает: видимая часть известна, когда он встанет
  clearTimeout(settleTimer)
  settleTimer = setTimeout(() => {
    if (following.value) scrollToCurrent(false)
  }, 450)
}, { flush: 'post', immediate: true })

watch(trackId, () => {
  trackChanged = true
  if (scroller) scroller.scrollTop = 0
})

function seekTo(i: number) {
  const line = lines.value?.[i]
  if (!line) return
  player.seek(line.tMs)
  following.value = true
  clearTimeout(resumeTimer)
}

function lineClass(i: number) {
  const c = current.value
  return { 'is-current': i === c, 'is-past': i < c }
}

// Ширины строк-заглушек: как у настоящего текста, разной длины
const SKELETON = ['74%', '58%', '82%', '46%', '68%', '77%', '52%', '64%']
</script>

<template>
  <div ref="root" class="ly" :class="`ly--${props.variant}`">
    <UiEmptyState v-if="!player.current" icon="ph:microphone-stage" :title="tg('player.nothing')" compact />

    <div v-else-if="state.status === 'loading' || state.status === 'idle'" class="ly__skeleton" :aria-label="t('loading')" role="status">
      <UiSkeleton v-for="w in SKELETON" :key="w" variant="line" :width="w" class="ly__skel" />
    </div>

    <UiEmptyState v-else-if="state.status === 'error'" icon="ph:microphone-stage" :title="t('failed')" compact>
      <UiButton size="sm" variant="secondary" @click="retry">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>

    <UiEmptyState v-else-if="data?.kind === 'instrumental'" icon="ph:music-notes-simple" :title="t('instrumental')" compact />

    <div v-else-if="lines" class="ly__lines">
      <button
        v-for="(line, i) in lines"
        :key="i"
        ref="lineEls"
        type="button"
        class="ly__line"
        :class="lineClass(i)"
        :aria-current="i === current ? 'true' : undefined"
        @click="seekTo(i)"
      >
        <span v-if="line.text" class="ly__text">{{ line.text }}</span>
        <span v-else class="ly__gap" :aria-label="t('break')"><i /><i /><i /></span>
      </button>
      <div class="ly__pillbar" :style="{ bottom: `${pillBottom}px` }">
        <Transition name="ly-pill">
          <button v-if="!following" type="button" class="ly__pill" @click="resume">
            <Icon name="ph:arrow-down" class="ly__pill-icon" :class="{ 'is-up': pillUp }" />
            {{ t('back') }}
          </button>
        </Transition>
      </div>
    </div>

    <div v-else-if="stanzas.length" class="ly__plain">
      <p v-for="(s, i) in stanzas" :key="i" class="ly__stanza">
        <template v-for="(l, j) in s" :key="j">{{ l }}<br v-if="j < s.length - 1"></template>
      </p>
    </div>

    <UiEmptyState v-else icon="ph:microphone-stage" :title="t('none')" compact />
  </div>
</template>

<style scoped>
.ly { --ly-size: 22px; --ly-gap: 6px; position: relative; padding: 12px 8px 0; }
.ly--sheet { --ly-size: 26px; --ly-gap: 8px; padding: 4px 8px 0; }

/* ---------- Синхронный текст ---------- */
.ly__lines { display: flex; flex-direction: column; align-items: stretch; padding-bottom: 45vh; }
.ly__line {
  display: block;
  width: 100%;
  margin: 0;
  padding: var(--ly-gap) 8px;
  border: 0;
  border-radius: var(--r-2);
  background: transparent;
  color: var(--text-2);
  font: inherit;
  font-size: var(--ly-size);
  font-weight: 700;
  letter-spacing: -0.015em;
  line-height: 1.28;
  text-align: left;
  text-wrap: pretty;
  cursor: pointer;
  transform: scale(0.94);
  transform-origin: left center;
  transition: color var(--dur-3) var(--ease-out), transform var(--dur-3) var(--ease-out);
}
.ly__line.is-past { color: var(--text-4); }
.ly__line.is-current { color: var(--text-1); transform: none; }
.ly__line:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
@media (hover: hover) {
  .ly__line:hover { background: var(--fill-1); }
}
.ly__text { display: block; }
.ly__gap { display: inline-flex; gap: 6px; height: calc(var(--ly-size) * 1.28); align-items: center; }
.ly__gap i { width: 7px; height: 7px; border-radius: 50%; background: currentColor; }
.ly__line.is-current .ly__gap i { animation: ly-dot 1.4s var(--ease-in-out) infinite; }
.ly__line.is-current .ly__gap i:nth-child(2) { animation-delay: 0.2s; }
.ly__line.is-current .ly__gap i:nth-child(3) { animation-delay: 0.4s; }
@keyframes ly-dot {
  0%, 100% { opacity: 0.35; transform: scale(0.8); }
  50% { opacity: 1; transform: none; }
}

/* «Вернуться к текущей строке»: прилипает к низу видимой части ленты */
.ly__pillbar { position: sticky; z-index: 1; display: flex; justify-content: center; height: 0; pointer-events: none; }
.ly__pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 14px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--text-1);
  box-shadow: var(--shadow-1);
  color: var(--on-light);
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 600;
  white-space: nowrap;
  cursor: pointer;
  pointer-events: auto;
  transform: translateY(-100%);
}
.ly__pill:hover { background: #fff; }
.ly__pill:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.ly__pill-icon { font-size: 15px; }
.ly__pill-icon.is-up { transform: rotate(180deg); }
.ly-pill-enter-active, .ly-pill-leave-active { transition: opacity var(--dur-2) var(--ease-out); }
.ly-pill-enter-from, .ly-pill-leave-to { opacity: 0; }
@media (pointer: coarse) {
  .ly__pill { height: 44px; padding: 0 18px; }
}

/* ---------- Простой текст ---------- */
.ly__plain { display: flex; flex-direction: column; gap: 18px; padding: 4px 8px 32px; }
.ly__stanza { margin: 0; color: var(--text-2); font-size: 17px; font-weight: 600; line-height: 1.5; text-wrap: pretty; }
.ly--sheet .ly__stanza { font-size: 19px; }

/* ---------- Загрузка ---------- */
.ly__skeleton { display: flex; flex-direction: column; gap: 22px; padding: 14px 8px; }
.ly__skel { height: 18px; border-radius: 9px; }
.ly--sheet .ly__skel { height: 22px; border-radius: 11px; }

@media (prefers-reduced-motion: reduce) {
  .ly__line { transition: none; }
  .ly__line.is-current .ly__gap i { animation: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "loading": "Загружаем текст",
    "failed": "Текст не загрузился",
    "none": "Текста нет",
    "instrumental": "Без слов",
    "break": "Проигрыш",
    "back": "Вернуться к текущей строке"
  },
  "en": {
    "loading": "Loading lyrics",
    "failed": "Couldn’t load lyrics",
    "none": "No lyrics",
    "instrumental": "Instrumental",
    "break": "Instrumental break",
    "back": "Back to current line"
  }
}
</i18n>
