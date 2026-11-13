// Форматирование для интерфейса: длительности табличными цифрами, числа с разделителем разрядов,
// русские формы слов. Неразрывный пробел держит число рядом со словом.

export const NBSP = ' '

export type Locale = 'ru' | 'en'

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
  if (locale === 'en') return h ? `${h}${NBSP}h ${m}${NBSP}min` : `${m}${NBSP}min`
  return h ? `${h}${NBSP}ч ${m}${NBSP}мин` : `${m}${NBSP}мин`
}

export function formatNumber(n: number, locale: Locale = 'ru'): string {
  return n.toLocaleString(locale === 'ru' ? 'ru-RU' : 'en-US')
}

const ruRules = new Intl.PluralRules('ru-RU')

/** Форма слова для числа: plural(5, ['трек', 'трека', 'треков']) → «треков» */
export function plural(n: number, forms: [string, string, string]): string {
  const rule = ruRules.select(Math.abs(n))
  if (rule === 'one') return forms[0]
  if (rule === 'few') return forms[1]
  return forms[2]
}

const NOUNS = {
  track: { ru: ['трек', 'трека', 'треков'], en: ['track', 'tracks'] },
  album: { ru: ['альбом', 'альбома', 'альбомов'], en: ['album', 'albums'] },
  artist: { ru: ['артист', 'артиста', 'артистов'], en: ['artist', 'artists'] },
  playlist: { ru: ['плейлист', 'плейлиста', 'плейлистов'], en: ['playlist', 'playlists'] },
  listener: { ru: ['слушатель', 'слушателя', 'слушателей'], en: ['listener', 'listeners'] },
  release: { ru: ['релиз', 'релиза', 'релизов'], en: ['release', 'releases'] },
  user: { ru: ['пользователь', 'пользователя', 'пользователей'], en: ['user', 'users'] },
  session: { ru: ['сессия', 'сессии', 'сессий'], en: ['session', 'sessions'] },
  file: { ru: ['файл', 'файла', 'файлов'], en: ['file', 'files'] },
  image: { ru: ['картинка', 'картинки', 'картинок'], en: ['image', 'images'] },
  recording: { ru: ['запись', 'записи', 'записей'], en: ['recording', 'recordings'] },
  play: { ru: ['прослушивание', 'прослушивания', 'прослушиваний'], en: ['play', 'plays'] },
} as const

export type Noun = keyof typeof NOUNS

/** «412 треков», «1 трек» */
export function formatCount(n: number, noun: Noun, locale: Locale = 'ru'): string {
  const forms = NOUNS[noun]
  if (locale === 'en') return `${formatNumber(n, 'en')}${NBSP}${n === 1 ? forms.en[0] : forms.en[1]}`
  return `${formatNumber(n)}${NBSP}${plural(n, forms.ru as unknown as [string, string, string])}`
}

/** Коротко для карточек: «2,5 млн слушателей», «18,2 тыс. слушателей», «2.5M listeners»; меньше 10 000 — целиком */
export function formatCompactCount(n: number, noun: Noun, locale: Locale = 'ru'): string {
  // Перед словом — обычный пробел: в узкой карточке слово уходит на вторую строку, а число не разрывается
  const forms = NOUNS[noun]
  if (Math.abs(n) < 10_000) {
    const word = locale === 'en' ? forms.en[n === 1 ? 0 : 1] : plural(n, forms.ru as unknown as [string, string, string])
    return `${formatNumber(n, locale)} ${word}`
  }
  const short = new Intl.NumberFormat(locale === 'ru' ? 'ru-RU' : 'en-US', { notation: 'compact', maximumFractionDigits: 1 })
    .format(n)
    .replace(/\s/g, NBSP)
  // После «тыс.» и «млн» — всегда родительный множественного: «2 млн слушателей»
  return `${short} ${locale === 'en' ? forms.en[1] : forms.ru[2]}`
}

const MONTHS_RU = ['янв.', 'февр.', 'марта', 'апр.', 'мая', 'июня', 'июля', 'авг.', 'сент.', 'окт.', 'нояб.', 'дек.']
const MONTHS_EN = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

function parseDay(iso: string): { y: number, m: number, d: number } {
  const [y, m, d] = iso.slice(0, 10).split('-').map(Number)
  return { y: y ?? 1970, m: m ?? 1, d: d ?? 1 }
}

/** «6 нояб.», а для прошлых лет — «6 нояб. 2024» */
export function formatShortDate(iso: string, locale: Locale = 'ru', now = new Date()): string {
  const { y, m, d } = parseDay(iso)
  const sameYear = y === now.getFullYear()
  if (locale === 'en') return `${MONTHS_EN[m - 1]} ${d}${sameYear ? '' : `, ${y}`}`
  return `${d}${NBSP}${MONTHS_RU[m - 1]}${sameYear ? '' : `${NBSP}${y}`}`
}

const MONTHS_RU_FULL = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря']
const MONTHS_EN_FULL = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']

/** День выхода релиза: «12 декабря», в другом году — «15 января 2027»; «завтра», «сегодня» */
export function formatReleaseDay(iso: string, locale: Locale = 'ru', now = new Date()): string {
  const { y, m, d } = parseDay(iso)
  const day = Date.UTC(y, m - 1, d)
  const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate())
  const diff = Math.round((day - today) / 86_400_000)
  if (diff === 0) return locale === 'en' ? 'today' : 'сегодня'
  if (diff === 1) return locale === 'en' ? 'tomorrow' : 'завтра'
  const sameYear = y === now.getFullYear()
  if (locale === 'en') return `${MONTHS_EN_FULL[m - 1]} ${d}${sameYear ? '' : `, ${y}`}`
  return `${d}${NBSP}${MONTHS_RU_FULL[m - 1]}${sameYear ? '' : `${NBSP}${y}`}`
}

export function formatYear(iso: string | null | undefined): string {
  return iso ? iso.slice(0, 4) : ''
}

/** «2 ч назад», «вчера», «5 нояб.» */
export function formatRelative(iso: string, locale: Locale = 'ru', now = new Date()): string {
  const diff = (now.getTime() - new Date(iso).getTime()) / 1000
  if (locale === 'en') {
    if (diff < 60) return 'just now'
    if (diff < 3600) return `${Math.round(diff / 60)} min ago`
    if (diff < 86400) return `${Math.round(diff / 3600)} h ago`
    return formatShortDate(iso, 'en', now)
  }
  if (diff < 60) return 'только что'
  if (diff < 3600) return `${Math.round(diff / 60)}${NBSP}мин назад`
  if (diff < 86400) return `${Math.round(diff / 3600)}${NBSP}ч назад`
  if (diff < 172800) return 'вчера'
  return formatShortDate(iso, 'ru', now)
}

/** Короткий предлог не остаётся в конце строки: «Похоже на Lowborn» */
export function typographRu(text: string): string {
  return text.replace(/(^|\s)(на|в|во|с|со|к|ко|о|об|у|и|а|не|по|за|из|от|до) /gi, `$1$2${NBSP}`)
}

export function artistNames(artists: { name: string, role?: string }[]): string {
  return artists.filter(a => a.role !== 'featured').map(a => a.name).join(', ') || artists.map(a => a.name).join(', ')
}

/** «460 КБ», «1,2 ГБ» */
export function formatBytes(bytes: number, locale: Locale = 'ru'): string {
  const units = locale === 'en' ? ['B', 'KB', 'MB', 'GB', 'TB'] : ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ']
  let v = Math.max(0, bytes)
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  const num = v.toLocaleString(locale === 'ru' ? 'ru-RU' : 'en-US', { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 })
  return `${num}${NBSP}${units[i]}`
}

/** «5 нояб., 14:32» — для журналов */
export function formatDateTime(iso: string, locale: Locale = 'ru'): string {
  return new Intl.DateTimeFormat(locale === 'en' ? 'en-GB' : 'ru-RU', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }).format(new Date(iso))
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
