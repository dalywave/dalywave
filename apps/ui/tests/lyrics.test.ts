// Тексты песен: строки, проигрыши, подпевки, правило пословной подсветки. bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { buildLyrics, estimateSingMs, lyricAt, lyricProgress, plainStanzas, splitWords, syllables, wordMode, wordProgress } from '../app/utils/lyrics'
import type { LyricLine } from '../app/utils/lyrics'

const L = (tMs: number, text: string) => ({ tMs, text })
const lines = (items: ReturnType<typeof buildLyrics>) => items.filter((x): x is LyricLine => x.type === 'line')

describe('syllables', () => {
  test('counts vowels, a word without vowels still counts', () => {
    expect(syllables('романтичного')).toBe(5)
    expect(syllables('в')).toBe(1)
    expect(syllables('throne')).toBe(2)
    expect(syllables('—')).toBe(1)
  })
})

describe('splitWords', () => {
  test('time is shared by syllables and covers the whole span', () => {
    const w = splitWords('да что романтичного', 1000, 1400)
    expect(w.map(x => x.w)).toEqual(['да', 'что', 'романтичного'])
    expect(w[0]!.s).toBe(1000)
    expect(w[2]!.e).toBeCloseTo(2400)
    // 1 + 1 + 5 слогов: последнее слово — 5/7 времени
    expect(w[2]!.e - w[2]!.s).toBeCloseTo(1000)
  })
})

describe('wordMode', () => {
  test('plausible pace — words', () => {
    expect(wordMode(3000, 10, true)).toBe('words')
  })
  test('stretched line — whole line', () => {
    expect(wordMode(9000, 10, true)).toBe('line')
  })
  test('too dense — whole line', () => {
    expect(wordMode(800, 10, true)).toBe('line')
  })
  test('unknown end or a single syllable — whole line', () => {
    expect(wordMode(3000, 10, false)).toBe('line')
    expect(wordMode(400, 1, true)).toBe('line')
  })
})

describe('buildLyrics', () => {
  test('intro longer than 4 s is a break, short intro is not', () => {
    expect(buildLyrics([L(6000, 'раз два три'), L(9000, 'четыре пять')], 20000)[0]).toMatchObject({ type: 'break', intro: true, tMs: 0, endMs: 6000 })
    expect(buildLyrics([L(2000, 'раз два три'), L(5000, 'четыре пять')], 20000)[0]!.type).toBe('line')
  })

  test('empty LRCLIB line is a break until the next text, short ones are skipped', () => {
    const items = buildLyrics([L(1000, 'раз два три'), L(4000, ''), L(14000, 'четыре пять шесть'), L(17000, ''), L(18000, 'семь восемь')], 30000)
    expect(items.map(x => x.type)).toEqual(['line', 'break', 'line', 'line', 'end'])
    expect(items[1]).toMatchObject({ tMs: 4000, endMs: 14000 })
  })

  test('long pause after a line becomes an estimated end plus a break', () => {
    const items = buildLyrics([L(1000, 'хочешь солнце вместо лампы'), L(30000, 'хочешь за окошком альпы')], 40000)
    const est = estimateSingMs('хочешь солнце вместо лампы')
    expect(items[0]).toMatchObject({ type: 'line', endMs: 1000 + est, mode: 'line' })
    expect(items[1]).toMatchObject({ type: 'break', tMs: 1000 + est, endMs: 30000 })
  })

  test('long tail after the last line releases the highlight', () => {
    const items = buildLyrics([L(1000, 'раз два три'), L(4000, 'четыре пять шесть')], 120000)
    expect(items.at(-1)).toMatchObject({ type: 'end', endMs: 120000 })
  })

  test('backing vocals in brackets go to their own line', () => {
    const [a, b] = lines(buildLyrics([L(1000, 'в небе как синичка, отлично (тоже мне)'), L(4500, '(у-у-у)'), L(7000, 'дальше')], 20000))
    expect(a).toMatchObject({ main: 'в небе как синичка, отлично', backing: 'тоже мне', onlyBacking: false })
    expect(a!.backWords?.map(w => w.w)).toEqual(['тоже', 'мне'])
    // Подпевка звучит после основного текста
    expect(a!.backWords![0]!.s).toBeGreaterThanOrEqual(a!.words.at(-1)!.e - 1)
    expect(b).toMatchObject({ main: 'у-у-у', onlyBacking: true, backWords: null })
  })

  test('words finish before the next line starts', () => {
    const [a] = lines(buildLyrics([L(1000, 'можете мне не объяснять'), L(4000, 'и ваша мне была')], 20000))
    expect(a!.mode).toBe('words')
    expect(a!.words.at(-1)!.e).toBeLessThan(4000)
    expect(a!.words.at(-1)!.e).toBeCloseTo(1000 + 3000 * 0.9)
  })

  test('stretched line is lit whole', () => {
    const [a] = lines(buildLyrics([L(1000, 'о-о'), L(6800, 'и ваша мне была')], 20000))
    expect(a!.mode).toBe('line')
  })

  test('indices follow the order and unsorted input is sorted', () => {
    const items = buildLyrics([L(5000, 'два три'), L(1000, 'раз раз')], 9000)
    expect(items.map(x => x.i)).toEqual(items.map((_, i) => i))
    expect((items[0] as LyricLine).main).toBe('раз раз')
  })

  test('no text at all — nothing to show', () => {
    expect(buildLyrics([L(0, ''), L(1000, '')], 5000)).toEqual([])
  })
})

describe('lyricAt / progress', () => {
  const items = [{ tMs: 0 }, { tMs: 1000 }, { tMs: 5000 }]
  test('last item started by ms, with lead', () => {
    expect(lyricAt(items, -500, 0)).toBe(-1)
    expect(lyricAt(items, 999, 0)).toBe(0)
    expect(lyricAt(items, 760)).toBe(1)
    expect(lyricAt(items, 99999)).toBe(2)
    expect(lyricAt([], 100)).toBe(-1)
  })
  test('progress is clamped', () => {
    expect(lyricProgress({ tMs: 1000, endMs: 3000 }, 2000)).toBe(0.5)
    expect(lyricProgress({ tMs: 1000, endMs: 3000 }, 0)).toBe(0)
    expect(lyricProgress({ tMs: 1000, endMs: 3000 }, 9000)).toBe(1)
  })
  test('short words are not lit faster than 120 ms', () => {
    expect(wordProgress({ w: 'в', s: 1000, e: 1030 }, 1060)).toBeCloseTo(0.5)
  })
})

describe('plainStanzas', () => {
  test('repeated stanza is a chorus, its repeats fold', () => {
    const s = plainStanzas('a\nb\n\nПрипев раз\nприпев два\n\nc\nd\n\nприпев раз,\nприпев два\n\nодна\n\nодна')
    expect(s.map(x => [x.chorus, x.repeat])).toEqual([[false, false], [true, false], [false, false], [true, true], [false, false], [false, false]])
  })
  test('empty text', () => {
    expect(plainStanzas('')).toEqual([])
    expect(plainStanzas(null)).toEqual([])
  })
})
