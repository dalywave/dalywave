<script setup lang="ts">
// Громкость: кнопка выключает звук, ползунок — уровень. Колесо мыши над ползунком тоже работает
withDefaults(defineProps<{ forceHover?: boolean }>(), {})

const volume = defineModel<number>('volume', { required: true })
const muted = defineModel<boolean>('muted', { default: false })
const { t } = useI18n()

const icon = computed(() => {
  if (muted.value || volume.value === 0) return 'ph:speaker-x'
  return volume.value < 0.5 ? 'ph:speaker-low' : 'ph:speaker-high'
})

const level = computed({
  get: () => (muted.value ? 0 : volume.value),
  set: (v: number) => {
    volume.value = v
    if (v > 0) muted.value = false
  },
})

function onWheel(e: WheelEvent) {
  e.preventDefault()
  level.value = Math.min(1, Math.max(0, level.value - Math.sign(e.deltaY) * 0.05))
}
</script>

<template>
  <div class="vol" @wheel="onWheel">
    <UiIconButton :icon="icon" :label="muted ? t('player.unmute') : t('player.mute')" size="sm" @click="muted = !muted" />
    <UiSlider v-model="level" class="vol__slider" :label="t('player.volume')" :value-text="`${Math.round(level * 100)} %`" :force-state="forceHover ? 'hover' : undefined" />
  </div>
</template>

<style scoped>
.vol { display: flex; align-items: center; gap: 4px; width: 132px; flex: none; }
.vol__slider { flex: 1; min-width: 0; }
@media (max-width: 1279px) { .vol { width: 112px; } }
</style>
