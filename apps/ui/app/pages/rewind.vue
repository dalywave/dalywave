<script setup lang="ts">
import type { Rewind, RewindCue, RewindMore, RewindStyle, RewindYears } from '~/api/types'
import { ApiError } from '~/api/types'
import { recordColors, slideColors } from '~/utils/rewind-colors'
import { shareOrDownload } from '~/utils/rewind-share'
import { parseRewindStyle, rewindSlides, type RewindSlide } from '~/utils/rewind-slides'

// «Итоги года» историями: полосы прогресса сверху, нажатие справа — дальше, слева — назад, удержание —
// пауза, свайп вниз — закрыть. На компьютере — рамка телефона над затемнённым экраном и стрелки.
// Три стиля — «Волна года», «Разворот», «Пластинка» (components/rewind/Story*.vue): какой — решает
// сервер (настройка события в админке), администратор смотрит любой (?preview=1&style=…&year=…).
// Итоги — событие: пока оно закрыто, слушателя возвращает на главную.
// На слайдах звучит музыка года с припева (useRewindAudio): наложение между слайдами, на паузе тише.
const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const player = usePlayerStore()
const ui = useUiStore()
const auth = useAuthStore()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })

interface StoryApi { shareCard: () => Promise<Blob> }

const data = shallowRef<Rewind | null>(null)
const cues = shallowRef<RewindCue[]>([])
const more = shallowRef<RewindMore | null>(null)
const state = ref<'loading' | 'error' | 'ready'>('loading')
const year = ref<number | null>(null)
const style = ref<RewindStyle>('wave')
const slides = shallowRef<RewindSlide[]>([])
const story = ref<StoryApi | null>(null)
// Предпросмотр администратора: любой год и стиль, без отметки «посмотрено»
const preview = computed(() => route.query.preview !== undefined && auth.isAdmin)
useHead({ title: () => (year.value ? t('title', { year: year.value }) : t('titleShort')) })

/** Палитры слайдов: с сервера, а для картинок — посчитанные по обложке */
async function resolveSlides(r: Rewind): Promise<RewindSlide[]> {
  return Promise.all(rewindSlides(r).map(async (s) => {
    let palette = s.palette?.filter(Boolean) ?? []
    if (!palette.length && s.image) {
      const c = await coverColors(bridge.resolveUrl(s.image)).catch(() => null)
      if (c) palette = [c.vivid, c.second, c.deep]
    }
    return { kind: s.kind, ms: s.ms, palette, shift: s.shift ?? 0 }
  }))
}

async function load() {
  state.value = 'loading'
  let years: RewindYears
  try {
    years = await bridge.rewind.years()
  }
  catch (e) {
    // Итоги закрыты: заходить некуда
    if (e instanceof ApiError && e.code === 'not_open') return void navigateTo('/', { replace: true })
    state.value = 'error'
    return
  }
  try {
    const q = Number(route.query.year)
    year.value = Number.isInteger(q) && q > 1999 ? q : years.current
    style.value = (preview.value ? parseRewindStyle(route.query.style) : null) ?? years.style
    const y = year.value
    // Музыку сервер собирает вместе с итогами; не вышло — истории идут без звука
    const [r, track] = await Promise.all([bridge.rewind.get(y), bridge.rewind.soundtrack(y).catch(() => null)])
    data.value = r
    cues.value = track?.cues ?? []
    slides.value = await resolveSlides(r)
    const start = Number(route.query.slide)
    index.value = Number.isInteger(start) ? Math.min(Math.max(0, start), slides.value.length - 1) : 0
    elapsed.value = 0
    state.value = 'ready'
    if (r.plays > 0) void loadMore(y)
    // Посмотрел — карточка «Твои итоги готовы» на главной больше не зовёт
    if (!preview.value && years.open && y === years.current && !years.seen) {
      void bridge.rewind.seen(y).catch(() => {})
    }
  }
  catch (e) {
    if (e instanceof ApiError && e.code === 'not_open') return void navigateTo('/', { replace: true })
    state.value = 'error'
  }
}
onMounted(load)

/** «Ещё послушать» — к концу историй; отдельным запросом, чтобы не задерживать начало */
async function loadMore(y: number) {
  more.value = await bridge.rewind.more(y).catch(() => null)
}

// ---------- Ход историй ----------

const index = ref(0)
const elapsed = ref(0)
const holding = ref(false)
const keyPaused = ref(false)
const hidden = ref(false)
// Разработка: стоять на слайде (?hold) — для скриншотов
const frozen = import.meta.dev && route.query.hold !== undefined
const slide = computed(() => slides.value[index.value] ?? null)
/** Направление последнего перехода: от него зависят шторка «Разворота» и плёнка «Пластинки» */
const dir = ref<1 | -1>(1)
// Цвет полос, кнопок и поля под слайдом — по стилю: волна тёмная, разворот — цвет слайда, пластинка — тёмная в тон
const chrome = computed(() => {
  const s = slide.value
  if (style.value === 'spread') {
    const c = slideColors(s?.palette, s?.shift)
    return { bg: c.bg, ink: c.ink }
  }
  if (style.value === 'record') return { bg: recordColors(s?.palette).bg, ink: '#f3ecdf' }
  return { bg: '#030406', ink: '#ffffff' }
})
const paused = computed(() => holding.value || keyPaused.value || hidden.value || frozen || state.value !== 'ready')
const progress = computed(() => (frozen ? 0.5 : slide.value ? Math.min(1, elapsed.value / slide.value.ms) : 0))
const last = computed(() => index.value >= slides.value.length - 1)

function go(i: number) {
  const to = Math.min(Math.max(0, i), Math.max(0, slides.value.length - 1))
  dir.value = to < index.value ? -1 : 1
  index.value = to
  elapsed.value = 0
  bridge.haptic('selection')
}
function next() {
  if (last.value) elapsed.value = slide.value?.ms ?? 0
  else go(index.value + 1)
}
function prev() {
  go(index.value - 1)
}

let raf = 0
let lastTs = 0
function tick(ts: number) {
  const dt = lastTs ? Math.min(100, ts - lastTs) : 0
  lastTs = ts
  if (!paused.value && slide.value && elapsed.value < slide.value.ms) {
    elapsed.value += dt
    if (elapsed.value >= slide.value.ms && !last.value) go(index.value + 1)
  }
  raf = requestAnimationFrame(tick)
}

function onVisibility() {
  hidden.value = document.visibilityState === 'hidden'
}

function close() {
  sound.close()
  if (window.history.state?.back) router.back()
  else void navigateTo('/')
}

function onKey(e: KeyboardEvent) {
  if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return
  if (e.key === 'ArrowRight') next()
  else if (e.key === 'ArrowLeft') prev()
  else if (e.key === 'Escape') close()
  else if (e.key === ' ') keyPaused.value = !keyPaused.value
  else return
  e.preventDefault()
}

onMounted(() => {
  raf = requestAnimationFrame(tick)
  window.addEventListener('keydown', onKey)
  document.addEventListener('visibilitychange', onVisibility)
})
onBeforeUnmount(() => {
  cancelAnimationFrame(raf)
  window.removeEventListener('keydown', onKey)
  document.removeEventListener('visibilitychange', onVisibility)
})

// ---------- Касания: нажатие, удержание, свайп вниз ----------

const stage = ref<HTMLElement | null>(null)
const dragY = ref(0)
let down: { x: number, y: number, id: number } | null = null
let holdTimer = 0
let dragging = false

function onDown(e: PointerEvent) {
  if (e.button !== 0) return
  down = { x: e.clientX, y: e.clientY, id: e.pointerId }
  dragging = false
  window.clearTimeout(holdTimer)
  holdTimer = window.setTimeout(() => (holding.value = true), 200)
}
function onMove(e: PointerEvent) {
  if (!down || e.pointerId !== down.id) return
  const dx = e.clientX - down.x
  const dy = e.clientY - down.y
  if (!dragging && dy > 12 && Math.abs(dy) > Math.abs(dx) * 1.2) {
    dragging = true
    holding.value = true
    window.clearTimeout(holdTimer)
    stage.value?.setPointerCapture(e.pointerId)
  }
  if (dragging) dragY.value = Math.max(0, dy)
}
function onUp(e: PointerEvent) {
  if (!down || e.pointerId !== down.id) return
  window.clearTimeout(holdTimer)
  const wasHolding = holding.value
  const start = down
  down = null
  holding.value = false
  if (dragging) {
    dragging = false
    if (dragY.value > 110) return close()
    dragY.value = 0
    return
  }
  if (wasHolding) return
  const rect = stage.value?.getBoundingClientRect()
  if (!rect) return
  if (start.x - rect.left < rect.width * 0.32) prev()
  else next()
}
function onCancel() {
  window.clearTimeout(holdTimer)
  down = null
  dragging = false
  holding.value = false
  dragY.value = 0
}

// ---------- Звук: музыка года с припева, наложение между слайдами ----------

// Без звука, когда страницу снимает автомат (скриншоты, проверки)
const silent = import.meta.client && navigator.webdriver
const sound = useRewindAudio()
/** Звук «Ещё послушать»: первый готовый трек подборки с трети */
const moreCue = computed<RewindCue | null>(() => {
  const tr = more.value?.items.find(x => x.audioStatus === 'ready')
  return tr ? { slide: 'more', track: tr, startMs: Math.round(tr.durationMs * 0.3), startFrom: 'estimate' } : null
})
const hasSound = computed(() => cues.value.length > 0 || !!moreCue.value)
watch([slides, cues, moreCue], () => {
  sound.audio.setCues(slides.value.map(x => x.kind), moreCue.value ? [...cues.value, moreCue.value] : cues.value)
}, { immediate: true })
watch([index, () => state.value], () => {
  if (!silent && state.value === 'ready') void sound.audio.show(index.value)
})
watch(() => holding.value || keyPaused.value, d => sound.audio.setDucked(d))
watch(hidden, h => sound.audio.setHidden(h))

// ---------- Поделиться и плейлист ----------

const busy = ref<'share' | 'playlist' | 'more' | null>(null)

async function share() {
  const r = data.value
  if (!r || busy.value || !story.value) return
  busy.value = 'share'
  try {
    const blob = await story.value.shareCard()
    const how = await shareOrDownload(blob, `dalywave-${r.year}.png`, t('title', { year: r.year }))
    if (how === 'downloaded') toast.show({ text: t('saved'), icon: 'ph:download-simple' })
  }
  catch {
    toast.error(t('shareFailed'))
  }
  finally {
    busy.value = null
  }
}

async function playlist() {
  const r = data.value
  if (!r || busy.value) return
  if (r.playlistId) return openPlaylist(r.playlistId)
  busy.value = 'playlist'
  try {
    const id = await bridge.rewind.savePlaylist(r.year)
    data.value = { ...r, playlistId: id }
    toast.show({
      text: t('playlistSaved', { year: r.year }),
      icon: 'ph:playlist-fill',
      action: { label: t('open'), onClick: () => openPlaylist(id) },
    })
  }
  catch {
    toast.error(t('playlistFailed'))
  }
  finally {
    busy.value = null
  }
}

function openPlaylist(id: string) {
  sound.close()
  void navigateTo(`/playlist/${id}`)
}

// ---------- Подобрать ещё ----------

/** Волна из итогов года: звучание самого слушаемого. Истории закрываются, играет волна */
function startWave() {
  const y = data.value?.year
  if (!y) return
  sound.close(false)
  void player.startWave(undefined, { seed: { type: 'year', text: String(y) }, title: t('waveTitle', { year: y }) })
  close()
}

function playMore() {
  const list = more.value?.items
  const y = data.value?.year
  if (!list?.length || !y) return
  sound.close(false)
  void player.playTracks(list, 0, { type: 'mix', id: null, title: t('moreTitle', { year: y }) })
  close()
}

async function saveMore() {
  const m = more.value
  const y = data.value?.year
  if (!m || !y || busy.value) return
  if (m.playlistId) return openPlaylist(m.playlistId)
  busy.value = 'more'
  try {
    const id = await bridge.rewind.saveMore(y, m.items.map(x => x.id))
    more.value = { ...m, playlistId: id }
    toast.show({ text: t('moreSaved', { year: y }), icon: 'ph:playlist-fill', action: { label: t('open'), onClick: () => openPlaylist(id) } })
  }
  catch {
    toast.error(t('playlistFailed'))
  }
  finally {
    busy.value = null
  }
}

const framed = computed(() => !ui.isPhone)
const STORIES = {
  wave: resolveComponent('RewindStoryWave'),
  spread: resolveComponent('RewindStorySpread'),
  record: resolveComponent('RewindStoryRecord'),
}
</script>

<template>
  <Teleport to="body">
    <div
      class="rw"
      :class="[`rw--${style}`, { 'rw--framed': framed }]"
      role="dialog"
      aria-modal="true"
      :aria-label="year ? t('title', { year }) : t('titleShort')"
      :style="{ '--bg': chrome.bg, '--ink': chrome.ink }"
    >
      <div v-if="framed" class="rw__backdrop" @click="close" />

      <button v-if="framed && state === 'ready'" type="button" class="rw__side rw__side--prev" :disabled="index === 0" :aria-label="t('prev')" @click="prev">
        <Icon name="ph:caret-left" />
      </button>

      <div
        ref="stage"
        class="rw__stage"
        :class="{ 'is-dragging': dragY > 0, 'is-held': holding }"
        :style="dragY ? { transform: `translateY(${dragY}px) scale(${1 - Math.min(0.08, dragY / 3000)})` } : undefined"
        @pointerdown="onDown"
        @pointermove="onMove"
        @pointerup="onUp"
        @pointercancel="onCancel"
        @contextmenu.prevent
      >
        <component
          :is="STORIES[style]"
          v-if="state === 'ready' && data && slide"
          ref="story"
          :slides="slides"
          :index="index"
          :dir="dir"
          :r="data"
          :more="more"
          :busy="busy"
          :frozen="frozen"
          @share="share"
          @playlist="playlist"
          @wave="startWave"
          @play-more="playMore"
          @save-more="saveMore"
        />
        <div v-else class="rw__state">
          <template v-if="state === 'loading'">
            <Icon name="ph:circle-notch" class="rw__spin" />
          </template>
          <template v-else>
            <p class="rw__error">{{ t('failed') }}</p>
            <button type="button" class="rw__retry" @pointerdown.stop @pointerup.stop @click.stop="load">{{ t('retry') }}</button>
          </template>
        </div>

        <!-- Полосы прогресса и кнопки поверх слайда -->
        <div class="rw__top">
          <div v-if="state === 'ready'" class="rw__bars" role="progressbar" :aria-valuenow="index + 1" aria-valuemin="1" :aria-valuemax="slides.length" :aria-label="t('progress', { n: index + 1, of: slides.length })">
            <span v-for="(s, i) in slides" :key="s.kind" class="rw__bar">
              <i :style="{ transform: `scaleX(${i < index ? 1 : i === index ? progress : 0})` }" />
            </span>
          </div>
          <div class="rw__tools" @pointerdown.stop @pointerup.stop>
            <button v-if="hasSound && !silent" type="button" class="rw__tool" :aria-label="sound.muted.value ? t('soundOn') : t('soundOff')" @click="sound.toggleMute()">
              <Icon :name="sound.muted.value ? 'ph:speaker-slash' : 'ph:speaker-high'" />
            </button>
            <button type="button" class="rw__tool" :aria-label="t('close')" @click="close">
              <Icon name="ph:x-bold" />
            </button>
          </div>
        </div>
      </div>

      <button v-if="framed && state === 'ready'" type="button" class="rw__side rw__side--next" :disabled="last" :aria-label="t('next')" @click="next">
        <Icon name="ph:caret-right" />
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.rw {
  position: fixed;
  inset: 0;
  z-index: 240;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg);
  touch-action: none;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
}
.rw--framed { gap: 24px; background: none; }
.rw__backdrop { position: absolute; inset: 0; background: rgba(4, 5, 7, 0.86); }

.rw__stage {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  container-type: size;
  background: var(--bg);
  cursor: pointer;
  transition: transform var(--dur-3) var(--ease-out);
  --rw-top: var(--safe-top);
  --rw-bottom: var(--safe-bottom);
}
.rw__stage.is-dragging { transition: none; }
.rw--framed .rw__stage {
  width: auto;
  height: min(calc(100dvh - 64px), 900px);
  aspect-ratio: 9 / 16;
  border-radius: 28px;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06);
  --rw-top: 0px;
  --rw-bottom: 0px;
}

.rw__top {
  position: absolute;
  inset: 0 0 auto;
  z-index: 2;
  padding: calc(var(--rw-top) + 10px) 10px 0;
  pointer-events: none;
}
.is-held .rw__top { opacity: 0.4; transition: opacity var(--dur-2); }
.rw__bars { display: flex; gap: 4px; }
.rw__bar { position: relative; flex: 1; height: 3px; border-radius: 999px; background: color-mix(in srgb, var(--ink) 28%, transparent); overflow: hidden; }
.rw__bar i { position: absolute; inset: 0; background: var(--ink); transform-origin: left; }
.rw__tools { display: flex; justify-content: flex-end; gap: 2px; margin-top: 4px; pointer-events: auto; }
.rw__tool {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--ink);
  font-size: 22px;
  cursor: pointer;
  transition: background var(--dur-1);
}
.rw__tool:hover { background: color-mix(in srgb, var(--ink) 12%, transparent); }
.rw__tool:focus-visible { outline: 2px solid var(--ink); outline-offset: -2px; }

.rw__side {
  position: relative;
  z-index: 1;
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  border: 0;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
  font-size: 20px;
  cursor: pointer;
  transition: background var(--dur-1), opacity var(--dur-1);
}
.rw__side:hover:not(:disabled) { background: rgba(255, 255, 255, 0.18); }
.rw__side:disabled { opacity: 0.25; cursor: default; }
.rw__side:focus-visible { outline: 2px solid #fff; outline-offset: 2px; }

.rw__state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px; height: 100%; color: var(--ink); }
.rw__spin { font-size: 32px; animation: spin 0.9s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.rw__error { font-size: 17px; font-weight: 600; }
.rw__retry { min-height: 44px; padding: 0 22px; border: 2px solid var(--ink); border-radius: 999px; background: none; color: var(--ink); font: inherit; font-weight: 700; cursor: pointer; }
@media (prefers-reduced-motion: reduce) {
  .rw__stage { transition: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Итоги {year}",
    "titleShort": "Итоги года",
    "saved": "Картинка сохранена",
    "shareFailed": "Не получилось собрать картинку",
    "playlistSaved": "«Твои лучшие треки {year}» — в медиатеке",
    "playlistFailed": "Не получилось сохранить плейлист",
    "moreSaved": "«Ещё послушать {year}» — в\u00a0медиатеке",
    "moreTitle": "Ещё послушать {year}",
    "waveTitle": "Волна {year}",
    "open": "Открыть",
    "failed": "Итоги не загрузились",
    "retry": "Ещё раз",
    "prev": "Назад",
    "next": "Дальше",
    "close": "Закрыть",
    "soundOn": "Включить звук",
    "soundOff": "Выключить звук",
    "progress": "Слайд {n} из {of}"
  },
  "uk": {
    "title": "Підсумки {year}",
    "titleShort": "Підсумки року",
    "saved": "Зображення збережено",
    "shareFailed": "Не вдалося зібрати зображення",
    "playlistSaved": "«Твої найкращі треки {year}» — у медіатеці",
    "playlistFailed": "Не вдалося зберегти плейлист",
    "moreSaved": "«Ще послухати {year}» — у\u00a0медіатеці",
    "moreTitle": "Ще послухати {year}",
    "waveTitle": "Хвиля {year}",
    "open": "Відкрити",
    "failed": "Підсумки не завантажилися",
    "retry": "Ще раз",
    "prev": "Назад",
    "next": "Далі",
    "close": "Закрити",
    "soundOn": "Увімкнути звук",
    "soundOff": "Вимкнути звук",
    "progress": "Слайд {n} з {of}"
  },
  "en": {
    "title": "Your {year}",
    "titleShort": "Year in review",
    "saved": "Image saved",
    "shareFailed": "Couldn’t make the image",
    "playlistSaved": "“Your top tracks {year}” is in your library",
    "playlistFailed": "Couldn’t save the playlist",
    "moreSaved": "“More to hear {year}” is in your library",
    "moreTitle": "More to hear {year}",
    "waveTitle": "Your {year} wave",
    "open": "Open",
    "failed": "Couldn’t load your review",
    "retry": "Try again",
    "prev": "Back",
    "next": "Next",
    "close": "Close",
    "soundOn": "Sound on",
    "soundOff": "Sound off",
    "progress": "Slide {n} of {of}"
  }
}
</i18n>
