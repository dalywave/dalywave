// Картинка «Пластинки» 1080×1920: чек итогов на тёмном поле в тон обложки — как итоговый слайд.
// Бумага плоская, без зерна (в соцсетях оно превращается в грязь), зубчатый край, штрихкод из года.
import type { RecordColors } from '~/utils/rewind-colors'
import { CARD_FONT, CARD_H, CARD_W, cardPng, fitText, newCard } from '~/utils/rewind-share'

export interface RecordCardRow { label: string, value: string }

export interface RecordCard {
  year: number
  /** «Итоги 2026» или «Итоги по сей день · 2026» */
  head: string
  name: string
  colors: RecordColors
  artistsLabel: string
  artists: RecordCardRow[]
  tracksLabel: string
  tracks: RecordCardRow[]
  totals: RecordCardRow[]
  thanks: string
  brand: string
}

const PAPER = '#f4f0e6'
const INK = '#1b1813'
const CREAM = '#f3ecdf'

/** Те же полосы, что на слайде: из года, всегда одинаковые */
function barcode(seed: string): { x: number, w: number }[] {
  let pos = 0
  let h = 2166136261
  const bars: { x: number, w: number }[] = []
  while (pos < 200) {
    h = Math.imul(h ^ seed.charCodeAt(pos % seed.length), 16777619) >>> 0
    const w = 1 + (h % 3)
    if ((h >> 3) & 1) bars.push({ x: pos, w })
    pos += w + 1
  }
  return bars
}

export async function renderRecordCard(card: RecordCard): Promise<Blob> {
  const { canvas, ctx } = await newCard()
  ctx.fillStyle = card.colors.bg
  ctx.fillRect(0, 0, CARD_W, CARD_H)

  // Щель принтера и чек
  const slotX = 90
  const slotW = CARD_W - slotX * 2
  ctx.fillStyle = '#050404'
  ctx.beginPath()
  ctx.roundRect(slotX, 150, slotW, 22, 11)
  ctx.fill()

  const px = 150
  const pw = CARD_W - px * 2
  const top = 166
  const pad = 64
  const left = px + pad
  const right = px + pw - pad
  const nA = Math.min(5, card.artists.length)
  const nT = Math.min(5, card.tracks.length)
  // Высота чека — по содержимому (те же шаги, что ниже при рисовании)
  const height = 110 + 58 + (card.name ? 42 : 0) + 40 + 70 + nA * 52 + 72 + nT * 52 + 60 + 62 + Math.max(0, card.totals.length - 1) * 52 + 60 + 160 + 56
  const bottom = Math.min(CARD_H - 230, top + height)

  // Бумага с зубчатым нижним краем
  const tooth = 24
  ctx.fillStyle = PAPER
  ctx.beginPath()
  ctx.moveTo(px, top)
  ctx.lineTo(px + pw, top)
  ctx.lineTo(px + pw, bottom)
  for (let xx = px + pw; xx > px; xx -= tooth) {
    ctx.lineTo(Math.max(px, xx - tooth / 2), bottom + tooth / 2)
    ctx.lineTo(Math.max(px, xx - tooth), bottom)
  }
  ctx.closePath()
  ctx.fill()

  const dashed = (y: number) => {
    ctx.save()
    ctx.strokeStyle = 'rgba(27, 24, 19, 0.4)'
    ctx.lineWidth = 3
    ctx.setLineDash([12, 9])
    ctx.beginPath()
    ctx.moveTo(left, y)
    ctx.lineTo(right, y)
    ctx.stroke()
    ctx.restore()
  }
  /** Строка чека: слева текст, справа значение, между ними точки */
  const row = (label: string, value: string, y: number, size = 34, weight = 600) => {
    ctx.font = `${weight} ${size}px ${CARD_FONT}`
    ctx.fillStyle = INK
    const vw = Math.min(ctx.measureText(value).width, (right - left) * 0.5)
    const v = fitText(ctx, value, (right - left) * 0.5)
    ctx.textAlign = 'right'
    ctx.fillText(v, right, y)
    ctx.textAlign = 'left'
    const l = fitText(ctx, label, right - left - vw - 60)
    ctx.fillText(l, left, y)
    const from = left + ctx.measureText(l).width + 14
    const to = right - vw - 14
    if (to > from) {
      ctx.save()
      ctx.strokeStyle = 'rgba(27, 24, 19, 0.35)'
      ctx.lineWidth = 3
      ctx.setLineDash([2, 8])
      ctx.lineCap = 'round'
      ctx.beginPath()
      ctx.moveTo(from, y - 8)
      ctx.lineTo(to, y - 8)
      ctx.stroke()
      ctx.restore()
    }
  }
  const section = (label: string, y: number) => {
    ctx.font = `700 26px ${CARD_FONT}`
    ctx.fillStyle = 'rgba(27, 24, 19, 0.6)'
    ctx.textAlign = 'left'
    ctx.fillText(label.toUpperCase().split('').join(' '), left, y)
  }

  // Шапка
  let y = top + 110
  ctx.textAlign = 'center'
  ctx.fillStyle = INK
  ctx.font = `800 64px ${CARD_FONT}`
  ctx.fillText(card.brand, CARD_W / 2, y)
  y += 58
  ctx.font = `600 30px ${CARD_FONT}`
  ctx.fillText(fitText(ctx, card.head.toUpperCase(), right - left), CARD_W / 2, y)
  if (card.name) {
    y += 42
    ctx.fillText(fitText(ctx, card.name.toUpperCase(), right - left), CARD_W / 2, y)
  }
  y += 40
  dashed(y)

  y += 70
  section(card.artistsLabel, y)
  card.artists.slice(0, nA).forEach((a, i) => row(`${i + 1}. ${a.label}`, a.value, (y += 52)))
  y += 72
  section(card.tracksLabel, y)
  card.tracks.slice(0, nT).forEach((tr, i) => row(`${i + 1}. ${tr.label}`, tr.value, (y += 52)))
  y += 40
  dashed(y)
  y += 20
  card.totals.forEach((tot, i) => row(tot.label, tot.value, (y += i === 0 ? 62 : 52), i === 0 ? 44 : 34, i === 0 ? 800 : 600))

  // Штрихкод и спасибо
  y += 60
  const bars = barcode(`dalywave-${card.year}`)
  const bw = right - left
  ctx.fillStyle = INK
  for (const b of bars) ctx.fillRect(left + (b.x / 200) * bw, y, (b.w / 200) * bw, 110)
  y += 160
  ctx.textAlign = 'center'
  ctx.font = `600 26px ${CARD_FONT}`
  ctx.fillStyle = 'rgba(27, 24, 19, 0.7)'
  ctx.fillText(card.thanks.toUpperCase(), CARD_W / 2, y)

  // Подпись внизу — на поле, кремом
  ctx.fillStyle = CREAM
  ctx.font = `800 40px ${CARD_FONT}`
  ctx.fillText(`${card.brand} · ${card.year}`, CARD_W / 2, CARD_H - 96)
  ctx.textAlign = 'left'

  return cardPng(canvas)
}
