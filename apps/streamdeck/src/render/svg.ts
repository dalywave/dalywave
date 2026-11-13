// Общие кирпичи картинок: цвета «Полночи», значки, текст с обрезкой и упаковка SVG в data URI.
// Stream Deck рисует SVG движком Qt (SVG Tiny): без масок, фильтров и clipPath — скругления
// обложек делаются рамкой цвета фона, а длинный текст режется здесь, по оценке ширины.
import { GLYPHS, type GlyphName } from './glyphs.gen'

export const C = {
  /** Фон кнопки: --ink-2…3 */
  bg: '#111318',
  /** Фон сенсорной полосы и экрана Neo — как у самого устройства */
  strip: '#000000',
  text1: '#f3f5f9',
  /** --text-2, сведённый на фон */
  text2: '#a5a9b1',
  text3: '#8b8f97',
  text4: '#4e525a',
  line: '#2a2e36',
  /** Акцент «Любое настроение» */
  accent: '#ff7eb3',
  /** Цвета значка приложения (волна) */
  brand: ['#8e6bff', '#d45bef', '#ff6f95', '#ffa25c'] as const,
}

export const FONT = 'Onest, \'SF Pro Text\', \'Segoe UI\', \'Helvetica Neue\', Arial, sans-serif'

/** Волна из значка приложения, поле 100×100 */
export const WAVE_PATH = 'M16 31C18.83 37.33 27.33 67.33 33 69C38.67 70.67 44.33 41 50 41C55.67 41 61.33 70.67 67 69C72.67 67.33 81.17 37.33 84 31'

export function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

/** Ширина знака в долях кегля — грубо, но достаточно, чтобы не вылезти за край */
function charWidth(ch: string): number {
  if (' '.includes(ch)) return 0.28
  if ('iljI.,:;\'|!`'.includes(ch)) return 0.3
  if ('ftr()[]{}-–'.includes(ch)) return 0.4
  if ('mwMWШЩЖМЮЫшщжмюы'.includes(ch)) return 0.86
  if (/[0-9]/.test(ch)) return 0.58
  if (/[A-ZА-ЯЁ]/.test(ch)) return 0.68
  return 0.56
}

export function textWidth(text: string, size: number, weight = 400): number {
  const bold = weight >= 600 ? 1.06 : 1
  let w = 0
  for (const ch of text) w += charWidth(ch)
  return w * size * bold
}

/** Обрезает текст многоточием под ширину */
export function fit(text: string, maxWidth: number, size: number, weight = 400): string {
  if (textWidth(text, size, weight) <= maxWidth) return text
  const chars = [...text]
  while (chars.length > 1 && textWidth(`${chars.join('').trimEnd()}…`, size, weight) > maxWidth) chars.pop()
  return `${chars.join('').trimEnd()}…`
}

/** Значок Phosphor (поле 256) в квадрате `size` с центром в (cx, cy) */
export function glyph(name: GlyphName, cx: number, cy: number, size: number, color: string = C.text1, opacity = 1): string {
  const k = size / 256
  const body = GLYPHS[name].replace(/#fff/g, color)
  const op = opacity < 1 ? ` opacity="${opacity}"` : ''
  return `<g transform="translate(${r(cx - size / 2)} ${r(cy - size / 2)}) scale(${r(k, 4)})"${op}>${body}</g>`
}

/** Волна приложения: градиент бренда или один цвет */
export function waveGlyph(cx: number, cy: number, size: number, colors: readonly string[] = C.brand, id = 'wg'): string {
  const k = size / 100
  const stops = colors.map((c, i) => `<stop offset="${r(i / Math.max(1, colors.length - 1), 3)}" stop-color="${c}"/>`).join('')
  const stroke = colors.length > 1 ? `url(#${id})` : colors[0]
  const defs = colors.length > 1 ? `<defs><linearGradient id="${id}" x1="0" y1="0" x2="1" y2="1">${stops}</linearGradient></defs>` : ''
  return `${defs}<g transform="translate(${r(cx - size / 2)} ${r(cy - size / 2)}) scale(${r(k, 4)})"><path d="${WAVE_PATH}" fill="none" stroke="${stroke}" stroke-width="11.5" stroke-linecap="round" stroke-linejoin="round"/></g>`
}

export function text(x: number, y: number, value: string, size: number, color: string, weight = 600, anchor: 'start' | 'middle' | 'end' = 'middle'): string {
  return `<text x="${r(x)}" y="${r(y)}" font-family="${FONT}" font-size="${size}" font-weight="${weight}" fill="${color}" text-anchor="${anchor}">${esc(value)}</text>`
}

/** Рамка со скруглёнными углами изнутри: закрывает углы картинки цветом фона */
export function cornerMask(x: number, y: number, w: number, h: number, radius: number, color: string): string {
  const R = radius
  const outer = `M${x - 1} ${y - 1}H${x + w + 1}V${y + h + 1}H${x - 1}Z`
  const inner = `M${x + R} ${y}H${x + w - R}A${R} ${R} 0 0 1 ${x + w} ${y + R}V${y + h - R}A${R} ${R} 0 0 1 ${x + w - R} ${y + h}H${x + R}A${R} ${R} 0 0 1 ${x} ${y + h - R}V${y + R}A${R} ${R} 0 0 1 ${x + R} ${y}Z`
  return `<path d="${outer}${inner}" fill="${color}" fill-rule="evenodd"/>`
}

export function image(x: number, y: number, w: number, h: number, href: string): string {
  return `<image x="${x}" y="${y}" width="${w}" height="${h}" preserveAspectRatio="xMidYMid slice" xlink:href="${href}" href="${href}"/>`
}

export function svg(w: number, h: number, body: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`
}

export function dataUri(markup: string): string {
  return `data:image/svg+xml;base64,${Buffer.from(markup, 'utf8').toString('base64')}`
}

function r(n: number, digits = 2): number {
  const k = 10 ** digits
  return Math.round(n * k) / k
}
