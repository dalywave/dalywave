// Картинки «Итогов года» 1080×1920 для историй в соцсетях: у каждого стиля своя (lib/rewind/share-*.ts),
// здесь — общие кирпичи холста. Делится через системное «Поделиться» (сайт и iPhone), иначе скачивается.

export const CARD_W = 1080
export const CARD_H = 1920
export const CARD_FONT = '"Onest Variable", ui-sans-serif, system-ui, sans-serif'

/** Холст карточки; шрифт подгружается заранее, иначе первая картинка выйдет системным */
export async function newCard(): Promise<{ canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D }> {
  await Promise.all([500, 600, 700, 800].map(w => document.fonts?.load(`${w} 64px "Onest Variable"`).catch(() => null)))
  const canvas = document.createElement('canvas')
  canvas.width = CARD_W
  canvas.height = CARD_H
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('canvas')
  ctx.textBaseline = 'alphabetic'
  return { canvas, ctx }
}

export function loadCardImage(url: string | null | undefined): Promise<HTMLImageElement | null> {
  if (!url) return Promise.resolve(null)
  return new Promise((resolve) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.decoding = 'async'
    img.onload = () => resolve(img)
    img.onerror = () => resolve(null)
    img.src = url
  })
}

/** Строка, обрезанная многоточием по ширине */
export function fitText(ctx: CanvasRenderingContext2D, text: string, max: number): string {
  if (ctx.measureText(text).width <= max) return text
  let s = text
  while (s.length > 1 && ctx.measureText(`${s}…`).width > max) s = s.slice(0, -1)
  return `${s.trimEnd()}…`
}

/** Картинка «cover» в прямоугольник со скруглением (radius = половина — круг) */
export function drawCardImage(ctx: CanvasRenderingContext2D, img: HTMLImageElement, x: number, y: number, w: number, h: number, radius = 0, focusY = 0.5) {
  ctx.save()
  ctx.beginPath()
  ctx.roundRect(x, y, w, h, radius)
  ctx.clip()
  const s = Math.max(w / img.width, h / img.height)
  const dw = img.width * s
  const dh = img.height * s
  ctx.drawImage(img, x + (w - dw) / 2, y + (h - dh) * focusY, dw, dh)
  ctx.restore()
}

/** Волосок-разделитель */
export function cardHairline(ctx: CanvasRenderingContext2D, x1: number, y: number, x2: number, color: string, width = 2) {
  ctx.save()
  ctx.strokeStyle = color
  ctx.lineWidth = width
  ctx.beginPath()
  ctx.moveTo(x1, y)
  ctx.lineTo(x2, y)
  ctx.stroke()
  ctx.restore()
}

export async function cardPng(canvas: HTMLCanvasElement): Promise<Blob> {
  const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, 'image/png'))
  if (!blob) throw new Error('png')
  return blob
}

/** Системное «Поделиться» с файлом, где оно есть; иначе — скачать. `shared` — ушло в системное окно */
export async function shareOrDownload(blob: Blob, name: string, title: string, type = 'image/png'): Promise<'shared' | 'downloaded' | 'cancelled'> {
  const file = new File([blob], name, { type })
  if (typeof navigator.share === 'function' && navigator.canShare?.({ files: [file] })) {
    try {
      await navigator.share({ files: [file], title })
      return 'shared'
    }
    catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') return 'cancelled'
    }
  }
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
  return 'downloaded'
}
