// Геометрия накладной полосы прокрутки (Windows): bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { dragScrollTop, MIN_THUMB, pageDelta, thumbGeometry, wheelPixels } from '../app/utils/scrollbars'

describe('накладная полоса прокрутки', () => {
  test('прокручивать нечего — ползунка нет', () => {
    expect(thumbGeometry({ scrollTop: 0, scrollHeight: 600, clientHeight: 600, track: 590 })).toBeNull()
    expect(thumbGeometry({ scrollTop: 0, scrollHeight: 1200, clientHeight: 600, track: 0 })).toBeNull()
  })

  test('размер ползунка — доля видимого, место — доля прокрученного', () => {
    const m = { scrollHeight: 2000, clientHeight: 500, track: 500 }
    expect(thumbGeometry({ ...m, scrollTop: 0 })).toEqual({ size: 125, offset: 0 })
    expect(thumbGeometry({ ...m, scrollTop: 1500 })).toEqual({ size: 125, offset: 375 })
    expect(thumbGeometry({ ...m, scrollTop: 750 })!.offset).toBeCloseTo(187.5)
    // Отскок за край (резиновая прокрутка) не выводит ползунок с дорожки
    expect(thumbGeometry({ ...m, scrollTop: -40 })!.offset).toBe(0)
    expect(thumbGeometry({ ...m, scrollTop: 1600 })!.offset).toBe(375)
  })

  test('в длинном списке ползунок не тоньше минимума', () => {
    // «Любимые» на 10 000 строк по 56 px
    const g = thumbGeometry({ scrollTop: 280_000, scrollHeight: 560_000, clientHeight: 800, track: 740 })!
    expect(g.size).toBe(MIN_THUMB)
    expect(g.offset).toBeCloseTo((280_000 / (560_000 - 800)) * (740 - MIN_THUMB))
    // Короткая дорожка — ползунок не длиннее неё
    expect(thumbGeometry({ scrollTop: 0, scrollHeight: 2000, clientHeight: 20, track: 16 })!.size).toBe(16)
  })

  test('перетаскивание: ход ползунка по дорожке — вся прокрутка', () => {
    const m = { scrollHeight: 2000, clientHeight: 500, track: 500 }
    expect(dragScrollTop(0, 375, m, 125)).toBe(1500)
    expect(dragScrollTop(0, 125, m, 125)).toBe(500)
    expect(dragScrollTop(600, -1000, m, 125)).toBe(0)
    expect(dragScrollTop(600, 1000, m, 125)).toBe(1500)
    expect(dragScrollTop(0, 50, { scrollHeight: 500, clientHeight: 500, track: 500 }, 500)).toBe(0)
  })

  test('щелчок по дорожке — страница к щелчку, по ползунку — ничего', () => {
    expect(pageDelta(10, 100, 50, 600)).toBe(-540)
    expect(pageDelta(400, 100, 50, 600)).toBe(540)
    expect(pageDelta(120, 100, 50, 600)).toBe(0)
  })

  test('колесо над полосой — в пиксели', () => {
    expect(wheelPixels(100, 0, 600)).toBe(100)
    expect(wheelPixels(3, 1, 600)).toBe(120)
    expect(wheelPixels(1, 2, 600)).toBe(600)
  })
})
