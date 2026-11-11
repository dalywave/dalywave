/** Горячие клавиши приложения: пробел, ⇧← ⇧→, L, ⌘K. Не мешают вводу текста */
export function useShortcuts() {
  const player = usePlayerStore()
  const ui = useUiStore()

  function typing(e: KeyboardEvent) {
    const el = e.target as HTMLElement | null
    if (!el) return false
    return el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName)
  }

  function onKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey
    if (mod && e.key.toLowerCase() === 'k') {
      e.preventDefault()
      ui.focusSearch()
      return
    }
    if (typing(e) || mod || e.altKey || e.defaultPrevented) return
    // Пробел на кнопке нажимает кнопку — не перехватываем
    if (e.code === 'Space' && !(e.target as HTMLElement).closest('button, a, [role="slider"], [role="menuitem"]')) {
      e.preventDefault()
      void player.toggle()
    }
    else if (e.shiftKey && e.key === 'ArrowRight') {
      e.preventDefault()
      void player.next('skipped')
    }
    else if (e.shiftKey && e.key === 'ArrowLeft') {
      e.preventDefault()
      void player.prev()
    }
    else if (e.key.toLowerCase() === 'l' && !e.shiftKey && player.current) {
      void player.toggleLike()
    }
  }

  onMounted(() => window.addEventListener('keydown', onKey))
  onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
}
