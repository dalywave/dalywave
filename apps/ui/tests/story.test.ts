// Видео-история и «Скоро выйдет»: строка текста в момент кадра, движение волны от звука, день выхода
import { describe, expect, test } from 'bun:test'
import { frameLevels, lineAt } from '../app/lib/story/levels'
import { formatReleaseDay as format } from '../app/utils/format'

const formatReleaseDay = (...a: Parameters<typeof format>) => format(...a).replace(/\u00a0/g, ' ')

/** Звук без Web Audio: то, что frameLevels берёт у AudioBuffer */
function buffer(sampleRate: number, seconds: number, fill: (t: number) => number): AudioBuffer {
  const data = new Float32Array(Math.round(sampleRate * seconds))
  for (let i = 0; i < data.length; i++) data[i] = fill(i / sampleRate)
  return { sampleRate, length: data.length, numberOfChannels: 1, getChannelData: () => data } as unknown as AudioBuffer
}

describe('строка текста', () => {
  const lines = [{ atMs: 0, text: 'первая' }, { atMs: 4000, text: '' }, { atMs: 6000, text: 'вторая' }]
  test('та, что звучит', () => {
    expect(lineAt(lines, 0)?.text).toBe('первая')
    expect(lineAt(lines, 3999)?.text).toBe('первая')
    expect(lineAt(lines, 6500)?.text).toBe('вторая')
  })
  test('пустая строка — проигрыш, без подписи', () => {
    expect(lineAt(lines, 5000)).toBeNull()
    expect(lineAt([], 1000)).toBeNull()
  })
})

describe('волна от звука', () => {
  test('кадр на каждую 1/30 с, громче — больше энергии', () => {
    // 2 с тишины, потом 2 с громкого синуса
    const levels = frameLevels(buffer(8000, 4, t => (t < 2 ? 0 : 0.6 * Math.sin(2 * Math.PI * 220 * t))), 30)
    expect(levels.length).toBeGreaterThanOrEqual(120)
    const quiet = levels[50]!.energy
    const loud = levels[115]!.energy
    expect(loud).toBeGreaterThan(quiet + 0.3)
    expect(loud).toBeLessThanOrEqual(1)
  })
  test('удар после тишины — пульс', () => {
    const levels = frameLevels(buffer(8000, 3, t => (t >= 1.5 && t < 1.6 ? 0.9 * Math.sin(2 * Math.PI * 80 * t) : 0.001)), 30)
    const peak = Math.max(...levels.slice(44, 52).map(l => l.beat))
    expect(peak).toBeGreaterThan(0.3)
    expect(levels[20]!.beat).toBeLessThan(0.05)
  })
})

describe('день выхода', () => {
  const now = new Date(2026, 10, 13, 12)
  test('в этом году — без года, в другом — с годом', () => {
    expect(formatReleaseDay('2026-12-12', 'ru', now)).toBe('12 декабря')
    expect(formatReleaseDay('2027-01-15', 'ru', now)).toBe('15 января 2027')
    expect(formatReleaseDay('2027-01-15', 'en', now)).toBe('January 15, 2027')
  })
  test('сегодня и завтра — словами', () => {
    expect(formatReleaseDay('2026-11-13', 'ru', now)).toBe('сегодня')
    expect(formatReleaseDay('2026-11-14', 'ru', now)).toBe('завтра')
    expect(formatReleaseDay('2026-11-14', 'en', now)).toBe('tomorrow')
  })
})
