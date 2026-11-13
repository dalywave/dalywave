// Звук сайта: два «дека» на HTMLAudioElement + WebAudio. Громкость трека выравнивается
// усилением дека, переход — наложением двух деков, анализатор отдаёт звук волне.
// Трек без звука (поток ещё не на сервере) «играет» по часам: интерфейс ведёт себя так же.
// Эквалайзер — десять колоколов BiquadFilterNode между деками и общей громкостью (те же полосы и
// предусиление, что в движке приложений); затухание таймера сна — свой узел перед выходом.
import { EQ_BANDS_HZ, EQ_Q, activeGains, preampDb, type EqConfig } from '~/utils/eq'
import type { AudioEngine, AudioEngineEvents, AudioLevels, SleepState, TrackSource } from '../types'

type Listener<E extends keyof AudioEngineEvents> = AudioEngineEvents[E]

const dbToGain = (db: number) => Math.pow(10, db / 20)

class Deck {
  readonly el: HTMLAudioElement
  gain: GainNode | null = null
  source: TrackSource | null = null
  durationMs = 0
  // Часы для трека без звука
  private silentPos = 0
  private silentStart = 0
  private silentPlaying = false

  constructor() {
    this.el = new Audio()
    this.el.preload = 'auto'
    this.el.crossOrigin = 'anonymous'
  }

  get silent(): boolean {
    return !this.source
  }

  load(source: TrackSource | null, durationMs: number, startMs = 0) {
    this.source = source
    this.durationMs = source?.durationMs ?? durationMs
    this.silentPlaying = false
    this.silentPos = startMs
    if (source) {
      if (this.el.src !== new URL(source.url, location.href).href) this.el.src = source.url
      this.el.currentTime = startMs / 1000
    }
    else {
      this.el.removeAttribute('src')
      this.el.load()
    }
  }

  async play() {
    if (this.silent) {
      if (!this.silentPlaying) {
        this.silentStart = performance.now()
        this.silentPlaying = true
      }
      return
    }
    await this.el.play()
  }

  pause() {
    if (this.silent) {
      if (this.silentPlaying) this.silentPos = this.position()
      this.silentPlaying = false
      return
    }
    this.el.pause()
  }

  seek(ms: number) {
    if (this.silent) {
      this.silentPos = ms
      this.silentStart = performance.now()
      return
    }
    this.el.currentTime = ms / 1000
  }

  position(): number {
    if (this.silent) return Math.min(this.durationMs, this.silentPos + (this.silentPlaying ? performance.now() - this.silentStart : 0))
    return this.el.currentTime * 1000
  }

  duration(): number {
    if (!this.silent && Number.isFinite(this.el.duration) && this.el.duration > 0) return this.el.duration * 1000
    return this.durationMs
  }

  playing(): boolean {
    return this.silent ? this.silentPlaying : !this.el.paused
  }

  baseGain(): number {
    return dbToGain(this.source?.gainDb ?? 0)
  }
}

export class WebAudioEngine implements AudioEngine {
  readonly native = false
  private decks: [Deck, Deck]
  private active = 0
  private ctx: AudioContext | null = null
  private master: GainNode | null = null
  private _analyser: AnalyserNode | null = null
  private volume = 0.72
  private muted = false
  private ticker = 0
  private fading = false
  private eqIn: GainNode | null = null
  private eqBands: BiquadFilterNode[] = []
  private eqGains: number[] | null = null
  private sleepFade: GainNode | null = null
  private listeners: { [E in keyof AudioEngineEvents]: Set<Listener<E>> } = {
    time: new Set(), ended: new Set(), state: new Set(), error: new Set(), advance: new Set(), sleep: new Set(), sound: new Set(),
  }

  constructor() {
    this.decks = [new Deck(), new Deck()]
    for (const deck of this.decks) {
      deck.el.addEventListener('ended', () => {
        if (deck === this.current && !this.fading) this.emit('ended')
      })
      deck.el.addEventListener('waiting', () => deck === this.current && this.emit('state', { playing: true, buffering: true }))
      deck.el.addEventListener('playing', () => deck === this.current && this.emit('state', { playing: true, buffering: false }))
      deck.el.addEventListener('error', () => deck === this.current && deck.source && this.emit('error', deck.el.error))
    }
  }

  private get current(): Deck {
    return this.decks[this.active]!
  }

  private get other(): Deck {
    return this.decks[1 - this.active]!
  }

  get analyser(): AnalyserNode | null {
    return this._analyser
  }

  get positionMs(): number {
    return this.current.position()
  }

  get durationMs(): number {
    return this.current.duration()
  }

  /** Элементы обоих деков: окно AirPlay в Safari вызывается у элемента */
  get elements(): HTMLAudioElement[] {
    return this.decks.map(d => d.el)
  }

  /** Элемент, который звучит сейчас */
  get element(): HTMLAudioElement {
    return this.current.el
  }

  get playing(): boolean {
    return this.current.playing()
  }

  audioTime(): number {
    return this.current.position() / 1000
  }

  // Уровень берёт сам анализатор волны; очередь и переходы ведёт плеер интерфейса
  levels(): AudioLevels | null {
    return null
  }

  setUpcoming() {}

  configure() {}

  get eqSupported(): boolean {
    return typeof window !== 'undefined' && !!(window.AudioContext ?? (window as unknown as { webkitAudioContext?: unknown }).webkitAudioContext)
  }

  setEq(eq: EqConfig | null) {
    this.eqGains = activeGains(eq)
    this.applyEq()
  }

  // Сайт помнит эквалайзер в localStorage (stores/sound.ts)
  async savedEq(): Promise<EqConfig | null> {
    return null
  }

  // Таймер сна сайта ведёт интерфейс: затухание — fadeOut, пауза — плеер
  setSleep() {}

  async sleepState(): Promise<SleepState | null> {
    return null
  }

  fadeOut(ms: number | null) {
    const node = this.sleepFade
    const ctx = this.ctx
    if (!node || !ctx) return
    const t = ctx.currentTime
    node.gain.cancelScheduledValues(t)
    node.gain.setValueAtTime(node.gain.value, t)
    if (ms === null) {
      node.gain.linearRampToValueAtTime(1, t + 0.3)
      return
    }
    // Равномощная кривая, как у движка приложений: cos от текущего уровня до тишины
    const from = node.gain.value
    const curve = new Float32Array(64)
    for (let i = 0; i < curve.length; i++) curve[i] = from * Math.cos((i / (curve.length - 1)) * Math.PI / 2)
    node.gain.setValueCurveAtTime(curve, t, Math.max(0.05, ms / 1000))
  }

  /** Полосы и предусиление — плавно (~50 мс), без щелчков */
  private applyEq() {
    const ctx = this.ctx
    if (!ctx || !this.eqIn) return
    const g = this.eqGains
    const t = ctx.currentTime
    this.eqBands.forEach((b, i) => b.gain.setTargetAtTime(g?.[i] ?? 0, t, 0.05))
    const pre = g ? Math.pow(10, preampDb(g, ctx.sampleRate) / 20) : 1
    this.eqIn.gain.setTargetAtTime(pre, t, 0.05)
  }

  // Граф собирается при первом запуске: браузер разрешает звук только после жеста пользователя
  private ensureGraph() {
    if (this.ctx) return
    try {
      const Ctx = window.AudioContext ?? (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
      const ctx = new Ctx({ latencyHint: 'playback' })
      const master = ctx.createGain()
      const analyser = ctx.createAnalyser()
      analyser.fftSize = 2048
      analyser.smoothingTimeConstant = 0.55
      // Деки → предусиление → 10 полос → громкость → анализатор → затухание таймера сна → выход
      const eqIn = ctx.createGain()
      const bands = EQ_BANDS_HZ.map((hz) => {
        const b = ctx.createBiquadFilter()
        b.type = 'peaking'
        b.frequency.value = Math.min(hz, ctx.sampleRate * 0.45)
        b.Q.value = EQ_Q
        b.gain.value = 0
        return b
      })
      let tail: AudioNode = eqIn
      for (const b of bands) {
        tail.connect(b)
        tail = b
      }
      tail.connect(master)
      const sleepFade = ctx.createGain()
      master.connect(analyser)
      analyser.connect(sleepFade)
      sleepFade.connect(ctx.destination)
      for (const deck of this.decks) {
        const src = ctx.createMediaElementSource(deck.el)
        deck.gain = ctx.createGain()
        src.connect(deck.gain)
        deck.gain.connect(eqIn)
      }
      this.ctx = ctx
      this.master = master
      this._analyser = analyser
      this.eqIn = eqIn
      this.eqBands = bands
      this.sleepFade = sleepFade
      this.applyEq()
      this.applyVolume()
      this.current.gain?.gain.setValueAtTime(this.current.baseGain(), ctx.currentTime)
    }
    catch (e) {
      // Без WebAudio звук всё равно играет — через громкость элемента
      this.emit('error', e)
    }
  }

  private applyVolume() {
    const v = this.muted ? 0 : this.volume
    if (this.master && this.ctx) this.master.gain.setTargetAtTime(v, this.ctx.currentTime, 0.02)
    else for (const deck of this.decks) deck.el.volume = v
  }

  private startTicker() {
    if (this.ticker) return
    this.ticker = window.setInterval(() => {
      const deck = this.current
      this.emit('time', deck.position())
      if (deck.silent && deck.playing() && deck.position() >= deck.duration() && !this.fading) {
        deck.pause()
        this.emit('ended')
      }
    }, 100)
  }

  private stopTicker() {
    clearInterval(this.ticker)
    this.ticker = 0
  }

  load(source: TrackSource | null, opts: { durationMs: number, startMs?: number }) {
    this.other.pause()
    this.current.pause()
    this.current.load(source, opts.durationMs, opts.startMs ?? 0)
    if (this.ctx) this.current.gain?.gain.setValueAtTime(this.current.baseGain(), this.ctx.currentTime)
    this.emit('time', opts.startMs ?? 0)
  }

  async play() {
    this.ensureGraph()
    if (this.ctx?.state === 'suspended') await this.ctx.resume().catch(() => {})
    try {
      await this.current.play()
      this.startTicker()
      this.emit('state', { playing: true, buffering: false })
    }
    catch (e) {
      this.emit('state', { playing: false, buffering: false })
      this.emit('error', e)
    }
  }

  pause() {
    this.current.pause()
    this.other.pause()
    this.stopTicker()
    this.emit('time', this.current.position())
    this.emit('state', { playing: false, buffering: false })
  }

  seek(ms: number) {
    this.current.seek(ms)
    this.emit('time', ms)
  }

  setVolume(volume: number) {
    this.volume = Math.min(1, Math.max(0, volume))
    this.applyVolume()
  }

  setMuted(muted: boolean) {
    this.muted = muted
    this.applyVolume()
  }

  crossfade(next: TrackSource | null, opts: { durationMs: number, fadeMs: number, startMs?: number }) {
    const from = this.current
    const to = this.other
    to.load(next, opts.durationMs, opts.startMs ?? 0)
    const ctx = this.ctx
    const fade = Math.max(0, opts.fadeMs) / 1000
    this.fading = true
    if (ctx && from.gain && to.gain && fade > 0) {
      const t = ctx.currentTime
      to.gain.gain.cancelScheduledValues(t)
      to.gain.gain.setValueAtTime(0.0001, t)
      to.gain.gain.exponentialRampToValueAtTime(to.baseGain(), t + fade)
      from.gain.gain.cancelScheduledValues(t)
      from.gain.gain.setValueAtTime(Math.max(0.0001, from.gain.gain.value), t)
      from.gain.gain.exponentialRampToValueAtTime(0.0001, t + fade)
    }
    void to.play().catch(e => this.emit('error', e))
    this.active = 1 - this.active
    window.setTimeout(() => {
      from.pause()
      if (ctx && from.gain) from.gain.gain.setValueAtTime(from.baseGain(), ctx.currentTime)
      this.fading = false
    }, opts.fadeMs + 50)
    this.startTicker()
    this.emit('time', opts.startMs ?? 0)
  }

  on<E extends keyof AudioEngineEvents>(event: E, cb: AudioEngineEvents[E]): () => void {
    const set = this.listeners[event] as Set<AudioEngineEvents[E]>
    set.add(cb)
    return () => set.delete(cb)
  }

  private emit<E extends keyof AudioEngineEvents>(event: E, ...args: Parameters<AudioEngineEvents[E]>) {
    for (const cb of this.listeners[event] as Set<(...a: Parameters<AudioEngineEvents[E]>) => void>) cb(...args)
  }
}
