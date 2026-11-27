// Слова утилит из словарей: язык без словаря (xx) показывается по-английски, не по-русски — bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { formatBytes, formatCount, formatLongDuration, formatRelative, formatReleaseDay, formatShortDate, NBSP } from '../app/utils/format'
import { translateMixes } from '../app/utils/mixTitles'
import { notificationTime } from '../app/utils/notifications'
import { presetMeta, suggestPresetName, waveSummary } from '../app/utils/wave-options'

// Языка нет в i18n/locales: типы знают только настроенные языки
const XX = 'xx' as 'en'
const now = new Date(2026, 10, 12, 15, 0, 0)
const ago = (s: number) => new Date(now.getTime() - s * 1000).toISOString()

describe('format — язык без словаря', () => {
  test('формы слов, единицы, длительность', () => {
    expect(formatCount(1, 'track', XX)).toBe(`1${NBSP}track`)
    expect(formatCount(5, 'listener', XX)).toBe(`5${NBSP}listeners`)
    expect(formatBytes(500, XX)).toBe(`500${NBSP}B`)
    expect(formatLongDuration(98 * 60_000, XX)).toBe(`1${NBSP}h 38${NBSP}min`)
  })
  test('даты и время', () => {
    expect(formatShortDate('2026-11-06', XX, now)).toBe('Nov 6')
    expect(formatShortDate('2024-03-06', XX, now)).toBe('Mar 6, 2024')
    expect(formatReleaseDay('2026-11-12', XX, now)).toBe('today')
    expect(formatReleaseDay('2026-11-13', XX, now)).toBe('tomorrow')
    expect(formatReleaseDay('2026-12-15', XX, now)).toBe('December 15')
    expect(formatRelative(ago(10), XX, now)).toBe('just now')
    expect(formatRelative(ago(300), XX, now)).toBe('5 min ago')
    expect(formatRelative(ago(90_000), XX, now)).toBe('yesterday')
    expect(notificationTime(ago(10), now, XX)).toBe('now')
    expect(notificationTime(ago(300), now, XX)).toBe('5m')
    expect(notificationTime(new Date(2026, 10, 11, 9).toISOString(), now, XX)).toBe('yesterday')
  })
})

describe('волна — описание на языке интерфейса', () => {
  const s = { mood: 'calm', activity: 'work', character: 'mix', language: 'none' } as const
  test('ru, uk, en', () => {
    expect(waveSummary(s)).toBe(`Спокойная волна для работы${NBSP}— знакомое и новое, без слов`)
    expect(waveSummary(s, 'uk')).toBe(`Спокійна хвиля для роботи${NBSP}— знайоме й нове, без слів`)
    expect(waveSummary(s, 'en')).toBe('Calm wave for work — familiar and new, instrumental')
    expect(waveSummary({ mood: 'any', activity: 'any', character: 'fav', language: 'any' })).toBe(`Волна под любое настроение${NBSP}— только любимое`)
    expect(suggestPresetName(s)).toBe('Работа · спокойное')
    expect(presetMeta({ mood: 'any', activity: 'any', character: 'mix', language: 'any' }, 'uk')).toBe('знайоме й нове')
  })
  test('язык без словаря — по-английски', () => {
    expect(waveSummary(s, XX)).toBe('Calm wave for work — familiar and new, instrumental')
    expect(suggestPresetName({ mood: 'any', activity: 'any', character: 'mix', language: 'any' }, XX)).toBe('My wave')
  })
})

describe('миксы сервера — язык без словаря', () => {
  test('по-английски, а не как пришло с сервера', () => {
    const data = {
      items: [
        { kind: 'mix', generator: 'daily_mix', title: 'Микс дня 2', description: 'Burning House, Bluebridge и другие' },
        { kind: 'mix', generator: 'discover_weekly', title: 'Открытия недели', description: 'и другие новые для тебя' },
        { kind: 'mix', generator: 'repeat_rewind', title: 'Старые хиты на повторе', description: 'Когда-то крутились без остановки' },
        { name: 'Ночь', icon: 'moon-stars', settings: {} },
      ],
    }
    expect(translateMixes(data, 'xx')).toBe(true)
    expect(data.items.map(p => ('title' in p ? p.title : p.name))).toEqual(['Daily Mix 2', 'Discover Weekly', 'Repeat Rewind', 'Night'])
    expect(data.items.map(p => ('description' in p ? p.description : ''))).toEqual(['Burning House, Bluebridge and more', 'New to you', 'Once on heavy rotation', ''])
    expect(translateMixes(data, 'xx')).toBe(false)
  })
})
