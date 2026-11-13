// Звук истории → движение волны по кадрам и строка текста в момент кадра. Без DOM — проверяется bun test

export interface StoryLine {
  /** От начала истории, мс */
  atMs: number
  text: string
}

// ---------- Звук → движение волны ----------

export interface FrameLevel { energy: number, beat: number, bands: [number, number, number] }

const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v))
const norm = (x: number) => clamp((20 * Math.log10(Math.max(x, 1e-6)) + 66) / 60, 0, 1)

/** Уровни по кадрам: громкость, бас, середина и верх, как у волны в приложении (engine.ts) */
export function frameLevels(audio: AudioBuffer, fps = 30): FrameLevel[] {
  const sr = audio.sampleRate
  const ch = Array.from({ length: audio.numberOfChannels }, (_, i) => audio.getChannelData(i))
  const per = Math.floor(sr / fps)
  const total = Math.ceil(audio.length / per)
  const aLow = 1 - Math.exp((-2 * Math.PI * 150) / sr)
  const aMid = 1 - Math.exp((-2 * Math.PI * 2000) / sr)
  let low = 0
  let mid = 0
  const raw: { rms: number, bands: [number, number, number] }[] = []
  for (let f = 0; f < total; f++) {
    let s = 0
    let sl = 0
    let sm = 0
    let sh = 0
    const end = Math.min(audio.length, (f + 1) * per)
    for (let i = f * per; i < end; i++) {
      let x = 0
      for (const c of ch) x += c[i]!
      x /= ch.length
      low += aLow * (x - low)
      mid += aMid * (x - mid)
      s += x * x
      sl += low * low
      sm += (mid - low) * (mid - low)
      sh += (x - mid) * (x - mid)
    }
    const n = Math.max(1, end - f * per)
    raw.push({ rms: Math.sqrt(s / n), bands: [norm(Math.sqrt(sl / n)), norm(Math.sqrt(sm / n)), norm(Math.sqrt(sh / n))] })
  }
  const dt = 1 / fps
  let peak = 0.05
  let energy = 0.3
  let onset = 0
  let mean = 0
  let dev = 0
  let prev: [number, number, number] = [0, 0, 0]
  let lastOnset = -1
  const out: FrameLevel[] = []
  raw.forEach((r, f) => {
    peak = Math.max(peak * Math.exp(-dt * 0.15), r.rms, 0.02)
    const target = clamp(Math.pow(r.rms / peak, 1.4) * 0.95 + r.bands[0] * 0.25, 0, 1)
    energy += (target - energy) * (1 - Math.exp(-dt * 4))
    let flux = 0
    for (let i = 0; i < 3; i++) flux += Math.max(0, r.bands[i]! - prev[i]!)
    flux /= 3
    prev = r.bands
    mean += (flux - mean) * 0.06
    dev += (Math.abs(flux - mean) - dev) * 0.06
    if (flux > mean + 1.7 * dev + 0.01 && f - lastOnset > fps * 0.17) {
      lastOnset = f
      onset = 1
    }
    onset *= Math.exp(-dt * 7)
    out.push({ energy, beat: onset * 0.95 * clamp(r.rms / peak, 0, 1), bands: r.bands })
  })
  return out
}

/** Строка текста в момент `ms` истории */
export function lineAt(lines: StoryLine[], ms: number): StoryLine | null {
  let cur: StoryLine | null = null
  for (const l of lines) {
    if (l.atMs <= ms) cur = l
    else break
  }
  return cur && cur.text.trim() ? cur : null
}
