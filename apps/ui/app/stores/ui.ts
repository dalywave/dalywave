import { defineStore } from 'pinia'

export type PanelTab = 'queue' | 'lyrics' | 'devices' | 'about' | 'jam'

// Состояние оболочки: правая панель, полноэкранный плеер, ширина экрана
export const useUiStore = defineStore('ui', () => {
  const panelOpen = ref(false)
  const panelTab = ref<PanelTab>('queue')
  const playerOpen = ref(false)
  /** Текст песни «сценой» внутри полного плеера: на телефоне — вместо обложки, на iPad — вкладка
   *  рядом с плеером, на компьютере — колонка справа от обложки */
  const playerLyrics = ref(false)
  /** Шторка на телефоне вместо правой панели */
  const sheetTab = ref<PanelTab | null>(null)
  /** Счётчик запросов «фокус в поиск» (⌘K) — поле поиска следит за ним */
  const searchFocus = ref(0)

  const isPhone = useMediaQuery('(max-width: 699px)')
  const isDesktop = useMediaQuery('(min-width: 1100px)')
  const isTablet = computed(() => !isPhone.value && !isDesktop.value)
  /** Узкая боковая панель из значков */
  const isRail = useMediaQuery('(min-width: 700px) and (max-width: 1023px)')
  /** Планшет под пальцем (iPad), как Spotify на iPad: без боковой панели и верхней строки — внизу
   *  плавающие вкладки и карточка плеера, плеер на весь экран, очередь и текст — листами.
   *  iPad с трекпадом — тоже он: главный указатель всё равно палец */
  const coarse = useMediaQuery('(pointer: coarse)')
  const ipad = import.meta.client && useBridge().platform === 'ipados'
  const isTouch = computed(() => !isPhone.value && (coarse.value || ipad))
  /** Интерфейс под палец — телефон или планшет: крупные цели, листы вместо панелей, без подсказок */
  const isTouchUi = computed(() => isPhone.value || isTouch.value)
  /** Планшет горизонтально (iPad от 1000 pt): места хватает, чтобы держать плеер колонкой сбоку.
   *  Ориентация — тоже условие: большой iPad вертикально шире 1000 pt, но колонка там тесная */
  const wide = useMediaQuery('(min-width: 1000px) and (orientation: landscape)')
  const canSidePlayer = computed(() => isTouch.value && wide.value)
  /** «Плеер сбоку» сейчас на экране: включён в настройках, планшет лежит горизонтально и что-то играет.
   *  Вертикально — обычная карточка внизу, даже если настройка включена */
  const sidePlayer = computed(() => canSidePlayer.value && usePrefsStore().sidePlayer && !!usePlayerStore().current)
  const reducedMotion = useMediaQuery('(prefers-reduced-motion: reduce)')

  /** Под пальцем текст открывается в самом плеере, а не листом поверх */
  function openLyricsInPlayer(): boolean {
    if (!isTouchUi.value || !usePlayerStore().current) return false
    playerLyrics.value = true
    playerOpen.value = true
    sheetTab.value = null
    return true
  }

  function openPanel(tab: PanelTab) {
    if (tab === 'lyrics' && openLyricsInPlayer()) return
    if (isTouchUi.value) {
      sheetTab.value = tab
      return
    }
    panelTab.value = tab
    panelOpen.value = true
  }

  function togglePanel(tab: PanelTab) {
    if (tab === 'lyrics' && isTouchUi.value && playerOpen.value) {
      playerLyrics.value = !playerLyrics.value
      return
    }
    if (tab === 'lyrics' && openLyricsInPlayer()) return
    if (isTouchUi.value) {
      sheetTab.value = sheetTab.value === tab ? null : tab
      return
    }
    if (panelOpen.value && panelTab.value === tab) panelOpen.value = false
    else openPanel(tab)
  }

  // Плеер под пальцем сворачивается своей анимацией (вниз, в мини-плеер); её регистрирует FullPlayer
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
    if (isTouchUi.value && collapser) await collapser()
    playerOpen.value = false
  }

  /** Переход из меню или ссылки, которые могут быть над плеером */
  async function navigate(to: string) {
    await collapsePlayer()
    await navigateTo(to)
  }

  function focusSearch() {
    if (isTouchUi.value) {
      void navigateTo('/search')
      return
    }
    searchFocus.value++
  }

  return { panelOpen, panelTab, playerOpen, playerLyrics, sheetTab, searchFocus, isPhone, isDesktop, isTablet, isRail, isTouch, isTouchUi, canSidePlayer, sidePlayer, reducedMotion, openPanel, togglePanel, focusSearch, onCollapsePlayer, collapsePlayer, navigate }
})
