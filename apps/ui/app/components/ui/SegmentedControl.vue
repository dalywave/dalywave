<script setup lang="ts" generic="T extends string">
import { ToggleGroupItem, ToggleGroupRoot } from 'reka-ui'

export interface SegmentOption<V extends string> {
  value: V
  label: string
  icon?: string
  /** Цветная точка (настроение) */
  dot?: string
  disabled?: boolean
  hint?: string
}

const props = withDefaults(defineProps<{
  options: SegmentOption<T>[]
  label: string
  size?: 'sm' | 'md'
  block?: boolean
}>(), { size: 'md' })

const model = defineModel<T>({ required: true })

function update(v: unknown) {
  // Повторный щелчок по выбранному не снимает выбор
  if (typeof v === 'string' && v) model.value = v as T
}
</script>

<template>
  <ToggleGroupRoot
    type="single"
    :model-value="model"
    :aria-label="props.label"
    class="seg"
    :class="[`seg--${size}`, { 'seg--block': block }]"
    @update:model-value="update"
  >
    <ToggleGroupItem
      v-for="o in options"
      :key="o.value"
      :value="o.value"
      :disabled="o.disabled"
      :title="o.hint"
      class="seg__item"
    >
      <span v-if="o.dot" class="seg__dot" :style="{ '--dot': o.dot }" />
      <Icon v-if="o.icon" :name="o.icon" class="seg__icon" />
      {{ o.label }}
    </ToggleGroupItem>
  </ToggleGroupRoot>
</template>

<style scoped>
.seg {
  display: inline-flex;
  padding: 3px;
  gap: 2px;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  max-width: 100%;
}
.seg--block { display: flex; width: 100%; }
.seg__item {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 30px;
  padding: 0 14px;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  font-size: var(--fs-small);
  font-weight: 600;
  color: var(--text-2);
  white-space: nowrap;
  transition: color var(--dur-1), background-color var(--dur-2), box-shadow var(--dur-2);
}
.seg--block .seg__item { flex: 1; padding: 0 8px; }
.seg--sm .seg__item { height: 26px; padding: 0 10px; font-size: var(--fs-caption); }
.seg__icon { font-size: 16px; }
.seg__item:hover { color: var(--text-1); }
.seg__item[data-state='on'] { color: var(--text-1); background: var(--ink-4); box-shadow: 0 2px 8px -2px rgba(0, 0, 0, 0.6), inset 0 0 0 1px var(--line); }
.seg__item[data-disabled] { color: var(--text-4); pointer-events: none; }
.seg__dot { width: 8px; height: 8px; border-radius: 50%; background: var(--dot); box-shadow: 0 0 8px var(--dot); }
</style>
