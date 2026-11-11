import type { ToastOptions } from '~/types/ui'

export interface ToastItem extends ToastOptions {
  id: number
  open: boolean
}

const toasts = ref<ToastItem[]>([])
let seq = 0

/** Короткие уведомления внизу экрана. Для любого удаления — с кнопкой «Отменить» */
export function useToast() {
  function show(options: ToastOptions): number {
    const id = ++seq
    // Больше трёх сразу — шум: старые уходят
    toasts.value = [...toasts.value.filter(t => t.open).slice(-2), { ...options, id, open: true }]
    return id
  }

  function dismiss(id: number) {
    const t = toasts.value.find(x => x.id === id)
    if (t) t.open = false
  }

  function remove(id: number) {
    toasts.value = toasts.value.filter(t => t.id !== id)
  }

  function error(text: string) {
    return show({ text, tone: 'error', icon: 'ph:warning-circle-fill', duration: 6000 })
  }

  return { toasts, show, dismiss, remove, error }
}
