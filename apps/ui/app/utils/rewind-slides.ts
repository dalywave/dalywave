// Слайды «Итогов года»: какие истории показать и откуда у каждой цвет. Общие для трёх стилей —
// «Волна года», «Разворот» и «Пластинка» рисуют одни и те же слайды каждый по-своему.
import type { Rewind, RewindStyle } from '~/api/types'

export type SlideKind = 'intro' | 'minutes' | 'days' | 'first' | 'tracks' | 'toy' | 'artists' | 'albums' | 'sound' | 'discoveries' | 'personality' | 'summary' | 'wave' | 'more' | 'empty'

export const REWIND_STYLES: RewindStyle[] = ['wave', 'spread', 'record']

export interface SlideSource {
  kind: SlideKind
  ms: number
  /** Откуда цвет: палитра альбома с сервера или картинка */
  palette?: string[] | null
  image?: string | null
  /** Какой по живости цвет взять: соседние слайды с одной обложкой различаются */
  shift?: number
}

/** Слайд с готовой палитрой (самый заметный цвет первым) — из неё каждый стиль берёт свои цвета */
export interface RewindSlide {
  kind: SlideKind
  ms: number
  palette: string[]
  shift: number
}

/** Истории по данным года: слайд без данных не показывается, пустой год — один слайд */
export function rewindSlides(r: Rewind): SlideSource[] {
  if (r.plays === 0) return [{ kind: 'empty', ms: 8000 }]
  const top = r.topTracks.map(x => x.track)
  const al = r.topAlbums.map(a => a.album)
  const toy = r.trackOfYear?.track ?? null
  // Палитры нескольких обложек: поле берёт самый живой цвет из них
  const pal = (from: number) => [...al.slice(from), ...al.slice(0, from)].flatMap(a => a.palette ?? [])
  const list: (SlideSource | false)[] = [
    { kind: 'intro', ms: 6000, palette: pal(0), image: al[0]?.image?.small },
    { kind: 'minutes', ms: 8000, palette: pal(1), shift: 1, image: al[1]?.image?.small },
    { kind: 'days', ms: 8000, image: r.topDay?.track?.album?.image?.small ?? al[2]?.image?.small, palette: r.topDay?.track ? null : pal(2) },
    !!r.firstTrack && { kind: 'first', ms: 7000, image: r.firstTrack.track.album?.image?.small },
    top.length > 0 && { kind: 'tracks', ms: 9000, image: top[0]?.album?.image?.small },
    !!toy && { kind: 'toy', ms: 8000, image: toy.album?.image?.small, shift: 1 },
    r.topArtists.length > 0 && { kind: 'artists', ms: 9000, image: r.topArtists[0]?.artist.image?.small },
    al.length > 0 && { kind: 'albums', ms: 8000, palette: al[0]?.palette, image: al[0]?.image?.small, shift: 1 },
    { kind: 'sound', ms: 8000, palette: pal(3), shift: 2 },
    { kind: 'discoveries', ms: 8000, image: r.discoveries[0]?.artist.image?.small ?? al[4]?.image?.small, palette: r.discoveries[0] ? null : al[4]?.palette },
    { kind: 'personality', ms: 8000, image: r.topArtists[1]?.artist.image?.small, palette: r.topArtists[1] ? null : al[1]?.palette, shift: 1 },
    { kind: 'summary', ms: 12_000, image: r.topArtists[0]?.artist.image?.small, palette: r.topArtists[0] ? null : al[0]?.palette },
    // Подобрать ещё: волна из любимого за год и новое, похожее на него
    { kind: 'wave', ms: 8000, palette: pal(2), shift: 2, image: al[2]?.image?.small },
    { kind: 'more', ms: 12_000, palette: pal(4), shift: 1, image: al[4]?.image?.small },
  ]
  return list.filter((s): s is SlideSource => !!s)
}

/** Стиль из адреса (предпросмотр администратора) */
export function parseRewindStyle(v: unknown): RewindStyle | null {
  return typeof v === 'string' && (REWIND_STYLES as string[]).includes(v) ? v as RewindStyle : null
}
