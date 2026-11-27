// Форматирование для интерфейса: длительности табличными цифрами, числа с разделителем разрядов,
// формы слов для чисел. Слова — в словарях (раздел format в i18n/locales), сюда приходят через tr().
// Неразрывный пробел держит число рядом со словом.
import type { Locale as I18nLocale } from 'vue-i18n'
import { tr } from './i18n-runtime'

export const NBSP = ' '

/** Код языка интерфейса: список — i18n.locales в nuxt.config.ts */
export type Locale = I18nLocale

/** Тег языка для Intl — код как есть; английский — en-US или en-GB, как принято в месте вызова */
export function intlLocale(locale: string, english: 'en-US' | 'en-GB' = 'en-US'): string {
  return locale === 'en' ? english : locale
}

/** 2:49 или 1:02:03 */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000))
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  const ss = String(s).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`
}

/** «1 ч 38 мин», «42 мин» */
export function formatLongDuration(ms: number, locale: Locale = 'ru'): string {
  const minutes = Math.max(1, Math.round(ms / 60000))
  const h = Math.floor(minutes / 60)
  const m = minutes % 60
  return h
    ? tr('format.duration.hours', { locale, named: { h, m } })
    : tr('format.duration.minutes', { locale, named: { m } })
}

export function formatNumber(n: number, locale: Locale = 'ru'): string {
  return n.toLocaleString(intlLocale(locale))
}

// У украинского те же формы для целых: 1 трек, 2 треки, 5 треків
const ruRules = new Intl.PluralRules('ru-RU')

/** Форма слова для числа: plural(5, ['трек', 'трека', 'треков']) → «треков» */
export function plural(n: number, forms: [string, string, string]): string {
  const rule = ruRules.select(Math.abs(n))
  if (rule === 'one') return forms[0]
  if (rule === 'few') return forms[1]
  return forms[2]
}

// Формы слов — format.noun.<имя> в словаре: «трек | трека | треков», «track | tracks»
export type Noun = 'track' | 'album' | 'artist' | 'playlist' | 'listener' | 'release' | 'user' | 'session' | 'file' | 'image' | 'recording' | 'play'

function nounFor(n: number, noun: Noun, locale: Locale): string {
  return tr(`format.noun.${noun}`, { locale, n })
}

// После «тыс.» и «млн» — форма для «много»: «2 млн слушателей», «2.5M listeners»
const MANY = 5

/** «412 треков», «1 трек» */
export function formatCount(n: number, noun: Noun, locale: Locale = 'ru'): string {
  return `${formatNumber(n, locale)}${NBSP}${nounFor(n, noun, locale)}`
}

/** Коротко для карточек: «2,5 млн слушателей», «18,2 тыс. слушателей», «2.5M listeners»; меньше 10 000 — целиком */
export function formatCompactCount(n: number, noun: Noun, locale: Locale = 'ru'): string {
  // Перед словом — обычный пробел: в узкой карточке слово уходит на вторую строку, а число не разрывается
  if (Math.abs(n) < 10_000) return `${formatNumber(n, locale)} ${nounFor(n, noun, locale)}`
  const short = new Intl.NumberFormat(intlLocale(locale), { notation: 'compact', maximumFractionDigits: 1 })
    .format(n)
    .replace(/\s/g, NBSP)
  return `${short} ${nounFor(MANY, noun, locale)}`
}

function parseDay(iso: string): { y: number, m: number, d: number } {
  const [y, m, d] = iso.slice(0, 10).split('-').map(Number)
  return { y: y ?? 1970, m: m ?? 1, d: d ?? 1 }
}

/** День и месяц по шаблону словаря: «6 нояб.», «Nov 6»; в другом году — с годом */
function dayMonth(d: number, month: string, y: number, sameYear: boolean, locale: Locale): string {
  const named = { d: String(d), month, y: String(y) }
  return tr(sameYear ? 'format.date.dayMonth' : 'format.date.dayMonthYear', { locale, named })
}

/** «6 нояб.», а для прошлых лет — «6 нояб. 2024» */
export function formatShortDate(iso: string, locale: Locale = 'ru', now = new Date()): string {
  const { y, m, d } = parseDay(iso)
  return dayMonth(d, tr(`format.monthShort.${m - 1}`, { locale }), y, y === now.getFullYear(), locale)
}

/** День выхода релиза: «12 декабря», в другом году — «15 января 2027»; «завтра», «сегодня» */
export function formatReleaseDay(iso: string, locale: Locale = 'ru', now = new Date()): string {
  const { y, m, d } = parseDay(iso)
  const day = Date.UTC(y, m - 1, d)
  const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate())
  const diff = Math.round((day - today) / 86_400_000)
  if (diff === 0) return tr('format.today', { locale })
  if (diff === 1) return tr('format.tomorrow', { locale })
  // Месяц в родительном падеже: «12 декабря»
  return dayMonth(d, tr(`format.monthLong.${m - 1}`, { locale }), y, y === now.getFullYear(), locale)
}

export function formatYear(iso: string | null | undefined): string {
  return iso ? iso.slice(0, 4) : ''
}

/** «2 ч назад», «вчера», «5 нояб.» */
export function formatRelative(iso: string, locale: Locale = 'ru', now = new Date()): string {
  const diff = (now.getTime() - new Date(iso).getTime()) / 1000
  if (diff < 60) return tr('format.relative.justNow', { locale })
  if (diff < 3600) return tr('format.relative.minutes', { locale, named: { n: Math.round(diff / 60) } })
  if (diff < 86400) return tr('format.relative.hours', { locale, named: { n: Math.round(diff / 3600) } })
  if (diff < 172800) return tr('format.relative.yesterday', { locale })
  return formatShortDate(iso, locale, now)
}

/** Короткий предлог не остаётся в конце строки: «Похоже на Lowborn» (и по-украински: «Схоже на») */
export function typographRu(text: string): string {
  return text.replace(/(^|\s)(на|в|во|с|со|к|ко|о|об|у|и|а|не|по|за|из|от|до|з|зі|із|від|та|й|і) /gi, `$1$2${NBSP}`)
}

export function artistNames(artists: { name: string, role?: string }[]): string {
  return artists.filter(a => a.role !== 'featured').map(a => a.name).join(', ') || artists.map(a => a.name).join(', ')
}

// Единицы — format.bytes в словаре: Б, КБ, МБ, ГБ, ТБ
const BYTE_UNITS = 5

/** «460 КБ», «1,2 ГБ» */
export function formatBytes(bytes: number, locale: Locale = 'ru'): string {
  let v = Math.max(0, bytes)
  let i = 0
  while (v >= 1024 && i < BYTE_UNITS - 1) {
    v /= 1024
    i++
  }
  const num = v.toLocaleString(intlLocale(locale), { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 })
  return `${num}${NBSP}${tr(`format.bytes.${i}`, { locale })}`
}

/** «5 нояб., 14:32» — для журналов */
export function formatDateTime(iso: string, locale: Locale = 'ru'): string {
  return new Intl.DateTimeFormat(intlLocale(locale, 'en-GB'), { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }).format(new Date(iso))
}

const CODEC_NAMES: Record<string, string> = { opus: 'Opus', aac: 'AAC', mp3: 'MP3', vorbis: 'Vorbis', flac: 'FLAC', alac: 'ALAC', pcm: 'WAV' }
const LOSSLESS = new Set(['flac', 'alac', 'pcm'])

/** Что на самом деле играет: «Opus 131», «FLAC». null — если сервер не сказал кодек */
export function qualityLabel(source: { codec?: string, bitrateKbps?: number }): string | null {
  if (!source.codec) return null
  const name = CODEC_NAMES[source.codec] ?? source.codec.toUpperCase()
  if (LOSSLESS.has(source.codec) || !source.bitrateKbps) return name
  return `${name}${NBSP}${Math.round(source.bitrateKbps)}`
}
