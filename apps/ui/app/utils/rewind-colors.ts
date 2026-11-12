// Цвета историй «Итогов года» — из обложек: плоское поле, второй цвет для фигур и цвет текста,
// который на этом поле читается. Без градиентов: цвет приходит из музыки, а не из интерфейса.

export interface SlideColors {
  bg: string
  /** Второй цвет обложки для фигур и полос, контрастный к полю */
  accent: string
  /** Текст: чёрный или белый — что читается на поле */
  ink: string
  /** Второстепенный текст */
  soft: string
}

type Hsl = [number, number, number]

function hexToRgb(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim())
  if (!m) return null
  const n = Number.parseInt(m[1]!, 16)
  return [(n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255]
}

function rgbToHsl([r, g, b]: [number, number, number]): Hsl {
  const mx = Math.max(r, g, b)
  const mn = Math.min(r, g, b)
  const l = (mx + mn) / 2
  if (mx === mn) return [0, 0, l]
  const d = mx - mn
  const s = l > 0.5 ? d / (2 - mx - mn) : d / (mx + mn)
  let h = mx === r ? (g - b) / d + (g < b ? 6 : 0) : mx === g ? (b - r) / d + 2 : (r - g) / d + 4
  h /= 6
  return [h, s, l]
}

function hslToHex([h, s, l]: Hsl): string {
  const f = (n: number) => {
    const k = (n + h * 12) % 12
    const a = s * Math.min(l, 1 - l)
    const c = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))
    return Math.round(Math.min(1, Math.max(0, c)) * 255).toString(16).padStart(2, '0')
  }
  return `#${f(0)}${f(8)}${f(4)}`
}

/** Относительная яркость WCAG */
function luminance(hex: string): number {
  const rgb = hexToRgb(hex)
  if (!rgb) return 0
  const lin = rgb.map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
  return 0.2126 * lin[0]! + 0.7152 * lin[1]! + 0.0722 * lin[2]!
}

export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p)
  return (x! + 0.05) / (y! + 0.05)
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v))

const DARK_INK = '#0b0b10'
const FALLBACK: SlideColors = { bg: '#16161c', accent: '#ff7eb3', ink: '#ffffff', soft: 'rgba(255, 255, 255, 0.72)' }

/**
 * Палитра обложки (самый заметный цвет первым) → цвета слайда. Поле — самый живой цвет обложки,
 * приведённый к насыщенной средней яркости; серые обложки остаются серыми, но глубокими.
 * `shift` — какой по счёту живой цвет взять, чтобы соседние слайды с одной обложкой различались.
 */
export function slideColors(palette: (string | null | undefined)[] | null | undefined, shift = 0): SlideColors {
  const hsl = (palette ?? []).map(c => (c ? hexToRgb(c) : null)).filter((c): c is [number, number, number] => !!c).map(rgbToHsl)
  if (!hsl.length) return FALLBACK
  const ranked = hsl
    .map((c, i) => ({ c, score: c[1] * (1 - Math.abs(c[2] - 0.5)) + (i === 0 ? 0.05 : 0) }))
    .sort((a, b) => b.score - a.score)
  const pick = ranked[Math.min(shift, ranked.length - 1)]!.c
  const grey = pick[1] < 0.14
  const bg = hslToHex(grey ? [pick[0], pick[1], clamp(pick[2], 0.16, 0.24)] : [pick[0], clamp(pick[1], 0.45, 0.88), clamp(pick[2], 0.3, 0.56)])
  const ink = contrast(bg, '#ffffff') >= contrast(bg, DARK_INK) ? '#ffffff' : DARK_INK
  const light = ink === DARK_INK
  // Второй цвет — самый далёкий по тону от поля, доведённый до контраста с ним
  const bgHsl = rgbToHsl(hexToRgb(bg)!)
  const far = hsl
    .filter(c => c !== pick)
    .map(c => ({ c, d: Math.min(Math.abs(c[0] - bgHsl[0]), 1 - Math.abs(c[0] - bgHsl[0])) * (c[1] + 0.2) + Math.abs(c[2] - bgHsl[2]) * 0.4 }))
    .sort((a, b) => b.d - a.d)[0]?.c ?? [bgHsl[0], bgHsl[1], bgHsl[2]]
  let accentHsl: Hsl = [far[0], Math.max(far[1], grey ? far[1] : 0.5), light ? 0.22 : 0.74]
  if (contrast(hslToHex(accentHsl), bg) < 2.2) accentHsl = [bgHsl[0], Math.min(1, bgHsl[1] + 0.1), light ? 0.14 : 0.86]
  return {
    bg,
    accent: hslToHex(accentHsl),
    ink,
    soft: light ? 'rgba(11, 11, 16, 0.7)' : 'rgba(255, 255, 255, 0.74)',
  }
}
