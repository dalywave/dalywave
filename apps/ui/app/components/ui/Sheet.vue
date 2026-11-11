<script setup lang="ts">
import { DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'

// Шторка: снизу на телефоне (очередь, текст, полноэкранный плеер), справа на большом экране
withDefaults(defineProps<{
  title: string
  description?: string
  side?: 'bottom' | 'right' | 'full'
  /** Заголовок виден; иначе только для экранного диктора */
  showTitle?: boolean
}>(), { side: 'bottom', showTitle: true })

const open = defineModel<boolean>('open', { default: false })
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogPortal>
      <DialogOverlay v-if="side !== 'full'" class="sheet-overlay" />
      <DialogContent class="sheet" :class="`sheet--${side}`" :aria-describedby="description ? undefined : undefined">
        <div v-if="side === 'bottom'" class="sheet__grab" aria-hidden="true" />
        <header v-if="showTitle" class="sheet__head">
          <DialogTitle class="sheet__title">{{ title }}</DialogTitle>
          <slot name="head-actions" />
        </header>
        <DialogTitle v-else class="sr-only">{{ title }}</DialogTitle>
        <DialogDescription v-if="description" class="sr-only">{{ description }}</DialogDescription>
        <div class="sheet__body">
          <slot :close="() => (open = false)" />
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style>
.sheet-overlay { position: fixed; inset: 0; z-index: 150; background: var(--scrim); animation: fade-in var(--dur-2) var(--ease-out); }
.sheet {
  position: fixed;
  z-index: 160;
  display: flex;
  flex-direction: column;
  background: var(--glass-strong);
  -webkit-backdrop-filter: blur(36px) saturate(1.5);
  backdrop-filter: blur(36px) saturate(1.5);
  box-shadow: var(--shadow-pop);
  outline: none;
}
.sheet--bottom {
  left: 0;
  right: 0;
  bottom: 0;
  max-height: min(86dvh, 760px);
  padding-bottom: var(--safe-bottom);
  border-radius: 24px 24px 0 0;
  animation: sheet-in var(--dur-3) var(--ease-out);
}
.sheet--right { top: 8px; right: 8px; bottom: 8px; width: min(400px, calc(100vw - 16px)); border-radius: var(--r-5); animation: pop-in var(--dur-3) var(--ease-out); }
.sheet--full { inset: 0; border-radius: 0; background: var(--ink-1); animation: sheet-in var(--dur-4) var(--ease-out); }
.sheet__grab { width: 40px; height: 5px; margin: 8px auto 2px; border-radius: 3px; background: var(--line-strong); flex: none; }
.sheet__head { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 20px 8px; flex: none; }
.sheet__title { margin: 0; font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; }
.sheet__body { flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
.sheet__body::-webkit-scrollbar { display: none; }
.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
</style>
