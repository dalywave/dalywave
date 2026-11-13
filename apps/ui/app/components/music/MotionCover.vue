<script setup lang="ts">
import type { Track } from '~/api/types'

// Видео вместо обложки (как Canvas у Spotify): зацикленный кусок клипа трека без звука, нет клипа —
// живая обложка альбома. Кадр обрезается по рамке (object-fit: cover): в квадрате обложки и на
// вертикальном фоне телефона (`tall`). Ставится внутрь блока обложки с position: relative.
// Нет видео, выключено в настройках или «уменьшение движения» — не рисуется вовсе
const props = withDefaults(defineProps<{
  track?: Pick<Track, 'id'> | null
  shape?: 'square' | 'tall'
  active?: boolean
}>(), { track: null, shape: 'square', active: true })

const motion = useMotionStore()
const src = computed(() => motion.src(props.track, props.shape))
</script>

<template>
  <div v-if="src" class="mc">
    <MusicLoopVideo :src="src" :active="active" />
  </div>
</template>

<style scoped>
.mc {
  position: absolute;
  inset: 0;
  z-index: 1;
  overflow: hidden;
  border-radius: inherit;
  pointer-events: none;
}
</style>
