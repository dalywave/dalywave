import { defineStore } from 'pinia'

export type PanelTab = 'queue' | 'lyrics' | 'devices' | 'about'

// Состояние оболочки: правая панель, полноэкранный плеер, ширина экрана
export const useUiStore = defineStore('ui', () => {
  const panelOpen = ref(false)
  const panelTab = ref<PanelTab>('queue')
  const playerOpen = ref(false)
  /** Шторка на телефоне вместо правой панели */
  const sheetTab = ref<PanelTab | null>(null)
  /** Счётчик запросов «фокус в поиск» (⌘K) — поле поиска следит за ним */
  const searchFocus = ref(0)

  const isPhone = useMediaQuery('(max-width: 699px)')
  const isDesktop = useMediaQuery('(min-width: 1100px)')
  const isTablet = computed(() => !isPhone.value && !isDesktop.value)
  /** Узкая боковая панель из значков */
  const isRail = useMediaQuery('(min-width: 700px) and (max-width: 1023px)')
  const reducedMotion = useMediaQuery('(prefers-reduced-motion: reduce)')

  function openPanel(tab: PanelTab) {
    if (isPhone.value) {
      sheetTab.value = tab
      return
    }
    panelTab.value = tab
    panelOpen.value = true
  }

  function togglePanel(tab: PanelTab) {
    if (isPhone.value) {
      sheetTab.value = sheetTab.value === tab ? null : tab
      return
    }
    if (panelOpen.value && panelTab.value === tab) panelOpen.value = false
    else openPanel(tab)
  }

  // Плеер на телефоне сворачивается своей анимацией (вниз, в мини-плеер); её регистрирует FullPlayer
  let collapser: (() => Promise<void>) | null = null
  function onCollapsePlayer(fn: () => Promise<void>) {
    collapser = fn
    return () => {
      if (collapser === fn) collapser = null
    }
  }

  /** Свернуть плеер и дождаться конца анимации: переход на страницу — после, а не под плеером */
  async function collapsePlayer() {
    sheetTab.value = null
    if (!playerOpen.value) return
    if (isPhone.value && collapser) await collapser()
    playerOpen.value = false
  }

  /** Переход из меню или ссылки, которые могут быть над плеером */
  async function navigate(to: string) {
    await collapsePlayer()
    await navigateTo(to)
  }

  function focusSearch() {
    if (isPhone.value) {
      void navigateTo('/search')
      return
    }
    searchFocus.value++
  }

  return { panelOpen, panelTab, playerOpen, sheetTab, searchFocus, isPhone, isDesktop, isTablet, isRail, reducedMotion, openPanel, togglePanel, focusSearch, onCollapsePlayer, collapsePlayer, navigate }
})
