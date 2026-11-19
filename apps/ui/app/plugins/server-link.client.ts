// Ссылка `dalywave://server?url=…` (QR-код в админке и на странице входа сайта): экран «Адрес сервера»
// с подставленным адресом — и до входа, и после. Сам адрес не меняется, пока человек не подтвердит
export default defineNuxtPlugin(() => {
  if (isMiniWindow()) return
  const bridge = useBridge()
  const appServer = bridge.appServer
  if (!appServer) return
  const router = useRouter()
  appServer.onLink((url) => {
    // Экран уже открыт — он следит за ?url= и проверит новый адрес
    const open = router.currentRoute.value.path === '/server'
    void navigateTo({ path: '/server', query: { url } }, { replace: open })
  })
})
