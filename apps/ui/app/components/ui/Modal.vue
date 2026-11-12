<script setup lang="ts">
import { DialogClose, DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'

// Окно поверх страницы: на телефоне — лист снизу (UiSheet), на компьютере — панель по центру.
// Слоты одни и те же: `head-actions` у заголовка, содержимое, `footer`
const props = withDefaults(defineProps<{
  title: string
  description?: string
  showTitle?: boolean
  /** Ширина панели на компьютере */
  width?: number
  /** Лист на телефоне: по содержимому или с раскрытием во весь экран */
  detents?: 'fit' | 'expand'
  /** Высота панели на компьютере фиксирована (список внутри прокручивается сам) */
  tall?: boolean
}>(), { showTitle: true, width: 420, detents: 'fit', tall: false })

const open = defineModel<boolean>('open', { default: false })
const ui = useUiStore()
const { tg } = useGlobalT()
const style = computed(() => ({ width: `${props.width}px` }))

// Фокус — в поле с autofocus (поиск, название), иначе никуда: рамка на крестике ни к чему
function onAutoFocus(e: Event) {
  e.preventDefault()
  const el = (e.currentTarget as HTMLElement | null)?.querySelector<HTMLElement>('[autofocus]')
  el?.focus()
}
</script>

<template>
  <UiSheet v-if="ui.isPhone" v-model:open="open" :title="title" :description="description" :show-title="showTitle" :detents="detents">
    <template v-if="$slots['head-actions']" #head-actions>
      <slot name="head-actions" />
    </template>
    <template #default="{ close }">
      <slot :close="close" />
    </template>
    <template v-if="$slots.footer" #footer="{ close }">
      <slot name="footer" :close="close" />
    </template>
  </UiSheet>
  <DialogRoot v-else v-model:open="open">
    <DialogPortal>
      <DialogOverlay class="uim-overlay" />
      <DialogContent class="uim" :class="{ 'uim--tall': tall }" :style="style" :aria-describedby="description ? undefined : undefined" @open-auto-focus="onAutoFocus">
        <header class="uim__head" :class="{ 'sr-only': !showTitle }">
          <DialogTitle class="uim__title">{{ title }}</DialogTitle>
          <slot name="head-actions" />
          <DialogClose as-child>
            <UiIconButton icon="ph:x" :label="tg('common.close')" size="sm" :tooltip="false" class="uim__close" />
          </DialogClose>
        </header>
        <DialogDescription v-if="description" class="sr-only">{{ description }}</DialogDescription>
        <div class="uim__body thin-scrollbar">
          <slot :close="() => (open = false)" />
        </div>
        <footer v-if="$slots.footer" class="uim__foot">
          <slot name="footer" :close="() => (open = false)" />
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style>
.uim-overlay { position: fixed; inset: 0; z-index: 180; background: var(--overlay); animation: uim-fade var(--dur-2) var(--ease-out); }
.uim {
  position: fixed;
  left: 50%;
  top: 50%;
  z-index: 181;
  display: flex;
  flex-direction: column;
  max-width: calc(100vw - 32px);
  max-height: min(640px, calc(100dvh - 64px));
  border-radius: var(--r-4);
  background: var(--ink-2);
  box-shadow: 0 0 0 1px var(--line), var(--shadow-pop);
  transform: translate(-50%, -50%);
  outline: none;
  animation: uim-in var(--dur-2) var(--ease-out);
}
.uim--tall { height: min(640px, calc(100dvh - 64px)); }
.uim__head { display: flex; align-items: center; gap: 8px; min-height: 56px; padding: 8px 12px 4px 20px; flex: none; }
.uim__title { flex: 1; min-width: 0; margin: 0; overflow: hidden; font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; text-overflow: ellipsis; white-space: nowrap; }
.uim__close { flex: none; color: var(--text-2); }
.uim__body { flex: 1 1 auto; min-height: 0; overflow-y: auto; overscroll-behavior: contain; }
.uim__foot { flex: none; padding: 12px 20px 16px; border-top: 1px solid var(--hair); }
@keyframes uim-in { from { opacity: 0; transform: translate(-50%, calc(-50% + 8px)) scale(0.98); } }
@keyframes uim-fade { from { opacity: 0; } }
@media (prefers-reduced-motion: reduce) {
  .uim, .uim-overlay { animation: none; }
}
</style>
