// Настройки волны: варианты групп, сохранённые сочетания и человеческое описание
import type { WavePreset } from '~/api/types'
import type { WaveActivity, WaveCharacter, WaveLanguage, WaveMood, WaveSettings } from '~/bridge/types'

export interface WaveOption<T extends string> {
  value: T
  icon: string
  /** Ключ перевода: wave.<group>.<value> */
  key: string
  hintKey?: string
}

export const MOOD_OPTIONS: WaveOption<WaveMood>[] = [
  { value: 'any', icon: 'ph:sparkle', key: 'wave.mood.any' },
  { value: 'energetic', icon: 'ph:lightning', key: 'wave.mood.energetic' },
  { value: 'happy', icon: 'ph:smiley', key: 'wave.mood.happy' },
  { value: 'calm', icon: 'ph:waves', key: 'wave.mood.calm' },
  { value: 'sad', icon: 'ph:cloud-rain', key: 'wave.mood.sad' },
]

export const ACTIVITY_OPTIONS: WaveOption<WaveActivity>[] = [
  { value: 'any', icon: 'ph:infinity', key: 'wave.activity.any' },
  { value: 'wake', icon: 'ph:sun-horizon', key: 'wave.activity.wake', hintKey: 'wave.activity.wakeHint' },
  { value: 'road', icon: 'ph:car', key: 'wave.activity.road' },
  { value: 'work', icon: 'ph:laptop', key: 'wave.activity.work' },
  { value: 'sport', icon: 'ph:barbell', key: 'wave.activity.sport', hintKey: 'wave.activity.sportHint' },
  { value: 'rest', icon: 'ph:coffee', key: 'wave.activity.rest' },
  { value: 'sleep', icon: 'ph:moon-stars', key: 'wave.activity.sleep', hintKey: 'wave.activity.sleepHint' },
  { value: 'party', icon: 'ph:confetti', key: 'wave.activity.party' },
]

export const CHARACTER_OPTIONS: WaveOption<WaveCharacter>[] = [
  { value: 'fav', icon: 'ph:heart', key: 'wave.character.fav' },
  { value: 'mix', icon: 'ph:shuffle-angular', key: 'wave.character.mix' },
  { value: 'new', icon: 'ph:compass', key: 'wave.character.new' },
  { value: 'pop', icon: 'ph:trend-up', key: 'wave.character.pop' },
  { value: 'rare', icon: 'ph:diamond', key: 'wave.character.rare' },
]

export const LANGUAGE_OPTIONS: WaveOption<WaveLanguage>[] = [
  { value: 'any', icon: 'ph:globe-simple', key: 'wave.language.any' },
  { value: 'ru', icon: 'ph:translate', key: 'wave.language.ru' },
  { value: 'foreign', icon: 'ph:airplane-tilt', key: 'wave.language.foreign' },
  { value: 'none', icon: 'ph:microphone-slash', key: 'wave.language.none' },
]

/** Иконки сохранённых сочетаний — полные имена: сборка берёт в комплект только упомянутые в коде */
export const PRESET_ICONS = [
  'ph:laptop', 'ph:car', 'ph:moon-stars', 'ph:coffee', 'ph:barbell', 'ph:sun-horizon', 'ph:confetti',
  'ph:headphones', 'ph:waves', 'ph:lightning', 'ph:cloud-rain', 'ph:heart', 'ph:book-open', 'ph:sparkle',
] as const

/** Иконка сочетания: на сервере — имя без префикса; незнакомое (с другого устройства) — искра */
export function presetIcon(name: string): string {
  const full = `ph:${name}`
  return (PRESET_ICONS as readonly string[]).includes(full) ? full : 'ph:sparkle'
}

/** Иконка по умолчанию для новых настроек: занятие, иначе настроение */
export function suggestIcon(s: WaveSettings): string {
  const icon = s.activity !== 'any'
    ? ACTIVITY_OPTIONS.find(o => o.value === s.activity)?.icon
    : MOOD_OPTIONS.find(o => o.value === s.mood)?.icon
  return icon && (PRESET_ICONS as readonly string[]).includes(icon) ? icon : 'ph:sparkle'
}

/** Настройки с сервера: пропущенные поля — по умолчанию, как их понимает сервер */
export function fromApiSettings(s: WavePreset['settings']): WaveSettings {
  return { mood: s.mood ?? 'any', activity: s.activity ?? 'any', character: s.character ?? 'mix', language: s.language ?? 'any' }
}

const ACTIVITY_NOUN_RU: Record<WaveActivity, string> = { any: '', wake: 'Утро', road: 'Дорога', work: 'Работа', sport: 'Тренировка', rest: 'Отдых', sleep: 'Сон', party: 'Вечеринка' }
const MOOD_NEUT_RU: Record<WaveMood, string> = { any: '', energetic: 'Бодрое', happy: 'Весёлое', calm: 'Спокойное', sad: 'Грустное' }
const CHARACTER_SHORT_RU: Record<WaveCharacter, string> = { fav: 'любимое', mix: '', new: 'новое', pop: 'популярное', rare: 'редкое' }
const LANGUAGE_SHORT_RU: Record<WaveLanguage, string> = { any: '', ru: 'на русском', foreign: 'иностранное', none: 'без слов' }
const ACTIVITY_NOUN_EN: Record<WaveActivity, string> = { any: '', wake: 'Morning', road: 'Road', work: 'Work', sport: 'Workout', rest: 'Unwind', sleep: 'Sleep', party: 'Party' }
const MOOD_EN_SHORT: Record<WaveMood, string> = { any: '', energetic: 'Energetic', happy: 'Happy', calm: 'Calm', sad: 'Sad' }
const CHARACTER_SHORT_EN: Record<WaveCharacter, string> = { fav: 'favourites', mix: '', new: 'new', pop: 'popular', rare: 'rare' }
const LANGUAGE_SHORT_EN: Record<WaveLanguage, string> = { any: '', ru: 'Russian', foreign: 'foreign', none: 'instrumental' }

function shortParts(s: WaveSettings, locale: 'ru' | 'en'): string[] {
  const ru = locale === 'ru'
  const parts = [
    (ru ? ACTIVITY_NOUN_RU : ACTIVITY_NOUN_EN)[s.activity],
    (ru ? MOOD_NEUT_RU : MOOD_EN_SHORT)[s.mood],
    (ru ? LANGUAGE_SHORT_RU : LANGUAGE_SHORT_EN)[s.language],
    (ru ? CHARACTER_SHORT_RU : CHARACTER_SHORT_EN)[s.character],
  ].filter(Boolean)
  return parts.map((p, i) => (i === 0 ? p.charAt(0).toUpperCase() + p.slice(1) : p.toLowerCase()))
}

/** Название для нового сочетания: «Спокойное · без слов», «Работа · спокойное» */
export function suggestPresetName(s: WaveSettings, locale: 'ru' | 'en' = 'ru'): string {
  const parts = shortParts(s, locale).slice(0, 2)
  return parts.length ? parts.join(' · ') : (locale === 'ru' ? 'Моя волна' : 'My wave')
}

/** Подпись под названием сочетания: настроение, язык, характер — «спокойное, без слов» */
export function presetMeta(s: WaveSettings, locale: 'ru' | 'en' = 'ru'): string {
  const ru = locale === 'ru'
  const parts = [
    (ru ? MOOD_NEUT_RU : MOOD_EN_SHORT)[s.mood],
    (ru ? LANGUAGE_SHORT_RU : LANGUAGE_SHORT_EN)[s.language],
    (ru ? CHARACTER_SHORT_RU : CHARACTER_SHORT_EN)[s.character],
  ].filter(Boolean).map(p => p.toLowerCase())
  if (!parts.length) return ru ? 'знакомое и новое' : 'familiar and new'
  return parts.join(', ')
}

/** Цвета шара настроения: глубина, тело, свет */
export const MOOD_ORB: Record<WaveMood, [string, string, string]> = {
  any: ['#4b21ff', '#ff3d7a', '#ff9a3c'],
  energetic: ['#7a2208', '#ff6a1a', '#ffd27a'],
  happy: ['#5c0f5e', '#ff4fa8', '#ffe9a8'],
  calm: ['#0a2a52', '#1d8cbc', '#aef0ff'],
  sad: ['#1c1946', '#5b50c4', '#d3ccff'],
}

const MOOD_FEM_RU: Record<WaveMood, string> = { any: '', energetic: 'Бодрая', happy: 'Весёлая', calm: 'Спокойная', sad: 'Грустная' }
const ACTIVITY_RU: Record<WaveActivity, string> = { any: '', wake: 'для пробуждения', road: 'в дорогу', work: 'для работы', sport: 'для тренировки', rest: 'для отдыха', sleep: 'перед сном', party: 'для вечеринки' }
const CHARACTER_RU: Record<WaveCharacter, string> = { fav: 'только любимое', mix: 'знакомое и новое', new: 'только новое', pop: 'популярное', rare: 'редкое' }
const LANGUAGE_RU: Record<WaveLanguage, string> = { any: '', ru: 'на русском', foreign: 'на иностранных языках', none: 'без слов' }

const MOOD_EN: Record<WaveMood, string> = { any: '', energetic: 'Energetic', happy: 'Happy', calm: 'Calm', sad: 'Sad' }
const ACTIVITY_EN: Record<WaveActivity, string> = { any: '', wake: 'to wake up', road: 'for the road', work: 'for work', sport: 'for a workout', rest: 'to unwind', sleep: 'before sleep', party: 'for a party' }
const CHARACTER_EN: Record<WaveCharacter, string> = { fav: 'favourites only', mix: 'familiar and new', new: 'new only', pop: 'popular', rare: 'rare finds' }
const LANGUAGE_EN: Record<WaveLanguage, string> = { any: '', ru: 'in Russian', foreign: 'in other languages', none: 'instrumental' }

/** «Спокойная волна для работы — знакомое и новое, без слов» */
export function waveSummary(s: WaveSettings, locale: 'ru' | 'en' = 'ru'): string {
  const NB = ' '
  if (locale === 'en') {
    const head = [MOOD_EN[s.mood], s.mood === 'any' ? 'Wave' : 'wave', ACTIVITY_EN[s.activity] || (s.mood === 'any' ? 'for any mood' : '')].filter(Boolean).join(' ')
    return `${head} — ${[CHARACTER_EN[s.character], LANGUAGE_EN[s.language]].filter(Boolean).join(', ')}`
  }
  const head = s.mood === 'any'
    ? ['Волна', ACTIVITY_RU[s.activity] || 'под любое настроение']
    : [MOOD_FEM_RU[s.mood], 'волна', ACTIVITY_RU[s.activity]]
  const tail = [CHARACTER_RU[s.character], LANGUAGE_RU[s.language]].filter(Boolean).join(', ')
  return `${head.filter(Boolean).join(' ')}${NB}— ${tail}`
}

export function sameSettings(a: WaveSettings, b: WaveSettings): boolean {
  return a.mood === b.mood && a.activity === b.activity && a.character === b.character && a.language === b.language
}
