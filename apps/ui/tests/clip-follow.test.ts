import { describe, expect, test } from 'bun:test'
import { followTarget } from '../app/utils/clip-follow'

const at = (positionMs: number) => () => positionMs

describe('клип в такт с треком', () => {
  test('видео отстаёт от трека на вступление клипа', () => {
    // Клип начался за 30 с до трека (сценка), секунда клипа — 1,02 с трека
    const f = { offsetMs: -30_000, rate: 1.02, position: at(30_000) }
    expect(followTarget(f, 260)!.at).toBeCloseTo((30_000 + 30_000) / 1.02 / 1000, 5)
    expect(followTarget(f, 260)!.rate).toBe(1.02)
  })

  test('до начала клипа и после его конца кадров нет', () => {
    expect(followTarget({ offsetMs: 5000, rate: 1, position: at(2000) }, 200)).toBeNull()
    expect(followTarget({ offsetMs: 0, rate: 1, position: at(201_000) }, 200)).toBeNull()
  })

  test('длина видео ещё неизвестна — хвост не отсекается', () => {
    expect(followTarget({ offsetMs: 0, rate: 1, position: at(500_000) }, Number.NaN)!.at).toBe(500)
  })

  test('сценка посреди клипа: после неё — второй кусок со своим сдвигом', () => {
    // «Бриллианты» ЛСП: до 132 с клипа сдвиг −8,3 с, после сценки −13,7 с
    const segments = [
      { videoStartMs: 0, videoEndMs: 135_000, offsetMs: -8300, rate: 1 },
      { videoStartMs: 135_000, videoEndMs: 172_000, offsetMs: -13_700, rate: 1 },
    ]
    const f = (pos: number) => followTarget({ offsetMs: -8300, rate: 1, segments, position: at(pos) }, 172)
    expect(f(100_000)!.at).toBeCloseTo(108.3, 5)
    expect(f(140_000)!.at).toBeCloseTo(153.7, 5)
    // У стыка: сценку клипа (135–140 с видео) трек перескакивает
    expect(f(126_000)!.at).toBeCloseTo(134.3, 5)
    expect(f(127_000)!.at).toBeCloseTo(140.7, 5)
    // Конец трека дальше конца клипа — кадров нет
    expect(f(160_000)).toBeNull()
  })
})
