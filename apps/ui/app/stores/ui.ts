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

  function focusSearch() {
    if (isPhone.value) {
      void navigateTo('/search')
      return
    }
    searchFocus.value++
  }

  return { panelOpen, panelTab, playerOpen, sheetTab, searchFocus, isPhone, isDesktop, isTablet, isRail, reducedMotion, openPanel, togglePanel, focusSearch }
})
