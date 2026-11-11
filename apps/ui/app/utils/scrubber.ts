// Геометрия полосы прокрутки — общая для плеера, мини-плеера и миниатюр в настройках

/**
 * Столбики формы волны в координатах 1000×100: SVG растягивается по ширине, а толщину штриха
 * задаёт CSS в пикселях (non-scaling-stroke), поэтому вид не зависит от размера полосы
 */
export function barsPath(peaks: number[]): string {
  const n = peaks.length || 1
  const w = 1000 / n
  let out = ''
  for (let i = 0; i < n; i++) {
    const v = Math.max(0.14, Math.min(1, peaks[i] ?? 0.3))
    const h = v * 86
    const cx = (i * w + w / 2).toFixed(2)
    out += `M${cx} ${(50 - h / 2).toFixed(1)}V${(50 + h / 2).toFixed(1)}`
  }
  return out
}

/** Доля 0…1 от длительности; null, если значения или длительности нет */
export function ratioOf(ms: number | null | undefined, durationMs: number): number | null {
  if (ms == null || !(durationMs > 0)) return null
  return Math.min(1, Math.max(0, ms / durationMs))
}

/** Где на полосе ромб перехода: только если переход начнётся раньше конца трека */
export function markerRatio(transitionMs: number | null | undefined, durationMs: number): number | null {
  return transitionMs != null && transitionMs < durationMs ? ratioOf(transitionMs, durationMs) : null
}
