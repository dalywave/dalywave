// Картинка «Поделиться» в стиле «Волна года»: кадр настоящей волны в цветах года и итоги поверх —
// год, топ-5 артистов и треков, минуты, жанр и характер. 1080×1920, как истории в соцсетях.
import { CARD_FONT, CARD_H, CARD_W, cardHairline, cardPng, fitText, newCard } from '~/utils/rewind-share'
import { renderWaveStill } from './wave'

export interface WaveShareCard {
  year: number
  kicker: string
  palette: [string, string, string, string]
  months: number[]
  artistsLabel: string
  artists: string[]
  tracksLabel: string
  tracks: string[]
  stats: { label: string, value: string }[]
}

const WHITE = '#ffffff'
const SOFT = 'rgba(255, 255, 255, 0.66)'
const LINE = 'rgba(255, 255, 255, 0.16)'

export async function renderWaveShare(card: WaveShareCard): Promise<Blob> {
  const { canvas, ctx } = await newCard()
  const pad = 84
  ctx.fillStyle = card.palette[0]
  ctx.fillRect(0, 0, CARD_W, CARD_H)
  // Гребень — через середину картинки, между годом и списками
  const wave = renderWaveStill(CARD_W, CARD_H, { tilt: -0.12, offset: 0.04, amp: 0.85, freq: 1.8, energy: 0.55, vig: 0.5 }, card.palette, { months: card.months })
  if (wave) ctx.drawImage(wave, 0, 0)

  // Шапка: подпись и огромный год
  ctx.fillStyle = WHITE
  ctx.font = `700 44px ${CARD_FONT}`
  ctx.fillText('DalyWave', pad, 168)
  ctx.fillStyle = SOFT
  ctx.font = `600 40px ${CARD_FONT}`
  ctx.fillText(fitText(ctx, card.kicker, CARD_W - pad * 2), pad, 236)
  ctx.fillStyle = WHITE
  ctx.font = `800 300px ${CARD_FONT}`
  ctx.letterSpacing = '-14px'
  ctx.fillText(String(card.year), pad - 12, 520)
  ctx.letterSpacing = '0px'

  // Две колонки с волосками
  let y = 1040
  const gap = 56
  const colW = (CARD_W - pad * 2 - gap) / 2
  const column = (x: number, label: string, items: string[]) => {
    ctx.fillStyle = SOFT
    ctx.font = `600 34px ${CARD_FONT}`
    ctx.fillText(label, x, y)
    items.slice(0, 5).forEach((item, i) => {
      const ly = y + 76 + i * 70
      ctx.fillStyle = 'rgba(255, 255, 255, 0.5)'
      ctx.font = `500 38px ${CARD_FONT}`
      ctx.fillText(String(i + 1), x, ly)
      ctx.fillStyle = WHITE
      ctx.font = `600 40px ${CARD_FONT}`
      ctx.fillText(fitText(ctx, item, colW - 46), x + 46, ly)
    })
  }
  column(pad, card.artistsLabel, card.artists)
  column(pad + colW + gap, card.tracksLabel, card.tracks)

  // Минуты и жанр через вертикальный волосок, ниже — характер во всю ширину
  y += 76 + 5 * 70 + 24
  cardHairline(ctx, pad, y, CARD_W - pad, LINE)
  y += 74
  let x = pad
  const persona = card.stats.length > 2 ? card.stats[card.stats.length - 1] : null
  const row = (persona ? card.stats.slice(0, -1) : card.stats).slice(0, 2)
  row.forEach((s, i) => {
    ctx.font = `700 52px ${CARD_FONT}`
    const value = fitText(ctx, s.value, CARD_W - pad - x)
    const vw = ctx.measureText(value).width
    ctx.font = `600 32px ${CARD_FONT}`
    const w = Math.max(vw, ctx.measureText(s.label).width)
    ctx.fillStyle = SOFT
    ctx.fillText(s.label, x, y)
    ctx.fillStyle = WHITE
    ctx.font = `700 52px ${CARD_FONT}`
    ctx.fillText(value, x, y + 70)
    x += w + 44
    if (i < row.length - 1) {
      ctx.fillStyle = LINE
      ctx.fillRect(x, y - 34, 2, 116)
      x += 46
    }
  })
  if (persona) {
    y += 170
    ctx.fillStyle = SOFT
    ctx.font = `600 32px ${CARD_FONT}`
    ctx.fillText(persona.label, pad, y)
    ctx.fillStyle = WHITE
    ctx.font = `800 76px ${CARD_FONT}`
    ctx.letterSpacing = '-2px'
    ctx.fillText(fitText(ctx, persona.value, CARD_W - pad * 2), pad, y + 92)
    ctx.letterSpacing = '0px'
  }
  return cardPng(canvas)
}
