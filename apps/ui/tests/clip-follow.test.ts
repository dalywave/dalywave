import { describe, expect, test } from 'bun:test'
import { followTarget } from '../app/utils/clip-follow'

const at = (positionMs: number) => () => positionMs

describe('клип в такт с треком', () => {
  test('видео отстаёт от трека на вступление клипа', () => {
    // Клип начался за 12,3 с до трека (сценка) и чуть медленнее: секунда клипа — 1,02 с трека
    const f = { offsetMs: -12300, rate: 1.02, position: at(30000) }
    expect(followTarget(f, 240)).toBeCloseTo((30000 + 12300) / 1.02 / 1000, 5)
  })

  test('до начала клипа и после его конца кадров нет', () => {
    expect(followTarget({ offsetMs: 5000, rate: 1, position: at(2000) }, 200)).toBeNull()
    expect(followTarget({ offsetMs: 0, rate: 1, position: at(201000) }, 200)).toBeNull()
  })

  test('длина видео ещё неизвестна — хвост не отсекается', () => {
    expect(followTarget({ offsetMs: 0, rate: 1, position: at(500000) }, Number.NaN)).toBe(500)
  })
})
