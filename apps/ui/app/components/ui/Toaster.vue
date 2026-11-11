<script setup lang="ts">
import { ToastAction, ToastProvider, ToastRoot, ToastTitle, ToastViewport } from 'reka-ui'
import type { ToastItem } from '~/composables/useToast'

const { toasts, remove } = useToast()

function duration(t: ToastItem) {
  return t.duration ?? (t.undo || t.action ? 5000 : 2600)
}

function undo(t: ToastItem) {
  t.undo?.()
  t.open = false
}

function act(t: ToastItem) {
  t.action?.onClick()
  t.open = false
}

function onOpen(t: ToastItem, open: boolean) {
  t.open = open
  if (!open) window.setTimeout(() => remove(t.id), 250)
}
</script>

<template>
  <ToastProvider swipe-direction="down" label="Уведомление">
    <ToastRoot
      v-for="t in toasts"
      :key="t.id"
      :open="t.open"
      :duration="duration(t)"
      class="toast"
      :class="{ 'toast--error': t.tone === 'error' }"
      @update:open="v => onOpen(t, v)"
    >
      <Icon :name="t.icon ?? 'ph:check-circle-fill'" class="toast__icon" />
      <ToastTitle class="toast__text">{{ t.text }}</ToastTitle>
      <ToastAction v-if="t.undo" alt-text="Отменить" as-child>
        <button type="button" class="toast__action" @click="undo(t)">Отменить</button>
      </ToastAction>
      <ToastAction v-else-if="t.action" :alt-text="t.action.label" as-child>
        <button type="button" class="toast__action" @click="act(t)">{{ t.action.label }}</button>
      </ToastAction>
    </ToastRoot>
    <ToastViewport class="toast-viewport" />
  </ToastProvider>
</template>

<style>
.toast-viewport {
  position: fixed;
  left: 50%;
  bottom: calc(var(--toast-bottom, 104px) + var(--safe-bottom));
  z-index: 250;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  width: max-content;
  max-width: calc(100vw - 32px);
  margin: 0;
  padding: 0;
  list-style: none;
  outline: none;
}
.toast {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 48px;
  max-width: min(520px, calc(100vw - 32px));
  padding: 6px 6px 6px 14px;
  border-radius: var(--r-4);
  background: var(--glass-strong);
  -webkit-backdrop-filter: blur(28px) saturate(1.5);
  backdrop-filter: blur(28px) saturate(1.5);
  box-shadow: var(--shadow-pop);
  font-size: var(--fs-body);
}
.toast[data-state='open'] { animation: toast-in var(--dur-3) var(--ease-out); }
.toast[data-state='closed'] { animation: fade-in var(--dur-2) reverse forwards; }
.toast[data-swipe='move'] { transform: translateY(var(--reka-toast-swipe-move-y)); }
.toast__icon { flex: none; font-size: 18px; color: var(--accent); }
.toast__text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding-right: 8px; }
.toast__action {
  height: 36px;
  padding: 0 14px;
  border: 0;
  border-radius: 12px;
  background: transparent;
  font-weight: 650;
  color: var(--accent);
  white-space: nowrap;
  transition: background-color var(--dur-1);
}
.toast__action:hover { background: var(--fill-2); }
.toast--error { box-shadow: var(--shadow-pop), inset 0 0 0 1px color-mix(in oklab, var(--danger) 30%, transparent); }
.toast--error .toast__icon { color: var(--danger); }
.toast--error .toast__action { color: var(--text-1); }
</style>
