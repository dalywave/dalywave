// Сенсорная полоса Stream Deck + (200×100 на крутилку) и экран Neo (232×50). Раскладки — в
// rest.daly.wave.sdPlugin/layouts/*.json, здесь — значения их полей. Текст рисует сам Stream Deck
// (его шрифт, многоточие на краю), обложки и значки — SVG.
import type { GlyphName } from './glyphs.gen'
import { roundedArt, stripGlyph } from './keys'
import { C, dataUri, glyph, svg } from './svg'

export const LAYOUT = {
  track: 'layouts/track.json',
  volume: 'layouts/volume.json',
  list: 'layouts/list.json',
  neo: 'layouts/neo.json',
} as const

/** Значения полей полосы для setFeedback */
export type Fields = Record<string, string | number | { value?: string | number, enabled?: boolean, opacity?: number, color?: string, bar_fill_c?: string }>

const COVER = 64

/** Затемнение обложки на паузе со значком «играть» поверх */
function pausedOverlay(size: number): string {
  const r = Math.round(size * 0.12)
  return dataUri(svg(size, size, `<rect width="${size}" height="${size}" rx="${r}" ry="${r}" fill="#000" fill-opacity="0.5"/>${glyph('play-fill', size / 2, size / 2, size * 0.42, C.text1)}`))
}

export interface TrackStrip {
  cover: string | null
  title: string
  subtitle: string
  /** 0…1 */
  progress: number
  playing: boolean
}

export function trackFields(s: TrackStrip): Fields {
  return {
    cover: roundedArt(s.cover, COVER),
    state: { value: pausedOverlay(COVER), enabled: !s.playing },
    title: s.title,
    subtitle: s.subtitle,
    progress: { value: Math.round(Math.min(1, Math.max(0, s.progress)) * 1000) },
  }
}

export interface VolumeStrip {
  /** 0…1 */
  level: number
  muted: boolean
  label: string
  track: string
}

export function volumeGlyph(level: number, muted: boolean): GlyphName {
  if (muted || level <= 0) return 'speaker-x-bold'
  return level < 0.5 ? 'speaker-low-bold' : 'speaker-high-bold'
}

export function volumeFields(s: VolumeStrip): Fields {
  const pct = Math.round(Math.min(1, Math.max(0, s.level)) * 100)
  return {
    icon: stripGlyph(volumeGlyph(s.level, s.muted), 30, s.muted ? C.text3 : C.text1),
    level: s.label,
    bar: { value: s.muted ? 0 : pct, bar_fill_c: s.muted ? C.text4 : C.text1 },
    track: s.track,
  }
}

export interface ListStrip {
  cover: string | null
  fallback: GlyphName
  /** Цвета волны вместо обложки */
  colors?: readonly string[]
  title: string
  subtitle: string
  counter: string
}

export function listFields(s: ListStrip): Fields {
  let cover: string
  if (s.cover) cover = roundedArt(s.cover, COVER)
  else if (s.colors) cover = roundedArt(null, COVER, s.fallback, C.strip, s.colors)
  else cover = roundedArt(null, COVER, s.fallback)
  return { cover, title: s.title, subtitle: s.subtitle, counter: s.counter }
}

export function neoFields(s: { cover: string | null, title: string, subtitle: string }): Fields {
  return { cover: roundedArt(s.cover, 42), title: s.title, subtitle: s.subtitle }
}


