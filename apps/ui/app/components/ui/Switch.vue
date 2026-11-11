<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from 'reka-ui'

defineProps<{
  label: string
  hint?: string
  disabled?: boolean
  /** Подпись только для экранного диктора: в строке настроек её роль играет название строки */
  hideLabel?: boolean
}>()

const model = defineModel<boolean>({ default: false })
const id = useId()
</script>

<template>
  <div class="sw" :class="{ 'is-disabled': disabled }">
    <SwitchRoot :id="id" v-model="model" :disabled="disabled" class="sw__track">
      <SwitchThumb class="sw__thumb" />
    </SwitchRoot>
    <label :for="id" class="sw__text" :class="{ 'sr-only': hideLabel }">
      <span>{{ label }}</span>
      <span v-if="hint" class="sw__hint">{{ hint }}</span>
    </label>
  </div>
</template>

<style scoped>
.sw { display: inline-flex; align-items: center; gap: 12px; }
.sw__track {
  position: relative;
  width: 40px;
  height: 24px;
  flex: none;
  border: 0;
  border-radius: 12px;
  background: var(--fill-3);
  box-shadow: inset 0 0 0 1px var(--line);
  transition: background-color var(--dur-2);
}
.sw__track:hover { box-shadow: inset 0 0 0 1px var(--line-strong); }
.sw__track[data-state='checked'] { background: var(--accent); box-shadow: none; }
.sw__thumb {
  display: block;
  width: 18px;
  height: 18px;
  margin-left: 3px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
  transition: transform var(--dur-2) var(--ease-spring);
}
.sw__thumb[data-state='checked'] { transform: translateX(16px); }
.sw__text { display: flex; flex-direction: column; cursor: pointer; font-size: var(--fs-body); }
.sw__hint { font-size: var(--fs-caption); color: var(--text-3); }
.sw.is-disabled { color: var(--text-4); pointer-events: none; }
.sw.is-disabled .sw__track { opacity: 0.4; }
</style>
