<script setup lang="ts">
import { DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'
import type { WaveMood } from '~/bridge/types'
import { getWaveEngine } from '~/lib/wave/engine'
import { resamplePeaks } from '~/utils/peaks'

// Плеер на весь экран. Большой экран — сцена поверх содержимого (боковая панель и панель плеера
// остаются), волна обнимает обложку. Телефон — отдельный слой с управлением под большим пальцем.
// Планшет под пальцем (iPad) — тот же слой на весь экран и то же смахивание вниз, но горизонтально
// обложка с управлением слева, а справа «Текст / Очередь»; вертикально — обложка сверху, а текст
// и очередь — листом в нижней части экрана
const props = withDefaults(defineProps<{ variant?: 'stage' | 'phone' | 'tablet' }>(), { variant: 'stage' })

const player = usePlayerStore()
const connect = useConnectStore()
const library = useLibraryStore()
const ui = useUiStore()
const { menuFor } = useTrackActions()
const { t } = useI18n()
// Версии песни (оригинал, slowed, sped up): хранилище живёт, пока жив плеер, — оно же заранее
// ставит в очередь выбранную у исполнителя версию
useVersionsStore()

const entry = computed(() => player.current)
const track = computed(() => entry.value?.track ?? null)
const liked = computed(() => (track.value ? library.isLiked(track.value) : false))
const art = ref<HTMLElement | null>(null)
// Переход запланирован, если есть следующий трек — или играет волна: она продолжится сама
const nextAt = computed(() => ((player.nextEntry || player.waveActive) && !player.pending && player.effectiveMode !== 'gapless' && player.effectiveRepeat !== 'one' && player.durationMs ? formatDuration(player.transitionAtMs) : null))
const phonePeaks = computed(() => (player.peaks.length ? resamplePeaks(player.peaks, 56) : Array.from({ length: 56 }, () => 0.18)))
const menuOpen = ref(false)
/** Планшет: что открыто рядом с плеером */
const pane = ref<'lyrics' | 'queue'>('lyrics')
const paneOptions = computed(() => [
  { value: 'lyrics' as const, label: t('player.lyrics') },
  { value: 'queue' as const, label: t('player.queue') },
])
const menuItems = computed(() => (menuOpen.value && track.value ? menuFor(track.value, { nowPlaying: true }) : []))

// ---------- Текст песни «сценой» ----------
// Телефон: кнопка «Текст» — обложка уезжает в шапку (рядом название и исполнитель), текст занимает
// экран, управление остаётся внизу. Слушает и не трогает — управление тихо уходит, касание его
// возвращает. Компьютер: текст колонкой справа от обложки. iPad: вкладка «Текст» рядом с плеером
const lyricsOn = computed(() => ui.playerLyrics)
/** Текст смонтирован, как только его открыли: повторное открытие — без загрузки и проезда */
const lyricsMounted = ref(ui.playerLyrics)
watch(lyricsOn, (v) => {
  if (v) lyricsMounted.value = true
})
watch(() => ui.playerLyrics, (v) => {
  if (v && props.variant === 'tablet') pane.value = 'lyrics'
}, { immediate: true })
const lyricsView = ref<{ synced: boolean } | null>(null)
const lyricsFollowing = ref(true)
const calm = ref(false)
let calmTimer: ReturnType<typeof setTimeout> | undefined
function wake() {
  calm.value = false
  clearTimeout(calmTimer)
  if (props.variant !== 'phone' || !lyricsOn.value) return
  calmTimer = setTimeout(() => {
    if (lyricsOn.value && lyricsFollowing.value && lyricsView.value?.synced && player.playing && !menuOpen.value && open.value) calm.value = true
  }, 4000)
}
watch([lyricsOn, lyricsFollowing, () => player.playing, menuOpen, () => ui.playerOpen], wake)

// Обложка «перелетает» между большой и маленькой в шапке (FLIP): меряем до и после смены раскладки
const mini = ref<HTMLElement | null>(null)
function flip(from: HTMLElement | null, to: () => HTMLElement | null) {
  const a = from?.getBoundingClientRect()
  void nextTick(() => {
    const el = to()
    if (!a?.width || !el || ui.reducedMotion) return
    const b = el.getBoundingClientRect()
    if (!b.width) return
    el.animate?.([
      { transformOrigin: 'top left', transform: `translate(${a.left - b.left}px, ${a.top - b.top}px) scale(${a.width / b.width})` },
      { transformOrigin: 'top left', transform: 'none' },
    ], { duration: 520, easing: 'cubic-bezier(0.16, 1, 0.3, 1)' })
  })
}
function toggleLyrics() {
  const on = !ui.playerLyrics
  if (props.variant === 'phone') flip(on ? art.value : mini.value, () => (on ? mini.value : art.value))
  ui.playerLyrics = on
}
/** Нижняя граница текста на телефоне — над полосой прокрутки; в тишине — до низа экрана */
const scrub = ref<{ $el: HTMLElement } | null>(null)
const lyricsBottom = ref(260)
function measureDock() {
  const s = scrub.value?.$el
  const body = s?.offsetParent as HTMLElement | null
  if (s && body) lyricsBottom.value = Math.max(120, body.clientHeight - s.offsetTop + 6)
}
watch(lyricsOn, () => void nextTick(() => {
  measureDock()
  // Сцена на компьютере: обложка переехала — волна снова находит её
  if (props.variant === 'stage') window.dispatchEvent(new Event('resize'))
}))

const { label: contextLabel, kicker, title: phoneTitle } = useNowPlayingContext()
/** Планшет горизонтально: закрепить плеер колонкой сбоку и свернуть этот слой */
const prefs = usePrefsStore()
function pinSide() {
  prefs.sidePlayer = true
  void ui.collapsePlayer()
}

const quality = computed(() => {
  if (player.source?.preview) return t('player.preview')
  if (!player.source) return null
  return qualityLabel(player.source) ?? t('player.quality')
})
const bpm = computed(() => (player.source?.bpm ? `${Math.round(player.source.bpm)} BPM` : null))

const mood = computed({
  get: () => player.wave.mood,
  set: (m: WaveMood) => player.setWave({ mood: m }),
})

const open = computed({
  get: () => ui.playerOpen && !!track.value,
  set: (v: boolean) => (ui.playerOpen = v),
})

function close() {
  ui.playerOpen = false
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.variant === 'stage' && ui.playerOpen) close()
}

// Обложка чуть дышит со звуком: пульс от настоящего сигнала, без звука — неподвижна
const beatEl = ref<HTMLElement | null>(null)
let offBeat: (() => void) | null = null
const audible = computed(() => player.playing && !player.pending && !player.muted && player.volume > 0)

onMounted(() => {
  window.addEventListener('keydown', onKey)
  window.addEventListener('resize', measureDock)
  offBeat = getWaveEngine().onFrame((s) => {
    const v = audible.value ? s.pulse.toFixed(3) : '0'
    beatEl.value?.style.setProperty('--beat', v)
    sheet.value?.style.setProperty('--beat', v)
  })
})

// ---------- Телефон: смахнуть вниз, чтобы свернуть ----------
// Слой едет за пальцем, фон за ним светлеет. Отпустили дальше четверти высоты или резким
// движением вниз — плеер уезжает в мини-плеер, иначе пружиной возвращается. Горизонтальные
// жесты (полоса прокрутки, обложка) и прокрутка внутри не перехватываются
const sheet = ref<HTMLElement | null>(null)
const dragY = ref(0)
const dragging = ref(false)
const settling = ref(false)
let gesture: { id: number, x: number, y: number, t: number, lastY: number, lastT: number, v: number, axis: 'y' | 'x' | null } | null = null
let swallowClick = false

const sheetStyle = computed(() => (dragY.value || dragging.value || settling.value ? { transform: `translate3d(0, ${dragY.value}px, 0)` } : undefined))
const scrimStyle = computed(() => {
  const h = sheet.value?.clientHeight || 800
  return { opacity: String(Math.max(0, 1 - Math.max(0, dragY.value) / h)) }
})

function scrollableAbove(el: HTMLElement | null): boolean {
  for (let n = el; n && n !== sheet.value; n = n.parentElement) {
    if (n.scrollTop > 0 && n.scrollHeight > n.clientHeight) return true
  }
  return false
}

function onSheetDown(e: PointerEvent) {
  if (!e.isPrimary || (e.pointerType === 'mouse' && e.button !== 0) || settling.value) return
  const target = e.target as HTMLElement
  // Свой жест у полосы прокрутки и полей; прокрученный список ниже начала — тоже прокрутка
  if (target.closest('[data-scrub], input, textarea, [data-no-dismiss]') || scrollableAbove(target)) return
  gesture = { id: e.pointerId, x: e.clientX, y: e.clientY, t: e.timeStamp, lastY: e.clientY, lastT: e.timeStamp, v: 0, axis: null }
}

function onSheetMove(e: PointerEvent) {
  const g = gesture
  if (!g || e.pointerId !== g.id) return
  const dx = e.clientX - g.x
  const dy = e.clientY - g.y
  if (!g.axis) {
    if (Math.hypot(dx, dy) < 8) return
    g.axis = Math.abs(dy) > Math.abs(dx) * 1.2 ? 'y' : 'x'
    if (g.axis === 'x') {
      gesture = null
      return
    }
    dragging.value = true
    try {
      sheet.value?.setPointerCapture(e.pointerId)
    }
    catch { /* касание уже кончилось */ }
  }
  const dt = Math.max(1, e.timeStamp - g.lastT)
  g.v = 0.7 * ((e.clientY - g.lastY) / dt) + 0.3 * g.v
  g.lastY = e.clientY
  g.lastT = e.timeStamp
  // Вверх — резиновое сопротивление
  dragY.value = dy >= 0 ? dy : -Math.min(40, Math.sqrt(-dy) * 3)
}

function settle(to: number, then?: () => void) {
  if (ui.reducedMotion) {
    dragY.value = 0
    then?.()
    return
  }
  settling.value = true
  dragY.value = to
  const el = sheet.value
  const done = () => {
    settling.value = false
    if (to !== 0) then?.()
    dragY.value = 0
  }
  if (!el) return done()
  let fired = false
  const finish = () => {
    if (fired) return
    fired = true
    el.removeEventListener('transitionend', finish)
    done()
  }
  el.addEventListener('transitionend', finish)
  setTimeout(finish, 420)
}

function onSheetUp(e: PointerEvent) {
  const g = gesture
  if (!g || e.pointerId !== g.id) return
  gesture = null
  if (!dragging.value) return
  dragging.value = false
  swallowClick = true
  setTimeout(() => (swallowClick = false), 0)
  const h = sheet.value?.clientHeight || 800
  const flick = g.v > 0.6 && dragY.value > 24
  if (dragY.value > h * 0.25 || flick) settle(h, close)
  else settle(0)
}

function onSheetCancel() {
  if (dragging.value) settle(0)
  dragging.value = false
  gesture = null
}

// Переход из меню над плеером: сначала плеер уезжает вниз, потом открывается страница
const offCollapse = props.variant !== 'stage'
  ? ui.onCollapsePlayer(() => new Promise<void>((resolve) => {
      const h = sheet.value?.clientHeight
      if (!h || !open.value) return resolve()
      settle(h, resolve)
    }))
  : null

function onSheetClick(e: MouseEvent) {
  if (!swallowClick) return
  e.stopPropagation()
  e.preventDefault()
}
onBeforeUnmount(() => {
  offCollapse?.()
  clearTimeout(calmTimer)
  window.removeEventListener('resize', measureDock)
  window.removeEventListener('keydown', onKey)
  offBeat?.()
})
</script>

<template>
  <!-- Большой экран: сцена поверх содержимого -->
  <Transition v-if="variant === 'stage'" name="stage">
    <section v-if="open && track" ref="beatEl" class="fp" :class="{ 'fp--lyrics': lyricsOn }" :aria-label="t('player.expand')">
      <div class="fp__wave">
        <MusicWaveCanvas layout="stage" :box="art" />
      </div>
      <UiIconButton class="fp__collapse" icon="ph:caret-down" :label="t('player.collapse')" :kbd="['Esc']" tone="glass" side="right" @click="close" />
      <div class="fp__context"><i class="fp__dot" :class="{ 'fp__dot--any': player.waveActive && player.wave.mood === 'any' }" />{{ contextLabel }}</div>
      <div v-if="player.waveActive" class="fp__mood">
        <span class="fp__mood-label">{{ t('wave.mood.label') }}</span>
        <MusicMoodOrbs v-model="mood" />
      </div>

      <div class="fp__stage">
        <div class="fp__col">
        <div ref="art" class="fp__art">
          <MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager />
          <div v-if="player.pending" class="fp__pending">
            <MusicPendingNote :pending="player.pending" long />
            <UiButton size="sm" icon="ph:skip-forward" @click="player.next('skipped')">{{ t('player.skip') }}</UiButton>
          </div>
        </div>
        <div class="fp__info">
          <h2 class="fp__title" :title="track.title">{{ track.title }}</h2>
          <!-- Версия песни (оригинал, slowed, sped up) — подписью у исполнителя, а не отдельным рядом -->
          <div class="fp__byline">
            <div class="fp__artist">
              <template v-for="(a, i) in track.artists" :key="a.id"><span v-if="i">, </span><NuxtLink :to="`/artist/${a.id}`" @click="close">{{ a.name }}</NuxtLink></template>
            </div>
            <MusicVersionSwitch size="md" />
          </div>
          <div v-if="entry?.reason" class="fp__reason"><Icon name="ph:sparkle-fill" />{{ typographRu(entry.reason) }}</div>
          <MusicConnectChip class="fp__connect" />
          <FriendsPrivateChip class="fp__connect" />
          <div class="fp__chips">
            <UiBadge v-if="quality" variant="quality" icon="ph:waveform">{{ quality }}</UiBadge>
            <UiBadge v-if="bpm" variant="meta">{{ bpm }}</UiBadge>
          </div>
        </div>
        <div v-if="player.waveActive" class="fp__reactions">
          <UiButton icon="ph:thumbs-down" @click="player.dislike()">{{ t('player.dislike') }}</UiButton>
          <UiButton icon="ph:arrow-bend-up-right" @click="player.notNow()">{{ t('player.notNowLabel') }}</UiButton>
          <UiButton icon="ph:sparkle" @click="player.moreLikeThis()">{{ t('player.moreLike') }}</UiButton>
        </div>
        <div v-else class="fp__reactions">
          <UiButton :icon="liked ? 'ph:heart-fill' : 'ph:heart'" @click="player.toggleLike()">{{ liked ? t('player.liked') : t('player.like') }}</UiButton>
          <UiButton v-if="track.album" icon="ph:disc" :to="`/album/${track.album.id}`" @click="close">{{ t('track.goAlbum') }}</UiButton>
        </div>
        </div>
        <!-- Текст справа от обложки: кнопка «Текст» в панели плеера внизу -->
        <Transition name="fp-lyrics">
          <div v-if="lyricsOn" class="fp__lyrics">
            <MusicLyricsView variant="stage" :anchor="0.36" />
          </div>
        </Transition>
      </div>
    </section>
  </Transition>

  <!-- Телефон: отдельный слой -->
  <DialogRoot v-else v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="pp-scrim" :style="scrimStyle" />
      <DialogContent
        v-if="track"
        class="pp"
        :class="{ 'is-dragging': dragging, 'is-settling': settling, 'pp--tablet': variant === 'tablet', 'is-lyrics': variant === 'phone' && lyricsOn, 'is-lyrics-pane': variant === 'tablet' && pane === 'lyrics', 'is-calm': calm }"
        :style="{ '--tint': player.waveActive ? 'var(--m2)' : 'var(--amb-1)' }"
        :aria-describedby="undefined"
        @open-auto-focus.prevent
        @pointerdown="onSheetDown"
        @pointerdown.capture="wake"
        @pointermove="onSheetMove"
        @pointerup="onSheetUp"
        @pointercancel="onSheetCancel"
        @click.capture="onSheetClick"
      >
        <div ref="sheet" class="pp__frame" :style="sheetStyle">
          <DialogTitle class="sr-only">{{ track.title }}</DialogTitle>
          <DialogDescription class="sr-only">{{ contextLabel }}</DialogDescription>
          <div class="pp__ambient" aria-hidden="true" />
          <div class="pp__wave"><MusicWaveCanvas :layout="variant === 'tablet' ? 'stage' : 'stage-phone'" :box="art" /></div>

          <!-- Планшет -->
          <div v-if="variant === 'tablet'" class="tp">
            <header class="tp__head">
              <UiIconButton icon="ph:caret-down" :label="t('player.collapse')" size="lg" tone="solid" :tooltip="false" @click="close" />
              <div class="pp__ctx">
                <div class="pp__kicker">{{ kicker }}</div>
                <div class="pp__ctx-title ellipsis">{{ phoneTitle }}</div>
              </div>
              <div class="tp__head-end">
                <!-- Горизонтально — закрепить сбоку, как у Spotify на iPad: без похода в настройки -->
                <UiIconButton v-if="ui.canSidePlayer" icon="ph:sidebar-simple" class="side-icon" :label="t('player.pinSide')" size="lg" tone="solid" :tooltip="false" @click="pinSide" />
                <UiMenu v-model:open="menuOpen" :items="menuItems">
                  <UiIconButton icon="ph:dots-three" :label="t('common.more')" size="lg" tone="solid" :tooltip="false" />
                </UiMenu>
              </div>
            </header>

            <div class="tp__main">
              <div class="tp__player">
                <div ref="art" class="tp__art">
                  <MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager />
                  <div v-if="player.pending" class="fp__pending">
                    <MusicPendingNote :pending="player.pending" long />
                    <UiButton size="sm" icon="ph:skip-forward" @click="player.next('skipped')">{{ t('player.skip') }}</UiButton>
                  </div>
                </div>

                <div class="tp__info">
                  <div class="pp__titlerow">
                    <div class="pp__titles">
                      <div class="pp__title tp__title ellipsis">{{ track.title }}</div>
                      <div class="pp__byline">
                        <div class="pp__artist ellipsis">
                          <template v-for="(a, i) in track.artists" :key="a.id"><span v-if="i">, </span><NuxtLink :to="`/artist/${a.id}`" @click="close">{{ a.name }}</NuxtLink></template>
                        </div>
                        <MusicVersionSwitch size="md" />
                      </div>
                    </div>
                    <UiBadge v-if="quality" variant="quality" icon="ph:waveform">{{ quality }}</UiBadge>
                    <span class="pp__like">
                      <UiIconButton v-if="!player.modesAvailable" icon="ph:thumbs-down" :label="t('player.dislike')" size="lg" tone="solid" :tooltip="false" @click="player.dislike()" />
                      <UiIconButton :icon="liked ? 'ph:heart-fill' : 'ph:heart'" :label="liked ? t('player.unlike') : t('player.like')" size="lg" :tone="liked ? 'liked' : 'solid'" :pressed="liked" :tooltip="false" @click="player.toggleLike()" />
                    </span>
                  </div>
                  <div v-if="entry?.reason" class="pp__reason ellipsis"><Icon name="ph:sparkle-fill" />{{ typographRu(entry.reason) }}</div>
                </div>

                <MusicScrubber
                  class="tp__scrub"
                  :peaks="phonePeaks"
                  :position-ms="player.positionMs"
                  :duration-ms="player.durationMs || track.durationMs"
                  :transition-ms="nextAt ? player.transitionAtMs : null"
                  size="lg"
                  times="below"
                  :label="t('player.seek')"
                  @seek="player.seek"
                />

                <MusicPlayerControls class="pp__controls tp__controls" :play-size="76" />

                <MusicConnectChip class="pp__connect" />
                <FriendsPrivateChip class="pp__connect" />
                <div class="tp__secondary">
                  <UiIconButton icon="ph:devices" :label="t('player.devices')" size="lg" :tooltip="false" @click="ui.openPanel('devices'); player.loadDevices()" />
                  <MusicTransitionModePicker v-model:mode="connect.transition" v-model:expressiveness="player.expressiveness" icon :web="!connect.djAvailable" :next-at="nextAt" />
                  <MusicSleepTimerButton />
                </div>
              </div>

              <!-- Текст и очередь: свой жест (прокрутка, перетаскивание) — плеер отсюда не смахивается -->
              <section class="tp__pane" :class="{ 'is-lyrics': pane === 'lyrics' }" data-no-dismiss>
                <UiSegmentedControl v-model="pane" :options="paneOptions" :label="t('player.panel')" block class="tp__tabs" />
                <div class="tp__pane-body no-scrollbar">
                  <MusicLyricsView v-if="pane === 'lyrics'" variant="stage" :anchor="0.3" :active="open" />
                  <MusicQueuePanel v-else />
                </div>
              </section>
            </div>
          </div>

          <div v-else class="pp__body">
            <header class="pp__head">
              <UiIconButton icon="ph:caret-down" :label="t('player.collapse')" size="lg" tone="solid" :tooltip="false" @click="close" />
              <!-- С текстом обложка уменьшается сюда: рядом название и исполнитель; нажатие — обратно к обложке -->
              <button v-if="lyricsOn" type="button" class="pp__now" :aria-label="t('player.hideLyrics')" @click="toggleLyrics">
                <span ref="mini" class="pp__now-art"><MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager /></span>
                <span class="pp__now-titles">
                  <span class="pp__now-title ellipsis">{{ track.title }}</span>
                  <span class="pp__now-artist ellipsis">{{ track.artists.map(a => a.name).join(', ') }}</span>
                </span>
              </button>
              <div v-else class="pp__ctx">
                <div class="pp__kicker">{{ kicker }}</div>
                <div class="pp__ctx-title ellipsis">{{ phoneTitle }}</div>
              </div>
              <UiMenu v-model:open="menuOpen" :items="menuItems">
                <UiIconButton icon="ph:dots-three" :label="t('common.more')" size="lg" tone="solid" :tooltip="false" />
              </UiMenu>
            </header>

            <div v-show="!lyricsOn" ref="art" class="pp__art">
              <MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager />
              <div v-if="player.pending" class="fp__pending">
                <MusicPendingNote :pending="player.pending" long />
                <UiButton size="sm" icon="ph:skip-forward" @click="player.next('skipped')">{{ t('player.skip') }}</UiButton>
              </div>
            </div>

            <div v-show="!lyricsOn" class="pp__info">
              <div class="pp__titlerow">
                <div class="pp__titles">
                  <div class="pp__title ellipsis">{{ track.title }}</div>
                  <div class="pp__byline">
                    <div class="pp__artist ellipsis">{{ track.artists.map(a => a.name).join(', ') }}</div>
                    <MusicVersionSwitch size="md" />
                  </div>
                </div>
                <UiBadge v-if="quality" variant="quality" icon="ph:waveform">{{ quality }}</UiBadge>
                <!-- Сердце у названия, как в Spotify; в волне рядом — «не нравится». В ряду кнопок — перемешивание и повтор -->
                <span class="pp__like">
                  <UiIconButton v-if="!player.modesAvailable" icon="ph:thumbs-down" :label="t('player.dislike')" size="lg" tone="solid" :tooltip="false" @click="player.dislike()" />
                  <UiIconButton :icon="liked ? 'ph:heart-fill' : 'ph:heart'" :label="liked ? t('player.unlike') : t('player.like')" size="lg" :tone="liked ? 'liked' : 'solid'" :pressed="liked" :tooltip="false" @click="player.toggleLike()" />
                </span>
              </div>
              <div v-if="entry?.reason" class="pp__reason ellipsis"><Icon name="ph:sparkle-fill" />{{ typographRu(entry.reason) }}</div>
            </div>

            <!-- Текст: между шапкой и управлением; свой жест — плеер отсюда не смахивается -->
            <Transition name="pp-lyrics">
              <div v-if="lyricsMounted" v-show="lyricsOn" class="pp__lyrics" :style="{ bottom: calm ? 'calc(var(--safe-bottom) + 8px)' : `${lyricsBottom}px` }">
                <MusicLyricsView ref="lyricsView" variant="stage" :anchor="76" :active="lyricsOn && open" @interact="wake" @following="lyricsFollowing = $event" />
              </div>
            </Transition>
            <i class="pp__spacer" aria-hidden="true" />

            <MusicScrubber
              ref="scrub"
              class="pp__scrub pp__dock"
              :peaks="phonePeaks"
              :position-ms="player.positionMs"
              :duration-ms="player.durationMs || track.durationMs"
              :transition-ms="nextAt ? player.transitionAtMs : null"
              size="lg"
              times="below"
              :label="t('player.seek')"
              @seek="player.seek"
            />

            <MusicPlayerControls class="pp__controls pp__dock" :play-size="80" />

            <MusicConnectChip class="pp__connect pp__dock" />
            <FriendsPrivateChip class="pp__connect pp__dock" />
            <div class="pp__secondary pp__dock">
              <UiIconButton icon="ph:devices" :label="t('player.devices')" size="lg" :tooltip="false" @click="ui.openPanel('devices'); player.loadDevices()" />
              <MusicTransitionModePicker v-model:mode="connect.transition" v-model:expressiveness="player.expressiveness" icon :web="!connect.djAvailable" :next-at="nextAt" />
              <MusicSleepTimerButton />
              <UiIconButton icon="ph:microphone-stage" :label="t('player.lyrics')" size="lg" :tooltip="false" :active="lyricsOn" :pressed="lyricsOn" @click="toggleLyrics" />
              <UiIconButton icon="ph:queue" :label="t('player.queue')" size="lg" :tooltip="false" @click="ui.openPanel('queue')" />
            </div>
          </div>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style scoped>
/* ---------- Сцена на большом экране ---------- */
.fp { position: absolute; inset: 0; z-index: 25; overflow: hidden; background: #020509; container-type: inline-size; }
.fp__wave { position: absolute; inset: 0; z-index: 0; }
.fp__wave::after { content: ''; position: absolute; inset: 0; pointer-events: none; background: radial-gradient(70% 60% at 50% 40%, transparent 40%, rgba(2, 4, 8, 0.6) 100%); }
.fp :deep(.fp__collapse) { position: absolute; left: 20px; top: 10px; z-index: 2; }
.fp__context {
  position: absolute;
  top: 12px;
  left: 50%;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  gap: 10px;
  height: 32px;
  max-width: 40%;
  padding: 0 14px;
  border-radius: var(--r-pill);
  background: rgba(8, 10, 14, 0.45);
  -webkit-backdrop-filter: blur(16px);
  backdrop-filter: blur(16px);
  box-shadow: inset 0 0 0 1px var(--line);
  font-size: var(--fs-small);
  font-weight: 600;
  white-space: nowrap;
  transform: translateX(-50%);
}
.fp__dot { width: 7px; height: 7px; flex: none; border-radius: 50%; background: var(--accent); }
.fp__dot--any { background: var(--brand-gradient); }
.fp__mood {
  position: absolute;
  right: 20px;
  top: 6px;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 8px 6px 14px;
  border-radius: var(--r-pill);
  background: rgba(8, 10, 14, 0.45);
  -webkit-backdrop-filter: blur(16px);
  backdrop-filter: blur(16px);
  box-shadow: inset 0 0 0 1px var(--line);
}
.fp__mood-label { font-size: var(--fs-caption); font-weight: 550; color: var(--text-3); }
.fp__stage { position: relative; z-index: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 28px; height: 100%; padding: 72px 40px 32px; }
.fp__col { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 28px; width: 100%; min-width: 0; }
/* С текстом: обложка и название — колонкой слева, текст — справа, как в прототипе А */
.fp--lyrics .fp__stage { flex-direction: row; align-items: stretch; gap: 0; padding: 0 0 0 40px; }
.fp--lyrics .fp__col { width: min(46%, 520px); flex: none; padding: 72px 24px 32px 0; }
.fp--lyrics .fp__art { width: min(400px, 46vh, 100%); }
.fp--lyrics .fp__info { max-width: 100%; }
.fp--lyrics .fp__title { font-size: var(--fs-h2); }
.fp__lyrics { position: relative; flex: 1; min-width: 0; padding: 0 56px 0 24px; }
.fp-lyrics-enter-active, .fp-lyrics-leave-active { transition: opacity var(--dur-4) var(--ease-out); }
.fp-lyrics-enter-from, .fp-lyrics-leave-to { opacity: 0; }
/* Тона обложки под текстом: волна тише, чтобы не спорить со строками */
.fp__wave { transition: opacity var(--dur-4) var(--ease-out); }
.fp--lyrics { background: color-mix(in oklab, var(--amb-3) 62%, #020509); }
.fp--lyrics .fp__wave { opacity: 0.4; }
/* Откуда играет — над колонкой с обложкой, а не над строками */
.fp--lyrics .fp__context { left: calc(40px + min(46%, 520px) / 2 - 12px); max-width: min(42%, 480px); }
.fp__art { position: relative; width: min(380px, 42vh, 100%); flex: none; border-radius: var(--r-4); }
.fp__art :deep(.art) { width: 100%; border-radius: var(--r-4); box-shadow: var(--shadow-cover); transform: scale(calc(1 + var(--beat, 0) * 0.012)); }
.fp__pending {
  position: absolute;
  inset: auto 12px 12px;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 14px;
  border-radius: var(--r-3);
  background: rgba(8, 10, 14, 0.72);
  -webkit-backdrop-filter: blur(18px);
  backdrop-filter: blur(18px);
  box-shadow: inset 0 0 0 1px var(--line);
  text-align: left;
}
.fp__info { display: flex; flex-direction: column; align-items: center; gap: 6px; width: 100%; max-width: 560px; min-width: 0; text-align: center; }
.fp__title { max-width: 100%; margin: 0; overflow: hidden; font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1.05; text-overflow: ellipsis; white-space: nowrap; }
.fp__artist { font-size: var(--fs-h4); font-weight: 500; color: var(--text-2); }
.fp__artist a:hover { color: var(--text-1); }
.fp__reason { display: inline-flex; align-items: center; gap: 5px; margin-top: 2px; font-size: var(--fs-small); color: var(--accent); }
.fp__chips { display: flex; flex-wrap: wrap; justify-content: center; gap: 6px; margin-top: 10px; }
.fp__connect { margin-top: 10px; max-width: 100%; }
.fp__byline { display: flex; align-items: center; justify-content: center; gap: 10px; max-width: 100%; min-width: 0; }
.fp__reactions { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px; }
.stage-enter-active, .stage-leave-active { transition: opacity var(--dur-3) var(--ease-out), transform var(--dur-3) var(--ease-out); }
.stage-enter-from, .stage-leave-to { opacity: 0; transform: translateY(24px); }

@media (max-height: 760px) {
  .fp__stage { gap: 18px; padding-top: 60px; }
  .fp__chips { display: none; }
}
/* Узкая сцена (планшет с открытой очередью): настроение — под названием контекста */
@container (max-width: 760px) {
  .fp__context { max-width: 70%; }
  .fp__mood { top: 54px; right: 50%; padding: 6px 10px; transform: translateX(50%); }
  .fp__mood-label { display: none; }
  .fp__stage { padding: 112px 24px 28px; }
  .fp__title { font-size: var(--fs-h2); }
}

/* ---------- Телефон ---------- */
.pp-scrim { position: fixed; inset: 0; z-index: 89; background: rgba(0, 0, 0, 0.55); animation: fade-in var(--dur-4) var(--ease-out); }
.pp { position: fixed; inset: 0; z-index: 90; outline: none; touch-action: none; }
.pp__frame {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background: #020509;
  animation: sheet-in var(--dur-4) var(--ease-out);
  will-change: transform;
}
.pp.is-dragging .pp__frame { border-radius: 24px 24px 0 0; }
.pp.is-settling .pp__frame { border-radius: 24px 24px 0 0; transition: transform 340ms cubic-bezier(0.22, 1, 0.36, 1); }
/* Мягкое свечение цветов обложки сверху: без ярких полос под строкой состояния */
.pp__ambient {
  position: absolute;
  inset: 0 0 auto;
  height: 78%;
  pointer-events: none;
  background:
    radial-gradient(90% 46% at 50% 30%, color-mix(in oklab, var(--amb-1) 34%, transparent), transparent 72%),
    radial-gradient(70% 40% at 12% 8%, color-mix(in oklab, var(--amb-2) 26%, transparent), transparent 70%),
    radial-gradient(70% 40% at 92% 14%, color-mix(in oklab, var(--amb-3) 30%, transparent), transparent 70%);
  transition: opacity var(--dur-mood);
}
/* Волна — тихо, за обложкой: сверху и снизу растворяется */
.pp__wave {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 70%;
  opacity: 0.5;
  pointer-events: none;
  -webkit-mask-image: linear-gradient(180deg, transparent calc(var(--safe-top) + 64px), #000 32%, #000 62%, transparent 92%);
  mask-image: linear-gradient(180deg, transparent calc(var(--safe-top) + 64px), #000 32%, #000 62%, transparent 92%);
}
.pp__body { position: relative; z-index: 1; display: flex; flex-direction: column; height: 100%; padding: var(--safe-top) 24px calc(var(--safe-bottom) + 10px); overflow-y: auto; scrollbar-width: none; }
.pp__body::-webkit-scrollbar { display: none; }
.pp__head { display: grid; grid-template-columns: 44px minmax(0, 1fr) 44px; align-items: center; height: 52px; margin: 0 -10px; flex: none; }
.pp__ctx { min-width: 0; text-align: center; }
.pp__kicker { font-size: 10.5px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.pp__ctx-title { font-size: 15px; font-weight: 650; }
.pp__art { position: relative; width: clamp(180px, calc(100dvh - var(--safe-top) - var(--safe-bottom) - 400px), 100%); margin: clamp(10px, 2.2vh, 18px) auto 0; flex: none; border-radius: 14px; }
.pp__art :deep(.art) { width: 100%; border-radius: 14px; box-shadow: 0 24px 48px -20px rgba(0, 0, 0, 0.85), 0 0 0 1px rgba(255, 255, 255, 0.06); transform: scale(calc(1 + var(--beat, 0) * 0.008)); }
.pp__info { margin-top: clamp(16px, 3.4vh, 28px); flex: none; }
.pp__titlerow { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.pp__titlerow :deep(.badge) { margin-top: 6px; flex: none; }
.pp__titles { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.pp__title { font-size: 24px; font-weight: 750; letter-spacing: -0.025em; line-height: 1.2; }
.pp__artist { font-size: 17px; color: var(--text-2); }
.pp__reason { display: flex; align-items: center; gap: 5px; margin-top: 6px; font-size: var(--fs-small); color: var(--accent); }
/* Исполнитель и подпись версии в одну строку: имя сжимается, подпись — нет */
.pp__byline { display: flex; align-items: center; gap: 10px; min-width: 0; }
.pp__byline .pp__artist { min-width: 0; }
.pp__scrub { margin-top: clamp(14px, 2.4vh, 20px); flex: none; }
.pp__controls { margin-top: clamp(8px, 2.1vh, 18px); padding: 0 4px; flex: none; }
.pp__like { display: inline-flex; flex: none; margin: -7px -8px 0 -4px; }
/* С сердцем значок качества прижат к нему, а не висит посередине */
.pp__titlerow:has(.pp__like) .pp__titles { flex: 1; }
.pp__connect { justify-content: center; margin-top: clamp(8px, 1.6vh, 14px); flex: none; }
.pp__secondary { display: flex; align-items: center; justify-content: space-between; margin-top: auto; padding-top: 10px; flex: none; }
/* ---------- Телефон: текст «сценой» ---------- */
.pp__spacer { display: none; }
.pp.is-lyrics .pp__spacer { display: block; flex: 1; }
.pp.is-lyrics .pp__secondary { margin-top: 0; }
.pp__now { display: flex; align-items: center; gap: 12px; min-width: 0; height: 52px; margin: 0 6px; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
.pp__now-art { width: 44px; height: 44px; flex: none; border-radius: 8px; box-shadow: 0 8px 18px -8px rgba(0, 0, 0, 0.6); }
.pp__now-art :deep(.art) { width: 100%; height: 100%; border-radius: 8px; }
.pp__now-titles { display: flex; flex-direction: column; min-width: 0; }
.pp__now-title { font-size: 16px; font-weight: 700; letter-spacing: -0.015em; line-height: 1.25; }
.pp__now-artist { font-size: 14px; color: var(--text-2); line-height: 1.3; }
.pp__now:active { opacity: 0.75; }
.pp__lyrics {
  position: absolute;
  top: calc(var(--safe-top) + 52px);
  left: 6px;
  right: 6px;
  z-index: 1;
  transition: bottom 520ms var(--ease-out), opacity 420ms var(--ease-out) 120ms;
}
.pp-lyrics-enter-from, .pp-lyrics-leave-to { opacity: 0; }
/* Слушает и не трогает — управление уходит, текст занимает экран */
.pp__dock { transition: opacity 420ms var(--ease-out), transform 520ms var(--ease-out); }
.pp.is-calm .pp__dock { opacity: 0; transform: translateY(18px); pointer-events: none; }
/* С текстом фон — тона обложки на весь экран, волна уходит, чтобы не спорить со строками */
.pp__wave, .pp__ambient { transition: opacity var(--dur-4) var(--ease-out), height var(--dur-4) var(--ease-out); }
.pp.is-lyrics .pp__wave { opacity: 0; }
.pp.is-lyrics .pp__ambient { height: 100%; }
.pp__frame { transition: background-color var(--dur-4) var(--ease-out); }
.pp.is-lyrics .pp__frame { background-color: color-mix(in oklab, var(--amb-3) 62%, #020509); }
/* ---------- Планшет ---------- */
.pp--tablet .pp__wave { height: 100%; opacity: 0.55; -webkit-mask-image: none; mask-image: none; }
.pp--tablet .pp__ambient { height: 100%; }
.tp {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: var(--safe-top) calc(32px + var(--safe-right)) calc(var(--safe-bottom) + 16px) calc(32px + var(--safe-left));
}
.tp__head { display: grid; grid-template-columns: minmax(44px, 1fr) minmax(0, 3fr) minmax(44px, 1fr); align-items: center; height: 64px; margin: 0 -8px; flex: none; }
.tp__head-end { display: flex; justify-content: flex-end; gap: 4px; }
.tp__main { display: flex; flex: 1; flex-direction: column; gap: 20px; min-height: 0; }
.tp__player { display: flex; flex-direction: column; align-items: stretch; width: min(560px, 100%); margin: 0 auto; flex: none; }
.tp__art { position: relative; width: min(100%, 38vh, 440px); margin: 8px auto 0; flex: none; border-radius: var(--r-4); }
.tp__art :deep(.art) { width: 100%; border-radius: var(--r-4); box-shadow: 0 28px 56px -24px rgba(0, 0, 0, 0.85), 0 0 0 1px rgba(255, 255, 255, 0.06); transform: scale(calc(1 + var(--beat, 0) * 0.008)); }
.tp__info { margin-top: 24px; }
.tp__title { font-size: 26px; }
.pp__artist a:active { color: var(--text-1); }
.tp__scrub { margin-top: 16px; }
.tp__controls { margin-top: 10px; padding: 0 12px; }
.tp__secondary { display: flex; align-items: center; justify-content: center; gap: 32px; margin-top: 8px; }
/* Вертикально: текст и очередь — листом снизу, во всю ширину */
.tp__pane {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  margin: 0 calc(-32px - var(--safe-right)) calc(-16px - var(--safe-bottom)) calc(-32px - var(--safe-left));
  padding: 14px 24px 0;
  border-radius: 28px 28px 0 0;
  background: rgba(14, 16, 21, 0.72);
  -webkit-backdrop-filter: blur(30px) saturate(1.3);
  backdrop-filter: blur(30px) saturate(1.3);
  box-shadow: 0 -1px 0 rgba(255, 255, 255, 0.08);
}
.tp__tabs { width: min(360px, 100%); margin: 0 auto; flex: none; }
.tp__pane-body { flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding-bottom: calc(var(--safe-bottom) + 16px); }
/* Горизонтально: обложка с управлением слева, текст и очередь — колонкой справа */
@media (orientation: landscape) {
  .tp__main { flex-direction: row; align-items: stretch; gap: 48px; }
  .tp__player { justify-content: center; width: min(460px, 42%); margin: 0; padding-bottom: 8px; }
  .tp__art { width: min(100%, 46vh); }
  .tp__pane { flex: 1; margin: 0; padding: 14px 16px 0; border-radius: var(--r-5); background: rgba(255, 255, 255, 0.04); -webkit-backdrop-filter: none; backdrop-filter: none; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06); }
  .tp__pane-body { padding-bottom: 16px; }
  /* Текст — прямо на сцене, без карточки: как справа от обложки на компьютере */
  .tp .tp__pane.is-lyrics { padding: 14px 0 0; background: none; box-shadow: none; }
}
/* Текст «сценой»: фон — тона обложки, лента сама держит строку, без своей прокрутки */
.pp.is-lyrics-pane .pp__frame { background-color: color-mix(in oklab, var(--amb-3) 62%, #020509); }
.pp.is-lyrics-pane .pp__wave { opacity: 0.3; }
.tp__pane.is-lyrics { background: color-mix(in oklab, var(--amb-3) 30%, rgba(10, 12, 16, 0.55)); }
.tp__pane.is-lyrics .tp__pane-body { position: relative; overflow: hidden; padding-bottom: 0; }

/* ---------- Светлая тема: светлая сцена ----------
   Плеер не выпадает тёмным островом между светлыми панелями. Фон — спокойная пастель тона
   обложки (те же --amb-*, что у фона приложения): яркость постоянная и высокая, насыщенность
   ограничена — на любой обложке, даже жёлтой, тёмный текст читается (вторичный ≥ 6,5:1,
   третичный ≥ 5:1). Волна — краской по бумаге и умножается на фон: бумага берёт цвет пастели */
:root[data-theme='light'] .fp,
:root[data-theme='light'] .pp {
  --text-2: rgba(16, 18, 24, 0.74);
  --text-3: rgba(16, 18, 24, 0.68);
  --pastel: color-mix(in oklab, var(--amb-1) 12%, #f3f3f5);
  --pastel-glow: color-mix(in oklab, var(--amb-1) 22%, transparent);
  --pastel-glow-2: color-mix(in oklab, var(--amb-2) 16%, transparent);
  /* Текст акцентом (почему играет, где играет) — глубже: на пастели обычный акцент не дотягивает до 4,5:1 */
  --accent-ink: color-mix(in oklab, var(--accent) 72%, #101218);
}
@supports (color: oklch(from red l c h)) {
  :root[data-theme='light'] .fp,
  :root[data-theme='light'] .pp {
    --pastel: oklch(from var(--amb-1) 0.945 min(c, 0.032) h);
    --pastel-glow: oklch(from var(--amb-1) 0.89 min(c, 0.07) h / 0.7);
    --pastel-glow-2: oklch(from var(--amb-2) 0.9 min(c, 0.06) h / 0.5);
  }
}
:root[data-theme='light'] .fp__reason,
:root[data-theme='light'] .pp__reason,
:root[data-theme='light'] .fp :deep(.cc:not(.is-lost)),
:root[data-theme='light'] .pp :deep(.cc:not(.is-lost)),
:root[data-theme='light'] .pp :deep(.scrub__mid) { color: var(--accent-ink); }
:root[data-theme='light'] .fp__pending { background: var(--glass-strong); box-shadow: inset 0 0 0 1px var(--hair), var(--shadow-1); }

/* Сцена на компьютере */
:root[data-theme='light'] .fp,
:root[data-theme='light'] .fp--lyrics {
  background:
    radial-gradient(60% 55% at 50% 38%, var(--pastel-glow), transparent 75%),
    radial-gradient(45% 40% at 96% 4%, var(--pastel-glow-2), transparent 72%),
    var(--pastel);
}
:root[data-theme='light'] .fp__wave { mix-blend-mode: multiply; opacity: 0.75; }
:root[data-theme='light'] .fp--lyrics .fp__wave { opacity: 0.22; }
:root[data-theme='light'] .fp__wave::after { display: none; }
:root[data-theme='light'] .fp__context,
:root[data-theme='light'] .fp__mood { background: rgba(255, 255, 255, 0.62); box-shadow: inset 0 0 0 1px var(--hair), 0 6px 18px -12px rgba(16, 18, 24, 0.25); }

/* Телефон и планшет */
:root[data-theme='light'] .pp-scrim { background: var(--overlay); }
:root[data-theme='light'] .pp__frame,
:root[data-theme='light'] .pp.is-lyrics .pp__frame,
:root[data-theme='light'] .pp.is-lyrics-pane .pp__frame { background-color: var(--pastel); }
:root[data-theme='light'] .pp__ambient {
  background:
    radial-gradient(90% 46% at 50% 30%, var(--pastel-glow), transparent 72%),
    radial-gradient(70% 40% at 92% 10%, var(--pastel-glow-2), transparent 70%);
}
:root[data-theme='light'] .pp__wave { mix-blend-mode: multiply; opacity: 0.6; }
:root[data-theme='light'] .pp.is-lyrics .pp__wave { opacity: 0; }
:root[data-theme='light'] .pp.is-lyrics-pane .pp__wave { opacity: 0.22; }
:root[data-theme='light'] .pp__art :deep(.art),
:root[data-theme='light'] .tp__art :deep(.art) { box-shadow: 0 24px 48px -24px rgba(16, 18, 24, 0.42), 0 0 0 1px rgba(16, 18, 24, 0.06); }
:root[data-theme='light'] .pp__now-art { box-shadow: 0 8px 18px -10px rgba(16, 18, 24, 0.4); }
:root[data-theme='light'] .tp__pane,
:root[data-theme='light'] .tp__pane.is-lyrics { background: rgba(255, 255, 255, 0.5); box-shadow: 0 -1px 0 var(--hair); }
@media (orientation: landscape) {
  :root[data-theme='light'] .tp__pane { background: rgba(255, 255, 255, 0.45); box-shadow: inset 0 0 0 1px var(--hair); }
  :root[data-theme='light'] .tp .tp__pane.is-lyrics { background: none; box-shadow: none; }
}

@media (prefers-reduced-motion: reduce) {
  .pp__frame, .pp-scrim { animation: none; }
  .pp.is-settling .pp__frame { transition: none; }
}
</style>
