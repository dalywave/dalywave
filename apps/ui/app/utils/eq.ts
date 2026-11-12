// Эквалайзер: те же десять полос и пресеты, что в движке приложений (crates/client/audio/src/eq.rs).
// Сайт играет их через WebAudio (BiquadFilterNode 'peaking' — тот же колокол RBJ), приложения — движком.

export type EqPresetId = 'off' | 'bass-boost' | 'bass-cut' | 'vocal' | 'acoustic' | 'electronic' | 'loudness' | 'custom'

/** Что выбрано: пресет и полосы «Своего» (помнятся и при другом пресете) */
export interface EqConfig {
  preset: EqPresetId
  gains: number[]
}

export const EQ_BANDS_HZ = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const
export const EQ_MAX_DB = 12
/** Ширина полосы — октава */
export const EQ_Q = Math.SQRT2

/** Пресеты по порядку в списке; «Свой» — последним */
export const EQ_PRESETS: { id: Exclude<EqPresetId, 'custom'>, gains: number[] }[] = [
  { id: 'off', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { id: 'bass-boost', gains: [6, 5.5, 4.5, 2.5, 0.5, 0, 0, 0, 0, 0] },
  { id: 'bass-cut', gains: [-6, -5.5, -4.5, -2.5, -0.5, 0, 0, 0, 0, 0] },
  { id: 'vocal', gains: [-2, -2, -1.5, 0, 2, 3.5, 3.5, 2, 0, -1] },
  { id: 'acoustic', gains: [3, 3, 2.5, 1, 1, 0.5, 1.5, 2.5, 2.5, 2] },
  { id: 'electronic', gains: [4.5, 4, 1.5, 0, -1.5, 1, 0.5, 1.5, 4, 4.5] },
  { id: 'loudness', gains: [5, 4, 2, 0, -1, -1, 0, 1, 3, 4] },
]

export const FLAT_GAINS = (): number[] => EQ_BANDS_HZ.map(() => 0)

/** Полосы, которые играют сейчас; null — эквалайзер выключен (звук мимо) */
export function activeGains(eq: EqConfig | null): number[] | null {
  if (!eq || eq.preset === 'off') return null
  const g = eq.preset === 'custom' ? eq.gains : EQ_PRESETS.find(p => p.id === eq.preset)?.gains
  if (!g) return null
  const clamped = EQ_BANDS_HZ.map((_, i) => Math.max(-EQ_MAX_DB, Math.min(EQ_MAX_DB, Number(g[i]) || 0)))
  return clamped.some(x => x !== 0) ? clamped : null
}

/** АЧХ колокола RBJ на частоте hz, дБ */
function peakingDb(f0: number, gainDb: number, hz: number, rate: number): number {
  if (gainDb === 0) return 0
  const a = Math.pow(10, gainDb / 40)
  const w0 = (2 * Math.PI * Math.min(f0, rate * 0.45)) / rate
  const alpha = Math.sin(w0) / (2 * EQ_Q)
  const cos = Math.cos(w0)
  const a0 = 1 + alpha / a
  const b0 = (1 + alpha * a) / a0
  const b1 = (-2 * cos) / a0
  const b2 = (1 - alpha * a) / a0
  const a1 = (-2 * cos) / a0
  const a2 = (1 - alpha / a) / a0
  const w = (2 * Math.PI * hz) / rate
  const br = b0 + b1 * Math.cos(w) + b2 * Math.cos(2 * w)
  const bi = -(b1 * Math.sin(w) + b2 * Math.sin(2 * w))
  const ar = 1 + a1 * Math.cos(w) + a2 * Math.cos(2 * w)
  const ai = -(a1 * Math.sin(w) + a2 * Math.sin(2 * w))
  return 10 * Math.log10((br * br + bi * bi) / (ar * ar + ai * ai))
}

export function responseDb(gains: number[], hz: number, rate = 48000): number {
  return EQ_BANDS_HZ.reduce((sum, f0, i) => sum + peakingDb(f0, gains[i] ?? 0, hz, rate), 0)
}

/** Предусиление, дБ (≤ 0): минус самый высокий подъём АЧХ — как в движке */
export function preampDb(gains: number[], rate = 48000): number {
  if (gains.every(g => g <= 0)) return 0
  const top = Math.min(rate / 2, 20000)
  let peak = 0
  const steps = 240
  for (let i = 0; i <= steps; i++) peak = Math.max(peak, responseDb(gains, 20 * Math.pow(top / 20, i / steps), rate))
  for (const f of EQ_BANDS_HZ) peak = Math.max(peak, responseDb(gains, f, rate))
  return -Math.max(0, peak)
}

/** Подпись полосы: «31», «1к», «16к» */
export function bandLabel(hz: number): string {
  return hz >= 1000 ? `${hz / 1000}к` : String(hz)
}
