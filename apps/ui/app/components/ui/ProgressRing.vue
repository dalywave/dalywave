<script setup lang="ts">
// Кольцо загрузки: в очереди (пунктир), идёт (дуга), готово (галочка), ошибка
const props = withDefaults(defineProps<{
  value?: number
  state?: 'queued' | 'progress' | 'done' | 'error'
  size?: number
  label?: string
}>(), { value: 0, state: 'progress', size: 24 })

const C = 2 * Math.PI * 10
const offset = computed(() => C * (1 - Math.min(1, Math.max(0, props.value))))
const text = computed(() => props.label ?? ({
  queued: 'В очереди на загрузку',
  progress: `Загрузка ${Math.round(props.value * 100)}\u00a0%`,
  done: 'Скачано',
  error: 'Не скачалось',
} as const)[props.state])
</script>

<template>
  <span class="pring" :class="`pring--${state}`" :style="{ '--size': `${size}px` }" role="img" :aria-label="text">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle class="pring__track" cx="12" cy="12" r="10" fill="none" stroke-width="2.2" />
      <circle v-if="state === 'progress'" class="pring__bar" cx="12" cy="12" r="10" fill="none" stroke-width="2.2" :stroke-dasharray="C.toFixed(2)" :stroke-dashoffset="offset.toFixed(2)" />
      <circle v-else-if="state === 'done'" class="pring__bar" cx="12" cy="12" r="10" fill="none" stroke-width="2.2" />
    </svg>
    <Icon v-if="state === 'done'" name="ph:check-bold" class="pring__icon" />
    <Icon v-else-if="state === 'queued'" name="ph:arrow-down" class="pring__icon" />
    <Icon v-else-if="state === 'error'" name="ph:exclamation-mark-bold" class="pring__icon" />
    <span v-else class="pring__stop" />
  </span>
</template>

<style scoped>
.pring { position: relative; display: inline-grid; place-items: center; flex: none; width: var(--size); height: var(--size); color: var(--accent); }
.pring svg { position: absolute; inset: 0; width: 100%; height: 100%; transform: rotate(-90deg); }
.pring__track { stroke: var(--fill-3); }
.pring__bar { stroke: currentColor; stroke-linecap: round; transition: stroke-dashoffset var(--dur-3) var(--ease-out); }
.pring__icon { font-size: calc(var(--size) * 0.46); }
.pring__stop { width: 30%; height: 30%; border-radius: 1.5px; background: currentColor; }
.pring--queued { color: var(--text-3); }
.pring--queued .pring__track { stroke-dasharray: 2 3; stroke: var(--text-4); }
.pring--error { color: var(--danger); }
.pring--error .pring__track { stroke: color-mix(in oklab, var(--danger) 40%, transparent); }
</style>
