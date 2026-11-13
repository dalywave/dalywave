<script setup lang="ts">
// Звучит отрывок с припева («послушать припев» перед включением): карточка над плеером —
// обложка, трек, сколько осталось; «Слушать» включает трек целиком, крестик возвращает прежнее
const hook = useHookPreviewStore()
const ui = useUiStore()
const player = usePlayerStore()
const { t } = useI18n()

// Как у уведомлений: над тем, что внизу экрана
const bottom = computed(() => {
  if (ui.isTouch) return ui.playerOpen ? 'calc(var(--safe-bottom) + 24px)' : 'calc(var(--safe-bottom) + var(--dock-h) + 28px)'
  if (!ui.isPhone) return 'calc(var(--bar-h) + 16px)'
  if (ui.playerOpen) return '76px'
  return player.current ? 'calc(var(--tabbar-h) + var(--mini-h) + 14px)' : 'calc(var(--tabbar-h) + 12px)'
})
const artists = computed(() => hook.track?.artists.map(a => a.name).join(', ') ?? '')

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && hook.track) hook.stop()
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <Transition name="hp">
    <div v-if="hook.track" class="hp" :style="{ '--hp-bottom': bottom }" role="status" :aria-label="`${t('hook.title')}: ${hook.track.title}`">
      <MusicCoverArt :image="hook.track.album?.image ?? null" size="small" :alt="hook.track.album?.title ?? ''" class="hp__art" />
      <div class="hp__text">
        <div class="hp__title ellipsis">{{ hook.track.title }}</div>
        <div class="hp__sub ellipsis">{{ artists }}</div>
      </div>
      <UiButton size="sm" variant="primary" class="hp__listen" @click="hook.listen()">{{ t('hook.play') }}</UiButton>
      <UiIconButton icon="ph:x" :label="t('hook.close')" size="sm" class="hp__close" :tooltip="false" @click="hook.stop()" />
      <span class="hp__bar" aria-hidden="true"><i :class="{ 'is-loading': hook.loading }" :style="{ transform: `scaleX(${hook.loading ? 1 : hook.progress})` }" /></span>
    </div>
  </Transition>
</template>

<style scoped>
.hp {
  position: fixed;
  left: 50%;
  bottom: var(--hp-bottom);
  z-index: 60;
  display: flex;
  align-items: center;
  gap: 12px;
  width: min(420px, calc(100vw - 32px));
  padding: 8px 8px 8px 8px;
  overflow: hidden;
  border-radius: var(--r-4);
  background: var(--ink-4);
  box-shadow: var(--shadow-pop);
  transform: translateX(-50%);
}
.hp__art { flex: none; width: 44px; border-radius: var(--r-1); }
.hp__text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
.hp__title { font-weight: 600; font-size: var(--fs-body); }
.hp__sub { font-size: var(--fs-small); color: var(--text-2); }
.hp__listen { flex: none; }
.hp__close { flex: none; color: var(--text-2); }
.hp__bar { position: absolute; left: 0; right: 0; bottom: 0; height: 2px; background: var(--hair); }
.hp__bar i { position: absolute; inset: 0; background: var(--accent); transform-origin: left; }
.hp__bar i.is-loading { opacity: 0.35; animation: hp-wait 1s var(--ease-in-out) infinite alternate; }
@keyframes hp-wait { from { opacity: 0.15; } to { opacity: 0.5; } }
.hp-enter-active, .hp-leave-active { transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out); }
.hp-enter-from, .hp-leave-to { opacity: 0; transform: translate(-50%, 10px); }

@media (pointer: coarse) {
  .hp { min-height: 60px; }
  .hp__close { --size: 44px; }
}
@media (max-width: 699px) {
  .hp { width: calc(100vw - 16px); padding: 8px 6px 8px 8px; }
}
@media (prefers-reduced-motion: reduce) {
  .hp__bar i.is-loading { animation: none; }
}
</style>
