<script setup lang="ts">
import { PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from 'reka-ui'

// Стеклянная панель у элемента: выбор режима переходов, устройства, настройки волны
withDefaults(defineProps<{
  side?: 'top' | 'bottom' | 'left' | 'right'
  align?: 'start' | 'center' | 'end'
  title?: string
  hint?: string
  width?: number
  /** Не ставить фокус внутрь при открытии мышью (список, где выбранное и так видно) */
  noAutofocus?: boolean
}>(), { side: 'bottom', align: 'start', width: 300 })

const open = defineModel<boolean>('open', { default: false })
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <slot name="trigger" />
    </PopoverTrigger>
    <PopoverPortal>
      <PopoverContent
        :side="side"
        :align="align"
        :side-offset="8"
        :collision-padding="12"
        class="pop"
        :style="{ width: `${width}px` }"
        @open-auto-focus="(e: Event) => noAutofocus && e.preventDefault()"
      >
        <header v-if="title" class="pop__head">
          <span class="pop__title">{{ title }}</span>
          <span v-if="hint" class="pop__hint">{{ hint }}</span>
        </header>
        <slot :close="() => (open = false)" />
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style>
.pop {
  z-index: 200;
  max-width: calc(100vw - 24px);
  max-height: min(70vh, var(--reka-popover-content-available-height, 70vh));
  overflow-y: auto;
  padding: 6px;
  border-radius: var(--r-4);
  background: var(--glass-strong);
  -webkit-backdrop-filter: blur(32px) saturate(1.6);
  backdrop-filter: blur(32px) saturate(1.6);
  box-shadow: var(--shadow-pop);
  transform-origin: var(--reka-popover-content-transform-origin);
  animation: pop-in var(--dur-2) var(--ease-out);
  scrollbar-width: none;
}
.pop--static { position: relative; z-index: auto; animation: none; max-height: none; }
.pop__head { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 10px 10px; }
.pop__title { font-size: var(--fs-small); font-weight: 650; color: var(--text-1); }
.pop__hint { font-size: var(--fs-caption); color: var(--text-3); }
.pop__sep { height: 1px; margin: 6px 4px; background: var(--hair); }
.pop__foot { padding: 10px 10px 6px; font-size: var(--fs-caption); color: var(--text-3); }
</style>
