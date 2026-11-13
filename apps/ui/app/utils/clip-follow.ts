/** Клип целиком, сопоставленный с треком: позиция трека = `offsetMs` + позиция видео × `rate` */
export interface ClipFollow {
  offsetMs: number
  rate: number
  /** Позиция трека сейчас, мс */
  position: () => number
  playing: () => boolean
}

/** Видео расходится с треком больше чем на столько — перескакивает */
export const FOLLOW_TOLERANCE_S = 0.3

/**
 * Где должно быть видео (с) при нынешней позиции трека; `null` — у трека тут кадров клипа нет
 * (вступление трека до начала клипа или хвост после его конца). `duration` — длина видео, с
 * (пока неизвестна — NaN, тогда хвост не отсекается)
 */
export function followTarget(f: Pick<ClipFollow, 'offsetMs' | 'rate' | 'position'>, duration: number): number | null {
  const at = (f.position() - f.offsetMs) / f.rate / 1000
  if (at < 0) return null
  if (Number.isFinite(duration) && duration > 0 && at > duration - 0.05) return null
  return at
}
