import type { ChartSummary } from '~/api/types'

/** Плитки чартов — ровной заливкой, как жанры в поиске: цвет несёт обложка первого места */
const COLORS: Record<string, string> = {
  'ru': '#b4233c',
  'world': '#1f5fd1',
  'genre-132': '#c2185b',
  'genre-116': '#6a3fd1',
  'genre-152': '#5a5f6e',
  'genre-113': '#0f8a7e',
  'genre-106': '#1f6fb2',
  'genre-165': '#8a4b2a',
  'genre-85': '#2a6a6e',
  'genre-464': '#3a3d46',
  'genre-129': '#a3742a',
  'genre-98': '#6d5a3f',
}

/** Цвет плитки: приглушённый к тёмному, текст на нём — белый в обеих темах */
export function chartTile(id: string): string {
  return `color-mix(in oklab, ${COLORS[id] ?? '#4a4f5c'} 62%, #0b0c10)`
}

/** Контекст плеера для чарта: «Сейчас играет» показывает название, а не «Поиск» */
export function chartContext(c: Pick<ChartSummary, 'id'>, title: string) {
  return { type: 'search' as const, id: `chart:${c.id}`, title }
}
