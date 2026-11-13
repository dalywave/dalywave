// Редактор момента истории: какой кусок трека попадёт в видео. Окно фиксированной длины (10, 15, 30 с)
// двигается по форме волны; по умолчанию — с припева. Без DOM — проверяется bun test
import type { StoryLine } from './levels'

export const STORY_LENGTHS = [10_000, 15_000, 30_000] as const
export type StoryLength = (typeof STORY_LENGTHS)[number]
/** Превью Deezer: у трека без звука на сервере выбирать можно только внутри него */
export const PREVIEW_MS = 30_000
/** Шаг стрелок: 1 с, с Shift — 5 с */
export const STEP_MS = 1000
export const BIG_STEP_MS = 5000

/** Начало окна: не меньше нуля и так, чтобы окно целиком помещалось в звук */
export function clampStart(startMs: number, lengthMs: number, totalMs: number): number {
  return Math.max(0, Math.min(startMs, totalMs - lengthMs))
}

/** Стрелки, Home/End, PageUp/PageDown → новое начало окна; null — клавиша не наша */
export function keyStart(key: string, shift: boolean, startMs: number, lengthMs: number, totalMs: number): number | null {
  const step = shift ? BIG_STEP_MS : STEP_MS
  let next: number
  switch (key) {
    case 'ArrowLeft':
    case 'ArrowDown':
      next = startMs - step
      break
    case 'ArrowRight':
    case 'ArrowUp':
      next = startMs + step
      break
    case 'PageDown':
      next = startMs - lengthMs
      break
    case 'PageUp':
      next = startMs + lengthMs
      break
    case 'Home':
      next = 0
      break
    case 'End':
      next = totalMs
      break
    default:
      return null
  }
  // Стрелки двигают по целым секундам: после перетаскивания окно встаёт на сетку
  if (key.startsWith('Arrow')) next = Math.round(next / STEP_MS) * STEP_MS
  return clampStart(next, lengthMs, totalMs)
}

/** Перетаскивание: доля ширины под пальцем → начало окна. `grab` — где внутри окна взяли (0…1 окна) */
export function dragStart(ratio: number, grab: number, lengthMs: number, totalMs: number): number {
  const at = ratio * totalMs - grab * lengthMs
  // Полсекунды — шаг, который ещё слышен; мельче только дрожит
  return clampStart(Math.round(at / 500) * 500, lengthMs, totalMs)
}

/** Строки текста относительно начала истории: та, что звучит на старте, — с нуля */
export function storyLines(lines: { tMs: number, text: string }[], startMs: number, lengthMs: number): StoryLine[] {
  const out: StoryLine[] = []
  for (const l of lines) {
    const at = l.tMs - startMs
    if (at < 0) {
      out.length = 0
      out.push({ atMs: 0, text: l.text })
    }
    else if (at < lengthMs) out.push({ atMs: at, text: l.text })
  }
  return out
}

/** Форма волны из звука (превью Deezer): максимум в окне, сжатый к 0…1 по громкому месту */
export function channelPeaks(channels: Float32Array[], bars: number): number[] {
  const len = channels[0]?.length ?? 0
  if (!len || !bars) return []
  const raw: number[] = []
  for (let b = 0; b < bars; b++) {
    const a = Math.floor((b / bars) * len)
    const e = Math.max(a + 1, Math.floor(((b + 1) / bars) * len))
    let mx = 0
    for (const ch of channels) {
      // Каждый 4-й отсчёт: на столбик их тысячи, форма та же
      for (let i = a; i < e; i += 4) {
        const v = Math.abs(ch[i]!)
        if (v > mx) mx = v
      }
    }
    raw.push(mx)
  }
  const top = Math.max(...raw, 1e-4)
  return raw.map(v => Math.max(0.08, Math.min(1, Math.sqrt(v / top))))
}
