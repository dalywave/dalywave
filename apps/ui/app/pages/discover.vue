<script setup lang="ts">
import type { Track, TrackClip, TrackHook } from '~/api/types'
import type { WaveSettings } from '~/bridge/types'

// «Открытия»: вертикальная лента новых для слушателя треков, как в TikTok. Карточка — клип
// (кусок официального видео вокруг припева), живая обложка альбома или обложка с волной; звук —
// припев самого трека через плеер в режиме превью: очередь не трогается, после ленты играющее
// продолжается. Смахнуть вверх (колёсико, стрелки) — следующий; двойное нажатие или ♥ — в любимые;
// «+» — в плейлист; нажатие — трек целиком. Треки — из волны «Незнакомое», реакции учат её же
// Лента — слой поверх оболочки (Teleport в body): листы и меню оболочки (выбор плейлиста) — над ней

interface Card {
  track: Track
  reason: string
  clip: TrackClip | null
  /** Сколько раз спрашивали видео: «ищется» дольше — показываем запасное */
  asked: number
}

const bridge = useBridge()
const player = usePlayerStore()
const library = useLibraryStore()
const motion = useMotionStore()
const ui = useUiStore()
const pui = usePlaylistUi()
const router = useRouter()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const NEW: WaveSettings = { activity: 'any', character: 'new', mood: 'any', language: 'any' }
/** «Незнакомое» кончилось (всё недавно показано) — лента продолжается волной «Знакомое и новое» без любимого */
const MIX: WaveSettings = { ...NEW, character: 'mix' }
let settings = NEW
/** Заранее готовим видео стольких карточек вперёд */
const AHEAD = 8
/** Дозагрузка ленты, когда до конца осталось меньше */
const MORE_AT = 5
/** «Ищется» дольше стольких опросов — показываем живую обложку или обложку с волной */
const MAX_ASKS = 6
const ASK_EVERY_MS = 7000
/** Нарастание и затухание звука отрывка */
const FADE_IN_MS = 300
const FADE_OUT_MS = 900
/** Ушёл с карточки быстрее — это «пропуск» для волны */
const SKIP_MS = 4000

const cards = ref<Card[]>([])
const index = ref(0)
const loading = ref(true)
const failed = ref(false)
let session: string | null = null
let loadingMore = false

// ---------- Лента ----------

function push(items: { track: Track, reason: string }[]) {
  const seen = new Set(cards.value.map(c => c.track.id))
  for (const it of items) {
    // Лента — о новом: любимое сюда не попадает, даже если волна его подмешала
    if (seen.has(it.track.id) || library.isLiked(it.track)) continue
    seen.add(it.track.id)
    cards.value.push({ track: it.track, reason: it.reason, clip: null, asked: 0 })
  }
}

async function start() {
  loading.value = true
  failed.value = false
  try {
    for (const next of [NEW, MIX]) {
      settings = next
      const s = await bridge.wave.start(settings)
      session = s.id
      push(s.queue)
      if (cards.value.length < 10) await more()
      if (cards.value.length >= 5) break
    }
    void refreshVideos()
  }
  catch {
    failed.value = true
  }
  finally {
    loading.value = false
  }
}

async function more() {
  if (!session || loadingMore) return
  loadingMore = true
  try {
    const before = cards.value.length
    push(await bridge.wave.next(session, settings, 10, cards.value.map(c => c.track.id)))
    if (cards.value.length === before && settings === NEW) {
      settings = MIX
      const s = await bridge.wave.start(settings)
      session = s.id
      push(s.queue)
    }
  }
  catch {
    // Следующая попытка — при следующем смахивании
  }
  finally {
    loadingMore = false
  }
}

// ---------- Видео: клипы и живые обложки ближайших карточек ----------

let askTimer: ReturnType<typeof setTimeout> | undefined
async function refreshVideos() {
  clearTimeout(askTimer)
  const near = cards.value
    .slice(Math.max(0, index.value - 1), index.value + AHEAD)
    .filter(c => (!c.clip || c.clip.state === 'pending') && c.asked < MAX_ASKS)
  if (!near.length) return
  try {
    const got = await bridge.video.clips(near.map(c => c.track.id))
    for (const clip of got) {
      const card = cards.value.find(c => c.track.id === clip.trackId)
      if (!card) continue
      card.clip = clip
      card.asked++
      if (clip.motion) motion.remember(clip.motion)
    }
  }
  catch {
    near.forEach(c => c.asked++)
  }
  if (cards.value.slice(index.value, index.value + AHEAD).some(c => (!c.clip || c.clip.state === 'pending') && c.asked < MAX_ASKS)) {
    askTimer = setTimeout(() => void refreshVideos(), ASK_EVERY_MS)
  }
}

/** Что показывает карточка: клип, живая обложка или обложка с волной */
function mediaOf(c: Card): { kind: 'clip' | 'motion' | 'cover', src: string | null } {
  if (!motion.allowed) return { kind: 'cover', src: null }
  if (c.clip?.state === 'ready' && c.clip.url) return { kind: 'clip', src: c.clip.url }
  const m = c.clip?.motion?.state === 'ready' ? c.clip.motion : null
  const src = m ? (ui.isPhone ? (m.tall ?? m.square) : (m.square ?? m.tall)) : null
  if (src) return { kind: 'motion', src }
  return { kind: 'cover', src: null }
}

// ---------- Звук: припев карточки, по кругу ----------

const progress = ref(0)
const loopMs = ref(15_000)
/** Номер круга: видео начинается заново вместе со звуком */
const cycle = ref(0)
const hooks = new Map<string, TrackHook>()
let run = 0
let raf = 0
let startedAt = 0
let enteredAt = 0
let silent = false

function stopClock() {
  if (raf) cancelAnimationFrame(raf)
  raf = 0
}

function tick() {
  const elapsed = performance.now() - startedAt
  const len = loopMs.value
  progress.value = Math.min(1, elapsed / len)
  const fadeIn = Math.min(1, elapsed / FADE_IN_MS)
  const fadeOut = Math.min(1, Math.max(0, (len - elapsed) / FADE_OUT_MS))
  if (player.previewing) player.previewGain = Math.min(fadeIn, fadeOut)
  if (elapsed >= len) {
    void playCard(index.value, true)
    return
  }
  raf = requestAnimationFrame(tick)
}

async function playCard(i: number, again = false) {
  const card = cards.value[i]
  const id = ++run
  stopClock()
  progress.value = 0
  if (!card) return
  try {
    let hook = hooks.get(card.track.id)
    if (!hook) {
      hook = await bridge.catalog.hook(card.track.id)
      hooks.set(card.track.id, hook)
    }
    if (id !== run) return
    // С клипом круг — длина клипа: видео и звук начинаются вместе
    const clipMs = card.clip?.state === 'ready' ? card.clip.durationMs : null
    loopMs.value = Math.min(hook.lengthMs, clipMs ?? hook.lengthMs)
    let ok = false
    const fade = { fadeMs: again ? 0 : 350 }
    if (hook.ready) ok = await player.startPreview(card.track, hook.startMs, fade)
    else if (hook.previewUrl) {
      ok = await player.startPreview(card.track, hook.startMs, {
        ...fade,
        resolved: { kind: 'ready', source: { url: hook.previewUrl, durationMs: 30_000, preview: true }, peaks: null },
      })
    }
    if (id !== run) return
    silent = !ok
    if (ok) player.previewGain = 0
  }
  catch {
    if (id !== run) return
    silent = true
  }
  // Без звука карточка всё равно идёт по кругу: видео и полоска не стоят
  cycle.value++
  startedAt = performance.now()
  raf = requestAnimationFrame(tick)
}

// ---------- Прокрутка: какая карточка на экране ----------

const scroller = ref<HTMLElement | null>(null)
let settleTimer: ReturnType<typeof setTimeout> | undefined

function onScroll() {
  const el = scroller.value
  if (!el) return
  const i = Math.round(el.scrollTop / Math.max(1, el.clientHeight))
  clearTimeout(settleTimer)
  settleTimer = setTimeout(() => setIndex(i), 120)
}

function setIndex(i: number) {
  const next = Math.max(0, Math.min(cards.value.length - 1, i))
  if (next === index.value && (raf || silent)) return
  const prev = cards.value[index.value]
  if (prev && next !== index.value && session && performance.now() - enteredAt < SKIP_MS) {
    void bridge.wave.feedback(session, prev.track.id, 'skip', 0).catch(() => {})
  }
  index.value = next
  enteredAt = performance.now()
  void playCard(next)
  if (cards.value.length - next <= MORE_AT) void more()
  void refreshVideos()
}

function go(delta: number) {
  const el = scroller.value
  if (!el) return
  const target = Math.max(0, Math.min(cards.value.length - 1, index.value + delta))
  el.scrollTo({ top: target * el.clientHeight, behavior: ui.reducedMotion ? 'auto' : 'smooth' })
}

function onKey(e: KeyboardEvent) {
  if (pui.pickOpen || e.metaKey || e.ctrlKey || e.altKey) return
  const target = e.target as HTMLElement | null
  if (target?.closest('input, textarea, [contenteditable="true"]')) return
  if (e.key === 'ArrowDown' || e.key === 'j' || e.key === 'PageDown') {
    e.preventDefault()
    go(1)
  }
  else if (e.key === 'ArrowUp' || e.key === 'k' || e.key === 'PageUp') {
    e.preventDefault()
    go(-1)
  }
  else if (e.key === 'Escape') close()
  else if (e.key === 'l') void like(cards.value[index.value])
  else if (e.key === 'Enter') void listen(index.value)
}

// ---------- Действия ----------

const burst = ref<{ i: number, x: number, y: number, n: number } | null>(null)
let tapTimer: ReturnType<typeof setTimeout> | undefined
let lastTap = 0

async function like(card: Card | undefined, only = false) {
  if (!card) return
  const liked = library.isLiked(card.track)
  if (liked && only) return
  bridge.haptic('light')
  try {
    await library.setLiked(card.track, !liked)
    if (!liked && session) void bridge.wave.feedback(session, card.track.id, 'like', 0).catch(() => {})
  }
  catch {
    toast.error(t('likeFailed'))
  }
}

/** Нажатие — трек целиком; два быстрых — в любимые (с сердцем под пальцем) */
function onTap(e: MouseEvent, i: number) {
  if ((e.target as HTMLElement).closest('button, a')) return
  const now = performance.now()
  if (now - lastTap < 280) {
    clearTimeout(tapTimer)
    lastTap = 0
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    burst.value = { i, x: e.clientX - rect.left, y: e.clientY - rect.top, n: (burst.value?.n ?? 0) + 1 }
    void like(cards.value[i], true)
    return
  }
  lastTap = now
  clearTimeout(tapTimer)
  tapTimer = setTimeout(() => void listen(i), 280)
}

/** Трек целиком: лента закрывается, дальше играют её треки с этого места обычным порядком */
async function listen(i: number) {
  const list = cards.value.slice(i).map(c => c.track)
  if (!list.length) return
  stopAll(false)
  await player.playTracks(list, 0, { type: 'wave', id: session, title: t('title') })
  // Плеер раскрывается, когда лента уже закрылась: смена страницы сама сворачивает его
  const off = router.afterEach(() => {
    off()
    setTimeout(() => (ui.playerOpen = true), 60)
  })
  leave()
}

function add(card: Card) {
  pui.pick([card.track])
}

function leave() {
  if (window.history.length > 1) router.back()
  else void navigateTo('/')
}

function close() {
  stopAll(true)
  leave()
}

function stopAll(resume: boolean) {
  run++
  stopClock()
  clearTimeout(askTimer)
  clearTimeout(settleTimer)
  clearTimeout(tapTimer)
  if (player.previewing) player.stopPreview(resume)
}

// Вкладка ушла в фон — звук и видео встают; вернулась — продолжают с начала круга
const visibility = useDocumentVisibility()
watch(visibility, (v) => {
  if (v === 'hidden') {
    run++
    stopClock()
    player.pausePreview()
  }
  else if (cards.value.length && !loading.value) void playCard(index.value)
})

onMounted(async () => {
  window.addEventListener('keydown', onKey)
  await start()
  if (cards.value.length) setIndex(0)
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  stopAll(true)
})

const wavebox = ref<HTMLElement | null>(null)
</script>

<template>
  <div class="dv-page">
  <Teleport to="body">
  <!-- Лента всегда тёмная, как кадр кино: тёмный остров и в светлой теме -->
  <div class="dv scheme-dark" :class="{ 'dv--touch': ui.isPhone }">
    <header class="dv__head">
      <UiIconButton :icon="ui.isTouchUi ? 'ph:caret-down' : 'ph:x'" :label="t('close')" :kbd="ui.isTouchUi ? undefined : ['Esc']" size="lg" tone="glass" side="right" @click="close" />
      <h1 class="dv__title">{{ t('title') }}</h1>
      <span class="dv__spacer" />
    </header>

    <div v-if="loading" class="dv__state">
      <UiSkeleton class="dv__skel" />
    </div>
    <div v-else-if="failed || !cards.length" class="dv__state">
      <UiEmptyState icon="ph:cards-three" :title="failed ? t('failed') : t('empty')" :text="failed ? t('failedText') : t('emptyText')" :tone="failed ? 'error' : undefined">
        <UiButton icon="ph:arrow-clockwise" @click="start">{{ t('retry') }}</UiButton>
      </UiEmptyState>
    </div>

    <div v-else ref="scroller" class="dv__scroller no-scrollbar" @scroll.passive="onScroll">
      <section
        v-for="(c, i) in cards"
        :key="c.track.id"
        class="dc"
        :class="{ 'is-active': i === index }"
        :aria-label="`${c.track.title} — ${c.track.artists.map(a => a.name).join(', ')}`"
        @click="onTap($event, i)"
      >
        <div class="dc__frame">
          <!-- Фон — тона обложки -->
          <div class="dc__backdrop" aria-hidden="true">
            <img v-if="Math.abs(i - index) <= 2 && c.track.album?.image" :src="bridge.resolveUrl(c.track.album.image.small)" alt="" loading="lazy">
          </div>

          <template v-if="Math.abs(i - index) <= 2">
            <div v-if="mediaOf(c).kind === 'clip'" class="dc__media dc__media--clip">
              <MusicCoverArt :image="c.track.album?.image" size="large" :alt="''" class="dc__poster" />
              <MusicLoopVideo :src="mediaOf(c).src" :active="i === index" :cycle="i === index ? cycle : 0" />
            </div>
            <div v-else-if="mediaOf(c).kind === 'motion'" class="dc__media" :class="ui.isPhone ? 'dc__media--tall' : 'dc__media--square'">
              <MusicCoverArt :image="c.track.album?.image" size="large" :alt="''" class="dc__poster" />
              <MusicLoopVideo :src="mediaOf(c).src" :active="i === index" />
            </div>
            <div v-else class="dc__cover">
              <div v-if="i === index" class="dc__wave"><MusicWaveCanvas layout="stage-phone" :box="wavebox" dark /></div>
              <div :ref="el => { if (i === index) wavebox = el as HTMLElement | null }" class="dc__art">
                <MusicCoverArt :image="c.track.album?.image" size="large" :alt="c.track.album?.title ?? ''" />
              </div>
            </div>
          </template>

          <div class="dc__shade" aria-hidden="true" />

          <div class="dc__info">
            <div class="dc__title">{{ c.track.title }}</div>
            <div class="dc__artist">
              <template v-for="(a, k) in c.track.artists" :key="a.id"><span v-if="k">, </span><NuxtLink :to="`/artist/${a.id}`" @click.stop="stopAll(true)">{{ a.name }}</NuxtLink></template>
            </div>
            <div v-if="c.reason" class="dc__reason">{{ typographRu(c.reason) }}</div>
          </div>

          <div class="dc__rail">
            <UiIconButton
              :icon="library.isLiked(c.track) ? 'ph:heart-fill' : 'ph:heart'"
              :label="library.isLiked(c.track) ? t('unlike') : t('like')"
              tone="glass"
              :class="{ dc__liked: library.isLiked(c.track) }"
              :pressed="library.isLiked(c.track)"
              size="xl"
              side="left"
              @click.stop="like(c)"
            />
            <UiIconButton icon="ph:plus" :label="t('add')" tone="glass" size="xl" side="left" @click.stop="add(c)" />
            <UiIconButton icon="ph:play-fill" :label="t('listen')" :kbd="ui.isTouchUi ? undefined : ['Enter']" tone="glass" size="xl" side="left" @click.stop="listen(i)" />
          </div>

          <div v-if="i === index" class="dc__progress" aria-hidden="true"><i :style="{ transform: `scaleX(${progress})` }" /></div>

          <Transition name="dc-burst">
            <Icon v-if="burst && burst.i === i" :key="burst.n" name="ph:heart-fill" class="dc__burst" :style="{ left: `${burst.x}px`, top: `${burst.y}px` }" @animationend="burst = null" />
          </Transition>
        </div>
      </section>
    </div>

    <!-- Компьютер: стрелки рядом с карточкой -->
    <nav v-if="!ui.isTouchUi && cards.length" class="dv__nav" :aria-label="t('nav')">
      <UiIconButton icon="ph:caret-up" :label="t('prev')" :kbd="['↑']" tone="glass" size="lg" side="left" :disabled="index === 0" @click="go(-1)" />
      <UiIconButton icon="ph:caret-down" :label="t('next')" :kbd="['↓']" tone="glass" size="lg" side="left" @click="go(1)" />
    </nav>
  </div>
  </Teleport>
  </div>
</template>

<style scoped>
.dv {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  flex-direction: column;
  background: var(--ink-0);
  color: var(--text-1);
}
.dv__head {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  z-index: 3;
  display: grid;
  grid-template-columns: 56px 1fr 56px;
  align-items: center;
  height: calc(var(--safe-top) + 60px);
  padding: var(--safe-top) 8px 0;
  pointer-events: none;
}
.dv__head > * { pointer-events: auto; }
.dv__title { margin: 0; font-size: 17px; font-weight: 700; letter-spacing: -0.01em; text-align: center; color: #fff; text-shadow: 0 1px 12px rgba(0, 0, 0, 0.35); pointer-events: none; }
.dv__state { display: grid; place-items: center; flex: 1; padding: 24px; }
.dv__skel { width: min(420px, 88vw); aspect-ratio: 9 / 16; max-height: 80dvh; border-radius: var(--r-4); }

.dv__scroller {
  flex: 1;
  overflow-y: scroll;
  overscroll-behavior: contain;
  scroll-snap-type: y mandatory;
}
.dc {
  position: relative;
  display: grid;
  place-items: center;
  height: 100%;
  scroll-snap-align: start;
  scroll-snap-stop: always;
  -webkit-touch-callout: none;
  user-select: none;
  cursor: pointer;
}
/* Телефон — на весь экран; iPad и компьютер — карточка 9:16 посередине */
.dc__frame {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  isolation: isolate;
  background: var(--ink-2);
}
.dv:not(.dv--touch) .dc__frame {
  width: auto;
  height: calc(100% - 40px);
  aspect-ratio: 9 / 16;
  border-radius: var(--r-4);
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06);
}
.dc__backdrop { position: absolute; inset: -10%; z-index: 0; }
.dc__backdrop img { width: 100%; height: 100%; object-fit: cover; filter: blur(48px) saturate(1.25) brightness(0.55); transform: scale(1.1); }

.dc__media { position: absolute; z-index: 1; overflow: hidden; }
/* Клип 16:9 — кадрирован до 4:5: крупнее, но без потери половины кадра */
.dc__media--clip { left: 0; right: 0; top: 50%; aspect-ratio: 4 / 5; transform: translateY(-56%); }
.dc__media--tall { inset: 0; }
.dc__media--square { left: 8%; right: 8%; top: 50%; aspect-ratio: 1; transform: translateY(-58%); border-radius: var(--r-3); }
.dc__poster { position: absolute; inset: 0; width: 100%; height: 100%; aspect-ratio: auto; border-radius: inherit; }
.dc__poster :deep(.art__img) { object-fit: cover; }
.dc__media--clip .dc__poster { filter: blur(18px) brightness(0.7); transform: scale(1.1); }

.dc__cover { position: absolute; inset: 0; z-index: 1; display: grid; place-items: center; }
.dc__wave { position: absolute; inset: 0; opacity: 0.6; }
.dc__art { position: relative; width: min(72%, 360px); margin-bottom: 18%; border-radius: var(--r-3); }
.dc__art :deep(.art) { width: 100%; border-radius: var(--r-3); box-shadow: 0 28px 56px -24px rgba(0, 0, 0, 0.85); }

/* Подписи читаются поверх любого кадра */
.dc__shade {
  position: absolute;
  inset: auto 0 0;
  z-index: 2;
  height: 46%;
  background: linear-gradient(180deg, transparent, rgba(0, 0, 0, 0.62));
  pointer-events: none;
}
.dc__info {
  position: absolute;
  left: 18px;
  right: 92px;
  bottom: calc(var(--safe-bottom) + 30px);
  z-index: 3;
  color: #fff;
}
.dc__title { font-size: 24px; font-weight: 750; letter-spacing: -0.02em; line-height: 1.15; text-wrap: balance; }
.dc__artist { margin-top: 4px; font-size: 16px; color: rgba(255, 255, 255, 0.82); }
.dc__artist a { cursor: pointer; }
.dc__artist a:hover { color: #fff; text-decoration: underline; text-underline-offset: 3px; }
.dc__reason { margin-top: 10px; font-size: var(--fs-small); color: rgba(255, 255, 255, 0.66); }
.dc__rail {
  position: absolute;
  right: 12px;
  bottom: calc(var(--safe-bottom) + 24px);
  z-index: 3;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.dc__rail :deep(.dc__liked) { color: var(--accent); }
.dc__progress {
  position: absolute;
  left: 18px;
  right: 18px;
  bottom: calc(var(--safe-bottom) + 12px);
  z-index: 3;
  height: 2px;
  overflow: hidden;
  border-radius: 1px;
  background: rgba(255, 255, 255, 0.18);
}
.dc__progress i { display: block; height: 100%; background: rgba(255, 255, 255, 0.85); transform-origin: left; }
.dc__burst {
  position: absolute;
  z-index: 4;
  width: 96px;
  height: 96px;
  margin: -48px 0 0 -48px;
  color: var(--accent);
  pointer-events: none;
  animation: dc-burst 720ms var(--ease-out) forwards;
}
@keyframes dc-burst {
  0% { opacity: 0; transform: scale(0.4) rotate(-12deg); }
  25% { opacity: 1; transform: scale(1.1) rotate(6deg); }
  60% { opacity: 1; transform: scale(1) rotate(0); }
  100% { opacity: 0; transform: translateY(-40px) scale(0.9); }
}

.dv__nav {
  position: absolute;
  right: 28px;
  top: 50%;
  z-index: 3;
  display: flex;
  flex-direction: column;
  gap: 12px;
  transform: translateY(-50%);
}

/* Компьютер: подписи и кнопки — в карточке, место под шапку не нужно */
.dv:not(.dv--touch) .dc__info { bottom: 30px; }
.dv:not(.dv--touch) .dc__rail { bottom: 24px; }
.dv:not(.dv--touch) .dc__progress { bottom: 12px; }
.dv:not(.dv--touch) .dv__title { display: none; }

@media (prefers-reduced-motion: reduce) {
  .dv__scroller { scroll-behavior: auto; }
  .dc__burst { animation-duration: 1ms; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Открытия",
    "close": "Закрыть",
    "like": "В любимые",
    "unlike": "Убрать из любимых",
    "likeFailed": "Не получилось изменить любимые",
    "add": "В плейлист",
    "listen": "Слушать целиком",
    "nav": "Листать",
    "prev": "Предыдущий",
    "next": "Следующий",
    "empty": "Пока нечего показать",
    "emptyText": "Послушайте немного — и здесь появится новое для вас",
    "failed": "Не получилось загрузить",
    "failedText": "Проверьте соединение и попробуйте ещё раз",
    "retry": "Повторить"
  },
  "en": {
    "title": "Discover",
    "close": "Close",
    "like": "Like",
    "unlike": "Unlike",
    "likeFailed": "Couldn't update your likes",
    "add": "Add to playlist",
    "listen": "Play full track",
    "nav": "Browse",
    "prev": "Previous",
    "next": "Next",
    "empty": "Nothing to show yet",
    "emptyText": "Listen a little and new music will show up here",
    "failed": "Couldn't load",
    "failedText": "Check your connection and try again",
    "retry": "Retry"
  }
}
</i18n>
