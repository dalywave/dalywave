// Форма волны трека для ползунка. Сервер посчитает настоящую при обработке аудио; пока её нет,
// форма строится по типичной структуре песни — детерминированно от id, чтобы не прыгала.

function hashString(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

export function seededRandom(seed: string | number): () => number {
  let a = typeof seed === 'number' ? seed >>> 0 : hashString(seed)
  return () => {
    a = (a + 0x6d2b79f5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v))

/** Сжать огибающую до нужного числа столбиков, беря максимум в каждом окне */
export function resamplePeaks(src: number[], bars: number): number[] {
  if (!src.length) return Array.from({ length: bars }, () => 0.1)
  const out: number[] = []
  for (let i = 0; i < bars; i++) {
    const a = Math.floor((i / bars) * src.length)
    const b = Math.max(a + 1, Math.floor(((i + 1) / bars) * src.length))
    let mx = 0
    for (let j = a; j < b; j++) mx = Math.max(mx, src[j] ?? 0)
    out.push(clamp(mx, 0.08, 1))
  }
  return out
}

/** Вступление, куплеты, припевы, бридж и концовка — с шумом от id трека */
export function syntheticPeaks(id: string, bars: number): number[] {
  const r = seededRandom(id)
  const sections: [number, number][] = [[0, 0.3], [0.08, 0.56], [0.24, 0.88], [0.37, 0.6], [0.5, 0.92], [0.64, 0.46], [0.74, 0.96], [0.9, 0.34], [0.97, 0.16]]
  const raw: number[] = []
  for (let i = 0; i < bars; i++) {
    const x = i / Math.max(1, bars - 1)
    let level = sections[0]![1]
    for (const [start, l] of sections) if (x >= start) level = l
    raw.push(clamp(level * (0.7 + 0.3 * r()) + (r() - 0.5) * 0.1, 0.08, 1))
  }
  return raw.map((v, i) => (v * 2 + (raw[i - 1] ?? v) + (raw[i + 1] ?? v)) / 4)
}

/** Темп и энергия для пульса волны, пока анализ трека не пришёл с сервера */
export function syntheticRhythm(id: string): { bpm: number, energy: number } {
  const r = seededRandom(`${id}:rhythm`)
  return { bpm: Math.round(78 + r() * 64), energy: 0.35 + r() * 0.45 }
}
