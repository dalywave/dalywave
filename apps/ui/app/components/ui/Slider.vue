<script setup lang="ts">
import { SliderRange, SliderRoot, SliderThumb, SliderTrack } from 'reka-ui'

// Тонкий ползунок: бегунок появляется при наведении и фокусе, заполнение — акцент при наведении
const props = withDefaults(defineProps<{
  label: string
  min?: number
  max?: number
  step?: number
  disabled?: boolean
  valueText?: string
  forceState?: 'hover'
}>(), { min: 0, max: 1, step: 0.01 })

const model = defineModel<number>({ required: true })
const value = computed({
  get: () => [model.value],
  set: (v: number[] | undefined) => {
    if (v?.length) model.value = v[0]!
  },
})
</script>

<template>
  <SliderRoot
    v-model="value"
    :min="props.min"
    :max="props.max"
    :step="props.step"
    :disabled="disabled"
    class="sl"
    :class="{ [`is-${forceState}`]: forceState }"
  >
    <SliderTrack class="sl__track">
      <SliderRange class="sl__range" />
    </SliderTrack>
    <SliderThumb class="sl__thumb" :aria-label="label" :aria-valuetext="valueText" />
  </SliderRoot>
</template>

<style scoped>
.sl {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
  height: 20px;
  touch-action: none;
  user-select: none;
  cursor: pointer;
}
.sl__track { position: relative; flex: 1; height: 4px; border-radius: 4px; background: rgba(232, 236, 244, 0.18); overflow: hidden; }
.sl__range { position: absolute; height: 100%; border-radius: inherit; background: var(--text-1); transition: background-color var(--dur-1); }
.sl__thumb {
  display: block;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
  transform: scale(0);
  transition: transform var(--dur-1) var(--ease-out);
}
.sl:hover .sl__thumb, .sl.is-hover .sl__thumb, .sl__thumb:focus-visible, .sl[data-dragging] .sl__thumb { transform: scale(1); }
.sl:hover .sl__range, .sl.is-hover .sl__range { background: var(--accent); }
.sl__thumb:focus-visible { outline: none; box-shadow: 0 0 0 3px var(--focus); }
.sl[data-disabled] { opacity: 0.4; pointer-events: none; }
</style>
