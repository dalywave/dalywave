// «Пульс в такт»: «Сейчас играет» и полоса мягко вспыхивают на долю трека — по сетке долей из анализа
// (`/api/tracks/{id}/analysis`, та же, что у «Почувствуй бит»). Сетки нет — пульса нет.
// Stream Deck не любит частые картинки: вспышка — два кадра, и не чаще ~2,5 раза в секунду
// (на быстром треке — через долю).
import { EventEmitter } from 'node:events'
import { player } from './player'

/** Ближе этого доли не показываем: 150 BPM и быстрее — через одну */
const MIN_GAP_MS = 400
/** Сколько длится вспышка */
export const FLASH_MS = 160

/** Следующая доля после `pos` с учётом минимального промежутка от предыдущей показанной */
export function nextBeat(beats: number[], pos: number, lastShown: number | null): number | null {
  // Двоичный поиск первой доли после позиции
  let lo = 0
  let hi = beats.length
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if ((beats[mid] ?? 0) <= pos) lo = mid + 1
    else hi = mid
  }
  for (let i = lo; i < beats.length; i++) {
    const b = beats[i] ?? 0
    if (lastShown === null || b - lastShown >= MIN_GAP_MS) return b
  }
  return null
}

export interface PulseEvents {
  /** true — вспышка началась, false — погасла */
  beat: [boolean]
}

class Pulse extends EventEmitter<PulseEvents> {
  private users = 0
  private timer: NodeJS.Timeout | null = null
  private offTimer: NodeJS.Timeout | null = null
  private beats: number[] = []
  private trackId: string | null = null
  private lastShown: number | null = null
  private generation = 0

  constructor() {
    super()
    this.setMaxListeners(100)
    player.on('change', () => this.resync())
  }

  /** Сколько экземпляров хотят пульс; без них таймеры не крутятся */
  use(on: boolean): void {
    this.users = Math.max(0, this.users + (on ? 1 : -1))
    this.resync()
  }

  get active(): boolean {
    return this.users > 0 && player.playing && this.beats.length > 0
  }

  private async resync(): Promise<void> {
    const gen = ++this.generation
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    if (!this.users || !player.playing || !player.track) return
    const id = player.track.id
    if (id !== this.trackId) {
      this.trackId = id
      this.beats = []
      this.lastShown = null
      this.beats = await player.beats(id)
      if (gen !== this.generation) return
    }
    this.schedule()
  }

  private schedule(): void {
    if (!this.beats.length || !player.playing) return
    const pos = player.position()
    const beat = nextBeat(this.beats, pos, this.lastShown !== null && this.lastShown < pos ? this.lastShown : null)
    if (beat === null) return
    const delay = Math.max(0, beat - pos)
    this.timer = setTimeout(() => {
      this.timer = null
      this.lastShown = beat
      this.emit('beat', true)
      if (this.offTimer) clearTimeout(this.offTimer)
      this.offTimer = setTimeout(() => this.emit('beat', false), FLASH_MS)
      this.schedule()
    }, delay)
  }
}

export const pulse = new Pulse()
