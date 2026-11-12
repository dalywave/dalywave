// Уведомления: подписи времени в центре, значки видов, ключ VAPID для подписки браузера.
// Без зависимостей от Nuxt — проверяется `bun test`

export type Lang = 'ru' | 'en'

const MIN = 60_000
const HOUR = 60 * MIN
const DAY = 24 * HOUR

function startOfDay(d: Date) {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
}

/** Когда пришло: «сейчас», «5 мин», «3 ч», «вчера», «12 нояб.» (в этом году — без года) */
export function notificationTime(iso: string, now: Date = new Date(), lang: Lang = 'ru'): string {
  const at = new Date(iso)
  const diff = now.getTime() - at.getTime()
  if (Number.isNaN(diff)) return ''
  if (diff < MIN) return lang === 'en' ? 'now' : 'сейчас'
  if (diff < HOUR) return lang === 'en' ? `${Math.floor(diff / MIN)}m` : `${Math.floor(diff / MIN)} мин`
  const days = Math.round((startOfDay(now) - startOfDay(at)) / DAY)
  if (days <= 0) return lang === 'en' ? `${Math.floor(diff / HOUR)}h` : `${Math.floor(diff / HOUR)} ч`
  if (days === 1) return lang === 'en' ? 'yesterday' : 'вчера'
  if (days < 7) return new Intl.DateTimeFormat(lang === 'en' ? 'en-GB' : 'ru-RU', { weekday: 'short' }).format(at)
  const sameYear = at.getFullYear() === now.getFullYear()
  return new Intl.DateTimeFormat(lang === 'en' ? 'en-GB' : 'ru-RU', sameYear ? { day: 'numeric', month: 'short' } : { day: 'numeric', month: 'short', year: 'numeric' }).format(at)
}

/** Значок вида уведомления, когда картинки нет */
export function notificationIcon(kind: string): string {
  switch (kind) {
    case 'release': return 'ph:vinyl-record'
    case 'import': return 'ph:arrow-square-in'
    case 'rewind': return 'ph:calendar-star'
    case 'review': return 'ph:scales'
    case 'sources': return 'ph:warning-circle'
    default: return 'ph:bell-simple'
  }
}

/** Ссылка уведомления — путь сайта; чужое (адрес другого сайта, javascript:) не открываем */
export function notificationRoute(link: string | null | undefined): string | null {
  if (!link || !link.startsWith('/') || link.startsWith('//')) return null
  return link
}

/** Открытый ключ VAPID (base64url) → байты для `pushManager.subscribe` */
export function vapidKey(base64url: string): Uint8Array<ArrayBuffer> {
  const pad = '='.repeat((4 - (base64url.length % 4)) % 4)
  const b64 = (base64url + pad).replace(/-/g, '+').replace(/_/g, '/')
  const raw = atob(b64)
  const out = new Uint8Array(new ArrayBuffer(raw.length))
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i)
  return out
}

/** Тот же ли ключ у существующей подписки браузера (ключ сервера могли сменить) */
export function sameKey(a: ArrayBuffer | null | undefined, b: Uint8Array): boolean {
  if (!a) return false
  const x = new Uint8Array(a)
  return x.length === b.length && x.every((v, i) => v === b[i])
}

/** Минуты от полуночи ↔ «23:00» для поля времени тихих часов */
export function minutesToTime(m: number): string {
  const h = Math.floor(m / 60) % 24
  return `${String(h).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
}

export function timeToMinutes(s: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(s.trim())
  if (!m) return null
  const h = Number(m[1])
  const min = Number(m[2])
  return h < 24 && min < 60 ? h * 60 + min : null
}

/** Как назвать этот браузер в списке устройств */
export function browserName(ua: string): string {
  const os = /iPhone/.test(ua) ? 'iPhone' : /iPad/.test(ua) ? 'iPad' : /Android/.test(ua) ? 'Android' : /Mac OS X/.test(ua) ? 'Mac' : /Windows/.test(ua) ? 'Windows' : /Linux/.test(ua) ? 'Linux' : ''
  const browser = /Edg\//.test(ua) ? 'Edge' : /YaBrowser/.test(ua) ? 'Яндекс Браузер' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : 'Браузер'
  return os ? `${browser}, ${os}` : browser
}
