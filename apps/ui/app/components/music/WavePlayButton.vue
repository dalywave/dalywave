<script setup lang="ts">
import { getWaveEngine } from '~/lib/wave/engine'

// Кнопка волны: белое ядро, кольцо в цветах настроения вращается и пульсирует в долю
const props = withDefaults(defineProps<{
  playing?: boolean
  loading?: boolean
  size?: number
  label?: string
  /** Неподвижные состояния для галереи */
  forceState?: 'hover' | 'focus'
  beat?: boolean
}>(), { size: 72, beat: true })

defineEmits<{ click: [] }>()

const el = ref<HTMLElement | null>(null)
let off: (() => void) | null = null

onMounted(() => {
  if (!props.beat) return
  off = getWaveEngine().onFrame((s) => {
    el.value?.style.setProperty('--beat', s.beat.toFixed(3))
  })
})
onBeforeUnmount(() => off?.())
</script>

<template>
  <button
    ref="el"
    type="button"
    class="wp"
    :class="[loading ? 'is-loading' : playing ? 'is-playing' : 'is-idle', forceState && `is-${forceState}`]"
    :style="{ '--size': `${size}px` }"
    :aria-label="label ?? (playing ? 'Пауза' : 'Слушать волну')"
    @click="$emit('click')"
  >
    <span class="wp__halo" />
    <span class="wp__ring" />
    <span class="wp__core">
      <Icon :name="playing && !loading ? 'ph:pause-fill' : 'ph:play-fill'" class="wp__icon" :class="{ 'wp__icon--play': !playing || loading }" />
    </span>
  </button>
</template>

<style scoped>
.wp {
  --size: 72px;
  position: relative;
  display: grid;
  place-items: center;
  flex: none;
  width: var(--size);
  height: var(--size);
  border: 0;
  border-radius: 50%;
  background: transparent;
  isolation: isolate;
}
.wp__halo, .wp__ring {
  position: absolute;
  border-radius: 50%;
  background: conic-gradient(from 0deg, var(--m3), var(--m2), var(--m1), var(--m4), var(--m3));
  animation: spin 10s linear infinite;
  pointer-events: none;
}
.wp__halo { inset: -18%; filter: blur(16px); opacity: 0.4; z-index: -2; scale: calc(1 + var(--beat, 0) * 0.16); transition: opacity var(--dur-3); }
.wp__ring {
  inset: 0;
  z-index: -1;
  opacity: 0.75;
  -webkit-mask: radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 2.5px));
  mask: radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 2.5px));
}
.wp__core {
  display: grid;
  place-items: center;
  width: calc(100% - 10px);
  height: calc(100% - 10px);
  border-radius: 50%;
  background: radial-gradient(120% 120% at 30% 20%, #fff, #e9edf4 60%, #d5dbe5);
  color: var(--on-light);
  box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.8), inset 0 -2px 6px rgba(0, 0, 0, 0.1);
  scale: calc(1 + var(--beat, 0) * 0.025);
  transition: transform var(--dur-2) var(--ease-out);
}
.wp__icon { font-size: calc(var(--size) * 0.34); }
.wp__icon--play { transform: translateX(6%); }
.wp:hover .wp__core, .wp.is-hover .wp__core { transform: scale(1.04); }
.wp:active .wp__core { transform: scale(0.95); }
.wp:focus-visible, .wp.is-focus { outline: 2px solid var(--focus); outline-offset: 4px; }
.wp.is-playing .wp__halo { opacity: 0.75; }
.wp.is-playing .wp__ring { opacity: 1; }
.wp.is-idle .wp__ring, .wp.is-idle .wp__halo { animation-duration: 22s; }
.wp.is-loading .wp__ring { background: conic-gradient(from 0deg, transparent 0 55%, var(--text-1)); animation-duration: 0.9s; opacity: 1; }
.wp.is-loading .wp__core { opacity: 0.85; }
</style>
