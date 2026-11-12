// Картинка «Итогов года» в стиле «Разворот», 1080×1920: плоское поле цвета обложки, волосок с подписью,
// огромный год, полоса обложек, топ-5 артистов и треков с волосками, минуты крупно и факты.
import type { SlideColors } from '~/utils/rewind-colors'
import { CARD_FONT, CARD_H, CARD_W, cardHairline, cardPng, drawCardImage, fitText, loadCardImage, newCard } from '~/utils/rewind-share'

export interface SpreadCard {
  year: number
  colors: SlideColors
  brand: string
  /** «Итоги 2026» или «Итоги по сей день» */
  title: string
  covers: string[]
  artistsLabel: string
  artists: string[]
  tracksLabel: string
  tracks: string[]
  /** Уже с разделителем разрядов */
  minutes: string
  minutesUnit: string
  facts: { label: string, value: string }[]
}

export async function renderSpreadCard(card: SpreadCard): Promise<Blob> {
  const { canvas, ctx } = await newCard()
  const { bg, ink, soft } = card.colors
  const line = ink === '#ffffff' ? 'rgba(255, 255, 255, 0.28)' : 'rgba(11, 11, 16, 0.28)'
  const pad = 84
  const right = CARD_W - pad

  ctx.fillStyle = bg
  ctx.fillRect(0, 0, CARD_W, CARD_H)

  // Волосок с подписью сверху
  let y = 150
  ctx.font = `600 34px ${CARD_FONT}`
  ctx.fillStyle = soft
  ctx.fillText(card.brand, pad, y)
  ctx.textAlign = 'right'
  ctx.fillText(card.title, right, y)
  ctx.textAlign = 'left'
  cardHairline(ctx, pad, y + 30, right, line)

  // Год — на всю ширину
  y += 380
  ctx.fillStyle = ink
  ctx.font = `800 400px ${CARD_FONT}`
  ctx.letterSpacing = '-28px'
  ctx.fillText(String(card.year), pad - 18, y)
  ctx.letterSpacing = '0px'

  // Полоса обложек
  y += 70
  const covers = (await Promise.all(card.covers.slice(0, 5).map(u => loadCardImage(u)))).filter((i): i is HTMLImageElement => !!i)
  if (covers.length) {
    const gap = 18
    const w = (CARD_W - pad * 2 - gap * 4) / 5
    covers.forEach((img, i) => drawCardImage(ctx, img, pad + i * (w + gap), y, w, w * 4 / 3))
    y += w * 4 / 3
  }

  // Две колонки: артисты и треки, строки через волоски
  y += 120
  const colGap = 56
  const colW = (CARD_W - pad * 2 - colGap) / 2
  const column = (x: number, label: string, items: string[]) => {
    ctx.fillStyle = soft
    ctx.font = `600 32px ${CARD_FONT}`
    ctx.fillText(label, x, y)
    items.slice(0, 5).forEach((item, i) => {
      const top = y + 28 + i * 88
      cardHairline(ctx, x, top, x + colW, line)
      ctx.fillStyle = soft
      ctx.font = `600 42px ${CARD_FONT}`
      ctx.fillText(String(i + 1), x, top + 60)
      ctx.fillStyle = ink
      ctx.font = `650 42px ${CARD_FONT}`
      ctx.fillText(fitText(ctx, item, colW - 52), x + 52, top + 60)
    })
  }
  column(pad, card.artistsLabel, card.artists)
  column(pad + colW + colGap, card.tracksLabel, card.tracks)

  // Минуты крупно и факты
  y = CARD_H - 330
  ctx.fillStyle = ink
  ctx.font = `800 210px ${CARD_FONT}`
  ctx.letterSpacing = '-12px'
  ctx.fillText(card.minutes, pad - 8, y)
  const w = ctx.measureText(card.minutes).width
  ctx.letterSpacing = '0px'
  ctx.font = `700 56px ${CARD_FONT}`
  ctx.fillText(card.minutesUnit, pad + w + 20, y)
  cardHairline(ctx, pad, y + 54, right, line)
  y += 130
  let x = pad
  for (const f of card.facts.slice(0, 2)) {
    ctx.fillStyle = soft
    ctx.font = `600 32px ${CARD_FONT}`
    ctx.fillText(f.label, x, y)
    ctx.fillStyle = ink
    ctx.font = `700 48px ${CARD_FONT}`
    const value = fitText(ctx, f.value, CARD_W - pad - x)
    ctx.fillText(value, x, y + 64)
    x += Math.max(ctx.measureText(value).width, 200) + 72
  }
  return cardPng(canvas)
}
