// Что на самом деле в кадре клипа: вшитые чёрные полосы (letterbox 2,39:1 внутри 16:9, вертикальное
// видео с полями) и «клип», который на деле картинка (обложка на размытом фоне — загрузки
// «audio»-каналов). Смотрим несколько первых кадров уменьшенными до 128×128 — один раз на клип,
// не каждый кадр. Чистые функции — для bun test; снятие кадров с <video> — в `probeVideo`

/** Полезная часть кадра — доли ширины и высоты видео */
export interface ClipCrop { x: number, y: number, w: number, h: number }

export const FULL_CROP: ClipCrop = Object.freeze({ x: 0, y: 0, w: 1, h: 1 })

/** Сторона уменьшенного кадра: 128 строк — полоса в 61 строку из 480 ловится с точностью до пикселя */
export const PROBE_SIZE = 128
/** Яркость, ниже которой пиксель — чёрный (сжатие даёт шум у чёрного 0–20) */
const BLACK = 26
/** Строка полосы: почти вся чёрная… */
const ROW_BLACK_SHARE = 0.97
/** …и в среднем совсем тёмная */
const ROW_MEAN = 14
/** Кадр почти весь чёрный (затемнение между сценами) — о полосах ничего не говорит */
const DARK_FRAME = 0.9
/** Полоса тоньше — не полоса, а край кадра */
const MIN_BAR = 0.02
/** Срезаем чуть больше найденного: на границе полосы сжатие оставляет тёмную кайму */
const EDGE = 0.006
/** Сетка для сравнения кадров между собой */
const THUMB = 16
/** Среднее отличие кадров (0–255), с которого картинка «движется» */
const MOVE = 2.5

export interface FrameSample {
  /** Полосы в строках/столбцах уменьшенного кадра */
  top: number
  bottom: number
  left: number
  right: number
  /** Доля чёрных пикселей */
  dark: number
  /** Яркость 16×16 — сравнить с другими кадрами */
  thumb: Float32Array
}

/** Яркость (0–255) из RGBA */
export function lumaOf(rgba: Uint8ClampedArray | Uint8Array): Uint8Array {
  const n = rgba.length >> 2
  const out = new Uint8Array(n)
  for (let i = 0; i < n; i++) {
    const o = i << 2
    out[i] = (rgba[o]! * 54 + rgba[o + 1]! * 183 + rgba[o + 2]! * 19) >> 8
  }
  return out
}

function lineBlack(luma: Uint8Array, size: number, from: number, step: number): boolean {
  let black = 0
  let sum = 0
  for (let i = 0, p = from; i < size; i++, p += step) {
    const v = luma[p]!
    sum += v
    if (v <= BLACK) black++
  }
  return black >= size * ROW_BLACK_SHARE && sum / size <= ROW_MEAN
}

/** Разобрать один уменьшенный кадр (яркость, `w`×`h`) */
export function sampleFrame(luma: Uint8Array, w: number, h: number): FrameSample {
  const row = (y: number) => lineBlack(luma, w, y * w, 1)
  const col = (x: number) => lineBlack(luma, h, x, w)
  let top = 0
  while (top < h && row(top)) top++
  let bottom = 0
  while (bottom < h - top && row(h - 1 - bottom)) bottom++
  let left = 0
  while (left < w && col(left)) left++
  let right = 0
  while (right < w - left && col(w - 1 - right)) right++

  let dark = 0
  const thumb = new Float32Array(THUMB * THUMB)
  const cw = w / THUMB
  const ch = h / THUMB
  for (let y = 0; y < h; y++) {
    const ty = Math.min(THUMB - 1, Math.floor(y / ch))
    for (let x = 0; x < w; x++) {
      const v = luma[y * w + x]!
      if (v <= BLACK) dark++
      thumb[ty * THUMB + Math.min(THUMB - 1, Math.floor(x / cw))]! += v
    }
  }
  const per = cw * ch
  for (let i = 0; i < thumb.length; i++) thumb[i]! /= per
  return { top, bottom, left, right, dark: dark / (w * h), thumb }
}

function diff(a: Float32Array, b: Float32Array): number {
  let s = 0
  for (let i = 0; i < a.length; i++) s += Math.abs(a[i]! - b[i]!)
  return s / a.length
}

/** Полезная часть по полосам, общим для всех светлых кадров (`size` — сторона уменьшенного кадра) */
export function cropOf(samples: FrameSample[], size = PROBE_SIZE): ClipCrop {
  const lit = samples.filter(s => s.dark < DARK_FRAME)
  if (!lit.length) return FULL_CROP
  // Полоса — только то, что чёрное во всех кадрах: тёмная сцена у края её не расширит
  const bar = (k: 'top' | 'bottom' | 'left' | 'right') => {
    const v = Math.min(...lit.map(s => s[k])) / size
    return v < MIN_BAR ? 0 : v + EDGE
  }
  let top = bar('top')
  let bottom = bar('bottom')
  let left = bar('left')
  let right = bar('right')
  // Срезать больше — это уже не поля, а тёмный кадр: лучше показать как есть
  if (top + bottom > 0.6) top = bottom = 0
  if (left + right > 0.75) left = right = 0
  if (!top && !bottom && !left && !right) return FULL_CROP
  return { x: left, y: top, w: 1 - left - right, h: 1 - top - bottom }
}

export type ProbeVerdict =
  /** Мало кадров — ещё смотрим */
  | { state: 'wait' }
  /** Показывать с такой полезной частью; `final` — больше не уточнится */
  | { state: 'show', crop: ClipCrop, final: boolean }
  /** Это не клип, а картинка: кадры не меняются */
  | { state: 'still' }

/** Кадров, после которых полосы больше не уточняются */
const ENOUGH = 12
/** Сколько секунд смотреть до решения «это картинка» */
export const STILL_AFTER = 5
/** Клип целиком в такт с треком: заставка или титры в начале могут стоять дольше — ждём больше */
export const STILL_AFTER_FULL = 15

/**
 * Решение по кадрам: `played` — сколько секунд видео прошло от первого кадра. Показать — как только
 * картинка сдвинулась и светлых кадров хватает, чтобы найти полосы; картинка без движения
 * `STILL_AFTER` секунд — не клип. Кадры сплошь тёмные — показать без обрезки
 */
export function verdict(samples: FrameSample[], played: number, stillAfter = STILL_AFTER): ProbeVerdict {
  if (!samples.length) return { state: 'wait' }
  const lit = samples.filter(s => s.dark < DARK_FRAME)
  const first = samples[0]!.thumb
  const moved = samples.some(s => diff(s.thumb, first) >= MOVE)
  if (moved && lit.length >= 4 && played >= 0.75) {
    return { state: 'show', crop: cropOf(samples), final: samples.length >= ENOUGH }
  }
  if (played >= stillAfter) {
    if (!moved && lit.length) return { state: 'still' }
    return { state: 'show', crop: cropOf(samples), final: true }
  }
  return { state: 'wait' }
}

/** Пропорции полезной части (ширина / высота) у видео `ratio` */
export function usefulRatio(ratio: number, crop: ClipCrop): number {
  return ratio * crop.w / crop.h
}

export interface Box { left: number, top: number, width: number, height: number }

/**
 * Где окно с полезной частью в рамке `box` и где в нём само видео (`video` — его размер в пикселях).
 * `cover` — полезная часть заполняет рамку (лишнее обрезается), `contain` — целиком, окно меньше рамки
 */
export function clipGeometry(box: { w: number, h: number }, video: { w: number, h: number }, crop: ClipCrop, fit: 'cover' | 'contain'): { win: Box, video: Box } {
  const cw = video.w * crop.w
  const ch = video.h * crop.h
  const s = fit === 'cover' ? Math.max(box.w / cw, box.h / ch) : Math.min(box.w / cw, box.h / ch)
  const ww = Math.min(box.w, cw * s)
  const wh = Math.min(box.h, ch * s)
  const vw = video.w * s
  const vh = video.h * s
  return {
    win: { left: (box.w - ww) / 2, top: (box.h - wh) / 2, width: ww, height: wh },
    video: { left: ww / 2 - vw * (crop.x + crop.w / 2), top: wh / 2 - vh * (crop.y + crop.h / 2), width: vw, height: vh },
  }
}

/** Разобранные клипы — до перезагрузки: второй показ сразу с верными рамками (ключ — ссылка без подписи) */
const known = new Map<string, ProbeVerdict & { state: 'show' | 'still' }>()
export const probeKey = (url: string) => url.split('?')[0]!
export function probed(url: string | null | undefined) {
  return url ? known.get(probeKey(url)) : undefined
}

/**
 * Смотреть кадры играющего видео 4 раза в секунду, пока не будет окончательного решения.
 * Видео с чужого сервера без CORS читать нельзя — тогда сразу «показывать как есть».
 * Возвращает остановку
 */
export function probeVideo(v: HTMLVideoElement, url: string, onVerdict: (r: ProbeVerdict & { state: 'show' | 'still' }) => void, stillAfter = STILL_AFTER): () => void {
  const done = known.get(probeKey(url))
  if (done) {
    onVerdict(done)
    if (done.state === 'still' || done.final) return () => {}
  }
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = PROBE_SIZE
  const ctx = canvas.getContext('2d', { willReadFrequently: true })
  const samples: FrameSample[] = []
  let played = 0
  let last = -1
  let shown = false
  const finish = (r: ProbeVerdict & { state: 'show' | 'still' }) => {
    known.set(probeKey(url), r)
    onVerdict(r)
  }
  const tick = () => {
    if (v.readyState < 2 || v.paused) return
    const t = v.currentTime
    if (t === last) return
    // Круг закончился — время пошло с нуля: считаем один шаг
    if (last >= 0) played += t > last ? Math.min(1, t - last) : 0.25
    last = t
    let luma: Uint8Array
    try {
      ctx!.drawImage(v, 0, 0, PROBE_SIZE, PROBE_SIZE)
      luma = lumaOf(ctx!.getImageData(0, 0, PROBE_SIZE, PROBE_SIZE).data)
    }
    catch {
      // Кадр закрыт (CORS) или не готов: полос не узнать — показываем как есть
      stop()
      finish({ state: 'show', crop: FULL_CROP, final: true })
      return
    }
    samples.push(sampleFrame(luma, PROBE_SIZE, PROBE_SIZE))
    const r = verdict(samples, played, stillAfter)
    if (r.state === 'wait') return
    if (r.state === 'still' || r.final) stop()
    // Уточнение без изменений не дёргает раскладку
    const prev = known.get(probeKey(url))
    if (shown && r.state === 'show' && prev?.state === 'show' && sameCrop(prev.crop, r.crop) && !r.final) return
    shown = true
    finish(r)
  }
  if (!ctx) {
    finish({ state: 'show', crop: FULL_CROP, final: true })
    return () => {}
  }
  const timer = setInterval(tick, 250)
  function stop() {
    clearInterval(timer)
  }
  return stop
}

export function sameCrop(a: ClipCrop, b: ClipCrop) {
  return Math.abs(a.x - b.x) < 0.004 && Math.abs(a.y - b.y) < 0.004 && Math.abs(a.w - b.w) < 0.004 && Math.abs(a.h - b.h) < 0.004
}
