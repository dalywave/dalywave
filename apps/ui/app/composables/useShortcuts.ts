/** Горячие клавиши приложения: пробел, ⇧← ⇧→, L, S (перемешивание), R (повтор), ⌘K, ⌘⇧M (мини-плеер), ⌘⌥B (свернуть боковую панель). Не мешают вводу текста */
export function useShortcuts() {
  const player = usePlayerStore()
  const ui = useUiStore()
  const bridge = useBridge()

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
    // Боковая панель компьютера: свернуть в рейку и развернуть. По месту клавиши: ⌥B на Mac
    // печатает «∫», а в русской раскладке это «и»
    if (mod && e.altKey && !e.shiftKey && e.code === 'KeyB' && !ui.isTouchUi) {
      e.preventDefault()
      ui.toggleSidebar()
      return
    }
    // Мини-плеер: приложение для компьютера
    if (mod && e.shiftKey && e.key.toLowerCase() === 'm' && bridge.miniPlayer) {
      e.preventDefault()
      void bridge.miniPlayer.open()
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
    // S и R — по месту клавиши, чтобы работали и в русской раскладке; в волне — только повтор трека
    else if ((e.code === 'KeyS' || e.code === 'KeyR') && !e.shiftKey && player.current && (player.modesAvailable || e.code === 'KeyR')) {
      e.preventDefault()
      if (e.code === 'KeyS') player.toggleShuffle()
      else player.cycleRepeat()
    }
  }

  onMounted(() => window.addEventListener('keydown', onKey))
  onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
}
