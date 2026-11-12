<script setup lang="ts">
// Низ экрана на планшете под пальцем (iPad), как у Spotify: плавающие стеклянные вкладки и рядом,
// в той же линии, карточка плеера цвета обложки. Ничего не играет — вкладки одни, по центру.
// Плеер сбоку (планшет горизонтально) — карточка не нужна: вкладки по центру колонки содержимого
const player = usePlayerStore()
const ui = useUiStore()
</script>

<template>
  <div class="dock" :class="{ 'dock--solo': !player.current || ui.sidePlayer, 'dock--side': ui.sidePlayer }">
    <ShellTabBar dock class="dock__tabs" />
    <MusicMiniPlayer v-if="!ui.sidePlayer" dock class="dock__player" />
  </div>
</template>

<style scoped>
.dock {
  position: fixed;
  left: calc(16px + var(--safe-left));
  right: calc(16px + var(--safe-right));
  bottom: calc(var(--safe-bottom) + 12px);
  z-index: 40;
  display: flex;
  align-items: stretch;
  gap: 12px;
  height: var(--dock-h);
  pointer-events: none;
}
.dock > * { pointer-events: auto; }
.dock__tabs { flex: 1.7 1 0; min-width: 0; }
.dock__player { flex: 1 1 0; min-width: 320px; }
.dock--solo { justify-content: center; }
.dock--solo .dock__tabs { flex: 0 1 560px; }
.dock--side { right: calc(var(--side-w) + var(--side-gap) * 2 + var(--safe-right)); }
.dock--side .dock__tabs { flex-basis: 480px; }
</style>
