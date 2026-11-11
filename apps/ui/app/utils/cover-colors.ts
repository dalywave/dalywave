// Цвета обложки для свечения фона и оттенка волны. Если сервер уже прислал палитру
// (AlbumSummary.palette), берём её; иначе считаем по уменьшенной картинке.

export interface CoverColors {
  vivid: string
  second: string
  deep: string
}

const cache = new Map<string, Promise<CoverColors | null>>()

const clamp = (v: number) => Math.min(1, Math.max(0, v))
const toHex = (r: number, g: number, b: number) => '#' + [r, g, b].map(v => Math.round(clamp(v) * 255).toString(16).padStart(2, '0')).join('')

export function colorsFromPalette(palette: string[] | null | undefined): CoverColors | null {
  if (!palette?.length) return null
  return { vivid: palette[0]!, second: palette[1] ?? palette[0]!, deep: palette[palette.length - 1]! }
}

export function coverColors(url: string | null | undefined): Promise<CoverColors | null> {
  if (!url || typeof document === 'undefined') return Promise.resolve(null)
  const hit = cache.get(url)
  if (hit) return hit
  const job = new Promise<CoverColors | null>((resolve) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.decoding = 'async'
    img.onload = () => {
      try {
        const size = 28
        const canvas = document.createElement('canvas')
        canvas.width = canvas.height = size
        const ctx = canvas.getContext('2d', { willReadFrequently: true })
        if (!ctx) return resolve(null)
        ctx.drawImage(img, 0, 0, size, size)
        const d = ctx.getImageData(0, 0, size, size).data
        const buckets = Array.from({ length: 12 }, () => ({ w: 0, r: 0, g: 0, b: 0 }))
        let dr = 0, dg = 0, db = 0, dn = 0
        for (let i = 0; i < d.length; i += 4) {
          const r = d[i]! / 255
          const g = d[i + 1]! / 255
          const b = d[i + 2]! / 255
          const mx = Math.max(r, g, b)
          const mn = Math.min(r, g, b)
          const s = mx ? (mx - mn) / mx : 0
          let h = 0
          if (mx !== mn) {
            if (mx === r) h = ((g - b) / (mx - mn) + 6) % 6
            else if (mx === g) h = (b - r) / (mx - mn) + 2
            else h = (r - g) / (mx - mn) + 4
          }
          const w = s * s * Math.sqrt(mx) + 0.02
          const k = buckets[Math.floor((h / 6) * 12) % 12]!
          k.w += w
          k.r += r * w
          k.g += g * w
          k.b += b * w
          if (mx < 0.55) {
            dr += r
            dg += g
            db += b
            dn++
          }
        }
        const sorted = buckets.filter(b => b.w > 0).sort((a, b) => b.w - a.w)
        const pick = (b: { w: number, r: number, g: number, b: number }) => toHex(b.r / b.w, b.g / b.w, b.b / b.w)
        const vivid = sorted[0] ? pick(sorted[0]) : '#3a5070'
        resolve({
          vivid,
          second: sorted[1] ? pick(sorted[1]) : vivid,
          deep: dn ? toHex(dr / dn, dg / dn, db / dn) : '#101820',
        })
      }
      catch {
        // Картинка с чужого адреса без CORS: цвета не прочитать
        resolve(null)
      }
    }
    img.onerror = () => resolve(null)
    img.src = url
  })
  cache.set(url, job)
  return job
}
