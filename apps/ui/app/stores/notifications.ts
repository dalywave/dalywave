import { defineStore } from 'pinia'
import type { Notification as AppNotification, PushInfo } from '~/api/types'
import type { PushPermission } from '~/bridge/types'

const PAGE = 30
/** Приложение iPhone: человек выключил push на этом устройстве — при запуске токен не отправляем */
const APP_OFF_KEY = 'dw.push.appOff'
/** «Не сейчас» у мягкого предложения включить уведомления */
const ASK_DISMISSED_KEY = 'dw.push.askDismissed'
const SW_URL = '/sw.js'

function readFlag(key: string): boolean {
  try {
    return localStorage.getItem(key) === '1'
  }
  catch {
    return false
  }
}

function writeFlag(key: string, on: boolean) {
  try {
    if (on) localStorage.setItem(key, '1')
    else localStorage.removeItem(key)
  }
  catch {
    // Без хранилища флаг проживёт до перезапуска
  }
}

/**
 * Уведомления: центр (список, непрочитанные, «Прочитать все»), живые `notification.new` и
 * `notification.read` по WebSocket — тост в окне, системное уведомление на компьютере без фокуса,
 * число на значке iPhone. И push этого устройства: Web Push на сайте (service worker /sw.js),
 * APNs в приложении для iPhone и iPad. Разрешение спрашивается только по просьбе человека
 */
export const useNotificationsStore = defineStore('notifications', () => {
  const bridge = useBridge()
  const toast = useToast()
  const ui = useUiStore()
  const t = useNuxtApp().$i18n.t

  const items = ref<AppNotification[]>([])
  const unread = ref(0)
  const next = ref<string | null>(null)
  const state = ref<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const loadingMore = ref(false)
  let started = false

  // ---------- Список ----------

  async function load() {
    if (state.value !== 'ready') state.value = 'loading'
    try {
      const page = await bridge.notifications.list(null, PAGE)
      items.value = page.items
      unread.value = page.unread
      next.value = page.next
      state.value = 'ready'
    }
    catch {
      if (state.value !== 'ready') state.value = 'error'
    }
  }

  async function loadMore() {
    if (!next.value || loadingMore.value) return
    loadingMore.value = true
    try {
      const page = await bridge.notifications.list(next.value, PAGE)
      const known = new Set(items.value.map(n => n.id))
      items.value.push(...page.items.filter(n => !known.has(n.id)))
      unread.value = page.unread
      next.value = page.next
    }
    catch {
      // Следующая попытка — при следующей прокрутке
    }
    finally {
      loadingMore.value = false
    }
  }

  function applyRead(ids: string[], all: boolean) {
    const now = new Date().toISOString()
    const set = new Set(ids)
    for (const n of items.value) {
      if (!n.readAt && (all || set.has(n.id))) n.readAt = now
    }
  }

  async function markRead(ids: string[]) {
    const fresh = ids.filter(id => items.value.some(n => n.id === id && !n.readAt))
    if (!fresh.length) return
    applyRead(fresh, false)
    unread.value = Math.max(0, unread.value - fresh.length)
    try {
      const r = await bridge.notifications.read(fresh)
      unread.value = r.unread
    }
    catch {
      // Не беда: прочтение догонит следующая загрузка
    }
  }

  async function markAllRead() {
    if (!unread.value && !items.value.some(n => !n.readAt)) return
    applyRead([], true)
    unread.value = 0
    try {
      const r = await bridge.notifications.read([])
      unread.value = r.unread
    }
    catch {
      toast.error(t('notify.readFailed'))
      void load()
    }
  }

  /** Нажатие на уведомление: прочитано и переход туда, куда оно ведёт */
  function open(n: AppNotification, link: string | null = n.link) {
    void markRead([n.id])
    const to = notificationRoute(link)
    if (to) void ui.navigate(to)
  }

  // ---------- Живые события ----------

  function onNew(n: AppNotification) {
    const at = items.value.findIndex(x => x.id === n.id)
    const wasUnread = at >= 0 && !items.value[at]!.readAt
    if (at >= 0) items.value.splice(at, 1)
    items.value.unshift(n)
    if (!n.readAt && !wasUnread) unread.value++
    announce(n)
  }

  /** Новое уведомление, пока приложение открыто: тост в окне; окно без фокуса — системное (компьютер) */
  function announce(n: AppNotification) {
    const focused = typeof document !== 'undefined' && document.visibilityState === 'visible' && document.hasFocus()
    if (!focused && bridge.systemNotify) {
      const route = notificationRoute(n.link)
      void bridge.systemNotify.show({ id: n.id, title: n.title, body: n.body, link: route ? `dalywave:/${route}` : null })
      return
    }
    if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return
    // В тост — только заголовок: строка одна, подробности — в центре уведомлений
    toast.show({
      text: n.title,
      icon: notificationIcon(n.kind),
      duration: 6000,
      action: notificationRoute(n.link) ? { label: t('notify.open'), onClick: () => open(n) } : undefined,
    })
  }

  // Число на значке приложения iPhone и iPad
  watch(unread, (count) => {
    bridge.appPush?.setBadge(count)
  })

  /** Подписка на живые события, первая загрузка, push этого устройства (оболочка зовёт при запуске) */
  function init() {
    if (started || !import.meta.client) return
    started = true
    bridge.live.on('notification.new', onNew)
    bridge.live.on('notification.read', (e) => {
      applyRead(e.ids, e.all)
      unread.value = e.unread
    })
    bridge.live.onReconnect(() => void load())
    void load()
    void resyncDevice()
    listenWorker()
  }

  // ---------- Push этого устройства ----------

  const push = ref<PushInfo | null>(null)
  const permission = ref<PushPermission>('default')
  const busy = ref(false)
  const askDismissed = ref(import.meta.client ? readFlag(ASK_DISMISSED_KEY) : false)

  const webSupported = import.meta.client && bridge.kind === 'web'
    && 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window
  const appSupported = ref(false)

  /** Можно ли включить push здесь: платформа умеет, канал на сервере настроен */
  const available = computed(() => {
    const p = push.value
    if (!p) return false
    if (bridge.appPush) return appSupported.value && p.apns
    return webSupported && !!p.webPushKey
  })
  /** Это устройство подписано */
  const enabled = computed(() => permission.value === 'granted' && !!push.value?.devices.some(d => d.current))
  /** Мягкое предложение включить: канал есть, система ещё не спрашивала, человек не отказался */
  const shouldAsk = computed(() => available.value && permission.value === 'default' && !enabled.value && !askDismissed.value)

  function dismissAsk() {
    askDismissed.value = true
    writeFlag(ASK_DISMISSED_KEY, true)
  }

  const offset = () => -new Date().getTimezoneOffset()

  async function loadPush() {
    try {
      push.value = await bridge.notifications.push()
    }
    catch {
      push.value = null
    }
    await readPermission()
  }

  async function readPermission() {
    if (bridge.appPush) {
      const s = await bridge.appPush.state()
      appSupported.value = s.supported
      permission.value = s.permission
    }
    else if (webSupported) {
      permission.value = Notification.permission
    }
  }

  async function worker(): Promise<ServiceWorkerRegistration> {
    const reg = await navigator.serviceWorker.getRegistration(SW_URL) ?? await navigator.serviceWorker.register(SW_URL)
    await navigator.serviceWorker.ready
    return reg
  }

  /** Подписка браузера на ключ сервера (старая на другой ключ заменяется) */
  async function browserSubscription(key: string): Promise<PushSubscription> {
    const reg = await worker()
    const want = vapidKey(key)
    const have = await reg.pushManager.getSubscription()
    if (have && sameKey(have.options.applicationServerKey, want)) return have
    await have?.unsubscribe().catch(() => false)
    return reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: want })
  }

  async function sendWeb(sub: PushSubscription) {
    const json = sub.toJSON()
    if (!json.endpoint || !json.keys?.p256dh || !json.keys.auth) throw new Error('subscription')
    await bridge.notifications.subscribe({
      channel: 'webpush',
      subscription: { endpoint: json.endpoint, keys: { p256dh: json.keys.p256dh, auth: json.keys.auth } },
      name: browserName(navigator.userAgent),
      utcOffsetMin: offset(),
    })
  }

  async function sendApp(prompt: boolean): Promise<boolean> {
    const r = await bridge.appPush!.register(prompt)
    if (!r) return false
    await bridge.notifications.subscribe({
      channel: 'apns',
      token: r.token,
      environment: r.environment,
      name: bridge.platform === 'ipados' ? 'iPad' : 'iPhone',
      utcOffsetMin: offset(),
    })
    try {
      localStorage.setItem('dw.push.apnsToken', r.token)
    }
    catch {
      // Без хранилища выключить можно и по списку устройств
    }
    return true
  }

  /** «Включить на этом устройстве»: системный запрос разрешения и подписка */
  async function enable(): Promise<boolean> {
    if (busy.value) return false
    busy.value = true
    try {
      if (!push.value) await loadPush()
      if (bridge.appPush) {
        writeFlag(APP_OFF_KEY, false)
        const ok = await sendApp(true)
        await readPermission()
        if (!ok) return false
      }
      else if (webSupported && push.value?.webPushKey) {
        permission.value = await Notification.requestPermission()
        if (permission.value !== 'granted') return false
        await sendWeb(await browserSubscription(push.value.webPushKey))
      }
      else {
        return false
      }
      dismissAsk()
      await loadPush()
      return true
    }
    catch {
      toast.error(t('notify.enableFailed'))
      return false
    }
    finally {
      busy.value = false
    }
  }

  /** Выключить push на этом устройстве (центр уведомлений остаётся) */
  async function disable() {
    if (busy.value) return
    busy.value = true
    try {
      if (bridge.appPush) {
        writeFlag(APP_OFF_KEY, true)
        let token: string | null = null
        try {
          token = localStorage.getItem('dw.push.apnsToken')
        }
        catch {
          token = null
        }
        // Токена не знаем — заново спросим его у системы (без запроса разрешения)
        token ??= (await bridge.appPush.register(false))?.token ?? null
        if (token) await bridge.notifications.unsubscribe({ token })
      }
      else if (webSupported) {
        const reg = await navigator.serviceWorker.getRegistration(SW_URL)
        const sub = await reg?.pushManager.getSubscription()
        if (sub) {
          await bridge.notifications.unsubscribe({ endpoint: sub.endpoint })
          await sub.unsubscribe().catch(() => false)
        }
      }
      await loadPush()
    }
    catch {
      toast.error(t('notify.disableFailed'))
    }
    finally {
      busy.value = false
    }
  }

  /** При запуске: разрешение уже есть — тихо переотправить подписку (токен мог смениться) */
  async function resyncDevice() {
    await loadPush()
    if (!available.value || permission.value !== 'granted') return
    try {
      if (bridge.appPush) {
        if (!readFlag(APP_OFF_KEY)) await sendApp(false)
      }
      else if (webSupported) {
        const reg = await navigator.serviceWorker.getRegistration(SW_URL)
        const sub = await reg?.pushManager.getSubscription()
        if (sub && push.value?.webPushKey) await sendWeb(await browserSubscription(push.value.webPushKey))
      }
      push.value = await bridge.notifications.push()
    }
    catch {
      // Подписка обновится при следующем запуске
    }
  }

  /** Пробное уведомление на все подписанные устройства */
  async function test() {
    try {
      const r = await bridge.notifications.test()
      toast.show({ text: r.devices ? t('notify.testSent') : t('notify.testCenter'), icon: 'ph:bell-simple' })
    }
    catch {
      toast.error(t('notify.testFailed'))
    }
  }

  /** Сайт: нажатие на системное уведомление при открытой вкладке — service worker просит перейти */
  function listenWorker() {
    if (!webSupported) return
    navigator.serviceWorker.addEventListener('message', (e: MessageEvent) => {
      const data = e.data as { type?: string, link?: string, id?: string } | null
      if (data?.type !== 'dw.notification.open') return
      if (data.id) void markRead([data.id])
      const to = notificationRoute(data.link)
      if (to) void ui.navigate(to)
    })
  }

  return {
    items, unread, next, state, loadingMore, push, permission, busy, available, enabled, shouldAsk,
    init, load, loadMore, markRead, markAllRead, open, loadPush, enable, disable, dismissAsk, test,
  }
})
