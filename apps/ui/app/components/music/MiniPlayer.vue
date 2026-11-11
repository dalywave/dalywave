<script setup lang="ts">
// Мини-плеер телефона над вкладками: нажатие раскрывает плеер, справа — пауза и следующий.
// Внизу — прогресс в том же виде, что выбран для полосы прокрутки: волна или линия
withDefaults(defineProps<{ inline?: boolean }>(), {})

const player = usePlayerStore()
const ui = useUiStore()
const { t } = useI18n()
const track = computed(() => player.current?.track ?? null)
const peaks = computed(() => resamplePeaks(player.peaks, 64))
const marker = computed(() => (player.nextEntry && !player.pending && player.effectiveMode !== 'gapless' ? markerRatio(player.transitionAtMs, player.durationMs) : null))

function open(e: MouseEvent) {
  if (swallow) {
    e.stopPropagation()
    e.preventDefault()
    return
  }
  if ((e.target as HTMLElement).closest('button')) return
  ui.playerOpen = true
}

// Смахнуть вверх — раскрыть плеер. Мини-плеер чуть едет за пальцем
const lift = ref(0)
let swipe: { id: number, x: number, y: number, lastY: number, lastT: number, v: number, active: boolean } | null = null
let swallow = false

function onDown(e: PointerEvent) {
  if (!e.isPrimary || (e.pointerType === 'mouse' && e.button !== 0)) return
  swipe = { id: e.pointerId, x: e.clientX, y: e.clientY, lastY: e.clientY, lastT: e.timeStamp, v: 0, active: false }
}

function onMove(e: PointerEvent) {
  const g = swipe
  if (!g || e.pointerId !== g.id) return
  const dx = e.clientX - g.x
  const dy = e.clientY - g.y
  if (!g.active) {
    if (Math.hypot(dx, dy) < 8) return
    if (dy >= 0 || Math.abs(dx) > Math.abs(dy)) {
      swipe = null
      return
    }
    g.active = true
    try {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    }
    catch { /* касание уже кончилось */ }
  }
  const dt = Math.max(1, e.timeStamp - g.lastT)
  g.v = 0.7 * ((e.clientY - g.lastY) / dt) + 0.3 * g.v
  g.lastY = e.clientY
  g.lastT = e.timeStamp
  lift.value = Math.max(-36, Math.min(0, dy * 0.5))
}

function onUp(e: PointerEvent) {
  const g = swipe
  if (!g || e.pointerId !== g.id) return
  swipe = null
  if (!g.active) return
  swallow = true
  setTimeout(() => (swallow = false), 0)
  if (e.clientY - g.y < -40 || g.v < -0.4) ui.playerOpen = true
  lift.value = 0
}

function onCancel() {
  swipe = null
  lift.value = 0
}
</script>

<template>
  <div
    v-if="track"
    class="mini"
    :class="{ 'mini--inline': inline, 'is-lifting': lift }"
    :style="lift ? { transform: `translate3d(0, ${lift}px, 0)` } : undefined"
    @click.capture="open"
    @pointerdown="onDown"
    @pointermove="onMove"
    @pointerup="onUp"
    @pointercancel="onCancel"
  >
    <MusicCoverArt :image="track.album?.image" size="small" :alt="track.album?.title ?? ''" />
    <button type="button" class="mini__meta" :aria-label="`${t('player.expand')}: ${track.title}`" @click="ui.playerOpen = true">
      <span class="mini__title ellipsis">{{ track.title }}</span>
      <MusicPendingNote v-if="player.pending" :pending="player.pending" />
      <span v-else class="mini__sub ellipsis"><Icon v-if="player.playing" name="ph:waveform" class="mini__wave" />{{ track.artists.map(a => a.name).join(', ') }}</span>
    </button>
    <div class="mini__actions">
      <UiIconButton :icon="player.playing ? 'ph:pause-fill' : 'ph:play-fill'" :label="player.playing ? t('common.pause') : t('common.play')" size="lg" tone="solid" :tooltip="false" @click="player.toggle()" />
      <UiIconButton icon="ph:skip-forward-fill" :label="t('player.next')" size="lg" tone="solid" :tooltip="false" @click="player.next('skipped')" />
    </div>
    <MusicScrubberMini class="mini__progress" :peaks="peaks" :progress="player.progress" :transition="marker" />
  </div>
</template>

<style scoped>
.mini {
  position: fixed;
  left: 8px;
  right: 8px;
  bottom: calc(var(--safe-bottom) + var(--tabbar-h) + 6px);
  z-index: 40;
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  height: var(--mini-h);
  padding: 0 6px 0 8px;
  border-radius: 16px;
  background: rgba(24, 27, 35, 0.8);
  -webkit-backdrop-filter: blur(30px) saturate(1.6);
  backdrop-filter: blur(30px) saturate(1.6);
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.07), 0 16px 32px -12px rgba(0, 0, 0, 0.9);
  overflow: hidden;
  cursor: pointer;
  touch-action: none;
  transition: transform 260ms cubic-bezier(0.22, 1, 0.36, 1);
  animation: toast-in var(--dur-3) var(--ease-out);
}
.mini.is-lifting { transition: none; }
@media (prefers-reduced-motion: reduce) {
  .mini { transition: none; }
}
.mini--inline { position: relative; left: auto; right: auto; bottom: auto; z-index: auto; width: 100%; animation: none; }
.mini :deep(.art) { width: 44px; border-radius: 8px; }
.mini__meta { display: flex; flex-direction: column; min-width: 0; padding: 0; border: 0; background: none; text-align: left; }
.mini__title { font-size: var(--fs-body); font-weight: 600; }
.mini__sub { display: flex; align-items: center; gap: 4px; font-size: var(--fs-caption); color: var(--text-2); }
.mini__wave { flex: none; font-size: 13px; color: var(--accent); }
.mini__actions { display: flex; align-items: center; }
.mini__actions :deep(.ib__icon) { font-size: 24px; }
.mini .mini__progress { position: absolute; left: 14px; right: 14px; bottom: 1px; }
</style>
