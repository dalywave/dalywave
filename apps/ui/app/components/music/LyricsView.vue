<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'
import type { LyricItem, LyricLine } from '~/utils/lyrics'

// Текст песни «сценой» (вариант А): крупные жирные строки, текущая яркая, спетые и будущие
// приглушены и чуть размыты. Лента едет пружиной, строки ниже догоняют её «лесенкой». Слова текущей
// строки загораются по оценке слогов (правило — utils/lyrics.ts, wordMode), проигрыш — три дышащие
// точки, подпевка в скобках — отдельной строкой. Нажатие на строку — перемотка; прокрутили сами —
// лента стоит, есть «К текущей строке», через 6 с она возвращается. Долгое нажатие или правый
// щелчок — поделиться строкой. Текст без времени — строфами с обычной прокруткой, повторы припева
// свёрнуты.
// stage — тёмная сцена плеера (телефон, iPad, плеер на весь экран), panel — правая панель на
// компьютере, по теме. Кегль — от ширины колонки (container query). Позиция — из плеера, поэтому
// работает и пульт
const props = withDefaults(defineProps<{
  variant?: 'panel' | 'stage'
  /** Где стоит текущая строка: доля высоты (≤ 1) или пиксели от верха */
  anchor?: number
  /** Показан ли текст сейчас: скрытый не крутит часы */
  active?: boolean
}>(), { variant: 'panel', anchor: 0.3, active: true })

const emit = defineEmits<{
  /** Человек трогает текст: колесо, палец, клавиши */
  interact: []
  following: [value: boolean]
}>()

const player = usePlayerStore()
const ui = useUiStore()
const { share } = useShare()
const { t } = useScopedI18n('components.music.LyricsView')
const { tg } = useGlobalT()

const track = computed(() => player.current?.track ?? null)
const trackId = computed(() => track.value?.id ?? null)
const { state, retry } = useTrackLyrics(trackId)
const data = computed(() => (state.value.status === 'ready' ? state.value.data : null))
const items = computed<LyricItem[]>(() => {
  if (data.value?.kind !== 'synced' || !data.value.lines.length) return []
  return buildLyrics(data.value.lines, player.durationMs || track.value?.durationMs || 0)
})
const synced = computed(() => items.value.length > 0)
const stanzas = computed(() => (data.value?.kind === 'plain' ? plainStanzas(data.value.plain) : []))

const clock = usePlaybackClock(computed(() => synced.value && props.active))
const current = computed(() => (synced.value ? lyricAt(items.value, clock.value) : -1))
const reduced = computed(() => ui.reducedMotion)

// ---------- Лента ----------
const view = ref<HTMLElement | null>(null)
const list = ref<HTMLElement | null>(null)
const following = ref(true)
const off = ref(0)
/** Без проезда: первая расстановка, новый трек, прыжок через полпесни */
const instant = ref(true)
/** Верх и высота каждого пункта в ленте — меряем после отрисовки и при смене размера */
let tops: number[] = []
let heights: number[] = []
let viewH = 0
const measured = ref(0)
let resumeTimer: ReturnType<typeof setTimeout> | undefined

const anchorPx = () => (props.anchor <= 1 ? viewH * props.anchor : props.anchor)

function measure() {
  const l = list.value
  const v = view.value
  if (!l || !v) return
  viewH = v.clientHeight
  const els = l.children
  tops = []
  heights = []
  for (let i = 0; i < els.length; i++) {
    const el = els[i] as HTMLElement
    tops.push(el.offsetTop)
    heights.push(el.offsetHeight)
  }
  measured.value++
}

function targetOff(i: number) {
  return (tops[Math.max(0, i)] ?? 0) - anchorPx()
}

function place(smooth: boolean) {
  if (!following.value) return
  if (!smooth || reduced.value) {
    instant.value = true
    requestAnimationFrame(() => requestAnimationFrame(() => (instant.value = false)))
  }
  off.value = targetOff(current.value)
}

/** Видимые пункты (с запасом): только им — пружина, размытие и свой слой */
const near = computed(() => {
  void measured.value
  const top = off.value - viewH * 0.6
  const bottom = off.value + viewH * 1.6
  let a = -1
  let b = -1
  for (let i = 0; i < tops.length; i++) {
    const y = tops[i]!
    if (y + heights[i]! >= top && a < 0) a = i
    if (y <= bottom) b = i
  }
  return [Math.max(0, a), b] as const
})

function lineStyle(i: number) {
  const [a, b] = near.value
  if (i < a || i > b) return undefined
  const d = i - current.value
  const dist = Math.abs(d)
  const blur = following.value && !reduced.value && dist > 0 ? Math.min(2.6, 0.45 + dist * 0.5) * (props.variant === 'panel' ? 0.7 : 1) : 0
  return {
    '--d': `${d < -1 ? 0 : Math.min(8, Math.max(0, d + 1)) * 42}ms`,
    '--blur': `${blur.toFixed(2)}px`,
  }
}

function lineClass(it: LyricItem) {
  const [a, b] = near.value
  const i = it.i
  return {
    'is-active': i === current.value,
    'is-past': i < current.value,
    'is-far': i < a || i > b,
    'is-whole': it.type === 'line' && it.mode === 'line',
    'is-back': it.type === 'line' && it.onlyBacking,
  }
}

// ---------- Рука: колесо, палец с инерцией, клавиши ----------
function setFollowing(v: boolean) {
  if (following.value === v) return
  following.value = v
  emit('following', v)
}

function clampOff(v: number) {
  const max = (list.value?.scrollHeight ?? 0) - viewH * 0.5
  return Math.min(Math.max(v, -anchorPx()), Math.max(max, -anchorPx()))
}

function manual(dy: number) {
  setFollowing(false)
  off.value = clampOff(off.value + dy)
  emit('interact')
  clearTimeout(resumeTimer)
  resumeTimer = setTimeout(resume, LYRICS_RESUME_MS)
}

function resume() {
  clearTimeout(resumeTimer)
  stopFling()
  setFollowing(true)
  place(true)
}

const pillUp = computed(() => {
  void measured.value
  const i = Math.max(0, current.value)
  return (tops[i] ?? 0) + (heights[i] ?? 0) < off.value
})

function onWheel(e: WheelEvent) {
  if (!synced.value) return
  e.preventDefault()
  manual(e.deltaMode === 1 ? e.deltaY * 32 : e.deltaY)
}

let drag: { id: number, y: number, lastY: number, lastT: number, v: number, moved: boolean } | null = null
let fling = 0
/** Только что тянули: щелчок по строке после этого — не перемотка */
let dragged = false

function stopFling() {
  if (fling) cancelAnimationFrame(fling)
  fling = 0
}

function onDown(e: PointerEvent) {
  emit('interact')
  if (!synced.value || !e.isPrimary || (e.pointerType === 'mouse' && e.button !== 0)) return
  stopFling()
  dragged = false
  drag = { id: e.pointerId, y: e.clientY, lastY: e.clientY, lastT: e.timeStamp, v: 0, moved: false }
}

function onMove(e: PointerEvent) {
  const g = drag
  if (!g || e.pointerId !== g.id) return
  if (!g.moved) {
    if (Math.abs(e.clientY - g.y) < 6) return
    g.moved = true
    dragged = true
    try {
      view.value?.setPointerCapture(e.pointerId)
    }
    catch { /* касание уже кончилось */ }
  }
  const dy = g.lastY - e.clientY
  const dt = Math.max(1, e.timeStamp - g.lastT)
  g.v = 0.75 * (dy / dt) + 0.25 * g.v
  g.lastY = e.clientY
  g.lastT = e.timeStamp
  manual(dy)
}

function onUp(e: PointerEvent) {
  const g = drag
  if (!g || e.pointerId !== g.id) return
  drag = null
  if (!g.moved) return
  setTimeout(() => (dragged = false), 300)
  // Инерция: лента докатывается и тормозит, как обычная прокрутка
  let v = g.v * 16
  if (Math.abs(v) < 1 || reduced.value) return
  const step = () => {
    v *= 0.94
    if (Math.abs(v) < 0.4) {
      fling = 0
      return
    }
    manual(v)
    fling = requestAnimationFrame(step)
  }
  fling = requestAnimationFrame(step)
}

function onKey(e: KeyboardEvent) {
  const step = { ArrowUp: -80, ArrowDown: 80, PageUp: -viewH * 0.8, PageDown: viewH * 0.8 }[e.key]
  if (step == null || !synced.value) return
  e.preventDefault()
  manual(step)
}

function seekTo(it: LyricItem) {
  if (dragged || menuJustOpened) return
  player.seek(it.tMs)
  stopFling()
  setFollowing(true)
  clearTimeout(resumeTimer)
}

// ---------- Слова и точки: каждый кадр трогаем только текущий пункт ----------
let litEls: { el: HTMLElement, s: number, e: number }[] = []
let dotEls: HTMLElement[] = []
let brkEl: HTMLElement | null = null

function bindActive(i: number) {
  litEls = []
  dotEls = []
  brkEl = null
  const it = items.value[i]
  const el = list.value?.children[i] as HTMLElement | undefined
  if (!it || !el) return
  if (it.type === 'line' && it.mode === 'words') {
    const words = [...it.words, ...(it.backWords ?? [])]
    el.querySelectorAll<HTMLElement>('.w').forEach((w, k) => {
      const word = words[k]
      if (word) litEls.push({ el: w, s: word.s, e: word.e })
    })
  }
  else if (it.type === 'break') {
    brkEl = el
    dotEls = [...el.querySelectorAll<HTMLElement>('i')]
  }
}

function frame(ms: number) {
  for (const w of litEls) w.el.style.setProperty('--p', wordProgress({ w: '', s: w.s, e: w.e }, ms).toFixed(3))
  const it = items.value[current.value]
  if (brkEl && it?.type === 'break') {
    const p = lyricProgress(it, ms)
    dotEls.forEach((d, k) => d.style.setProperty('--f', Math.min(1, Math.max(0, p * 3 - k)).toFixed(3)))
    brkEl.classList.toggle('is-ending', it.endMs - ms < 650)
  }
}

watch(clock, ms => frame(ms))

watch(current, (i, prev) => {
  // Ушедшая строка: слова остаются «загоревшимися», пока она гаснет
  for (const w of litEls) w.el.style.setProperty('--p', '1')
  void nextTick(() => {
    bindActive(i)
    frame(clock.value)
  })
  place(prev == null || Math.abs(i - prev) < 6)
})

// ---------- Размеры и новый текст ----------
let ro: ResizeObserver | null = null
onMounted(() => {
  ro = new ResizeObserver(() => {
    measure()
    // Высота окна меняется плавно (управление на телефоне уходит) — строка стоит, пока цель та же
    if (following.value && Math.abs(targetOff(current.value) - off.value) > 1) {
      instant.value = true
      off.value = targetOff(current.value)
      requestAnimationFrame(() => requestAnimationFrame(() => (instant.value = false)))
    }
  })
})
watch([view, list], ([v, l], old) => {
  for (const el of old ?? []) if (el) ro?.unobserve(el)
  if (v) ro?.observe(v)
  if (l) ro?.observe(l)
}, { flush: 'post' })

watch(items, async () => {
  clearTimeout(resumeTimer)
  stopFling()
  setFollowing(true)
  await nextTick()
  measure()
  place(false)
  bindActive(current.value)
  frame(clock.value)
}, { flush: 'post', immediate: true })

// Показали снова (лист, вкладка): строка на месте без проезда
watch(() => props.active, async (on) => {
  if (!on) return
  await nextTick()
  measure()
  setFollowing(true)
  place(false)
})

onBeforeUnmount(() => {
  ro?.disconnect()
  clearTimeout(resumeTimer)
  stopFling()
})

// ---------- Поделиться строкой ----------
const menuLine = ref<LyricLine | null>(null)
let menuJustOpened = false
function onMenuOpen(target: EventTarget | null) {
  const el = (target as HTMLElement | null)?.closest?.('[data-i]') as HTMLElement | null
  const it = el ? items.value[Number(el.dataset.i)] : null
  menuLine.value = it?.type === 'line' ? it : null
  // Долгое нажатие пальцем: отпускание — не перемотка
  menuJustOpened = true
  setTimeout(() => (menuJustOpened = false), 600)
}
const menuItems = computed<MenuEntry[]>(() => {
  const it = menuLine.value
  const tr = track.value
  if (!it || !tr) return []
  const quote = lineQuote(it)
  return [
    { label: t('shareLine'), icon: 'ph:export', onSelect: () => void share('track', tr.id, tr.title, quote) },
    { label: t('copyLine'), icon: 'ph:copy', onSelect: () => void share('track', tr.id, tr.title, quote, { copyOnly: true }) },
  ]
})
/** Строка и следующая за ней (если она сразу дальше), с подписью трека */
function lineQuote(it: LyricLine) {
  const next = items.value[it.i + 1]
  const lines = [it.text]
  if (next?.type === 'line') lines.push(next.text)
  const tr = track.value!
  return `${lines.join('\n')}\n— ${tr.artists.map(a => a.name).join(', ')}, «${tr.title}»`
}

// Ширины строк-заглушек: как у настоящего текста, разной длины
const SKELETON = ['74%', '58%', '82%', '46%', '68%', '77%']

/** Пружина как CSS linear(): мягкая, с маленьким перелётом (как в прототипе А) */
const SPRING = (() => {
  let x = 0
  let v = 0
  let time = 0
  const dt = 1 / 120
  const pts: number[] = []
  while (time < 2.5) {
    v += (140 * (1 - x) - 19 * v) * dt
    x += v * dt
    time += dt
    pts.push(x)
    if (time > 0.3 && Math.abs(1 - x) < 0.0015 && Math.abs(v) < 0.02) break
  }
  const step = Math.max(1, Math.round(pts.length / 40))
  const s = pts.filter((_, i) => i % step === 0).map(p => p.toFixed(3))
  return { ease: `linear(0, ${s.join(', ')}, 1)`, ms: `${Math.round(time * 1000)}ms` }
})()
const springVars = { '--ly-ease': SPRING.ease, '--ly-ms': SPRING.ms }

defineExpose({ synced })
</script>

<template>
  <div class="ly" :class="[`ly--${props.variant}`, { 'is-reduced': reduced }]" :style="springVars">
    <div v-if="!player.current" class="ly__state">
      <UiEmptyState icon="ph:microphone-stage" :title="tg('player.nothing')" compact />
    </div>

    <div v-else-if="state.status === 'loading' || state.status === 'idle'" class="ly__skeleton" :aria-label="t('loading')" role="status">
      <UiSkeleton v-for="w in SKELETON" :key="w" variant="line" :width="w" class="ly__skel" />
    </div>

    <div v-else-if="state.status === 'error'" class="ly__state">
      <UiEmptyState icon="ph:microphone-stage" :title="t('failed')" compact>
        <UiButton size="sm" variant="secondary" @click="retry">{{ tg('common.retry') }}</UiButton>
      </UiEmptyState>
    </div>

    <div v-else-if="data?.kind === 'instrumental'" class="ly__state">
      <UiEmptyState icon="ph:music-notes-simple" :title="t('instrumental')" compact />
    </div>

    <template v-else-if="synced">
      <UiContextMenu :items="menuItems" @open="onMenuOpen">
        <div
          ref="view"
          class="ly__view"
          tabindex="-1"
          data-no-dismiss
          :aria-label="t('region')"
          role="region"
          @wheel="onWheel"
          @pointerdown="onDown"
          @pointermove="onMove"
          @pointerup="onUp"
          @pointercancel="onUp"
          @keydown="onKey"
        >
          <div ref="list" class="ly__list" :class="{ 'is-instant': instant, 'is-manual': !following }" :style="{ '--off': `${-off}px` }">
            <template v-for="it in items" :key="it.i">
              <div
                v-if="it.type === 'line'"
                class="ly__line"
                :class="lineClass(it)"
                :style="lineStyle(it.i)"
                :data-i="it.i"
                role="button"
                tabindex="0"
                :aria-current="it.i === current ? 'true' : undefined"
                @click="seekTo(it)"
                @keydown.enter.prevent="seekTo(it)"
                @keydown.space.prevent="seekTo(it)"
              >
                <div class="ly__main"><template v-for="(w, k) in it.words" :key="k"><span class="w">{{ w.w }}</span>{{ ' ' }}</template></div>
                <div v-if="it.backWords" class="ly__back"><template v-for="(w, k) in it.backWords" :key="k"><span class="w">{{ w.w }}</span>{{ ' ' }}</template></div>
              </div>
              <div
                v-else-if="it.type === 'break'"
                class="ly__line ly__brk"
                :class="lineClass(it)"
                :style="lineStyle(it.i)"
                :data-i="it.i"
                role="button"
                tabindex="-1"
                :aria-label="t('break')"
                @click="seekTo(it)"
              >
                <span class="ly__dots"><i /><i /><i /></span>
              </div>
              <div v-else class="ly__line ly__end" :class="lineClass(it)" :data-i="it.i" aria-hidden="true" />
            </template>
          </div>
        </div>
      </UiContextMenu>
      <Transition name="ly-pill">
        <button v-if="!following" type="button" class="ly__pill" @click="resume">
          <Icon name="ph:arrow-down" class="ly__pill-icon" :class="{ 'is-up': pillUp }" />
          {{ t('back') }}
        </button>
      </Transition>
    </template>

    <div v-else-if="stanzas.length" class="ly__plain no-scrollbar" data-no-dismiss @pointerdown="emit('interact')">
      <template v-for="(s, i) in stanzas" :key="i">
        <details v-if="s.repeat" class="ly__repeat">
          <summary><span class="ly__label">{{ t('chorus') }}</span><Icon name="ph:caret-down" class="ly__caret" /></summary>
          <p class="ly__stanza"><template v-for="(l, j) in s.lines" :key="j">{{ l }}<br v-if="j < s.lines.length - 1"></template></p>
        </details>
        <div v-else class="ly__block">
          <span v-if="s.chorus" class="ly__label">{{ t('chorus') }}</span>
          <p class="ly__stanza"><template v-for="(l, j) in s.lines" :key="j">{{ l }}<br v-if="j < s.lines.length - 1"></template></p>
        </div>
      </template>
    </div>

    <div v-else class="ly__state">
      <UiEmptyState icon="ph:microphone-stage" :title="t('none')" compact />
    </div>
  </div>
</template>

<style scoped>
/* Цвета — от текста темы: на тёмной сцене белые, в светлой панели — чернила и серые */
.ly {
  --ly-hi: var(--text-1);
  --ly-lo: color-mix(in oklab, var(--text-1) 40%, transparent);
  --ly-dim: color-mix(in oklab, var(--text-1) 36%, transparent);
  --ly-fs: clamp(26px, 8cqi, 46px);
  --ly-plain: clamp(20px, 5.6cqi, 32px);
  --ly-pad: 14px;
  position: relative;
  height: 100%;
  min-height: 0;
  container-type: inline-size;
}
.ly--panel {
  --ly-fs: clamp(21px, 6.8cqi, 28px);
  --ly-plain: clamp(17px, 4.8cqi, 20px);
  --ly-pad: 10px;
}

/* ---------- Синхронный текст: лента на пружине ---------- */
.ly__view {
  position: absolute;
  inset: 0;
  overflow: hidden;
  outline: none;
  touch-action: none;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
  -webkit-mask-image: linear-gradient(180deg, transparent 0, #000 8%, #000 80%, transparent 100%);
  mask-image: linear-gradient(180deg, transparent 0, #000 8%, #000 80%, transparent 100%);
}
.ly__list { padding: 0 calc(var(--ly-pad) + 10px); }
.ly__line {
  position: relative;
  margin: 0 calc(-1 * var(--ly-pad));
  padding: 0.28em var(--ly-pad);
  border-radius: 14px;
  color: var(--ly-dim);
  font-size: var(--ly-fs);
  font-weight: 800;
  line-height: 1.16;
  letter-spacing: -0.022em;
  text-wrap: pretty;
  overflow-wrap: anywhere;
  cursor: pointer;
  filter: blur(var(--blur, 0px));
  transform: translate3d(0, var(--off), 0);
  transition: transform var(--ly-ms) var(--ly-ease) var(--d, 0ms), filter 600ms ease, color 450ms ease;
}
.ly__line:not(.is-far) { will-change: transform; }
.ly__list.is-instant .ly__line, .ly__line.is-far { transition: none; }
.ly__list.is-manual .ly__line { transition: filter 250ms ease, color 250ms ease; }
@media (hover: hover) {
  .ly__list.is-manual .ly__line:hover, .ly--panel .ly__line:hover { background: color-mix(in oklab, var(--ly-hi) 7%, transparent); }
}
.ly__line:focus-visible { outline: 2px solid var(--ly-hi); outline-offset: -2px; }
.ly__line.is-active { color: var(--ly-hi); }
.ly__main .w, .ly__back .w { display: inline-block; transition: transform 500ms var(--ease-out); }
/* Слово «загорается» слева направо по оценке слогов и чуть приподнимается */
.ly__line.is-active:not(.is-whole) .w {
  color: transparent;
  -webkit-background-clip: text;
  background-clip: text;
  background-image: linear-gradient(90deg, var(--ly-hi) calc(var(--p, 0) * 135% - 35%), var(--ly-lo) calc(var(--p, 0) * 135%));
  transform: translateY(calc(var(--p, 0) * -2px));
}
.ly__back { margin-top: 0.12em; font-size: 0.6em; font-weight: 700; letter-spacing: -0.01em; }
.ly__line.is-back { font-size: calc(var(--ly-fs) * 0.68); font-weight: 750; }

/* Проигрыш: три точки дышат и наполняются к началу строки, потом «лопаются» */
.ly__brk { padding-top: 0.5em; padding-bottom: 0.5em; }
.ly__dots { display: inline-flex; gap: 0.3em; transform-origin: 6px 50%; transition: transform 450ms var(--ease-out), opacity 450ms; }
.ly__dots i { width: max(9px, 0.3em); height: max(9px, 0.3em); border-radius: 50%; background: var(--ly-hi); opacity: 0.22; transform: scale(0.7); transition: opacity 300ms, transform 300ms; }
.ly__brk.is-active .ly__dots { animation: ly-breathe 1.7s ease-in-out infinite alternate; }
.ly__brk.is-active .ly__dots i { opacity: calc(0.28 + var(--f, 0) * 0.72); transform: scale(calc(0.72 + var(--f, 0) * 0.38)); }
.ly__brk.is-active.is-ending .ly__dots { animation: none; opacity: 0; transform: scale(0.2); }
@keyframes ly-breathe {
  from { transform: scale(1); }
  to { transform: scale(1.14); }
}
.ly__end { height: 40px; pointer-events: none; }

/* «К текущей строке» */
.ly__pill {
  position: absolute;
  bottom: 14px;
  left: 50%;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 40px;
  padding: 0 16px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--text-1);
  box-shadow: var(--shadow-1);
  color: var(--on-light);
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 650;
  white-space: nowrap;
  cursor: pointer;
  transform: translateX(-50%);
}
.ly__pill:hover { background: color-mix(in srgb, var(--text-1) 86%, var(--ink-1)); }
.ly__pill:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.ly__pill-icon { font-size: 15px; transition: transform var(--dur-3) var(--ease-out); }
.ly__pill-icon.is-up { transform: rotate(180deg); }
.ly-pill-enter-active, .ly-pill-leave-active { transition: opacity var(--dur-3) var(--ease-out), transform var(--dur-3) var(--ease-out); }
.ly-pill-enter-from, .ly-pill-leave-to { opacity: 0; transform: translate(-50%, 10px); }
@media (pointer: coarse) {
  .ly__pill { height: 44px; padding: 0 18px; }
}

/* ---------- Простой текст: строфы, обычная прокрутка ---------- */
.ly__plain {
  position: absolute;
  inset: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 24px var(--ly-pad) 48px;
  -webkit-mask-image: linear-gradient(180deg, transparent 0, #000 24px, #000 calc(100% - 48px), transparent 100%);
  mask-image: linear-gradient(180deg, transparent 0, #000 24px, #000 calc(100% - 48px), transparent 100%);
}
.ly__block, .ly__repeat { margin: 0 0 1.4em; font-size: var(--ly-plain); }
.ly__stanza { margin: 0; color: var(--ly-hi); font-size: var(--ly-plain); font-weight: 750; line-height: 1.3; letter-spacing: -0.018em; text-wrap: pretty; opacity: 0.92; }
.ly--panel .ly__stanza { font-weight: 650; line-height: 1.42; letter-spacing: -0.01em; }
.ly__label { display: block; margin-bottom: 6px; color: var(--ly-dim); font-size: 11px; font-weight: 700; letter-spacing: var(--tr-caps); text-transform: uppercase; }
.ly__repeat summary { display: flex; align-items: center; gap: 6px; min-height: 44px; margin-top: -0.6em; color: var(--ly-dim); cursor: pointer; list-style: none; }
.ly__repeat summary::-webkit-details-marker { display: none; }
.ly__repeat summary .ly__label { margin: 0; }
.ly__repeat summary:hover { color: var(--ly-hi); }
.ly__repeat summary:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 6px; }
.ly__caret { font-size: 14px; transition: transform var(--dur-3) var(--ease-out); }
.ly__repeat[open] .ly__caret { transform: rotate(180deg); }
.ly__repeat .ly__stanza { opacity: 0.6; }

/* ---------- Загрузка и пустые состояния ---------- */
.ly__skeleton { display: flex; flex-direction: column; gap: 26px; padding: 24px var(--ly-pad); }
.ly__skel { height: calc(var(--ly-fs) * 0.7); border-radius: 10px; }
.ly__state { display: grid; place-items: center; height: 100%; min-height: 200px; }

/* Меньше движения: без пружины, размытия и дыхания — простая смена */
.ly.is-reduced .ly__line { transition: color 200ms linear; filter: none; will-change: auto; }
.ly.is-reduced .ly__main .w, .ly.is-reduced .ly__back .w { transition: none; transform: none !important; }
.ly.is-reduced .ly__brk.is-active .ly__dots { animation: none; }
</style>
