// Картинки кнопок 144×144 (Stream Deck рисует их и на 72×72 — значки крупные, подписи короткие).
// Стиль «Полночь»: нейтральный тёмный фон, белый значок; включённое — белым с точкой, выключенное —
// приглушённым; цвет — только где он что-то значит (нравится, волна, настроение).
import type { GlyphName } from './glyphs.gen'
import { C, cornerMask, dataUri, fit, glyph, image, svg, text, textWidth, waveGlyph } from './svg'

export const KEY = 144

export type Tone = 'normal' | 'on' | 'off' | 'accent'

export interface IconFace {
  glyph: GlyphName | 'wave'
  /** Цвета волны (настроение); по умолчанию — градиент бренда */
  waveColors?: readonly string[]
  tone?: Tone
  /** Цвет значка вместо тона */
  color?: string
  label?: string
  /** Кольцо прогресса вокруг (таймер сна), 0…1 */
  ring?: number
  /** Крупный текст вместо значка (остаток таймера) */
  big?: string
}

export interface CoverFace {
  /** data URI обложки; нет — заглушка */
  cover: string | null
  /** 0…1 */
  progress: number
  playing: boolean
  /** Затемнение для пульса в такт: 0 — обложка как есть */
  shade?: number
  /** Подпись поверх (очередь: «Далее») */
  label?: string
}

const TONE: Record<Tone, string> = { normal: C.text1, on: C.text1, off: C.text3, accent: C.accent }

/** Подпись кнопки: кегль 21, длинная — мельче (до 16), и только потом многоточие */
export function shrink(label: string, width: number): [string, number] {
  for (const size of [21, 19, 17, 16]) {
    if (textWidth(label, size, 600) <= width) return [label, size]
  }
  return [fit(label, width, 16, 600), 16]
}

function ringPath(cx: number, cy: number, radius: number, p: number): string {
  const a = Math.min(0.9999, Math.max(0, p)) * Math.PI * 2
  const x = cx + radius * Math.sin(a)
  const y = cy - radius * Math.cos(a)
  const large = a > Math.PI ? 1 : 0
  return `M${cx} ${cy - radius}A${radius} ${radius} 0 ${large} 1 ${x.toFixed(2)} ${y.toFixed(2)}`
}

/** Значок с подписью — большинство кнопок */
export function iconKey(face: IconFace): string {
  const tone = face.tone ?? 'normal'
  const color = face.color ?? TONE[tone]
  const [label, labelSize] = face.label ? shrink(face.label, 132) : ['', 21]
  const parts: string[] = [`<rect width="${KEY}" height="${KEY}" fill="${C.bg}"/>`]
  if (face.ring !== undefined) {
    parts.push(`<circle cx="72" cy="72" r="58" fill="none" stroke="${C.line}" stroke-width="6"/>`)
    if (face.ring > 0) parts.push(`<path d="${ringPath(72, 72, 58, face.ring)}" fill="none" stroke="${C.text1}" stroke-width="6" stroke-linecap="round"/>`)
  }
  if (face.big) {
    parts.push(glyph(face.glyph === 'wave' ? 'moon-stars-bold' : face.glyph, 72, 44, 28, C.text2))
    parts.push(text(72, 92, face.big, 34, C.text1, 700))
  }
  else {
    const cy = label ? 60 : 72
    const size = label ? 60 : 72
    if (face.glyph === 'wave') parts.push(waveGlyph(72, cy, size * 1.25, face.waveColors ?? C.brand))
    else parts.push(glyph(face.glyph, 72, cy, size, color))
    if (label) parts.push(text(72, 122, label, labelSize, tone === 'off' ? C.text3 : C.text2, 600))
    else if (tone === 'on') parts.push(`<circle cx="72" cy="124" r="5" fill="${C.text1}"/>`)
  }
  return dataUri(svg(KEY, KEY, parts.join('')))
}

/** «Сейчас играет»: обложка на всю кнопку, тонкая полоса прогресса снизу, на паузе — значок «играть» */
export function coverKey(face: CoverFace): string {
  const parts: string[] = [`<rect width="${KEY}" height="${KEY}" fill="${C.bg}"/>`]
  if (face.cover) parts.push(image(0, 0, KEY, KEY, face.cover))
  else parts.push(glyph('music-notes-bold', 72, 66, 64, C.text4))
  const shade = (face.playing ? 0 : 0.45) + (face.shade ?? 0)
  if (shade > 0) parts.push(`<rect width="${KEY}" height="${KEY}" fill="#000" fill-opacity="${Math.min(0.8, shade).toFixed(2)}"/>`)
  if (!face.playing) parts.push(glyph('play-fill', 72, 66, 52, C.text1))
  if (face.label) {
    parts.push(`<rect y="98" width="${KEY}" height="46" fill="#000" fill-opacity="0.55"/>`)
    const [l, size] = shrink(face.label, 132)
    parts.push(text(72, 128, l, size, C.text1, 600))
  }
  // Полоса прогресса: дорожка и заполнение, 6 px у нижнего края
  const p = Math.min(1, Math.max(0, face.progress))
  parts.push(`<rect y="138" width="${KEY}" height="6" fill="#000" fill-opacity="0.55"/>`)
  if (p > 0) parts.push(`<rect y="138" width="${(KEY * p).toFixed(1)}" height="6" fill="${C.text1}"/>`)
  return dataUri(svg(KEY, KEY, parts.join('')))
}

/** Обложка в кнопке без прогресса (элемент коллекции, трек очереди) */
export function artKey(cover: string | null, fallback: GlyphName, label?: string): string {
  const parts: string[] = [`<rect width="${KEY}" height="${KEY}" fill="${C.bg}"/>`]
  if (cover) parts.push(image(0, 0, KEY, KEY, cover))
  else parts.push(glyph(fallback, 72, label ? 60 : 72, 64, C.text3))
  if (label) {
    if (cover) parts.push(`<rect y="98" width="${KEY}" height="46" fill="#000" fill-opacity="0.6"/>`)
    const [l, size] = shrink(label, 132)
    parts.push(text(72, 128, l, size, C.text1, 600))
  }
  return dataUri(svg(KEY, KEY, parts.join('')))
}

/** Обложка 0…1 с закруглёнными углами — для полосы и экрана Neo */
export function roundedArt(cover: string | null, size: number, fallback: GlyphName = 'music-notes-bold', bg: string = C.strip, waveColors?: readonly string[]): string {
  const radius = Math.round(size * 0.12)
  const parts: string[] = []
  if (cover) {
    parts.push(image(0, 0, size, size, cover))
  }
  else {
    parts.push(`<rect width="${size}" height="${size}" fill="#1b1f27"/>`)
    if (waveColors) parts.push(waveGlyph(size / 2, size / 2, size * 0.8, waveColors, 'wa'))
    else parts.push(glyph(fallback, size / 2, size / 2, size * 0.5, C.text3))
  }
  parts.push(cornerMask(0, 0, size, size, radius, bg))
  return dataUri(svg(size, size, parts.join('')))
}

/** Значок для полосы: на прозрачном фоне */
export function stripGlyph(name: GlyphName, size: number, color: string = C.text1, scale = 0.86): string {
  return dataUri(svg(size, size, glyph(name, size / 2, size / 2, size * scale, color)))
}

/** Кнопка без связи: «подключите» или «нет связи» */
export function statusKey(kind: 'connect' | 'offline' | 'noDevice', label: string): string {
  const g: GlyphName = kind === 'connect' ? 'plugs-bold' : kind === 'offline' ? 'cloud-slash-bold' : 'devices-bold'
  return iconKey({ glyph: g, tone: 'off', label })
}

/** Сплошная заливка для пульса полосы */
export function pulseFill(w: number, h: number, color = '#ffffff'): string {
  return dataUri(svg(w, h, `<rect width="${w}" height="${h}" fill="${color}"/>`))
}
