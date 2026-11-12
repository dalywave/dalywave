<script setup lang="ts">
// Мини-плеер телефона над вкладками: нажатие раскрывает плеер, справа — пауза и следующий.
// Внизу — тонкая сплошная линия прогресса. Пульт: вместо исполнителя — где играет; активное
// устройство пропало — тихое «Было на: …», а ▶ продолжает здесь с того же места.
// dock — карточка в нижней линии планшета рядом с вкладками: цвет — из обложки, справа «Где играть» и ▶
import type { ConnectKind } from '~/api/types'

withDefaults(defineProps<{ inline?: boolean, dock?: boolean }>(), {})

const player = usePlayerStore()
const connect = useConnectStore()
const ui = useUiStore()
const { t } = useI18n()
const track = computed(() => player.current?.track ?? null)
const ICON: Record<ConnectKind, string> = {
  laptop: 'ph:laptop',
  desktop: 'ph:desktop',
  phone: 'ph:device-mobile',
  tablet: 'ph:device-tablet',
  web: 'ph:browser',
}

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
    :class="{ 'mini--inline': inline, 'mini--dock': dock, 'is-lifting': lift }"
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
      <MusicWavePrepNote v-if="player.waveStarting" />
      <span v-else-if="connect.remote" class="mini__sub mini__sub--remote ellipsis"><Icon :name="ICON[connect.activeKind]" class="mini__dev" />{{ connect.activeName }}</span>
      <span v-else-if="connect.orphaned" class="mini__sub mini__sub--lost ellipsis">{{ t('connect.wasOn', { name: connect.activeName }) }}</span>
      <MusicPendingNote v-else-if="player.pending" :pending="player.pending" />
      <span v-else class="mini__sub ellipsis">{{ track.artists.map(a => a.name).join(', ') }}</span>
    </button>
    <div class="mini__actions">
      <UiIconButton v-if="dock" icon="ph:devices" :label="t('player.devices')" size="lg" tone="solid" :active="connect.remote" :tooltip="false" @click="ui.openPanel('devices')" />
      <UiIconButton :icon="player.playing ? 'ph:pause-fill' : 'ph:play-fill'" :label="player.playing ? t('common.pause') : connect.orphaned ? t('connect.continueHere') : t('common.play')" size="lg" tone="solid" :tooltip="false" @click="player.toggle()" />
      <UiIconButton v-if="!dock" icon="ph:skip-forward-fill" :label="t('player.next')" size="lg" tone="solid" :tooltip="false" @click="player.next('skipped')" />
    </div>
    <span class="mini__progress" aria-hidden="true"><i :style="{ transform: `scaleX(${player.progress})` }" /></span>
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
.mini__actions { display: flex; align-items: center; }
.mini__actions :deep(.ib__icon) { font-size: 24px; }
.mini__sub--remote { color: var(--accent); font-weight: 600; }
.mini__sub--lost { color: var(--text-3); }
.mini__dev { flex: none; font-size: 13px; }
/* Прогресс — тонкая сплошная линия по нижнему краю: пройденное акцентом, остальное — заливкой */
.mini__progress { position: absolute; left: 12px; right: 12px; bottom: 0; height: 2px; overflow: hidden; border-radius: 1px; background: var(--fill-3); }
.mini__progress i { position: absolute; inset: 0; background: var(--accent); transform-origin: left; }

/* Планшет: карточка в линии с вкладками. Фон — приглушённый цвет обложки (как у Spotify):
   цвет говорит, что играет, и не спорит с белым текстом */
.mini--dock {
  position: relative;
  left: auto;
  right: auto;
  bottom: auto;
  z-index: auto;
  grid-template-columns: 48px minmax(0, 1fr) auto;
  height: var(--dock-h);
  padding: 0 8px 0 8px;
  border-radius: 22px;
  background: color-mix(in oklab, var(--amb-1) 46%, var(--ink-1));
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08), 0 18px 36px -14px rgba(0, 0, 0, 0.9);
  transition: background-color var(--dur-mood) var(--ease-in-out), transform 260ms cubic-bezier(0.22, 1, 0.36, 1);
}
.mini--dock :deep(.art) { width: 48px; border-radius: 10px; }
.mini--dock .mini__sub { color: rgba(255, 255, 255, 0.72); font-size: var(--fs-small); }
.mini--dock .mini__progress { left: 16px; right: 16px; bottom: 0; background: rgba(255, 255, 255, 0.16); }
.mini--dock .mini__progress i { background: #fff; }
</style>
