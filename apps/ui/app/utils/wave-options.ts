// Настройки волны: варианты групп, сохранённые сочетания и человеческое описание
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

export interface WavePreset {
  id: string
  icon: string
  titleKey: string
  metaKey: string
  set: WaveSettings
}

export const WAVE_PRESETS: WavePreset[] = [
  { id: 'work', icon: 'ph:laptop', titleKey: 'wave.preset.work', metaKey: 'wave.preset.workMeta', set: { mood: 'calm', activity: 'work', character: 'mix', language: 'none' } },
  { id: 'road', icon: 'ph:car', titleKey: 'wave.preset.road', metaKey: 'wave.preset.roadMeta', set: { mood: 'energetic', activity: 'road', character: 'fav', language: 'ru' } },
  { id: 'night', icon: 'ph:moon-stars', titleKey: 'wave.preset.night', metaKey: 'wave.preset.nightMeta', set: { mood: 'sad', activity: 'rest', character: 'rare', language: 'any' } },
]

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
