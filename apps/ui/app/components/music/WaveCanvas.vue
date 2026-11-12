<script setup lang="ts">
import { getWaveEngine } from '~/lib/wave/engine'
import type { WaveLayout } from '~/lib/wave/engine'
import { popWaveHost, pushWaveHost, updateWaveHost } from '~/lib/wave/hosts'

// Место, где живёт волна. Холст один на приложение и переезжает между такими местами;
// последнее открытое место главнее, при закрытии волна возвращается в предыдущее
const props = withDefaults(defineProps<{
  layout?: WaveLayout
  /** Элемент, который волна «обнимает» свечением (обложка в плеере) */
  box?: HTMLElement | null
  /** Тёмная волна и на светлой теме — для тёмного острова (.scheme-dark) */
  dark?: boolean
}>(), { layout: 'hero', box: null, dark: false })

const el = ref<HTMLElement | null>(null)

onMounted(() => {
  if (el.value) pushWaveHost({ el: el.value, layout: props.layout, box: props.box, dark: props.dark })
})
watch(() => [props.layout, props.box, props.dark] as const, ([layout, box, dark]) => {
  if (el.value) updateWaveHost(el.value, { layout, box, dark })
})
onBeforeUnmount(() => {
  if (el.value) popWaveHost(el.value)
})

defineExpose({ remeasure: () => getWaveEngine().remeasure() })
</script>

<template>
  <div ref="el" class="wave-host" :class="{ 'wave-host--dark': dark }" aria-hidden="true" />
</template>

<style>
.wave-host { position: absolute; inset: 0; overflow: hidden; }
.wave-canvas { position: absolute; inset: 0; width: 100%; height: 100%; display: block; }
.wave-host { --wave-paper: #05060a; }
:root[data-theme='light'] .wave-host:not(.wave-host--dark) { --wave-paper: #fbfafc; }
.wave-fallback {
  background:
    radial-gradient(60% 40% at 60% 55%, color-mix(in oklab, var(--m2) 60%, transparent), transparent 70%),
    radial-gradient(40% 30% at 70% 50%, color-mix(in oklab, var(--m3) 40%, transparent), transparent 70%),
    var(--wave-paper);
}
</style>
