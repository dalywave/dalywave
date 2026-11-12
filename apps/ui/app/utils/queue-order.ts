// Перемешивание и повтор очереди плеера (stores/player.ts). Чистые функции — их проверяет
// queue-order.test.ts (`bun test`), плеер только применяет результат

import type { RepeatMode } from '~/bridge/types'

interface Entry { uid: string }

/** Перемешать (Фишер — Йейтс): исходный список не меняется */
export function shuffled<T>(list: readonly T[], rand: () => number = Math.random): T[] {
  const out = list.slice()
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1))
    ;[out[i], out[j]] = [out[j]!, out[i]!]
  }
  return out
}

/** Перемешивание включили: дальше — то же, но вразброс. `original` — порядок до перемешивания
 *  (с текущим треком во главе): по нему порядок вернётся, когда перемешивание выключат */
export function shuffleQueue<T extends Entry>(current: T | null, upcoming: readonly T[], rand: () => number = Math.random): { upcoming: T[], original: T[] } {
  return { upcoming: shuffled(upcoming, rand), original: current ? [current, ...upcoming] : upcoming.slice() }
}

/** Перемешивание выключили: после текущего трека — исходный порядок с его места (как в Spotify:
 *  несыгранное до него не возвращается). Текущего в исходном нет (играет добавленное вручную) —
 *  несыгранное в исходном порядке. Что добавилось уже после перемешивания, идёт следом как было */
export function restoreQueue<T extends Entry>(current: T | null, upcoming: readonly T[], original: readonly T[]): T[] {
  const left = new Set(upcoming.map(e => e.uid))
  const known = new Set(original.map(e => e.uid))
  const at = current ? original.findIndex(e => e.uid === current.uid) : -1
  const ordered = (at >= 0 ? original.slice(at + 1) : original).filter(e => left.has(e.uid))
  return [...ordered, ...upcoming.filter(e => !known.has(e.uid))]
}

/** Кнопка повтора по кругу, как в Spotify: выключен → вся очередь → один трек → выключен */
export function nextRepeat(mode: RepeatMode): RepeatMode {
  return mode === 'off' ? 'all' : mode === 'all' ? 'one' : 'off'
}

/** Что делать, когда трек доиграл сам: повтор одного — тот же трек сначала, иначе — следующий;
 *  очередь кончилась, а повтор всей — список с начала (новым кругом), нет — стоп */
export function afterTrack(repeat: RepeatMode, hasNext: boolean, canWrap: boolean): 'replay' | 'next' | 'wrap' | 'stop' {
  if (repeat === 'one') return 'replay'
  if (hasNext) return 'next'
  return repeat === 'all' && canWrap ? 'wrap' : 'stop'
}

/** Повтор всей: сколько новых кругов списка дописать, чтобы впереди было не меньше `ahead` треков
 *  (движок приложения доигрывает их и в фоне, без интерфейса) */
export function roundsNeeded(upcoming: number, listLength: number, ahead: number): number {
  if (listLength <= 0 || upcoming >= ahead) return 0
  return Math.ceil((ahead - upcoming) / listLength)
}

/** «Назад» как в Spotify: дальше 3 с от начала — трек сначала, иначе — предыдущий (если он есть) */
export function previousAction(positionMs: number, hasHistory: boolean): 'restart' | 'previous' {
  return positionMs > 3000 || !hasHistory ? 'restart' : 'previous'
}
