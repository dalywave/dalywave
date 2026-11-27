// Названия миксов, которые собирает сервер («Микс дня 1», «Открытия недели», «На повторе»…), хранятся
// по-русски. В английском и украинском интерфейсе они показываются на языке интерфейса: узнаём микс по
// `generator` (поле плейлиста вида `mix`) и переводим название и описание. Чужие и свои плейлисты не трогаем.
import type { Locale } from '~/utils/format'

type Target = Exclude<Locale, 'ru'>

/** Язык интерфейса для ответов сервера: задают plugins/locale.client.ts и смена языка в настройках */
let contentLocale: Locale = 'ru'
export function setContentLocale(locale: Locale) {
  contentLocale = locale
}
export function contentTarget(): Target | null {
  return contentLocale === 'ru' ? null : contentLocale
}

interface MixLike {
  kind?: unknown
  generator?: unknown
  title?: unknown
  description?: unknown
}

const ARTISTS_TAIL: [RegExp, Record<Target, string>][] = [
  [/ и другие новые для тебя$/, { en: ' and more new to you', uk: ' та інші нові для тебе' }],
  [/^и другие новые для тебя$/, { en: 'New to you', uk: 'Нове для тебе' }],
  [/ и другие$/, { en: ' and more', uk: ' та інші' }],
  [/^и другие$/, { en: 'Various artists', uk: 'Різні виконавці' }],
]

const DAILY: Record<Target, string> = { en: 'Daily Mix', uk: 'Мікс дня' }

const FIXED: Record<string, { title: Record<Target, string>, description?: Record<string, Record<Target, string>> }> = {
  discover_weekly: { title: { en: 'Discover Weekly', uk: 'Відкриття тижня' } },
  on_repeat: {
    title: { en: 'On Repeat', uk: 'На повторі' },
    description: { 'Что вы слушаете чаще всего прямо сейчас': { en: 'What you play most right now', uk: 'Що ви слухаєте найчастіше просто зараз' } },
  },
  repeat_rewind: {
    title: { en: 'Repeat Rewind', uk: 'Старі хіти на повторі' },
    description: { 'Когда-то крутились без остановки': { en: 'Once on heavy rotation', uk: 'Колись крутилися без упину' } },
  },
}

function translateDescription(text: string, to: Target, fixed?: Record<string, Record<Target, string>>): string {
  if (fixed?.[text]) return fixed[text][to]
  for (const [re, tail] of ARTISTS_TAIL) {
    if (re.test(text)) return text.replace(re, tail[to])
  }
  return text
}

/** Перевести микс на язык интерфейса (на месте). true — что-то поменялось */
export function translateMix(p: MixLike, to: Target): boolean {
  if (p.kind !== 'mix' || typeof p.generator !== 'string' || typeof p.title !== 'string') return false
  const before = `${p.title}\u0000${String(p.description ?? '')}`
  if (p.generator === 'daily_mix') {
    const n = /^Микс дня (\d+)$/.exec(p.title)
    if (n) p.title = `${DAILY[to]} ${n[1]}`
  }
  else if (FIXED[p.generator]) {
    if (/[а-яёі]/i.test(p.title)) p.title = FIXED[p.generator]!.title[to]
  }
  if (typeof p.description === 'string') p.description = translateDescription(p.description, to, FIXED[p.generator]?.description)
  return before !== `${p.title}\u0000${String(p.description ?? '')}`
}

// Сочетания волны, которые сервер засевает новому человеку (миграция wave_presets): узнаём по
// названию и значку — переименованные человеком не трогаем
const PRESETS: Record<string, [string, Record<Target, string>]> = {
  laptop: ['Работа', { en: 'Work', uk: 'Робота' }],
  'car': ['Дорога', { en: 'Road', uk: 'Дорога' }],
  'moon-stars': ['Ночь', { en: 'Night', uk: 'Ніч' }],
}

function translatePreset(p: { name?: unknown, icon?: unknown, settings?: unknown }, to: Target): boolean {
  if (typeof p.name !== 'string' || typeof p.icon !== 'string' || typeof p.settings !== 'object') return false
  const seeded = PRESETS[p.icon]
  if (!seeded || seeded[0] !== p.name || seeded[1][to] === p.name) return false
  p.name = seeded[1][to]
  return true
}

/** Обойти ответ сервера и перевести все миксы в нём */
export function translateMixes(value: unknown, to: Target, depth = 0): boolean {
  if (depth > 8 || value === null || typeof value !== 'object') return false
  let changed = false
  if (Array.isArray(value)) {
    for (const item of value) changed = translateMixes(item, to, depth + 1) || changed
    return changed
  }
  changed = translateMix(value as MixLike, to)
  changed = translatePreset(value as { name?: unknown }, to) || changed
  for (const v of Object.values(value)) {
    if (v !== null && typeof v === 'object') changed = translateMixes(v, to, depth + 1) || changed
  }
  return changed
}

/** Миксы по-английски (на месте) */
export function englishMixes(value: unknown): boolean {
  return translateMixes(value, 'en')
}
