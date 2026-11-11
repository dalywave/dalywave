/** Меню листом снизу: на телефоне по ширине или когда управляют пальцем (планшет) */
export function useMenuSheet() {
  const ui = useUiStore()
  const coarse = useMediaQuery('(pointer: coarse)')
  return computed(() => ui.isPhone || coarse.value)
}
