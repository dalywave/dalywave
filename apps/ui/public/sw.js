// Service worker сайта DalyWave — только для уведомлений (Web Push): показать пришедшее и открыть
// нужную страницу по нажатию. Ничего не кеширует: обновления сайта приходят как обычно.
// Регистрируется, только когда человек включает уведомления (stores/notifications.ts).
//
// Полезная нагрузка с сервера (зашифрована, JSON): { id, title, body, link, image, kind, unread }.

self.addEventListener('install', () => self.skipWaiting())
self.addEventListener('activate', event => event.waitUntil(self.clients.claim()))

/** Safari отзывает подписку, если push пришёл без показанного уведомления — там показываем всегда */
const isSafari = /Safari\//.test(self.navigator.userAgent) && !/Chrome\/|Chromium\/|Edg\//.test(self.navigator.userAgent)

self.addEventListener('push', (event) => {
  let data = {}
  try {
    data = event.data ? event.data.json() : {}
  }
  catch {
    data = { title: event.data ? event.data.text() : '' }
  }
  event.waitUntil((async () => {
    // Вкладка открыта и перед глазами — там уже тост из живых событий, второе уведомление ни к чему
    const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true })
    if (!isSafari && windows.some(c => c.visibilityState === 'visible' && c.focused)) return
    if (typeof data.unread === 'number' && self.navigator.setAppBadge) {
      await self.navigator.setAppBadge(data.unread).catch(() => {})
    }
    await self.registration.showNotification(data.title || 'DalyWave', {
      body: data.body || '',
      icon: '/icon-512.png',
      badge: '/favicon-32.png',
      tag: data.id || undefined,
      data: { id: data.id || null, link: typeof data.link === 'string' && data.link.startsWith('/') ? data.link : '/notifications' },
    })
  })())
})

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const { id, link } = event.notification.data || {}
  const target = link || '/'
  event.waitUntil((async () => {
    const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true })
    const same = windows.find(c => new URL(c.url).origin === self.location.origin)
    if (same) {
      await same.focus().catch(() => {})
      same.postMessage({ type: 'dw.notification.open', id, link: target })
      return
    }
    await self.clients.openWindow(target)
  })())
})
