// Вшитые чёрные полосы и «клип-картинка»: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { clipGeometry, cropOf, FULL_CROP, PROBE_SIZE, sampleFrame, usefulRatio, verdict } from '../app/utils/clip-frame'

const S = PROBE_SIZE

/** Кадр S×S: внутри прямоугольника — «картинка» с шумом и сдвигом `shift`, снаружи — чёрное с шумом сжатия */
function frame(o: { top?: number, bottom?: number, left?: number, right?: number, shift?: number, dim?: number } = {}) {
  const { top = 0, bottom = 0, left = 0, right = 0, shift = 0, dim = 1 } = o
  const l = new Uint8Array(S * S)
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const inside = y >= top && y < S - bottom && x >= left && x < S - right
      l[y * S + x] = inside ? Math.round((60 + ((x * 7 + y * 3 + shift * 29) % 160)) * dim) : (x * 13 + y * 7) % 9
    }
  }
  return l
}

describe('полосы в кадре', () => {
  test('letterbox 2,39:1 внутри 16:9 — сверху и снизу по ~12,7 %', () => {
    // 61 строка из 480 → 16 из 128
    const s = sampleFrame(frame({ top: 16, bottom: 16 }), S, S)
    expect(s.top).toBe(16)
    expect(s.bottom).toBe(16)
    expect(s.left).toBe(0)
    const c = cropOf([s, s, s, s])
    expect(c.y).toBeCloseTo(16 / S + 0.006, 3)
    expect(c.h).toBeCloseTo(1 - 2 * (16 / S + 0.006), 3)
    expect(c.x).toBe(0)
    expect(usefulRatio(16 / 9, c)).toBeGreaterThan(2.2)
  })

  test('поля по бокам у 4:3', () => {
    const c = cropOf([sampleFrame(frame({ left: 16, right: 16 }), S, S)])
    expect(c.x).toBeGreaterThan(0.12)
    expect(c.y).toBe(0)
  })

  test('без полос — кадр целиком; тонкая тёмная кайма — не полоса', () => {
    expect(cropOf([sampleFrame(frame(), S, S)])).toEqual(FULL_CROP)
    expect(cropOf([sampleFrame(frame({ top: 1 }), S, S)])).toEqual(FULL_CROP)
  })

  test('тёмная сцена у края не расширяет полосу: берётся общее для всех кадров', () => {
    const night = sampleFrame(frame({ top: 50, bottom: 16 }), S, S)
    const day = sampleFrame(frame({ top: 16, bottom: 16 }), S, S)
    expect(cropOf([night, day]).y).toBeCloseTo(16 / S + 0.006, 3)
  })

  test('затемнение между сценами не считается', () => {
    const black = sampleFrame(frame({ dim: 0 }), S, S)
    expect(black.dark).toBeGreaterThan(0.9)
    expect(cropOf([black])).toEqual(FULL_CROP)
    expect(cropOf([black, sampleFrame(frame({ top: 16, bottom: 16 }), S, S)]).y).toBeGreaterThan(0.12)
  })

  test('почти весь кадр «полоса» — это не поля, показать как есть', () => {
    expect(cropOf([sampleFrame(frame({ top: 50, bottom: 40 }), S, S)])).toEqual(FULL_CROP)
  })
})

describe('решение по кадрам', () => {
  const moving = (n: number) => Array.from({ length: n }, (_, i) => sampleFrame(frame({ top: 16, bottom: 16, shift: i }), S, S))
  const still = (n: number) => Array.from({ length: n }, () => sampleFrame(frame({ top: 16, bottom: 16 }), S, S))

  test('движется — показать, как только кадров хватает', () => {
    expect(verdict(moving(2), 0.3).state).toBe('wait')
    const r = verdict(moving(4), 0.8)
    expect(r.state).toBe('show')
    expect(r.state === 'show' && r.crop.y).toBeGreaterThan(0.12)
    expect(r.state === 'show' && r.final).toBe(false)
    expect(verdict(moving(12), 3).state === 'show' && (verdict(moving(12), 3) as { final: boolean }).final).toBe(true)
  })

  test('картинка без движения — не клип, но не сразу', () => {
    expect(verdict(still(8), 2).state).toBe('wait')
    expect(verdict(still(20), 5).state).toBe('still')
  })

  test('сплошь тёмное — показать без обрезки', () => {
    const dark = Array.from({ length: 20 }, () => sampleFrame(frame({ dim: 0 }), S, S))
    expect(verdict(dark, 5)).toEqual({ state: 'show', crop: FULL_CROP, final: true })
  })
})

describe('геометрия', () => {
  const lb = { x: 0, y: 0.13, w: 1, h: 0.74 }

  test('cover по полезной части: полос в рамке нет', () => {
    const g = clipGeometry({ w: 400, h: 400 }, { w: 854, h: 480 }, lb, 'cover')
    expect(g.win).toEqual({ left: 0, top: 0, width: 400, height: 400 })
    // Верх полезной части — не ниже верха рамки
    expect(g.video.top + g.video.height * lb.y).toBeLessThanOrEqual(0.001)
    expect(g.video.top + g.video.height * (lb.y + lb.h)).toBeGreaterThanOrEqual(399.999)
  })

  test('contain: окно по полезной части, по центру', () => {
    const g = clipGeometry({ w: 1440, h: 900 }, { w: 854, h: 480 }, lb, 'contain')
    expect(g.win.width).toBe(1440)
    expect(g.win.height).toBeCloseTo(1440 / (854 / (480 * 0.74)), 3)
    expect(g.win.top).toBeCloseTo((900 - g.win.height) / 2, 3)
    expect(g.video.top + g.video.height * lb.y).toBeCloseTo(0, 3)
  })

  test('без полос — как object-fit', () => {
    const g = clipGeometry({ w: 1440, h: 900 }, { w: 1600, h: 900 }, FULL_CROP, 'cover')
    expect(g.video).toEqual({ left: -80, top: 0, width: 1600, height: 900 })
  })
})
