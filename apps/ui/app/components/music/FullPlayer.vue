<script setup lang="ts">
import { DialogContent, DialogDescription, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'
import type { WaveMood } from '~/bridge/types'
import { getWaveEngine } from '~/lib/wave/engine'
import { resamplePeaks } from '~/utils/peaks'

// Плеер на весь экран. Большой экран — сцена поверх содержимого (боковая панель и панель плеера
// остаются), волна обнимает обложку. Телефон — отдельный слой с управлением под большим пальцем
const props = withDefaults(defineProps<{ variant?: 'stage' | 'phone' }>(), { variant: 'stage' })

const player = usePlayerStore()
const library = useLibraryStore()
const ui = useUiStore()
const bridge = useBridge()
const { menuFor } = useTrackActions()
const { t } = useI18n()

const entry = computed(() => player.current)
const track = computed(() => entry.value?.track ?? null)
const liked = computed(() => (track.value ? library.isLiked(track.value) : false))
const art = ref<HTMLElement | null>(null)
const nextAt = computed(() => (player.nextEntry && !player.pending && player.effectiveMode !== 'gapless' && player.durationMs ? formatDuration(player.transitionAtMs) : null))
const phonePeaks = computed(() => (player.peaks.length ? resamplePeaks(player.peaks, 56) : Array.from({ length: 56 }, () => 0.18)))
const menuOpen = ref(false)
const menuItems = computed(() => (menuOpen.value && track.value ? menuFor(track.value) : []))

const CONTEXT_KEY: Record<string, string> = { album: 'record.album', playlist: 'record.playlist', artist: 'record.artist', radio: 'record.radio' }
const contextLabel = computed(() => {
  const c = player.context
  if (!c) return ''
  if (c.type === 'wave') return player.wave.mood === 'any' ? t('wave.title') : `${t('wave.title')} · ${t(`wave.mood.${player.wave.mood}`)}`
  const kind = CONTEXT_KEY[c.type]
  return kind ? `${t(kind)} · ${c.title}` : c.title
})
const kicker = computed(() => (player.waveActive ? t('player.playingWave') : t('player.playingFrom')))
const phoneTitle = computed(() => (player.waveActive ? contextLabel.value : player.context?.title ?? ''))

const quality = computed(() => {
  if (player.source?.preview) return t('player.preview')
  if (!player.source) return null
  return qualityLabel(player.source) ?? t('player.quality')
})
const bpm = computed(() => (player.source?.bpm ? `${Math.round(player.source.bpm)} BPM` : null))
const liveStatus = computed(() => (track.value ? library.statusOf(track.value) : 'none'))
const statusLabel = computed(() => (track.value ? t(`track.status.${liveStatus.value}`) : ''))

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

// Обложка чуть дышит в долю
const beatEl = ref<HTMLElement | null>(null)
let offBeat: (() => void) | null = null

onMounted(() => {
  window.addEventListener('keydown', onKey)
  offBeat = getWaveEngine().onFrame((s) => {
    beatEl.value?.style.setProperty('--beat', s.beat.toFixed(3))
  })
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  offBeat?.()
})
</script>

<template>
  <!-- Большой экран: сцена поверх содержимого -->
  <Transition v-if="variant === 'stage'" name="stage">
    <section v-if="open && track" ref="beatEl" class="fp" :aria-label="t('player.expand')">
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
        <div ref="art" class="fp__art">
          <MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager />
          <div v-if="player.pending" class="fp__pending">
            <MusicPendingNote :pending="player.pending" long />
            <UiButton size="sm" icon="ph:skip-forward" @click="player.next('skipped')">{{ t('player.skip') }}</UiButton>
          </div>
        </div>
        <div class="fp__info">
          <h2 class="fp__title" :title="track.title">{{ track.title }}</h2>
          <div class="fp__artist">
            <template v-for="(a, i) in track.artists" :key="a.id"><span v-if="i">, </span><NuxtLink :to="`/artist/${a.id}`" @click="close">{{ a.name }}</NuxtLink></template>
          </div>
          <div v-if="entry?.reason" class="fp__reason"><Icon name="ph:sparkle-fill" />{{ typographRu(entry.reason) }}</div>
          <div class="fp__chips">
            <UiBadge v-if="quality" variant="quality" icon="ph:waveform">{{ quality }}</UiBadge>
            <UiBadge v-if="bpm" variant="meta">{{ bpm }}</UiBadge>
            <UiBadge v-if="!player.source?.preview && !player.pending" variant="meta" :icon="liveStatus === 'ready' ? 'ph:check-circle-fill' : 'ph:cloud-arrow-down'">{{ statusLabel }}</UiBadge>
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
    </section>
  </Transition>

  <!-- Телефон: отдельный слой -->
  <DialogRoot v-else v-model:open="open">
    <DialogPortal>
      <DialogContent v-if="track" class="pp" :aria-describedby="undefined" @open-auto-focus.prevent>
        <DialogTitle class="sr-only">{{ track.title }}</DialogTitle>
        <DialogDescription class="sr-only">{{ contextLabel }}</DialogDescription>
        <div class="pp__wave"><MusicWaveCanvas layout="full-phone" /></div>
        <div class="pp__body">
          <header class="pp__head">
            <UiIconButton icon="ph:caret-down" :label="t('player.collapse')" size="lg" tone="solid" :tooltip="false" @click="close" />
            <div class="pp__ctx">
              <div class="pp__kicker">{{ kicker }}</div>
              <div class="pp__ctx-title ellipsis">{{ phoneTitle }}</div>
            </div>
            <UiMenu v-model:open="menuOpen" :items="menuItems">
              <UiIconButton icon="ph:dots-three" :label="t('common.more')" size="lg" tone="solid" :tooltip="false" />
            </UiMenu>
          </header>

          <div class="pp__art">
            <MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager />
            <div v-if="player.pending" class="fp__pending">
              <MusicPendingNote :pending="player.pending" long />
              <UiButton size="sm" icon="ph:skip-forward" @click="player.next('skipped')">{{ t('player.skip') }}</UiButton>
            </div>
          </div>

          <div class="pp__info">
            <div class="pp__titlerow">
              <div class="pp__titles">
                <div class="pp__title ellipsis">{{ track.title }}</div>
                <div class="pp__artist ellipsis">{{ track.artists.map(a => a.name).join(', ') }}</div>
              </div>
              <UiBadge v-if="quality" variant="quality" icon="ph:waveform">{{ quality }}</UiBadge>
            </div>
            <div v-if="entry?.reason" class="pp__reason ellipsis"><Icon name="ph:sparkle-fill" />{{ typographRu(entry.reason) }}</div>
          </div>

          <MusicScrubber
            class="pp__scrub"
            :peaks="phonePeaks"
            :position-ms="player.positionMs"
            :duration-ms="player.durationMs || track.durationMs"
            :transition-ms="nextAt ? player.transitionAtMs : null"
            size="lg"
            times="below"
            :label="t('player.seek')"
            @seek="player.seek"
          />

          <div class="pp__controls">
            <UiIconButton icon="ph:thumbs-down" :label="t('player.dislike')" size="xl" tone="solid" :tooltip="false" @click="player.dislike()" />
            <UiIconButton icon="ph:skip-back-fill" :label="t('player.prev')" size="xl" tone="solid" :tooltip="false" @click="player.prev()" />
            <MusicWavePlayButton v-if="player.waveActive" :playing="player.playing" :loading="!!player.pending" :size="80" @click="player.toggle()" />
            <MusicPlayButton v-else :playing="player.playing" :loading="!!player.pending" :size="72" @click="player.toggle()" />
            <UiIconButton icon="ph:skip-forward-fill" :label="t('player.next')" size="xl" tone="solid" :tooltip="false" @click="player.next('skipped')" />
            <UiIconButton :icon="liked ? 'ph:heart-fill' : 'ph:heart'" :label="liked ? t('player.unlike') : t('player.like')" size="xl" :tone="liked ? 'liked' : 'solid'" :tooltip="false" @click="player.toggleLike()" />
          </div>

          <div class="pp__secondary">
            <UiIconButton icon="ph:devices" :label="t('player.devices')" size="lg" :tooltip="false" @click="ui.openPanel('devices'); player.loadDevices()" />
            <MusicTransitionModePicker v-model:mode="player.transition" v-model:expressiveness="player.expressiveness" :web="bridge.kind === 'web'" :next-at="nextAt" />
            <UiIconButton icon="ph:microphone-stage" :label="t('player.lyrics')" size="lg" :tooltip="false" @click="ui.openPanel('lyrics')" />
            <UiIconButton icon="ph:queue" :label="t('player.queue')" size="lg" :tooltip="false" @click="ui.openPanel('queue')" />
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
.fp__collapse { position: absolute; left: 20px; top: 12px; z-index: 2; }
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
.fp__dot { width: 7px; height: 7px; flex: none; border-radius: 50%; background: var(--accent); box-shadow: 0 0 8px var(--accent); }
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
.pp { position: fixed; inset: 0; z-index: 90; overflow: hidden; background: #020509; animation: sheet-in var(--dur-4) var(--ease-out); outline: none; }
.pp__wave { position: absolute; top: 0; left: 0; right: 0; height: 62%; }
.pp__wave::after { content: ''; position: absolute; inset: 0; pointer-events: none; background: linear-gradient(0deg, #020509 0%, rgba(2, 5, 9, 0) 34%), linear-gradient(180deg, rgba(2, 5, 9, 0.6) 0%, rgba(2, 5, 9, 0) 22%); }
.pp__body { position: relative; z-index: 1; display: flex; flex-direction: column; height: 100%; padding: var(--safe-top) 24px calc(var(--safe-bottom) + 8px); overflow-y: auto; }
.pp__head { display: grid; grid-template-columns: 44px minmax(0, 1fr) 44px; align-items: center; height: 52px; margin: 0 -10px; flex: none; }
.pp__ctx { min-width: 0; text-align: center; }
.pp__kicker { font-size: 10.5px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.pp__ctx-title { font-size: 15px; font-weight: 650; }
.pp__art { position: relative; width: min(100%, 44vh); margin: 18px auto 0; flex: none; }
.pp__art :deep(.art) { width: 100%; border-radius: 14px; box-shadow: var(--shadow-cover); }
.pp__info { margin-top: 26px; flex: none; }
.pp__titlerow { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.pp__titlerow :deep(.badge) { margin-top: 6px; flex: none; }
.pp__titles { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.pp__title { font-size: 24px; font-weight: 750; letter-spacing: -0.025em; line-height: 1.2; }
.pp__artist { font-size: 17px; color: var(--text-2); }
.pp__reason { display: flex; align-items: center; gap: 5px; margin-top: 6px; font-size: var(--fs-small); color: var(--accent); }
.pp__scrub { margin-top: 20px; flex: none; }
.pp__controls { display: flex; align-items: center; justify-content: space-between; margin-top: 18px; padding: 0 4px; flex: none; }
.pp__controls :deep(.ib__icon) { font-size: 28px; }
.pp__secondary { display: flex; align-items: center; justify-content: space-between; margin-top: auto; padding-top: 12px; flex: none; }
.pp__secondary :deep(.mode-chip) { height: 32px; }
</style>
