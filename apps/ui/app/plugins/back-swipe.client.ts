// «Назад» смахиванием от левого края на iPhone и iPad. Жест — штатный WKWebView (apps/shell,
// ios.rs): страница едет за пальцем, под ней — снимок предыдущей, по окончании — `popstate`, и Vue
// Router уходит назад так же, как кнопкой «назад» (PhoneNav). Здесь решается только, где жест
// уместен: на вложенной странице, если позади есть своя страница. На корневых вкладках, под листом
// или меню и при полном плеере — выключен: смахивание плеера вниз и листы остаются как были.
// На сайте и компьютерах ничего не делает (на сайте назад смахивает сам браузер)
const ROOTS = new Set(['/', '/search', '/library'])

export default defineNuxtPlugin(() => {
  const bridge = useBridge()
  if (bridge.kind !== 'tauri' || (bridge.platform !== 'ios' && bridge.platform !== 'ipados')) return

  const router = useRouter()
  const ui = useUiStore()

  // Есть ли позади своя страница: Vue Router кладёт её адрес в history.state.back. Открыли по
  // ссылке (виджет, Siri, «Поделиться») — позади ничего, «назад» ведёт на главную только кнопкой
  const readBack = () => !!(window.history.state as { back?: string | null } | null)?.back
  const hasBack = ref(readBack())
  const path = ref(router.currentRoute.value.path)
  router.afterEach((to) => {
    path.value = to.path
    hasBack.value = readBack()
  })

  // Модальные слои reka-ui (листы, меню, окна) запирают body: pointer-events и прокрутку
  const locked = ref(false)
  const readLock = () => {
    const s = document.body.style
    locked.value = s.pointerEvents === 'none' || s.overflow === 'hidden'
  }
  new MutationObserver(readLock).observe(document.body, { attributes: true, attributeFilter: ['style'] })
  readLock()

  const enabled = computed(() =>
    hasBack.value
    && !ROOTS.has(path.value)
    && !path.value.startsWith('/login')
    && path.value !== '/waiting'
    && !ui.playerOpen
    && ui.sheetTab === null
    && !locked.value,
  )
  watch(enabled, v => bridge.window.setBackSwipe(v), { immediate: true })
})
