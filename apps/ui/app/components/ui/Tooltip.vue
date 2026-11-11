<script setup lang="ts">
import { TooltipArrow, TooltipContent, TooltipPortal, TooltipRoot, TooltipTrigger } from 'reka-ui'

withDefaults(defineProps<{
  text?: string
  kbd?: string[]
  side?: 'top' | 'bottom' | 'left' | 'right'
  disabled?: boolean
  delay?: number
}>(), { side: 'top', delay: 450 })
</script>

<template>
  <TooltipRoot v-if="!disabled && (text || $slots.content)" :delay-duration="delay">
    <TooltipTrigger as-child>
      <slot />
    </TooltipTrigger>
    <TooltipPortal>
      <TooltipContent :side="side" :side-offset="8" :collision-padding="8" class="tooltip">
        <slot name="content">{{ text }}</slot>
        <UiKbd v-if="kbd?.length" :keys="kbd" class="tooltip__kbd" />
        <TooltipArrow class="tooltip__arrow" :width="10" :height="5" />
      </TooltipContent>
    </TooltipPortal>
  </TooltipRoot>
  <slot v-else />
</template>

<style>
.tooltip {
  z-index: 300;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 280px;
  padding: 6px 10px;
  border-radius: var(--r-2);
  background: var(--ink-4);
  box-shadow: var(--shadow-pop);
  font-size: var(--fs-caption);
  font-weight: 550;
  color: var(--text-1);
  animation: pop-in var(--dur-1) var(--ease-out);
  user-select: none;
}
.tooltip__arrow { fill: var(--ink-4); }
.tooltip__muted { color: var(--text-3); font-weight: 500; }
</style>
