<script setup lang="ts">
// Ряд управления плеера под пальцем: перемешивание, назад, ▶, вперёд, повтор. Один на полный плеер
// телефона и планшета и на плеер сбоку — отличаются только размером главной кнопки
withDefaults(defineProps<{ playSize?: number }>(), { playSize: 80 })

const player = usePlayerStore()
const { t } = useI18n()
const audible = computed(() => player.playing && !player.pending && !player.muted && player.volume > 0)
</script>

<template>
  <div class="pc">
    <MusicPlayModeButton kind="shuffle" size="lg" />
    <UiIconButton icon="ph:skip-back-fill" :label="t('player.prev')" size="xl" tone="solid" :tooltip="false" @click="player.prev()" />
    <MusicWavePlayButton
      variant="soft"
      :playing="player.playing"
      :loading="!!player.pending"
      :beat="audible"
      :size="playSize"
      :label="player.playing ? t('common.pause') : t('common.play')"
      @click="player.toggle()"
    />
    <UiIconButton icon="ph:skip-forward-fill" :label="t('player.next')" size="xl" tone="solid" :tooltip="false" @click="player.next('skipped')" />
    <MusicPlayModeButton kind="repeat" size="lg" />
  </div>
</template>

<style scoped>
.pc { display: flex; align-items: center; justify-content: space-between; }
.pc :deep(.ib__icon) { font-size: 28px; }
/* Перемешивание и повтор — тише главных кнопок, как в Spotify и Apple Music */
.pc :deep(.ib--lg .ib__icon) { font-size: 24px; }
</style>
