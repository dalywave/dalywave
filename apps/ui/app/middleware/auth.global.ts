import { shellServerChosen } from '~/bridge/tauri'

// Кто может открыть страницу. Вход — через провайдеров OIDC, свой аккаунт или аварийный пароль;
// человек без ролей видит «ждём доступа»; админка — только администраторам
export default defineNuxtRouteMiddleware(async (to) => {
  // Ссылки-приглашения и сброса пароля открываются и без входа, и с ним: у них свой токен
  if (to.path.startsWith('/invite/') || to.path.startsWith('/reset/')) {
    void useBridge().ready()
    return
  }
  // Мини-плеер приложения — только вид плеера из движка: входа и данных сервера ему не нужно
  if (isMiniWindow()) return to.path === '/mini' ? undefined : navigateTo('/mini')
  // «Слушать вместе»: страница гостя открыта без входа — у гостя свой токен этой сессии
  if (to.path.startsWith('/jam/')) return
  // Оверлей для стрима (OBS): без входа — у ссылки свой секретный ключ
  if (to.path === '/overlay') return
  // Политика конфиденциальности — открыта всем (ссылка со страницы входа и из App Store)
  if (to.path === '/privacy') return
  const auth = useAuthStore()
  const bridge = useBridge()
  // «Адрес сервера» — только в приложении; первый запуск сборки для App Store начинается с него
  if (to.path === '/server') {
    if (!bridge.appServer) return navigateTo('/login')
    void bridge.ready()
    return
  }
  if (bridge.appServer && !shellServerChosen()) return navigateTo('/server')
  // Сцена вечеринки: телевизор без входа показывает код, вход подтверждают с телефона
  if (to.path === '/party') {
    void bridge.ready()
    // Не «тихо»: у браузера телевизора после входа кодом нет отметки о входе, только cookie
    await auth.load()
    return
  }
  const isAuthPage = to.path === '/login' || to.path === '/waiting'
  const isDev = import.meta.dev && to.path.startsWith('/dev')

  void bridge.ready()
  // Возможности сервера — вместе с входом: первый экран сразу без выключенного на сервере
  const server = useServerStore()
  const features = server.load()
  await auth.load({ quiet: isAuthPage || isDev })
  if (!server.config) await features
  if (isDev) return
  // Вход приложения в системном браузере: экран входа нужен, даже если на сайте уже вошли
  if (to.path === '/login' && to.query.app !== undefined) return
  // Разработка: посмотреть экраны входа и ожидания, не выходя из аккаунта (?preview)
  if (import.meta.dev && isAuthPage && to.query.preview !== undefined) return

  if (auth.status === 'anonymous') {
    if (to.path === '/login') return
    return navigateTo({ path: '/login', query: to.fullPath !== '/' ? { next: to.fullPath } : {} })
  }
  if (auth.status === 'no-access') {
    if (to.path === '/waiting') return
    return navigateTo('/waiting')
  }
  // ready или offline (вход был, сервера сейчас нет)
  if (isAuthPage) {
    const next = typeof to.query.next === 'string' && to.query.next.startsWith('/') && !to.query.next.startsWith('//') ? to.query.next : '/'
    return navigateTo(next)
  }
  if (to.path.startsWith('/admin') && !auth.isAdmin && auth.status === 'ready') return navigateTo('/')
  // Выключенное на сервере: чарты — каталог Deezer, скачивания и «Спорные» — загрузчик
  if (!server.deezer && to.path.startsWith('/charts')) return navigateTo('/search')
  if (!server.downloader && DOWNLOADER_ADMIN.some(p => to.path.startsWith(p))) return navigateTo('/admin')
})

const DOWNLOADER_ADMIN = ['/admin/downloads', '/admin/disputed', '/admin/sources']
