<script setup lang="ts">
import type { Rewind, Track } from '~/api/types'
import type { SlideKind } from '~/components/rewind/Slide.vue'
import { type SlideColors, slideColors } from '~/utils/rewind-colors'
import { renderShareCard, shareOrDownload } from '~/utils/rewind-share'

// «Итоги года» историями: полосы прогресса сверху, нажатие справа — дальше, слева — назад, удержание —
// пауза, свайп вниз — закрыть. На компьютере — рамка телефона над затемнённым экраном и стрелки.
// На слайдах звучит кусок подходящего трека через тот же плеер — как превью, без записи в историю
const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const player = usePlayerStore()
const ui = useUiStore()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

interface SlideDef {
  kind: SlideKind
  ms: number
  /** Что звучит на слайде; нет — продолжает прежнее */
  track: Track | null
  /** Откуда цвет: палитра альбома с сервера или картинка */
  palette?: string[] | null
  image?: string | null
  shift?: number
}

const data = shallowRef<Rewind | null>(null)
const state = ref<'loading' | 'error' | 'ready'>('loading')
const year = ref<number | null>(null)
const colors = ref<SlideColors[]>([])
useHead({ title: () => (year.value ? t('title', { year: year.value }) : t('titleShort')) })

const ready = (tr: Track | null | undefined) => (tr?.audioStatus === 'ready' ? tr : null)

const slides = computed<SlideDef[]>(() => {
  const r = data.value
  if (!r) return []
  if (r.plays === 0) return [{ kind: 'empty', ms: 8000, track: null }]
  const top = r.topTracks.map(x => x.track)
  const al = r.topAlbums.map(a => a.album)
  const toy = r.trackOfYear?.track ?? null
  const byArtist = (id?: string) => top.find(tr => tr.artists.some(a => a.id === id)) ?? null
  const byAlbum = (id?: string) => top.find(tr => tr.album?.id === id) ?? null
  // Палитры нескольких обложек: поле берёт самый живой цвет из них
  const pal = (from: number) => [...al.slice(from), ...al.slice(0, from)].flatMap(a => a.palette ?? [])
  const list: (SlideDef | false)[] = [
    { kind: 'intro', ms: 6000, track: ready(toy), palette: pal(0), image: al[0]?.image?.small },
    { kind: 'minutes', ms: 8000, track: ready(top[1]) ?? ready(toy), palette: pal(1), shift: 1, image: al[1]?.image?.small },
    { kind: 'days', ms: 8000, track: ready(r.topDay?.track), image: r.topDay?.track?.album?.image?.small ?? al[2]?.image?.small, palette: r.topDay?.track ? null : pal(2) },
    !!r.firstTrack && { kind: 'first', ms: 7000, track: ready(r.firstTrack.track), image: r.firstTrack.track.album?.image?.small },
    top.length > 0 && { kind: 'tracks', ms: 9000, track: ready(top[0]), image: top[0]?.album?.image?.small },
    !!toy && { kind: 'toy', ms: 8000, track: ready(toy), image: toy.album?.image?.small, shift: 1 },
    r.topArtists.length > 0 && { kind: 'artists', ms: 9000, track: ready(byArtist(r.topArtists[0]?.artist.id)), image: r.topArtists[0]?.artist.image?.small },
    al.length > 0 && { kind: 'albums', ms: 8000, track: ready(byAlbum(al[0]?.id)), palette: al[0]?.palette, image: al[0]?.image?.small, shift: 1 },
    { kind: 'sound', ms: 8000, track: ready(top[2]), palette: pal(3), shift: 2 },
    { kind: 'discoveries', ms: 8000, track: null, image: r.discoveries[0]?.artist.image?.small ?? al[4]?.image?.small, palette: r.discoveries[0] ? null : al[4]?.palette },
    { kind: 'personality', ms: 8000, track: ready(top[3]), image: r.topArtists[1]?.artist.image?.small, palette: r.topArtists[1] ? null : al[1]?.palette, shift: 1 },
    { kind: 'summary', ms: 12_000, track: ready(toy), image: r.topArtists[0]?.artist.image?.small, palette: r.topArtists[0] ? null : al[0]?.palette },
  ]
  return list.filter((s): s is SlideDef => !!s)
})

async function resolveColors(list: SlideDef[]) {
  const out = await Promise.all(list.map(async (s) => {
    if (s.palette?.length) return slideColors(s.palette, s.shift)
    const c = s.image ? await coverColors(bridge.resolveUrl(s.image)) : null
    return slideColors(c ? [c.vivid, c.second, c.deep] : null, s.shift)
  }))
  colors.value = out
}

async function load() {
  state.value = 'loading'
  try {
    const q = Number(route.query.year)
    year.value = Number.isInteger(q) && q > 1999 ? q : (await bridge.rewind.years()).current
    data.value = await bridge.rewind.get(year.value)
    await resolveColors(slides.value)
    const start = Number(route.query.slide)
    index.value = Number.isInteger(start) ? Math.min(Math.max(0, start), slides.value.length - 1) : 0
    elapsed.value = 0
    state.value = 'ready'
  }
  catch {
    state.value = 'error'
  }
}
onMounted(load)

// ---------- Ход историй ----------

const index = ref(0)
const elapsed = ref(0)
const holding = ref(false)
const keyPaused = ref(false)
const hidden = ref(false)
// Разработка: стоять на слайде (?hold) — для скриншотов
const frozen = import.meta.dev && route.query.hold !== undefined
const slide = computed(() => slides.value[index.value] ?? null)
const current = computed(() => colors.value[index.value] ?? slideColors(null))
const paused = computed(() => holding.value || keyPaused.value || hidden.value || frozen || state.value !== 'ready')
const progress = computed(() => (frozen ? 0.5 : slide.value ? Math.min(1, elapsed.value / slide.value.ms) : 0))
const last = computed(() => index.value >= slides.value.length - 1)

function go(i: number) {
  index.value = Math.min(Math.max(0, i), Math.max(0, slides.value.length - 1))
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
  player.stopPreview()
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
  player.stopPreview()
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

// ---------- Звук: кусок трека слайда через плеер, без истории ----------

// Без звука, когда страницу снимает автомат (скриншоты)
const silent = import.meta.client && navigator.webdriver
const hasSound = computed(() => slides.value.some(s => s.track))
watch([index, () => state.value], () => {
  const tr = slide.value?.track
  if (silent || state.value !== 'ready' || !tr || player.previewing?.id === tr.id) return
  void player.startPreview(tr, Math.min(45_000, tr.durationMs * 0.3))
})

// ---------- Поделиться и плейлист ----------

const busy = ref<'share' | 'playlist' | null>(null)

async function share() {
  const r = data.value
  if (!r || busy.value) return
  busy.value = 'share'
  try {
    const persona = r.personality.id
    const blob = await renderShareCard({
      year: r.year,
      kicker: r.complete ? t('kicker') : t('kickerSoFar'),
      colors: current.value,
      photo: r.topArtists[0]?.artist.image?.large ? bridge.resolveUrl(r.topArtists[0].artist.image.large) : null,
      artistsLabel: t('artists'),
      artists: r.topArtists.map(a => a.artist.name),
      tracksLabel: t('tracks'),
      tracks: r.topTracks.map(x => x.track.title),
      stats: [
        { label: t('minutes'), value: formatNumber(r.minutes, lang.value) },
        ...(r.genres[0] ? [{ label: t('genre'), value: r.genres[0].name }] : []),
      ],
      personaLabel: t('persona'),
      persona: t(`personaTitle.${persona}`),
    })
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
  if (r.playlistId) {
    player.stopPreview()
    return void navigateTo(`/playlist/${r.playlistId}`)
  }
  busy.value = 'playlist'
  try {
    const id = await bridge.rewind.savePlaylist(r.year)
    data.value = { ...r, playlistId: id }
    toast.show({
      text: t('playlistSaved', { year: r.year }),
      icon: 'ph:playlist-fill',
      action: { label: t('open'), onClick: () => {
        player.stopPreview()
        void navigateTo(`/playlist/${id}`)
      } },
    })
  }
  catch {
    toast.error(t('playlistFailed'))
  }
  finally {
    busy.value = null
  }
}

const framed = computed(() => !ui.isPhone)
</script>

<template>
  <Teleport to="body">
    <div
      class="rw"
      :class="{ 'rw--framed': framed }"
      role="dialog"
      aria-modal="true"
      :aria-label="year ? t('title', { year }) : t('titleShort')"
      :style="{ '--bg': current.bg, '--ink': current.ink, '--soft': current.soft }"
    >
      <div v-if="framed" class="rw__backdrop" @click="close" />

      <button v-if="framed && state === 'ready'" type="button" class="rw__side rw__side--prev" :disabled="index === 0" :aria-label="t('prev')" @click="prev">
        <Icon name="ph:caret-left" />
      </button>

      <div
        ref="stage"
        class="rw__stage"
        :class="{ 'is-dragging': dragY > 0 }"
        :style="dragY ? { transform: `translateY(${dragY}px) scale(${1 - Math.min(0.08, dragY / 3000)})` } : undefined"
        @pointerdown="onDown"
        @pointermove="onMove"
        @pointerup="onUp"
        @pointercancel="onCancel"
        @contextmenu.prevent
      >
        <template v-if="state === 'ready' && data && slide">
          <RewindSlide
            :key="`${slide.kind}-${index}`"
            :kind="slide.kind"
            :r="data"
            :colors="current"
            :active="true"
            :busy="busy"
            @share="share"
            @playlist="playlist"
          />
        </template>
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
            <button v-if="hasSound && !silent" type="button" class="rw__tool" :aria-label="player.muted ? t('soundOn') : t('soundOff')" @click="player.toggleMute()">
              <Icon :name="player.muted ? 'ph:speaker-slash' : 'ph:speaker-high'" />
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
    "kicker": "Итоги года",
    "kickerSoFar": "Итоги по сей день",
    "artists": "Артисты",
    "tracks": "Треки",
    "minutes": "Минут",
    "genre": "Жанр",
    "persona": "Характер",
    "personaTitle": { "night_owl": "Ночной слушатель", "early_bird": "Ранняя пташка", "explorer": "Исследователь", "loyal": "Верный слушатель", "wave_rider": "На волне", "balanced": "Меломан" },
    "saved": "Картинка сохранена",
    "shareFailed": "Не получилось собрать картинку",
    "playlistSaved": "«Твои лучшие треки {year}» — в медиатеке",
    "playlistFailed": "Не получилось сохранить плейлист",
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
  "en": {
    "title": "Your {year}",
    "titleShort": "Year in review",
    "kicker": "Year in review",
    "kickerSoFar": "Your year so far",
    "artists": "Artists",
    "tracks": "Tracks",
    "minutes": "Minutes",
    "genre": "Genre",
    "persona": "Listening type",
    "personaTitle": { "night_owl": "Night owl", "early_bird": "Early bird", "explorer": "Explorer", "loyal": "Loyal listener", "wave_rider": "Wave rider", "balanced": "Music lover" },
    "saved": "Image saved",
    "shareFailed": "Couldn’t make the image",
    "playlistSaved": "“Your top tracks {year}” is in your library",
    "playlistFailed": "Couldn’t save the playlist",
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
