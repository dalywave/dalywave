// Названия миксов, которые собирает сервер («Микс дня 1», «Открытия недели», «На повторе»…), хранятся
// по-русски. В английском интерфейсе они показываются по-английски: узнаём микс по `generator`
// (поле плейлиста вида `mix`) и переводим название и описание. Чужие и свои плейлисты не трогаем.

/** Язык интерфейса для ответов сервера: задают plugins/locale.client.ts и смена языка в настройках */
let contentLocale: 'ru' | 'en' = 'ru'
export function setContentLocale(locale: 'ru' | 'en') {
  contentLocale = locale
}
export function contentIsEnglish(): boolean {
  return contentLocale === 'en'
}

interface MixLike {
  kind?: unknown
  generator?: unknown
  title?: unknown
  description?: unknown
}

const ARTISTS_TAIL: [RegExp, string][] = [
  [/ и другие новые для тебя$/, ' and more new to you'],
  [/^и другие новые для тебя$/, 'New to you'],
  [/ и другие$/, ' and more'],
  [/^и другие$/, 'Various artists'],
]

const FIXED: Record<string, { title: string, description?: Record<string, string> }> = {
  discover_weekly: { title: 'Discover Weekly' },
  on_repeat: { title: 'On Repeat', description: { 'Что вы слушаете чаще всего прямо сейчас': 'What you play most right now' } },
  repeat_rewind: { title: 'Repeat Rewind', description: { 'Когда-то крутились без остановки': 'Once on heavy rotation' } },
}

function translateDescription(text: string, fixed?: Record<string, string>): string {
  if (fixed?.[text]) return fixed[text]
  for (const [re, en] of ARTISTS_TAIL) {
    if (re.test(text)) return text.replace(re, en)
  }
  return text
}

/** Перевести микс на английский (на месте). true — что-то поменялось */
export function englishMix(p: MixLike): boolean {
  if (p.kind !== 'mix' || typeof p.generator !== 'string' || typeof p.title !== 'string') return false
  const before = `${p.title}\u0000${String(p.description ?? '')}`
  if (p.generator === 'daily_mix') {
    const n = /^Микс дня (\d+)$/.exec(p.title)
    if (n) p.title = `Daily Mix ${n[1]}`
  }
  else if (FIXED[p.generator]) {
    if (/[а-яё]/i.test(p.title)) p.title = FIXED[p.generator]!.title
  }
  if (typeof p.description === 'string') p.description = translateDescription(p.description, FIXED[p.generator]?.description)
  return before !== `${p.title}\u0000${String(p.description ?? '')}`
}

// Сочетания волны, которые сервер засевает новому человеку (миграция wave_presets): узнаём по
// названию и значку — переименованные человеком не трогаем
const PRESETS: Record<string, [string, string]> = {
  laptop: ['Работа', 'Work'],
  'car': ['Дорога', 'Road'],
  'moon-stars': ['Ночь', 'Night'],
}

function englishPreset(p: { name?: unknown, icon?: unknown, settings?: unknown }): boolean {
  if (typeof p.name !== 'string' || typeof p.icon !== 'string' || typeof p.settings !== 'object') return false
  const seeded = PRESETS[p.icon]
  if (!seeded || seeded[0] !== p.name) return false
  p.name = seeded[1]
  return true
}

/** Обойти ответ сервера и перевести все миксы в нём */
export function englishMixes(value: unknown, depth = 0): boolean {
  if (depth > 8 || value === null || typeof value !== 'object') return false
  let changed = false
  if (Array.isArray(value)) {
    for (const item of value) changed = englishMixes(item, depth + 1) || changed
    return changed
  }
  changed = englishMix(value as MixLike)
  changed = englishPreset(value as { name?: unknown }) || changed
  for (const v of Object.values(value)) {
    if (v !== null && typeof v === 'object') changed = englishMixes(v, depth + 1) || changed
  }
  return changed
}
