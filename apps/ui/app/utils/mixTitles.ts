// Названия миксов, которые собирает сервер («Микс дня 1», «Открытия недели», «На повторе»…), хранятся
// по-русски. В интерфейсе на другом языке они показываются на языке интерфейса: узнаём микс по
// `generator` (поле плейлиста вида `mix`) и переводим название и описание (utils.mixTitles в словарях;
// нет перевода — по-английски). Чужие и свои плейлисты не трогаем.
import type { Locale } from '~/utils/format'
import { tr } from './i18n-runtime'

/** Язык, на котором сервер пишет названия миксов */
const SERVER_LOCALE = 'ru'

/** Язык интерфейса для ответов сервера: задают plugins/locale.client.ts и смена языка в настройках */
let contentLocale: string = SERVER_LOCALE
export function setContentLocale(locale: Locale) {
  contentLocale = locale
}
/** На какой язык переводить миксы; null — интерфейс на языке сервера */
export function contentTarget(): string | null {
  return contentLocale === SERVER_LOCALE ? null : contentLocale
}

interface MixLike {
  kind?: unknown
  generator?: unknown
  title?: unknown
  description?: unknown
}

const K = 'utils.mixTitles'

// Описание сервера: «A, B и другие» → «A, B» + хвост; без имён — целиком
const ARTISTS_TAIL: [RegExp, string, string][] = [
  [/(^| )и другие новые для тебя$/, 'andMoreNew', 'newToYou'],
  [/(^| )и другие$/, 'andMore', 'variousArtists'],
]

// Миксы с постоянным названием; описание — по точному тексту сервера
const FIXED: Record<string, { description?: Record<string, string> }> = {
  discover_weekly: {},
  on_repeat: { description: { 'Что вы слушаете чаще всего прямо сейчас': 'on_repeat' } },
  repeat_rewind: { description: { 'Когда-то крутились без остановки': 'repeat_rewind' } },
}

function translateDescription(text: string, to: string, fixed?: Record<string, string>): string {
  const own = fixed?.[text]
  if (own) return tr(`${K}.description.${own}`, { locale: to })
  for (const [re, tail, alone] of ARTISTS_TAIL) {
    const m = re.exec(text)
    if (m) return m[1] ? `${text.slice(0, m.index)} ${tr(`${K}.tail.${tail}`, { locale: to })}` : tr(`${K}.alone.${alone}`, { locale: to })
  }
  return text
}

/** Перевести микс на язык интерфейса (на месте). true — что-то поменялось */
export function translateMix(p: MixLike, to: string): boolean {
  if (p.kind !== 'mix' || typeof p.generator !== 'string' || typeof p.title !== 'string') return false
  const before = `${p.title}\u0000${String(p.description ?? '')}`
  if (p.generator === 'daily_mix') {
    const n = /^Микс дня (\d+)$/.exec(p.title)
    if (n) p.title = tr(`${K}.dailyMix`, { locale: to, named: { n: n[1] } })
  }
  else if (FIXED[p.generator]) {
    if (/[а-яёі]/i.test(p.title)) p.title = tr(`${K}.title.${p.generator}`, { locale: to })
  }
  if (typeof p.description === 'string') p.description = translateDescription(p.description, to, FIXED[p.generator]?.description)
  return before !== `${p.title}\u0000${String(p.description ?? '')}`
}

// Сочетания волны, которые сервер засевает новому человеку (миграция wave_presets): узнаём по
// названию и значку — переименованные человеком не трогаем
const PRESETS: Record<string, [string, string]> = {
  'laptop': ['Работа', 'work'],
  'car': ['Дорога', 'road'],
  'moon-stars': ['Ночь', 'night'],
}

function translatePreset(p: { name?: unknown, icon?: unknown, settings?: unknown }, to: string): boolean {
  if (typeof p.name !== 'string' || typeof p.icon !== 'string' || typeof p.settings !== 'object') return false
  const seeded = PRESETS[p.icon]
  if (!seeded || seeded[0] !== p.name) return false
  const name = tr(`${K}.preset.${seeded[1]}`, { locale: to })
  if (name === p.name) return false
  p.name = name
  return true
}

/** Обойти ответ сервера и перевести все миксы в нём */
export function translateMixes(value: unknown, to: string, depth = 0): boolean {
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
