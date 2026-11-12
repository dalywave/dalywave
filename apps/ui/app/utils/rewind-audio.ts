// Музыка историй «Итогов года»: у слайда свой трек с припева (сервер — /rewind/{год}/soundtrack),
// между слайдами — наложение за 1,5 с, следующий трек готовится заранее, на паузе звук приглушается.
// Играет через плеер в режиме превью (stores/player.ts): без истории и скробблов, очередь человека не
// трогается, после закрытия плеер продолжает своё. На сайте наложение делают два дека движка, в
// приложении движок двух треков сразу не держит — громкость уходит вниз, трек меняется, громкость
// возвращается. Здесь только расписание — без Vue и без звука, чтобы его можно было проверить тестами.
import type { Track } from '~/api/types'
import type { SourceResult } from '~/bridge/types'

/** Наложение между слайдами */
export const REWIND_FADE_MS = 1500
/** Вступление первого трека */
export const REWIND_FADE_IN_MS = 900
/** Громкость на паузе историй (удержание пальцем) */
export const REWIND_DUCK = 0.3
/** Смена приглушения */
export const REWIND_DUCK_MS = 250
/** Шаг плавной громкости */
export const REWIND_STEP_MS = 30

export interface RewindCueLike {
  slide: string
  track: Track
  startMs: number
}

/**
 * Звук каждого слайда: свой трек или трек прежнего слайда — он продолжает играть. Первые слайды без
 * своего трека берут первый трек списка, чтобы музыка начиналась сразу.
 */
export function rewindTimeline<C extends RewindCueLike>(slides: string[], cues: C[]): (C | null)[] {
  const bySlide = new Map(cues.map(c => [c.slide, c]))
  let prev: C | null = cues[0] ?? null
  return slides.map((s) => {
    prev = bySlide.get(s) ?? prev
    return prev
  })
}

export type RewindStep = 'keep' | 'start' | 'switch'

/** Что сделать со звуком на слайде: оставить звучащее, начать с тишины или сменить трек */
export function rewindStep(playingTrackId: string | null, next: RewindCueLike | null): RewindStep {
  if (!next || next.track.id === playingTrackId) return 'keep'
  return playingTrackId ? 'switch' : 'start'
}

/** Что подготовить заранее: ближайший следующий слайд с другим треком */
export function rewindPreload<C extends RewindCueLike>(timeline: (C | null)[], index: number, playingTrackId: string | null): C | null {
  const here = timeline[index]?.track.id ?? playingTrackId
  for (let i = index + 1; i < timeline.length; i++) {
    const c = timeline[i]
    if (c && c.track.id !== here) return c
  }
  return null
}

/** Громкость историй поверх общей */
export function rewindGain(s: { muted: boolean, ducked: boolean }): number {
  if (s.muted) return 0
  return s.ducked ? REWIND_DUCK : 1
}

/** Значения громкости по шагам от `from` к `to`: равномощная кривая, без щелчков */
export function gainRamp(from: number, to: number, ms: number, stepMs = REWIND_STEP_MS): number[] {
  const n = Math.max(1, Math.round(ms / stepMs))
  const out: number[] = []
  for (let i = 1; i <= n; i++) {
    const k = i / n
    // Вверх — быстрее в начале (sin), вниз — медленнее в начале (cos): сумма мощностей ровная
    const shape = to >= from ? Math.sin((k * Math.PI) / 2) : 1 - Math.cos((k * Math.PI) / 2)
    out.push(i === n ? to : from + (to - from) * shape)
  }
  return out
}

/**
 * Слайд, чей трек уже на сервере, ближайший к `index`: сначала вперёд, потом назад. Нужен, когда
 * свой трек слайда ещё качается, а не звучит ничего — музыка начинается сразу
 */
export function rewindNearestReady<C extends RewindCueLike>(timeline: (C | null)[], index: number, ready: (id: string) => boolean): C | null {
  for (let i = index; i < timeline.length; i++) {
    const c = timeline[i]
    if (c && ready(c.track.id)) return c
  }
  for (let i = index - 1; i >= 0; i--) {
    const c = timeline[i]
    if (c && ready(c.track.id)) return c
  }
  return null
}

/** Приложение: наложения нет — громкость вниз за 40 % времени, вверх за остальное */
export function rewindDip(fadeMs = REWIND_FADE_MS): { outMs: number, inMs: number } {
  const outMs = Math.round(fadeMs * 0.4)
  return { outMs, inMs: fadeMs - outMs }
}

/** Что контроллеру нужно от плеера и окружения: в тестах — заглушки */
export interface RewindAudioPort {
  /** Движок приложения: два трека сразу не звучат */
  readonly native: boolean
  resolve(track: Track): Promise<SourceResult>
  /** Начать превью (или сменить звучащее; `fadeMs` — наложением) */
  start(track: Track, startMs: number, opts: { fadeMs: number, resolved: SourceResult }): Promise<boolean>
  pause(): void
  resume(): void
  /** Громкость превью поверх общей, 0…1 */
  setGain(gain: number): void
  /** Истории закрыты: плеер продолжает своё (`resume: false` — человек включает другое) */
  end(resume: boolean): void
  /** Сайт: начать подгружать файл заранее */
  warm?(url: string, startMs: number): void
  schedule(fn: () => void, ms: number): unknown
  cancel(handle: unknown): void
}

export class RewindAudio {
  private timeline: (RewindCueLike | null)[] = []
  private index = -1
  /** Что звучит (или начинает звучать) */
  private playing: RewindCueLike | null = null
  private gain = 0
  private muted = false
  private ducked = false
  private hidden = false
  private closed = false
  /** Номер смены трека: устаревшие ответы сервера не перебивают новый слайд */
  private run = 0
  private ramp: { handle: unknown, done: () => void } | null = null
  private sources = new Map<string, Promise<SourceResult>>()
  /** Треки, которые уже на сервере: остальные не спрашиваются — сервер сам скажет, когда докачает */
  private ready = new Set<string>()

  constructor(private readonly port: RewindAudioPort) {}

  setCues(slides: string[], cues: RewindCueLike[]) {
    this.timeline = rewindTimeline(slides, cues)
    for (const c of cues) {
      if (c.track.audioStatus === 'ready') this.ready.add(c.track.id)
    }
  }

  /** Трек, который звучит */
  get current(): Track | null {
    return this.playing?.track ?? null
  }

  get isMuted(): boolean {
    return this.muted
  }

  /** Слайд `index` на экране */
  async show(index: number): Promise<void> {
    if (this.closed) return
    this.index = index
    let next = this.timeline[index] ?? null
    // Свой трек ещё качается, а не звучит ничего: пока — ближайший готовый
    if (!this.playing && next && !this.ready.has(next.track.id)) next = rewindNearestReady(this.timeline, index, id => this.ready.has(id))
    const step = rewindStep(this.playing?.track.id ?? null, next)
    this.preload()
    if (step === 'keep' || !next) return
    const run = ++this.run
    const res = await this.source(next.track)
    if (run !== this.run || this.closed) return
    // Трек ещё качается на сервер: звучит прежний, а этот вступит, когда будет готов (trackReady)
    if (res.kind !== 'ready' || !res.source) return
    if (!this.playing) {
      this.playing = next
      this.setGain(0)
      await this.port.start(next.track, next.startMs, { fadeMs: 0, resolved: res })
      if (run !== this.run || this.closed) return
      if (this.hidden) this.port.pause()
      await this.fadeTo(this.target(), REWIND_FADE_IN_MS)
      return
    }
    if (this.port.native) {
      const { outMs, inMs } = rewindDip()
      await this.fadeTo(0, outMs)
      if (run !== this.run || this.closed) return
      this.playing = next
      await this.port.start(next.track, next.startMs, { fadeMs: 0, resolved: res })
      if (run !== this.run || this.closed) return
      if (this.hidden) this.port.pause()
      await this.fadeTo(this.target(), inMs)
      return
    }
    this.playing = next
    await this.port.start(next.track, next.startMs, { fadeMs: REWIND_FADE_MS, resolved: res })
  }

  /** Сервер докачал трек: если его слайд на экране, а звучит другой — вступает */
  trackReady(trackId: string) {
    this.ready.add(trackId)
    this.sources.delete(trackId)
    const want = this.timeline[this.index]
    if (!this.closed && want?.track.id === trackId && this.playing?.track.id !== trackId) void this.show(this.index)
  }

  setMuted(muted: boolean) {
    this.muted = muted
    void this.fadeTo(this.target(), REWIND_DUCK_MS)
  }

  /** Истории на паузе (удержание): звук приглушается, но не обрывается */
  setDucked(ducked: boolean) {
    if (this.ducked === ducked) return
    this.ducked = ducked
    void this.fadeTo(this.target(), REWIND_DUCK_MS)
  }

  /** Страница скрыта: звук на паузе */
  setHidden(hidden: boolean) {
    if (this.hidden === hidden) return
    this.hidden = hidden
    if (!this.playing) return
    if (hidden) this.port.pause()
    else this.port.resume()
  }

  close(resume = true) {
    if (this.closed) return
    this.closed = true
    this.run++
    this.stopRamp()
    if (this.playing) this.port.end(resume)
    this.playing = null
  }

  private target(): number {
    return rewindGain({ muted: this.muted, ducked: this.ducked })
  }

  private source(track: Track): Promise<SourceResult> {
    if (!this.ready.has(track.id)) return Promise.resolve({ kind: 'pending', status: 'acquiring', acquisitionId: null, queuePosition: null })
    let p = this.sources.get(track.id)
    if (!p) {
      p = this.port.resolve(track).catch((): SourceResult => ({ kind: 'unavailable' }))
      this.sources.set(track.id, p)
      // Не готовое спрашивается снова: сервер мог докачать
      void p.then((r) => {
        if (r.kind !== 'ready') this.sources.delete(track.id)
      })
    }
    return p
  }

  private preload() {
    const next = rewindPreload(this.timeline, this.index, this.playing?.track.id ?? null)
    if (!next) return
    void this.source(next.track).then((r) => {
      if (!this.closed && r.kind === 'ready' && r.source) this.port.warm?.(r.source.url, next.startMs)
    })
  }

  private setGain(g: number) {
    this.gain = g
    this.port.setGain(g)
  }

  private stopRamp() {
    if (!this.ramp) return
    this.port.cancel(this.ramp.handle)
    const done = this.ramp.done
    this.ramp = null
    done()
  }

  /** Плавно к громкости `to`; новая смена громкости прерывает прежнюю (её обещание завершается) */
  private fadeTo(to: number, ms: number): Promise<void> {
    this.stopRamp()
    const steps = gainRamp(this.gain, to, ms)
    return new Promise((resolve) => {
      let i = 0
      const tick = () => {
        this.setGain(steps[i]!)
        i++
        if (i >= steps.length) {
          this.ramp = null
          resolve()
          return
        }
        this.ramp = { handle: this.port.schedule(tick, REWIND_STEP_MS), done: resolve }
      }
      this.ramp = { handle: this.port.schedule(tick, REWIND_STEP_MS), done: resolve }
    })
  }
}
