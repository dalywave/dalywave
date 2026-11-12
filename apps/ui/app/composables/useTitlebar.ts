/**
 * Заголовок окна приложения на компьютере — для оболочки и админки. `titlebar` — чьи кнопки окна:
 * `mac-inset` — «светофор» поверх интерфейса (место под него — `--titlebar-inset`), `windows` — свои
 * кнопки в полосе сверху (`ShellTitleBar`). Во весь экран кнопок нет; без фокуса заголовок приглушён.
 */
export function useTitlebar() {
  const bridge = useBridge()
  const win = reactive({ maximized: false, focused: true, fullscreen: false })
  let off: (() => void) | null = null
  onMounted(() => {
    if (bridge.isDesktopApp) off = bridge.window.onState(s => Object.assign(win, s))
  })
  onBeforeUnmount(() => off?.())

  const controls = computed(() => (bridge.isDesktopApp && bridge.chrome && bridge.chrome.controls !== 'none' ? bridge.chrome.controls : null))
  const titlebar = computed(() => (controls.value && !win.fullscreen ? controls.value : null))
  const titlebarStyle = computed(() => (bridge.chrome ? { '--titlebar-size': `${bridge.chrome.height}px` } : undefined))
  return { win, titlebar, titlebarStyle }
}
