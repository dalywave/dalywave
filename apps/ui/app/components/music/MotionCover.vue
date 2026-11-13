<script setup lang="ts">
// Живая обложка альбома поверх обычной: квадратная — в рамке обложки, вертикальная (`tall`) — для
// фона телефона; вертикальной нет — квадратная. Ставится внутрь блока обложки с position: relative.
// Нет живой обложки, выключена в настройках или «уменьшение движения» — не рисуется вовсе
const props = withDefaults(defineProps<{
  albumId?: string | null
  shape?: 'square' | 'tall'
  active?: boolean
}>(), { shape: 'square', active: true })

const motion = useMotionStore()
const ready = ref(false)
const src = computed(() => {
  const m = motion.get(props.albumId)
  if (!m) return null
  return props.shape === 'tall' ? (m.tall ?? m.square) : (m.square ?? m.tall)
})
watch(src, () => (ready.value = false))
</script>

<template>
  <div v-if="src" class="mc" :class="{ 'is-ready': ready }">
    <MusicLoopVideo :src="src" :active="active" @ready="ready = $event" />
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
