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
  const auth = useAuthStore()
  const bridge = useBridge()
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
  await auth.load({ quiet: isAuthPage || isDev })
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
})
