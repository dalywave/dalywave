// Пульт: что играет и где, команды активному устройству, «нравится», плейлисты, сетка долей.
// Звук играет на устройстве человека (сайт, Mac, Windows, iPhone) — плагин только управляет.
import { EventEmitter } from 'node:events'
import { ApiError, api } from './api'
import { Link } from './link'
import type {
  ConnectDevice, PlayContext, PlaybackState, PlaylistDetail, QueueItem, RemoteCommand, RepeatMode,
  Track, TrackAnalysis, TransitionMode, WaveSettings,
} from './types'

export const TRANSITIONS: TransitionMode[] = ['gapless', 'smooth', 'smart', 'dj']
export const REPEATS: RepeatMode[] = ['off', 'all', 'one']

/** Не больше стольких треков в одной команде «играть список» */
const MAX_TRACKS = 300
/** Своё значение громкости держим столько, пока снимок не догонит */
const OPTIMISTIC_MS = 2500

export type Connection = 'unpaired' | 'connecting' | 'offline' | 'online'

export interface PlayerEvents {
  change: []
  /** Раз в секунду, пока играет: прогресс и таймеры */
  tick: []
}

export class Player extends EventEmitter<PlayerEvents> {
  readonly link = new Link()
  paired = false
  private likedCache = new Map<string, boolean>()
  private playlistCache = new Map<string, Promise<PlaylistDetail | null>>()
  private playlistTracks = new Map<string, Map<string, string[]>>()
  private playlistTitles = new Map<string, string>()
  private beatCache = new Map<string, Promise<number[]>>()
  private localVolume: { value: number, at: number } | null = null
  private localMuted: { value: boolean, at: number } | null = null
  private localFlags: { shuffle?: boolean, repeat?: RepeatMode, transition?: TransitionMode, at: number } | null = null
  private scheduled = false
  private ticker: NodeJS.Timeout | null = null
  private lastTrackId: string | null = null

  constructor() {
    super()
    this.setMaxListeners(200)
    this.link.on('status', () => this.changed())
    this.link.on('devices', () => this.changed())
    this.link.on('state', () => this.onState())
    this.link.on('library', entities => this.onLibrary(entities))
    this.ticker = setInterval(() => this.emit('tick'), 1000)
    this.ticker.unref?.()
  }

  // ---------- состояние ----------

  get connection(): Connection {
    if (!this.paired) return 'unpaired'
    const s = this.link.status
    return s === 'online' ? 'online' : s === 'offline' ? 'offline' : 'connecting'
  }

  get state(): PlaybackState | null {
    return this.link.state
  }

  get current(): QueueItem | null {
    return this.link.state?.current ?? null
  }

  get track(): Track | null {
    return this.current?.track ?? null
  }

  get playing(): boolean {
    return !!this.link.state?.playing && !!this.current
  }

  get devices(): ConnectDevice[] {
    return this.link.devices
  }

  /** Устройство, где играет звук, если оно на связи */
  get activeDevice(): ConnectDevice | null {
    const d = this.link.active
    return d?.online ? d : null
  }

  /** Куда слать команды: активное устройство или, если оно пропало, недавнее на связи */
  get target(): ConnectDevice | null {
    return this.activeDevice ?? this.fallbackDevice()
  }

  private fallbackDevice(): ConnectDevice | null {
    const own = this.link.devices.filter(d => d.online && !d.capabilities.remote)
    own.sort((a, b) => Date.parse(b.lastSeenAt) - Date.parse(a.lastSeenAt))
    return own[0] ?? null
  }

  position(): number {
    return this.link.position()
  }

  /** 0…1 */
  progress(): number {
    const d = this.link.state?.durationMs ?? 0
    return d > 0 ? Math.min(1, this.position() / d) : 0
  }

  get volume(): number {
    if (this.localVolume && Date.now() - this.localVolume.at < OPTIMISTIC_MS) return this.localVolume.value
    return this.link.state?.volume ?? 1
  }

  get muted(): boolean {
    if (this.localMuted && Date.now() - this.localMuted.at < OPTIMISTIC_MS) return this.localMuted.value
    return !!this.link.state?.muted
  }

  private flag<K extends 'shuffle' | 'repeat' | 'transition'>(key: K): PlaybackState[K] | undefined {
    const local = this.localFlags
    if (local && Date.now() - local.at < OPTIMISTIC_MS && local[key] !== undefined) return local[key] as PlaybackState[K]
    return this.link.state?.[key]
  }

  get shuffle(): boolean {
    return !!this.flag('shuffle')
  }

  get repeat(): RepeatMode {
    return this.flag('repeat') ?? 'off'
  }

  get transition(): TransitionMode {
    return this.flag('transition') ?? 'smart'
  }

  /** Очередь после текущего: добавленное вручную, затем остальное */
  upcoming(): QueueItem[] {
    const s = this.link.state
    return s ? [...(s.manual ?? []), ...(s.upcoming ?? [])] : []
  }

  /** Нравится ли текущий трек; null — ещё не знаем */
  liked(track: Track | null = this.track): boolean | null {
    if (!track) return null
    return this.likedCache.get(track.id) ?? track.liked
  }

  /** Есть ли трек в плейлисте; null — ещё не загрузили */
  inPlaylist(playlistId: string, track: Track | null = this.track): boolean | null {
    if (!track) return null
    const items = this.playlistTracks.get(playlistId)
    if (!items) {
      void this.loadPlaylist(playlistId)
      return null
    }
    return items.has(track.id)
  }

  /** Название плейлиста для подписи кнопки; пока не загрузили — null */
  playlistTitle(id: string): string | null {
    const title = this.playlistTitles.get(id)
    if (title === undefined) void this.loadPlaylist(id)
    return title ?? null
  }

  // ---------- события ----------

  changed(): void {
    if (this.scheduled) return
    this.scheduled = true
    setImmediate(() => {
      this.scheduled = false
      this.emit('change')
    })
  }

  private onState(): void {
    const id = this.track?.id ?? null
    if (id !== this.lastTrackId) {
      this.lastTrackId = id
      // «Нравится» в снимке мог устареть: спросим сервер
      if (id) void this.refreshLiked(id)
    }
    this.changed()
  }

  private onLibrary(entities: string[]): void {
    if (entities.includes('likedTracks') && this.track) void this.refreshLiked(this.track.id)
    if (entities.includes('playlists') || entities.includes('playlistItems')) {
      const ids = [...this.playlistTracks.keys()]
      this.playlistCache.clear()
      this.playlistTracks.clear()
      for (const id of ids) void this.loadPlaylist(id)
    }
    this.changed()
  }

  private async refreshLiked(id: string): Promise<void> {
    try {
      const t = await api.get<Track>(`/api/tracks/${id}`)
      this.likedCache.set(id, t.liked)
      this.changed()
    }
    catch {
      // без сети остаётся значение из снимка
    }
  }

  loadPlaylist(id: string): Promise<PlaylistDetail | null> {
    const hit = this.playlistCache.get(id)
    if (hit) return hit
    const p = api.get<PlaylistDetail>(`/api/playlists/${id}`).then((d) => {
      const map = new Map<string, string[]>()
      for (const it of d.items) map.set(it.track.id, [...(map.get(it.track.id) ?? []), it.id])
      this.playlistTracks.set(id, map)
      this.playlistTitles.set(id, d.playlist.title)
      this.changed()
      return d
    }).catch(() => {
      this.playlistCache.delete(id)
      return null
    })
    this.playlistCache.set(id, p)
    return p
  }

  /** Сетка долей трека, мс; нет анализа — пусто */
  beats(trackId: string): Promise<number[]> {
    const hit = this.beatCache.get(trackId)
    if (hit) return hit
    const p = api.get<TrackAnalysis>(`/api/tracks/${trackId}/analysis`)
      .then(a => ((a.bpmConfidence ?? 1) >= 0.3 ? a.beatsMs : []))
      .catch(() => [])
    this.beatCache.set(trackId, p)
    if (this.beatCache.size > 50) {
      const first = this.beatCache.keys().next().value
      if (first !== undefined) this.beatCache.delete(first)
    }
    return p
  }

  // ---------- команды ----------

  /** Команда устройству; false — не дошла (нет устройства, не ответило) */
  async send(command: RemoteCommand): Promise<boolean> {
    const to = this.target
    if (!to) return false
    const ack = await this.link.command(to.id, command)
    return ack.ok
  }

  async toggle(): Promise<boolean> {
    return this.playing ? this.pause() : this.play()
  }

  async play(): Promise<boolean> {
    // Активное устройство пропало, а снимок остался — продолжить на устройстве рядом
    if (!this.activeDevice && this.current) {
      const to = this.fallbackDevice()
      if (!to) return false
      return (await this.link.transfer(to.id, true)).ok
    }
    return this.send({ action: 'play' })
  }

  pause(): Promise<boolean> {
    return this.send({ action: 'pause' })
  }

  next(): Promise<boolean> {
    return this.send({ action: 'next' })
  }

  previous(): Promise<boolean> {
    return this.send({ action: 'previous' })
  }

  seekTo(ms: number): Promise<boolean> {
    const d = this.link.state?.durationMs ?? 0
    const positionMs = Math.round(Math.max(0, d > 0 ? Math.min(d - 500, ms) : ms))
    return this.send({ action: 'seek', positionMs })
  }

  seekBy(deltaMs: number): Promise<boolean> {
    return this.seekTo(this.position() + deltaMs)
  }

  /** Громкость 0…1; значение сразу видно на кнопках, снимок догонит */
  setVolume(v: number): Promise<boolean> {
    const volume = Math.round(Math.min(1, Math.max(0, v)) * 100) / 100
    this.localVolume = { value: volume, at: Date.now() }
    if (this.muted && volume > 0) this.localMuted = { value: false, at: Date.now() }
    this.changed()
    return this.send({ action: 'setVolume', volume })
  }

  adjustVolume(delta: number): Promise<boolean> {
    return this.setVolume(this.volume + delta)
  }

  setMuted(muted: boolean): Promise<boolean> {
    this.localMuted = { value: muted, at: Date.now() }
    this.changed()
    return this.send({ action: 'setMuted', muted })
  }

  setShuffle(shuffle: boolean): Promise<boolean> {
    this.localFlags = { ...this.localFlags, shuffle, at: Date.now() }
    this.changed()
    return this.send({ action: 'setShuffle', shuffle })
  }

  setRepeat(mode: RepeatMode): Promise<boolean> {
    this.localFlags = { ...this.localFlags, repeat: mode, at: Date.now() }
    this.changed()
    return this.send({ action: 'setRepeat', mode })
  }

  cycleRepeat(): Promise<boolean> {
    return this.setRepeat(REPEATS[(REPEATS.indexOf(this.repeat) + 1) % REPEATS.length] ?? 'off')
  }

  setTransition(mode: TransitionMode): Promise<boolean> {
    this.localFlags = { ...this.localFlags, transition: mode, at: Date.now() }
    this.changed()
    return this.send({ action: 'setTransition', mode })
  }

  /** Режимы переходов по кругу; «Диджей» — только где он есть */
  cycleTransition(): Promise<boolean> {
    const dj = this.target?.capabilities.dj ?? false
    const modes = TRANSITIONS.filter(m => m !== 'dj' || dj)
    const next = modes[(modes.indexOf(this.transition) + 1) % modes.length] ?? 'smart'
    return this.setTransition(next)
  }

  dislike(): Promise<boolean> {
    return this.send({ action: 'dislike' })
  }

  notNow(): Promise<boolean> {
    return this.send({ action: 'notNow' })
  }

  jumpTo(uid: string): Promise<boolean> {
    return this.send({ action: 'jumpTo', uid })
  }

  playTracks(tracks: Track[], index: number, context: PlayContext, shuffle = false): Promise<boolean> {
    if (!tracks.length) return Promise.resolve(false)
    // Окно вокруг выбранного трека, если список длиннее, чем стоит слать
    let start = 0
    if (tracks.length > MAX_TRACKS) start = Math.max(0, Math.min(index - 20, tracks.length - MAX_TRACKS))
    const slice = tracks.slice(start, start + MAX_TRACKS)
    return this.send({
      action: 'playTracks',
      trackIds: slice.map(t => t.id),
      index: Math.max(0, index - start),
      context,
      shuffle,
    })
  }

  startWave(settings?: WaveSettings): Promise<boolean> {
    return this.send({ action: 'startWave', settings: settings ?? null })
  }

  async transfer(deviceId: string): Promise<boolean> {
    if (deviceId === this.link.activeId) return true
    return (await this.link.transfer(deviceId, this.playing || undefined)).ok
  }

  // ---------- медиатека ----------

  async setLiked(track: Track, liked: boolean): Promise<boolean> {
    const before = this.liked(track)
    this.likedCache.set(track.id, liked)
    this.changed()
    try {
      if (liked) await api.put(`/api/library/tracks/${track.id}`)
      else await api.del(`/api/library/tracks/${track.id}`)
      return true
    }
    catch {
      if (before !== null) this.likedCache.set(track.id, before)
      else this.likedCache.delete(track.id)
      this.changed()
      return false
    }
  }

  async toggleLiked(): Promise<boolean> {
    const track = this.track
    if (!track) return false
    return this.setLiked(track, !this.liked(track))
  }

  /** Добавить текущий трек в плейлист или убрать из него */
  async togglePlaylist(playlistId: string): Promise<boolean> {
    const track = this.track
    if (!track) return false
    await this.loadPlaylist(playlistId)
    const items = this.playlistTracks.get(playlistId)
    if (!items) return false
    const present = items.get(track.id)
    try {
      if (present?.length) {
        await api.post(`/api/playlists/${playlistId}/items/remove`, { itemIds: present })
        items.delete(track.id)
      }
      else {
        const added = await api.post<{ items?: { id: string }[] }>(`/api/playlists/${playlistId}/items`, { trackIds: [track.id] })
        items.set(track.id, added?.items?.map(i => i.id) ?? [])
      }
      this.changed()
      return true
    }
    catch (e) {
      if (e instanceof ApiError && e.status === 404) this.playlistTracks.delete(playlistId)
      return false
    }
  }

  stop(): void {
    if (this.ticker) clearInterval(this.ticker)
    this.link.close()
  }
}

export const player = new Player()
