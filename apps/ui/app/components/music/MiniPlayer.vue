<script setup lang="ts">
// Мини-плеер телефона над вкладками: нажатие раскрывает плеер, справа — пауза и следующий
withDefaults(defineProps<{ inline?: boolean }>(), {})

const player = usePlayerStore()
const ui = useUiStore()
const { t } = useI18n()
const track = computed(() => player.current?.track ?? null)

function open(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('button')) return
  ui.playerOpen = true
}
</script>

<template>
  <div v-if="track" class="mini" :class="{ 'mini--inline': inline }" :style="{ '--p': player.progress }" @click="open">
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
    <span class="mini__progress" aria-hidden="true"><i /></span>
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
  animation: toast-in var(--dur-3) var(--ease-out);
}
.mini--inline { position: relative; left: auto; right: auto; bottom: auto; z-index: auto; width: 100%; animation: none; }
.mini :deep(.art) { width: 44px; border-radius: 8px; }
.mini__meta { display: flex; flex-direction: column; min-width: 0; padding: 0; border: 0; background: none; text-align: left; }
.mini__title { font-size: var(--fs-body); font-weight: 600; }
.mini__sub { display: flex; align-items: center; gap: 4px; font-size: var(--fs-caption); color: var(--text-2); }
.mini__wave { flex: none; font-size: 13px; color: var(--accent); }
.mini__actions { display: flex; align-items: center; }
.mini__actions :deep(.ib__icon) { font-size: 24px; }
.mini__progress { position: absolute; left: 14px; right: 14px; bottom: 0; height: 2px; border-radius: 2px; background: rgba(255, 255, 255, 0.1); overflow: hidden; }
.mini__progress i { display: block; width: calc(var(--p, 0) * 100%); height: 100%; background: var(--accent); }
</style>
