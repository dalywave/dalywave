/** Кусок клипа `[videoStartMs, videoEndMs)` на своей прямой: позиция трека = `offsetMs` + видео × `rate` */
export interface ClipFollowSegment {
  videoStartMs: number
  videoEndMs: number
  offsetMs: number
  rate: number
}

/**
 * Клип целиком, сопоставленный с треком: позиция трека = `offsetMs` + позиция видео × `rate`.
 * Сценка посреди клипа или вырезанный куплет — несколько кусков (`segments`), у каждого своя прямая
 */
export interface ClipFollow {
  offsetMs: number
  rate: number
  segments?: ClipFollowSegment[]
  /** Позиция трека сейчас, мс */
  position: () => number
  playing: () => boolean
}

/** Видео расходится с треком больше чем на столько — перескакивает */
export const FOLLOW_TOLERANCE_S = 0.3

/**
 * Где должно быть видео (с) при нынешней позиции трека и с какой скоростью оно идёт; `null` —
 * у трека тут кадров клипа нет (вступление трека до начала клипа, хвост после конца, место,
 * вырезанное из клипа). `duration` — длина видео, с (пока неизвестна — NaN: хвост не отсекается)
 */
export function followTarget(f: Pick<ClipFollow, 'offsetMs' | 'rate' | 'segments' | 'position'>, duration: number): { at: number, rate: number } | null {
  const pos = f.position()
  const fits = (at: number) => at >= 0 && !(Number.isFinite(duration) && duration > 0 && at > duration - 0.05)
  if (f.segments?.length) {
    for (const s of f.segments) {
      const at = (pos - s.offsetMs) / s.rate / 1000
      if (at * 1000 >= s.videoStartMs && at * 1000 < s.videoEndMs && fits(at)) return { at, rate: s.rate }
    }
    return null
  }
  const at = (pos - f.offsetMs) / f.rate / 1000
  return fits(at) ? { at, rate: f.rate } : null
}
