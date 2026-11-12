// «Итоги года» в трёх стилях: какие слайды показать, цвета стилей и стиль из адреса предпросмотра — bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import type { Rewind } from '../app/api/types'
import { contrast, recordColors, slideColors, waveColors } from '../app/utils/rewind-colors'
import { parseRewindStyle, rewindSlides } from '../app/utils/rewind-slides'

const track = (id: string) => ({ id, title: id, artists: [], album: { id: `al-${id}`, image: { small: `/img/${id}.jpg` } } })
const base = {
  year: 2026, plays: 120, minutes: 900, topTracks: [], topArtists: [], topAlbums: [], genres: [], moods: [], discoveries: [],
  firstTrack: null, trackOfYear: null, topDay: null, streak: null, newArtists: 0,
}
const year = (patch: Partial<Record<string, unknown>> = {}) => ({ ...base, ...patch }) as unknown as Rewind

describe('rewindSlides', () => {
  test('a silent year is one slide', () => {
    expect(rewindSlides(year({ plays: 0 })).map(s => s.kind)).toEqual(['empty'])
  })

  test('slides without data are skipped, the final two always close the story', () => {
    const kinds = rewindSlides(year()).map(s => s.kind)
    expect(kinds).not.toContain('first')
    expect(kinds).not.toContain('toy')
    expect(kinds).not.toContain('artists')
    expect(kinds.slice(-3)).toEqual(['summary', 'wave', 'more'])
  })

  test('a full year has all fourteen slides in order', () => {
    const t = track('a')
    const r = year({
      topTracks: [{ track: t, plays: 5, minutes: 10 }],
      topArtists: [{ artist: { id: 'x', name: 'X', image: null }, plays: 5, minutes: 10 }],
      topAlbums: [{ album: { id: 'al', title: 'A', palette: ['#ff0000'], image: null, artist: { name: 'X' } }, plays: 5, minutes: 10 }],
      firstTrack: { track: t, at: '2026-01-01T00:00:00Z' },
      trackOfYear: { track: t, plays: 5, minutes: 10, firstPlayed: '2026-01-01', peakMonth: 1 },
    })
    expect(rewindSlides(r).map(s => s.kind)).toEqual([
      'intro', 'minutes', 'days', 'first', 'tracks', 'toy', 'artists', 'albums', 'sound', 'discoveries', 'personality', 'summary', 'wave', 'more',
    ])
  })
})

describe('style colours', () => {
  test('wave palette: near-black ground, light crest; calm default without colours', () => {
    const [ground, , crest, light] = waveColors(['#e0402a', '#2a60e0'])
    expect(contrast(ground, '#000000')).toBeLessThan(1.2)
    expect(contrast(light, '#000000')).toBeGreaterThan(10)
    expect(crest).toMatch(/^#[0-9a-f]{6}$/)
    expect(waveColors(null)).toEqual(['#020509', '#0a2a52', '#1d8cbc', '#aef0ff'])
  })

  test('record ground stays dark, cream text reads on it', () => {
    for (const c of ['#ffd400', '#3a8f5a', '#cccccc']) {
      const { bg } = recordColors([c])
      expect(contrast(bg, '#f3ecdf')).toBeGreaterThan(10)
    }
  })

  test('spread field: ink readable on the field', () => {
    for (const c of ['#ffd400', '#1a1a80', '#e0402a']) {
      const s = slideColors([c])
      expect(contrast(s.bg, s.ink)).toBeGreaterThanOrEqual(4.5)
    }
  })
})

test('preview style from the address', () => {
  expect(parseRewindStyle('record')).toBe('record')
  expect(parseRewindStyle('spread')).toBe('spread')
  expect(parseRewindStyle('random')).toBeNull()
  expect(parseRewindStyle(undefined)).toBeNull()
})
