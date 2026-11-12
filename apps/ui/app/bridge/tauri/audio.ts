// Звук приложения: движок на Rust (crates/client/audio) за командами оболочки `player_*`.
// Плеер интерфейса передаёт ему текущий трек и очередь дальше: движок заранее готовит следующий,
// сводит без пауз или наложением и сам переходит к нему — в фоне, на экране блокировки, с пульта
// системы. Ссылки, которых нет или которые истекли, движок просит у сервера сам. Плееру остаются
// очередь и история: движок сообщает, к какому элементу перешёл (`advance`).
import type { EqConfig } from '~/utils/eq'
import type { AudioEngine, AudioEngineEvents, AudioLevels, EngineTrack, RepeatMode, SleepState, SleepTimer, TrackSource } from '../types'
import type { PlayerItem, PlayerSleep, PlayerTransition, Shell } from './shell'

type Listener<E extends keyof AudioEngineEvents> = AudioEngineEvents[E]

/** Уровень старше этого — волна его уже не рисует */
const LEVEL_FRESH_MS = 500
/** Волна не спрашивала уровень дольше — выключить события уровня в движке */
const LEVEL_IDLE_MS = 1500
/** Между событиями позиции (раз в 250 мс) интерфейс досчитывает её сам */
const TICK_MS = 100

export class NativeAudioEngine implements AudioEngine {
  readonly native = true
  readonly analyser = null
  private uid: string | null = null
  private pos = 0
  private posAt = 0
  private dur = 0
  private isPlaying = false
  private volume = 0.72
  private muted = false
  private level: AudioLevels | null = null
  private levelAt = 0
  private levelsOn = false
  private levelsReadAt = 0
  private ticker = 0
  private sentUpcoming = ''
  private sentConfig = ''
  // Команды уходят по одной: оболочка выполняет асинхронные команды параллельно, а «загрузить» и
  // «играть» должны прийти по порядку
  private chain: Promise<unknown> = Promise.resolve()
  private listeners: { [E in keyof AudioEngineEvents]: Set<Listener<E>> } = {
    time: new Set(), ended: new Set(), state: new Set(), error: new Set(), advance: new Set(), sleep: new Set(),
  }
  readonly eqSupported = true

  constructor(private readonly shell: () => Promise<Shell>) {
    void this.subscribe()
    window.setInterval(() => {
      if (this.levelsOn && performance.now() - this.levelsReadAt > LEVEL_IDLE_MS) this.setLevels(false)
    }, LEVEL_IDLE_MS)
  }

  private run(f: (shell: Shell) => Promise<unknown>) {
    this.chain = this.chain
      .then(() => this.shell())
      .then(f)
      .catch(e => this.emit('error', e))
  }

  private async subscribe() {
    const sh = await this.shell()
    await sh.on('player:state', ({ state, buffering }) => {
      const playing = state === 'playing' || (state === 'loading' && this.isPlaying)
      this.anchor()
      this.isPlaying = playing
      this.syncTicker()
      this.emit('state', { playing, buffering: buffering || state === 'loading' })
    })
    await sh.on('player:buffering', ({ buffering }) => this.emit('state', { playing: this.isPlaying, buffering }))
    await sh.on('player:position', ({ id, positionMs, durationMs }) => {
      if (id !== this.uid) return
      this.pos = positionMs
      this.posAt = performance.now()
      if (durationMs) this.dur = durationMs
      this.emit('time', positionMs)
    })
    await sh.on('player:track', ({ id, reason, durationMs }) => {
      if (id === this.uid) return
      this.uid = id
      this.pos = 0
      this.posAt = performance.now()
      if (durationMs) this.dur = durationMs
      this.emit('advance', id, reason)
    })
    await sh.on('player:error', ({ id, message }) => this.emit('error', new Error(message), id ?? undefined))
    await sh.on('player:ended', () => {
      this.anchor()
      this.isPlaying = false
      this.syncTicker()
      this.emit('ended')
    })
    await sh.on('player:level', (l) => {
      this.level = l
      this.levelAt = performance.now()
    })
    await sh.on('player:sleep', s => this.emit('sleep', toSleep(s)))
  }

  private emit<E extends keyof AudioEngineEvents>(event: E, ...args: Parameters<AudioEngineEvents[E]>) {
    for (const cb of this.listeners[event] as Set<(...a: Parameters<AudioEngineEvents[E]>) => void>) cb(...args)
  }

  /** Запомнить досчитанную позицию: дальше она растёт от этого места */
  private anchor() {
    this.pos = this.positionMs
    this.posAt = performance.now()
  }

  // Ползунок двигается плавно, как на сайте: между событиями движка позицию досчитываем сами
  private syncTicker() {
    if (this.isPlaying && !this.ticker) {
      this.ticker = window.setInterval(() => this.emit('time', this.positionMs), TICK_MS)
    }
    else if (!this.isPlaying && this.ticker) {
      clearInterval(this.ticker)
      this.ticker = 0
    }
  }

  get positionMs(): number {
    const p = this.isPlaying ? this.pos + (performance.now() - this.posAt) : this.pos
    return this.dur ? Math.min(p, this.dur) : p
  }

  get durationMs(): number {
    return this.dur
  }

  get playing(): boolean {
    return this.isPlaying
  }

  audioTime(): number {
    return this.positionMs / 1000
  }

  levels(): AudioLevels | null {
    this.levelsReadAt = performance.now()
    if (!this.levelsOn) this.setLevels(true)
    return this.level && performance.now() - this.levelAt < LEVEL_FRESH_MS ? this.level : null
  }

  private setLevels(on: boolean) {
    this.levelsOn = on
    if (!on) this.level = null
    this.run(sh => sh.player.setLevels(on))
  }

  private item(track: EngineTrack, source: TrackSource | null): PlayerItem {
    return {
      id: track.uid,
      trackId: track.trackId,
      // Движок ходит в сеть сам: относительный адрес (разработка, мок) — от адреса страницы
      url: source?.url ? new URL(source.url, location.href).href : null,
      loudnessLufs: source?.loudnessLufs ?? null,
      truePeakDb: source?.truePeakDb ?? null,
      durationMs: source?.durationMs ?? track.durationMs,
      title: track.title,
      artist: track.artist,
      album: track.album,
      artworkUrl: track.artworkUrl,
    }
  }

  load(source: TrackSource | null, opts: { durationMs: number, startMs?: number, track?: EngineTrack }) {
    const track = opts.track
    this.sentUpcoming = ''
    this.pos = opts.startMs ?? 0
    this.posAt = performance.now()
    this.dur = source?.durationMs ?? opts.durationMs
    this.isPlaying = false
    this.syncTicker()
    if (!track) {
      // Играть нечего (трек ещё скачивается на сервер): движок останавливаем
      this.uid = null
      this.run(sh => sh.player.stop())
      return
    }
    this.uid = track.uid
    const item = this.item(track, source)
    this.run(sh => sh.player.load([item], { startMs: opts.startMs ?? 0, autoplay: false }))
    this.emit('time', this.pos)
  }

  async play() {
    this.anchor()
    this.isPlaying = true
    this.syncTicker()
    this.run(sh => sh.player.play())
  }

  pause() {
    this.anchor()
    this.isPlaying = false
    this.syncTicker()
    this.run(sh => sh.player.pause())
    this.emit('time', this.pos)
  }

  seek(ms: number) {
    const to = Math.max(0, ms)
    this.pos = to
    this.posAt = performance.now()
    this.run(sh => sh.player.seek(to))
    this.emit('time', to)
  }

  setVolume(volume: number) {
    this.volume = Math.min(1, Math.max(0, volume))
    this.applyVolume()
  }

  setMuted(muted: boolean) {
    this.muted = muted
    this.applyVolume()
  }

  private applyVolume() {
    const v = this.muted ? 0 : this.volume
    this.run(sh => sh.player.setVolume(v))
  }

  // Наложение движок делает сам по очереди; сюда попадают только треки без очереди
  crossfade(next: TrackSource | null, opts: { durationMs: number, fadeMs: number, startMs?: number }) {
    this.load(next, { durationMs: opts.durationMs, startMs: opts.startMs })
    void this.play()
  }

  setUpcoming(tracks: EngineTrack[]) {
    if (!this.uid) return
    const items = tracks.map(t => this.item(t, t.source))
    const key = JSON.stringify(items.map(i => [i.id, i.url]))
    if (key === this.sentUpcoming) return
    this.sentUpcoming = key
    this.run(sh => sh.player.setUpcoming(items))
  }

  configure(opts: { transition: 'gapless' | 'crossfade' | 'dj', fadeMs: number, normalize: boolean, expressiveness?: number }) {
    const transition: PlayerTransition = opts.transition === 'gapless'
      ? { mode: 'gapless' }
      : opts.transition === 'dj'
        ? { mode: 'dj', expressiveness: opts.expressiveness ?? 0.6 }
        : { mode: 'crossfade', ms: opts.fadeMs }
    const key = JSON.stringify([transition, opts.normalize])
    if (key === this.sentConfig) return
    this.sentConfig = key
    this.run(async (sh) => {
      await sh.player.setTransition(transition)
      await sh.player.setNormalize(opts.normalize)
    })
  }

  // Эквалайзер и таймер сна — в движке: оболочка помнит эквалайзер этого устройства и включает
  // его с запуска, таймер сна работает и в фоне, когда WebView усыплён
  setEq(eq: EqConfig | null) {
    this.run(sh => sh.player.setEq(eq ? { preset: eq.preset, gains: eq.gains } : null))
  }

  async savedEq(): Promise<EqConfig | null> {
    const eq = await (await this.shell()).player.eq().catch(() => null)
    return eq ? { preset: eq.preset, gains: eq.gains } : null
  }

  setSleep(timer: SleepTimer | null) {
    this.run(sh => sh.player.setSleep(timer))
  }

  async sleepState(): Promise<SleepState | null> {
    const s = await (await this.shell()).player.sleep().catch(() => null)
    return s ? toSleep(s) : null
  }

  // Затухание таймера сна делает движок
  fadeOut() {}

  setModes(shuffle: boolean, repeat: RepeatMode) {
    this.run(sh => sh.player.setModes(shuffle, repeat))
  }

  on<E extends keyof AudioEngineEvents>(event: E, cb: AudioEngineEvents[E]): () => void {
    const set = this.listeners[event] as Set<AudioEngineEvents[E]>
    set.add(cb)
    return () => set.delete(cb)
  }
}

function toSleep(s: PlayerSleep): SleepState {
  return { timer: s.timer, endsAtMs: s.endsAtMs, fading: s.fading, finished: s.finished }
}
