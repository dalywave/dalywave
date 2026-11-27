// Настройки волны: варианты групп, сохранённые сочетания и человеческое описание
import type { WavePreset } from '~/api/types'
import type { WaveActivity, WaveCharacter, WaveLanguage, WaveMood, WaveSettings } from '~/bridge/types'
import type { Locale } from '~/utils/format'
import { tr } from './i18n-runtime'

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

// Слова описаний — utils.waveOptions в словарях. В коротких подписях не называется то, что как по
// умолчанию: любое настроение, занятие и язык, «знакомое и новое»
const W = 'utils.waveOptions'

/** Короткие слова для названия и подписи сочетания */
function shortWords(s: WaveSettings, locale: Locale) {
  const word = (group: string, value: string, skip: string) => (value === skip ? '' : tr(`${W}.short.${group}.${value}`, { locale }))
  return {
    activity: word('activity', s.activity, 'any'),
    mood: word('mood', s.mood, 'any'),
    language: word('language', s.language, 'any'),
    character: word('character', s.character, 'mix'),
  }
}

function shortParts(s: WaveSettings, locale: Locale): string[] {
  const w = shortWords(s, locale)
  const parts = [w.activity, w.mood, w.language, w.character].filter(Boolean)
  return parts.map((p, i) => (i === 0 ? p.charAt(0).toUpperCase() + p.slice(1) : p.toLowerCase()))
}

/** Название для нового сочетания: «Спокойное · без слов», «Работа · спокойное» */
export function suggestPresetName(s: WaveSettings, locale: Locale = 'ru'): string {
  const parts = shortParts(s, locale).slice(0, 2)
  return parts.length ? parts.join(' · ') : tr(`${W}.myWave`, { locale })
}

/** Подпись под названием сочетания: настроение, язык, характер — «спокойное, без слов» */
export function presetMeta(s: WaveSettings, locale: Locale = 'ru'): string {
  const w = shortWords(s, locale)
  const parts = [w.mood, w.language, w.character].filter(Boolean).map(p => p.toLowerCase())
  if (!parts.length) return tr(`${W}.familiarAndNew`, { locale })
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

/** «Спокойная волна для работы — знакомое и новое, без слов» */
export function waveSummary(s: WaveSettings, locale: Locale = 'ru'): string {
  const word = (group: string, value: string) => tr(`${W}.summary.${group}.${value}`, { locale })
  // Начало — по шаблону словаря: порядок слов у языков разный
  const head = s.mood === 'any'
    ? (s.activity === 'any' ? 'any' : 'activity')
    : (s.activity === 'any' ? 'mood' : 'moodActivity')
  const named = {
    mood: s.mood === 'any' ? '' : word('mood', s.mood),
    activity: s.activity === 'any' ? '' : word('activity', s.activity),
  }
  const tail = [word('character', s.character), s.language === 'any' ? '' : word('language', s.language)].filter(Boolean).join(', ')
  return tr(`${W}.summary.line`, { locale, named: { head: tr(`${W}.summary.head.${head}`, { locale, named }), tail } })
}

export function sameSettings(a: WaveSettings, b: WaveSettings): boolean {
  return a.mood === b.mood && a.activity === b.activity && a.character === b.character && a.language === b.language
}
