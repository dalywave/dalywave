<script setup lang="ts">
import { DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle, DialogTrigger } from 'reka-ui'
import type { MenuAction, MenuEntry, MenuHeading } from '~/types/ui'

// Меню на телефоне — лист снизу, как в «Музыке» и Spotify: ручка, шапка с обложкой,
// крупные строки группами, опасное красным, отдельная «Отмена». Смахнуть вниз или нажать
// мимо — закрыть. Пункты те же, что у выпадающего меню на большом экране
const props = defineProps<{
  items: MenuEntry[]
  /** Без кнопки-триггера: лист открывают снаружи (долгое нажатие на строку) */
  detached?: boolean
}>()

const open = defineModel<boolean>('open', { default: false })
const { t } = useI18n()
const scroller = ref<HTMLElement | null>(null)
// Список не прокручивается — тогда и из него можно смахнуть лист вниз
const scrolls = ref(false)
watch(open, async (v) => {
  if (!v) return
  await nextTick()
  scrolls.value = !!scroller.value && scroller.value.scrollHeight > scroller.value.clientHeight + 1
})

type Row = { kind: 'row', entry: MenuAction } | { kind: 'caption', label: string }
const stack = ref<MenuAction[]>([])
watch(open, (v) => {
  if (!v) stack.value = []
})

const level = computed(() => stack.value.at(-1)?.children ?? props.items)
const header = computed<MenuHeading | null>(() => {
  if (stack.value.length) return null
  const first = props.items.find(e => e.kind !== 'separator')
  return first?.kind === 'heading' ? first : null
})

// Пункты → группы: разделитель начинает новую группу, подзаголовок — подпись группы
const groups = computed(() => {
  const out: Row[][] = [[]]
  for (const e of level.value) {
    if (e === header.value) continue
    if (e.kind === 'separator') {
      if (out.at(-1)!.length) out.push([])
    }
    else if (e.kind === 'heading') {
      if (out.at(-1)!.length) out.push([])
      out.at(-1)!.push({ kind: 'caption', label: e.label })
    }
    else out.at(-1)!.push({ kind: 'row', entry: e })
  }
  return out.filter(g => g.some(r => r.kind === 'row'))
})

function pick(e: MenuAction) {
  if (e.disabled) return
  if (e.children?.length) {
    stack.value.push(e)
    return
  }
  open.value = false
  e.onSelect?.()
}

// ---------- Смахнуть вниз ----------
const panel = ref<HTMLElement | null>(null)
const dy = ref(0)
const settling = ref(false)
let drag: { id: number, y: number, x: number, lastY: number, lastT: number, v: number, active: boolean } | null = null

function onDown(e: PointerEvent) {
  if (!e.isPrimary || settling.value) return
  const scroller = (e.target as HTMLElement).closest('.as__scroll')
  // Внутри списка жест работает, только пока он не прокручен
  if (scroller && scroller.scrollTop > 0) return
  drag = { id: e.pointerId, y: e.clientY, x: e.clientX, lastY: e.clientY, lastT: e.timeStamp, v: 0, active: false }
}

function onMove(e: PointerEvent) {
  const d = drag
  if (!d || e.pointerId !== d.id) return
  const y = e.clientY - d.y
  if (!d.active) {
    if (Math.abs(y) < 8 && Math.abs(e.clientX - d.x) < 8) return
    if (y <= 0 || Math.abs(e.clientX - d.x) > Math.abs(y)) {
      drag = null
      return
    }
    d.active = true
    try {
      panel.value?.setPointerCapture(e.pointerId)
    }
    catch { /* касание уже кончилось */ }
  }
  const dt = Math.max(1, e.timeStamp - d.lastT)
  d.v = 0.7 * ((e.clientY - d.lastY) / dt) + 0.3 * d.v
  d.lastY = e.clientY
  d.lastT = e.timeStamp
  dy.value = Math.max(0, y)
}

function onUp(e: PointerEvent) {
  const d = drag
  if (!d || e.pointerId !== d.id) return
  drag = null
  if (!d.active) return
  const h = panel.value?.offsetHeight ?? 400
  if (dy.value > Math.min(120, h * 0.3) || d.v > 0.5) open.value = false
  settling.value = true
  dy.value = 0
  setTimeout(() => (settling.value = false), 260)
}

function onCancel() {
  drag = null
  dy.value = 0
}
</script>

<template>
  <DialogRoot v-model:open="open">
    <DialogTrigger v-if="!detached" as-child>
      <slot />
    </DialogTrigger>
    <DialogPortal>
      <DialogOverlay class="as-overlay" :style="dy ? { opacity: String(Math.max(0.2, 1 - dy / 300)) } : undefined" />
      <DialogContent
        class="as"
        :class="{ 'is-settling': settling }"
        :style="dy ? { transform: `translate3d(0, ${dy}px, 0)` } : undefined"
        :aria-describedby="undefined"
        @open-auto-focus.prevent
      >
        <DialogTitle class="sr-only">{{ header?.label ?? t('common.more') }}</DialogTitle>
        <DialogDescription v-if="header?.hint" class="sr-only">{{ header.hint }}</DialogDescription>
        <div
          ref="panel"
          class="as__panel"
          @pointerdown="onDown"
          @pointermove="onMove"
          @pointerup="onUp"
          @pointercancel="onCancel"
        >
          <div class="as__grab" aria-hidden="true" />
          <header v-if="header" class="as__head">
            <MusicCoverArt v-if="header.image !== undefined" class="as__cover" :image="header.image" size="small" :round="header.round" />
            <div class="as__head-text">
              <div class="as__title ellipsis">{{ header.label }}</div>
              <div v-if="header.hint" class="as__sub ellipsis">{{ header.hint }}</div>
            </div>
          </header>
          <header v-else-if="stack.length" class="as__head as__head--back">
            <button type="button" class="as__back" @click="stack.pop()">
              <Icon name="ph:caret-left" />{{ t('nav.back') }}
            </button>
            <div class="as__title ellipsis">{{ stack.at(-1)!.label }}</div>
          </header>
          <div ref="scroller" class="as__scroll" :class="{ 'is-scroll': scrolls }">
            <div v-for="(g, gi) in groups" :key="gi" class="as__group">
              <template v-for="(r, ri) in g" :key="ri">
                <div v-if="r.kind === 'caption'" class="as__caption">{{ r.label }}</div>
                <button
                  v-else
                  type="button"
                  class="as__row"
                  :class="{ 'is-danger': r.entry.danger, 'is-checked': r.entry.checked }"
                  :disabled="r.entry.disabled || undefined"
                  @click="pick(r.entry)"
                >
                  <Icon v-if="r.entry.icon" :name="r.entry.icon" class="as__icon" />
                  <span class="as__label">
                    <span class="ellipsis">{{ r.entry.label }}</span>
                    <span v-if="r.entry.hint" class="as__hint ellipsis">{{ r.entry.hint }}</span>
                  </span>
                  <Icon v-if="r.entry.children?.length" name="ph:caret-right" class="as__end" />
                  <Icon v-else-if="r.entry.checked" name="ph:check-bold" class="as__end as__check" />
                </button>
              </template>
            </div>
          </div>
        </div>
        <button type="button" class="as__cancel" @click="open = false">{{ t('common.cancel') }}</button>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style>
.as-overlay { position: fixed; inset: 0; z-index: 230; background: rgba(0, 0, 0, 0.5); animation: fade-in var(--dur-2) var(--ease-out); }
.as {
  position: fixed;
  left: 8px;
  right: 8px;
  bottom: calc(var(--safe-bottom) + 8px);
  z-index: 240;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - 40px);
  outline: none;
  animation: sheet-in var(--dur-3) var(--ease-out);
  will-change: transform;
}
.as.is-settling { transition: transform 240ms cubic-bezier(0.22, 1, 0.36, 1); }
.as__panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  border-radius: 18px;
  background: rgba(30, 33, 42, 0.86);
  -webkit-backdrop-filter: blur(40px) saturate(1.6);
  backdrop-filter: blur(40px) saturate(1.6);
  box-shadow: 0 0 0 0.5px rgba(255, 255, 255, 0.08), 0 20px 50px -12px rgba(0, 0, 0, 0.8);
  touch-action: none;
}
.as__grab { width: 36px; height: 5px; margin: 7px auto 0; flex: none; border-radius: 3px; background: rgba(255, 255, 255, 0.24); }
.as__head { display: flex; align-items: center; gap: 12px; min-width: 0; padding: 10px 16px 14px; flex: none; border-bottom: 0.5px solid rgba(255, 255, 255, 0.1); }
.as__cover { flex: none; }
.as__cover.art { width: 48px; border-radius: 8px; }
.as__cover.art--round { border-radius: 50%; }
.as__head-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.as__title { font-size: 16px; font-weight: 650; color: var(--text-1); }
.as__sub { font-size: 14px; color: var(--text-2); }
.as__head--back { gap: 8px; padding-top: 6px; padding-bottom: 10px; }
.as__back { display: inline-flex; align-items: center; gap: 2px; height: 36px; margin-left: -6px; padding: 0 8px 0 0; border: 0; background: none; font-size: 16px; color: var(--accent); flex: none; }
.as__scroll { min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
.as__scroll.is-scroll { touch-action: pan-y; }
.as__scroll::-webkit-scrollbar { display: none; }
.as__group + .as__group { border-top: 6px solid rgba(0, 0, 0, 0.22); }
.as__caption { padding: 12px 16px 4px; font-size: 12px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.as__row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  min-height: 54px;
  padding: 8px 16px;
  border: 0;
  background: transparent;
  font-size: 17px;
  text-align: left;
  color: var(--text-1);
  -webkit-tap-highlight-color: transparent;
}
.as__row + .as__row::before { content: ''; position: absolute; top: 0; left: 52px; right: 0; height: 0.5px; background: rgba(255, 255, 255, 0.1); }
.as__row:active { background: rgba(255, 255, 255, 0.08); }
.as__row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.as__row:disabled { color: var(--text-4); }
.as__row.is-danger, .as__row.is-danger .as__icon { color: var(--danger); }
.as__icon { flex: none; width: 22px; font-size: 22px; color: var(--text-2); }
.as__row.is-checked .as__icon { color: var(--accent); }
.as__label { display: flex; flex-direction: column; flex: 1; min-width: 0; }
.as__hint { font-size: 13px; color: var(--text-3); }
.as__end { flex: none; font-size: 18px; color: var(--text-3); }
.as__check { color: var(--accent); }
.as__cancel {
  flex: none;
  height: 56px;
  border: 0;
  border-radius: 18px;
  background: rgba(30, 33, 42, 0.92);
  -webkit-backdrop-filter: blur(40px) saturate(1.6);
  backdrop-filter: blur(40px) saturate(1.6);
  box-shadow: 0 0 0 0.5px rgba(255, 255, 255, 0.08);
  font-size: 17px;
  font-weight: 650;
  color: var(--text-1);
  -webkit-tap-highlight-color: transparent;
}
.as__cancel:active { background: rgba(48, 52, 64, 0.95); }
@media (prefers-reduced-motion: reduce) {
  .as, .as-overlay { animation: none; }
  .as.is-settling { transition: none; }
}
</style>
