// Кто может открыть страницу. Вход — только через daly.rest (или аварийный пароль);
// человек без ролей видит «ждём доступа»; админка — только администраторам
export default defineNuxtRouteMiddleware(async (to) => {
  // Мини-плеер приложения — только вид плеера из движка: входа и данных сервера ему не нужно
  if (isMiniWindow()) return to.path === '/mini' ? undefined : navigateTo('/mini')
  const auth = useAuthStore()
  const bridge = useBridge()
  const isAuthPage = to.path === '/login' || to.path === '/waiting'
  const isDev = import.meta.dev && to.path.startsWith('/dev')

  void bridge.ready()
  await auth.load({ quiet: isAuthPage || isDev })
  if (isDev) return
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
