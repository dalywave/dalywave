// Видео-история и «Скоро выйдет»: строка текста в момент кадра, движение волны от звука, день выхода
import { describe, expect, test } from 'bun:test'
import { aacDescription, aacLcConfig } from '../app/lib/story/aac'
import { PREVIEW_MS, channelPeaks, clampStart, dragStart, keyStart, storyLines } from '../app/lib/story/moment'
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

describe('описание AAC для MP4', () => {
  // Что отдаёт Safari 27 в decoderConfig.description: весь ES_Descriptor из esds, а не AudioSpecificConfig
  const safari44 = [3, 128, 128, 128, 34, 0, 0, 0, 4, 128, 128, 128, 20, 64, 20, 0, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 128, 128, 128, 2, 18, 16, 6, 128, 128, 128, 1, 2]
  const safari48 = [3, 128, 128, 128, 34, 0, 0, 0, 4, 128, 128, 128, 20, 64, 20, 0, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 128, 128, 128, 2, 17, 144, 6, 128, 128, 128, 1, 2]
  test('из ES_Descriptor Safari — сам AudioSpecificConfig', () => {
    expect([...aacDescription(new Uint8Array(safari44).buffer, 44_100, 2)]).toEqual([0x12, 0x10])
    expect([...aacDescription(new Uint8Array(safari48), 48_000, 2)]).toEqual([0x11, 0x90])
  })
  test('правильное описание (Chrome) — как есть', () => {
    expect([...aacDescription(new Uint8Array([0x12, 0x10]), 44_100, 2)]).toEqual([0x12, 0x10])
  })
  test('без описания или испорченное — AAC-LC по частоте и каналам', () => {
    expect([...aacDescription(undefined, 44_100, 2)]).toEqual([0x12, 0x10])
    expect([...aacDescription(new Uint8Array([3, 5, 0]), 44_100, 2)]).toEqual([0x12, 0x10])
    expect([...aacLcConfig(48_000, 2)]).toEqual([0x11, 0x90])
    expect([...aacLcConfig(22_050, 1)]).toEqual([0x13, 0x88])
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

describe('момент истории', () => {
  test('окно целиком в звуке', () => {
    expect(clampStart(200_000, 15_000, 180_000)).toBe(165_000)
    expect(clampStart(-3000, 10_000, 180_000)).toBe(0)
    expect(clampStart(5000, 30_000, PREVIEW_MS)).toBe(0)
  })
  test('стрелки: шаг 1 с, с Shift — 5 с, по сетке секунд; Home и End', () => {
    expect(keyStart('ArrowRight', false, 60_000, 15_000, 180_000)).toBe(61_000)
    expect(keyStart('ArrowLeft', true, 60_000, 15_000, 180_000)).toBe(55_000)
    expect(keyStart('ArrowRight', false, 60_500, 15_000, 180_000)).toBe(62_000)
    expect(keyStart('Home', false, 60_000, 15_000, 180_000)).toBe(0)
    expect(keyStart('End', false, 60_000, 15_000, 180_000)).toBe(165_000)
    expect(keyStart('ArrowRight', true, 163_000, 15_000, 180_000)).toBe(165_000)
    expect(keyStart('Enter', false, 60_000, 15_000, 180_000)).toBeNull()
  })
  test('перетаскивание: окно идёт за пальцем с того места, где взяли', () => {
    // Взяли за середину окна, палец на середине трека
    expect(dragStart(0.5, 0.5, 20_000, 200_000)).toBe(90_000)
    // У края — не дальше конца
    expect(dragStart(0.99, 0, 20_000, 200_000)).toBe(180_000)
  })
  test('текст — тот, что звучит в выбранном куске', () => {
    const lines = [{ tMs: 1000, text: 'раз' }, { tMs: 9000, text: 'два' }, { tMs: 14_000, text: 'три' }, { tMs: 40_000, text: 'четыре' }]
    expect(storyLines(lines, 10_000, 10_000)).toEqual([{ atMs: 0, text: 'два' }, { atMs: 4000, text: 'три' }])
    expect(storyLines(lines, 0, 30_000).map(l => l.text)).toEqual(['раз', 'два', 'три'])
  })
  test('форма волны из звука: громкое — выше, тишина — не ноль', () => {
    const ch = new Float32Array(4000)
    ch.fill(0.5, 2000)
    const p = channelPeaks([ch], 4)
    expect(p).toHaveLength(4)
    expect(p[3]).toBe(1)
    expect(p[0]).toBe(0.08)
  })
})
