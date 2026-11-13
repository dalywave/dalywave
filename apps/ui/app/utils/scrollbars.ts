// Геометрия накладной полосы прокрутки (plugins/scrollbars.client.ts): размер и место ползунка,
// перетаскивание, щелчок по дорожке. Чистые функции — проверяются bun test

export interface ScrollMetrics {
  scrollTop: number
  scrollHeight: number
  clientHeight: number
  /** Длина дорожки, по которой ходит ползунок, px */
  track: number
}

/** Короче этого ползунок не становится — даже в списке на 10 000 строк за него удобно взяться */
export const MIN_THUMB = 32

/** Размер и сдвиг ползунка от начала дорожки; null — прокручивать нечего */
export function thumbGeometry(m: ScrollMetrics, min = MIN_THUMB): { size: number, offset: number } | null {
  const max = m.scrollHeight - m.clientHeight
  if (max < 1 || m.track <= 0 || m.clientHeight <= 0) return null
  const size = Math.min(m.track, Math.max(Math.min(min, m.track), (m.track * m.clientHeight) / m.scrollHeight))
  const offset = (Math.min(Math.max(m.scrollTop, 0), max) / max) * (m.track - size)
  return { size, offset }
}

/** Куда прокрутить, когда ползунок утащили на dy px от места, где за него взялись */
export function dragScrollTop(startTop: number, dy: number, m: Omit<ScrollMetrics, 'scrollTop'>, size: number): number {
  const max = m.scrollHeight - m.clientHeight
  const free = m.track - size
  if (max < 1 || free <= 0) return startTop
  return Math.min(max, Math.max(0, startTop + (dy * max) / free))
}

/** Щелчок по пустой дорожке на высоте at — на страницу к щелчку (как у системной полосы) */
export function pageDelta(at: number, offset: number, size: number, clientHeight: number): number {
  const page = Math.round(clientHeight * 0.9)
  if (at < offset) return -page
  if (at > offset + size) return page
  return 0
}

/** Колесо над самой полосой: шаг в пикселях (строки и страницы — в пиксели) */
export function wheelPixels(delta: number, mode: number, clientHeight: number): number {
  if (mode === 1) return delta * 40
  if (mode === 2) return delta * clientHeight
  return delta
}
