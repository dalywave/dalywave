// Картинка итогового слайда «Итогов года» 1080×1920 для историй в соцсетях: то же, что на экране, —
// плоское поле цвета обложки, фото артиста года, топ-5 артистов и треков, минуты, жанр и характер.
// Делится через системное «Поделиться» (сайт и iPhone), иначе скачивается файлом.
import type { SlideColors } from './rewind-colors'

export interface ShareCard {
  year: number
  /** «Итоги года» или «Итоги по сей день» */
  kicker: string
  colors: SlideColors
  photo: string | null
  artistsLabel: string
  artists: string[]
  tracksLabel: string
  tracks: string[]
  stats: { label: string, value: string }[]
  personaLabel: string
  persona: string
}

const W = 1080
const H = 1920
const FONT = '"Onest Variable", ui-sans-serif, system-ui, sans-serif'

function loadImage(url: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.decoding = 'async'
    img.onload = () => resolve(img)
    img.onerror = () => resolve(null)
    img.src = url
  })
}

function fit(ctx: CanvasRenderingContext2D, text: string, max: number): string {
  if (ctx.measureText(text).width <= max) return text
  let s = text
  while (s.length > 1 && ctx.measureText(`${s}…`).width > max) s = s.slice(0, -1)
  return `${s.trimEnd()}…`
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath()
  ctx.roundRect(x, y, w, h, r)
}

/** PNG итогового слайда */
export async function renderShareCard(card: ShareCard): Promise<Blob> {
  await Promise.all([800, 600].map(w => document.fonts?.load(`${w} 64px "Onest Variable"`).catch(() => null)))
  const canvas = document.createElement('canvas')
  canvas.width = W
  canvas.height = H
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('canvas')
  const { bg, ink, soft, accent } = card.colors
  const pad = 84

  ctx.fillStyle = bg
  ctx.fillRect(0, 0, W, H)
  // Одна простая фигура цвета обложки — как на слайдах
  ctx.fillStyle = accent
  ctx.beginPath()
  ctx.arc(W - 30, H - 70, 230, 0, Math.PI * 2)
  ctx.fill()

  // Фото артиста года и год
  const photoSize = 380
  const photo = card.photo ? await loadImage(card.photo) : null
  let y = 150
  if (photo) {
    ctx.save()
    roundRect(ctx, pad, y, photoSize, photoSize, 24)
    ctx.clip()
    const s = Math.max(photoSize / photo.width, photoSize / photo.height)
    ctx.drawImage(photo, pad + (photoSize - photo.width * s) / 2, y + (photoSize - photo.height * s) / 2, photo.width * s, photo.height * s)
    ctx.restore()
  }
  const textX = photo ? pad + photoSize + 48 : pad
  ctx.fillStyle = soft
  ctx.font = `700 40px ${FONT}`
  ctx.textBaseline = 'alphabetic'
  ctx.fillText(fit(ctx, card.kicker, W - textX - pad), textX, y + photoSize - 190)
  ctx.fillStyle = ink
  ctx.font = `800 196px ${FONT}`
  ctx.fillText(String(card.year), textX - 8, y + photoSize - 8)

  // Две колонки: артисты и треки
  y += photoSize + 120
  const colW = (W - pad * 2 - 60) / 2
  const column = (x: number, label: string, items: string[]) => {
    ctx.fillStyle = soft
    ctx.font = `700 36px ${FONT}`
    ctx.fillText(label, x, y)
    items.slice(0, 5).forEach((item, i) => {
      const ly = y + 84 + i * 72
      ctx.fillStyle = soft
      ctx.font = `800 44px ${FONT}`
      ctx.fillText(String(i + 1), x, ly)
      ctx.fillStyle = ink
      ctx.fillText(fit(ctx, item, colW - 52), x + 52, ly)
    })
  }
  column(pad, card.artistsLabel, card.artists)
  column(pad + colW + 60, card.tracksLabel, card.tracks)

  // Минуты, жанр, характер
  y += 84 + 5 * 72 + 70
  ctx.strokeStyle = ink
  ctx.globalAlpha = 0.22
  ctx.lineWidth = 3
  ctx.beginPath()
  ctx.moveTo(pad, y)
  ctx.lineTo(W - pad, y)
  ctx.stroke()
  ctx.globalAlpha = 1
  y += 90
  card.stats.slice(0, 2).forEach((s, i) => {
    const x = pad + i * (colW + 60)
    ctx.fillStyle = soft
    ctx.font = `700 36px ${FONT}`
    ctx.fillText(s.label, x, y)
    ctx.fillStyle = ink
    ctx.font = `800 76px ${FONT}`
    ctx.fillText(fit(ctx, s.value, colW), x, y + 92)
  })
  y += 210
  ctx.fillStyle = soft
  ctx.font = `700 36px ${FONT}`
  ctx.fillText(card.personaLabel, pad, y)
  ctx.fillStyle = ink
  ctx.font = `800 96px ${FONT}`
  ctx.fillText(fit(ctx, card.persona, W - pad * 2), pad, y + 110)

  // Подпись сервиса
  ctx.fillStyle = ink
  ctx.font = `800 44px ${FONT}`
  ctx.fillText('DalyWave', pad, H - 110)

  const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, 'image/png'))
  if (!blob) throw new Error('png')
  return blob
}

/** Системное «Поделиться» с файлом, где оно есть; иначе — скачать. `shared` — ушло в системное окно */
export async function shareOrDownload(blob: Blob, name: string, title: string): Promise<'shared' | 'downloaded' | 'cancelled'> {
  const file = new File([blob], name, { type: 'image/png' })
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
