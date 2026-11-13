// Связь с сервером по WebSocket `/api/ws`: плагин — ещё одно устройство-пульт в «Управлении с другого
// устройства» (`connect.hello` с возможностью `remote`, как часы). Получает список устройств и снимки
// активного, шлёт команды и ждёт подтверждения `connect.ack`.
import { EventEmitter } from 'node:events'
import WebSocket from 'ws'
import type { ConnectAck, ConnectDevice, PlaybackState, RemoteCommand } from './types'

/** Как часто напоминать о себе: без этого через 15 с сервер считает пульт пропавшим */
const HEARTBEAT_MS = 5000
/** Сколько ждать подтверждения команды (сервер ждёт устройство до 4 с) */
const ACK_MS = 5000
const BACKOFF_MS = [1000, 2000, 5000, 10_000, 30_000]

export type LinkStatus = 'idle' | 'connecting' | 'online' | 'offline'

export interface LinkOptions {
  server: string
  token: string
  deviceId: string
  name: string
  version: string
}

interface Pending {
  resolve: (ack: ConnectAck) => void
  timer: NodeJS.Timeout
}

type ServerMessage =
  | { type: 'hello', serverTime: string }
  | { type: 'connect.devices', devices: ConnectDevice[], activeDeviceId: string | null }
  | { type: 'connect.state', deviceId: string, state: PlaybackState }
  | { type: 'connect.active', deviceId: string | null, state: PlaybackState | null }
  | { type: 'connect.ack', id: string, ok: boolean, error?: string | null }
  | { type: 'library.changed', entities: string[] }
  | { type: 'resync' }
  | { type: string }

export interface LinkEvents {
  status: [LinkStatus]
  devices: []
  state: []
  library: [string[]]
  unauthorized: []
}

export class Link extends EventEmitter<LinkEvents> {
  status: LinkStatus = 'idle'
  devices: ConnectDevice[] = []
  activeId: string | null = null
  state: PlaybackState | null = null
  /** Часы сервера минус часы компьютера, мс */
  offset = 0
  private ws: WebSocket | null = null
  private opts: LinkOptions | null = null
  private heartbeat: NodeJS.Timeout | null = null
  private retry: NodeJS.Timeout | null = null
  private attempt = 0
  private pending = new Map<string, Pending>()
  private seq = 0

  /** Подключиться (или переподключиться с новыми настройками); null — отключиться */
  configure(opts: LinkOptions | null): void {
    const same = opts && this.opts && JSON.stringify(opts) === JSON.stringify(this.opts)
    if (same && this.ws) return
    this.close()
    this.opts = opts
    this.devices = []
    this.activeId = null
    this.state = null
    if (opts) this.open()
    else this.setStatus('idle')
  }

  get connected(): boolean {
    return this.status === 'online'
  }

  /** Сейчас по часам сервера */
  now(): number {
    return Date.now() + this.offset
  }

  /** Позиция трека на сейчас: пока играет, время идёт */
  position(): number {
    const s = this.state
    if (!s) return 0
    const base = s.positionMs ?? 0
    if (!s.playing || s.pending || !s.at) return base
    const pos = base + Math.max(0, this.now() - Date.parse(s.at))
    return s.durationMs ? Math.min(s.durationMs, pos) : pos
  }

  get active(): ConnectDevice | null {
    return this.devices.find(d => d.id === this.activeId) ?? null
  }

  /** Команда устройству; ответ — подтверждение или `offline`/`timeout` */
  command(to: string, command: RemoteCommand): Promise<ConnectAck> {
    const id = this.nextId()
    return this.request(id, { type: 'connect.command', id, to, command })
  }

  transfer(to: string, play?: boolean): Promise<ConnectAck> {
    const id = this.nextId()
    return this.request(id, { type: 'connect.transfer', id, to, play })
  }

  close(): void {
    if (this.retry) clearTimeout(this.retry)
    if (this.heartbeat) clearInterval(this.heartbeat)
    this.retry = null
    this.heartbeat = null
    for (const [id, p] of this.pending) {
      clearTimeout(p.timer)
      p.resolve({ id, ok: false, error: 'offline' })
    }
    this.pending.clear()
    const ws = this.ws
    this.ws = null
    if (ws) {
      ws.removeAllListeners()
      ws.on('error', () => {})
      ws.close()
    }
  }

  private nextId(): string {
    this.seq += 1
    return `sd-${Date.now().toString(36)}-${this.seq}`
  }

  private request(id: string, msg: object): Promise<ConnectAck> {
    return new Promise((resolve) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        resolve({ id, ok: false, error: 'offline' })
        return
      }
      const timer = setTimeout(() => {
        this.pending.delete(id)
        resolve({ id, ok: false, error: 'timeout' })
      }, ACK_MS)
      this.pending.set(id, { resolve, timer })
      this.send(msg)
    })
  }

  private send(msg: object): void {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg))
  }

  private setStatus(s: LinkStatus): void {
    if (this.status === s) return
    this.status = s
    this.emit('status', s)
  }

  private open(): void {
    const opts = this.opts
    if (!opts) return
    this.setStatus(this.status === 'online' ? 'online' : 'connecting')
    const url = `${opts.server.replace(/^http/, 'ws')}/api/ws`
    const ws = new WebSocket(url, {
      headers: { 'Authorization': `Bearer ${opts.token}`, 'User-Agent': `DalyWave-StreamDeck/${opts.version}` },
      handshakeTimeout: 10_000,
    })
    this.ws = ws
    ws.on('open', () => {
      this.attempt = 0
      this.send({
        type: 'connect.hello',
        deviceId: opts.deviceId,
        name: opts.name,
        kind: 'desktop',
        platform: 'streamdeck',
        appVersion: opts.version,
        capabilities: { remote: true },
      })
      this.heartbeat = setInterval(() => this.send({ type: 'connect.heartbeat' }), HEARTBEAT_MS)
      this.setStatus('online')
    })
    ws.on('message', (data) => {
      try {
        this.handle(JSON.parse(String(data)) as ServerMessage)
      }
      catch {
        // незнакомое сообщение пропускаем: сервер может быть новее
      }
    })
    ws.on('unexpected-response', (_req, res) => {
      if (res.statusCode === 401 || res.statusCode === 403) this.emit('unauthorized')
    })
    ws.on('close', (code) => {
      if (this.ws !== ws) return
      if (this.heartbeat) clearInterval(this.heartbeat)
      this.heartbeat = null
      this.ws = null
      // 4001 — сессию отозвали («Доступ → устройства» или выход)
      if (code === 4001) {
        this.emit('unauthorized')
        return
      }
      this.scheduleRetry()
    })
    ws.on('error', () => {
      // за ошибкой всегда следует close
    })
  }

  private scheduleRetry(): void {
    this.setStatus('offline')
    const delay = BACKOFF_MS[Math.min(this.attempt, BACKOFF_MS.length - 1)] ?? 30_000
    this.attempt += 1
    this.retry = setTimeout(() => {
      this.retry = null
      this.open()
    }, delay)
  }

  /** Компьютер проснулся: не ждать паузы между попытками */
  wake(): void {
    if (this.retry && this.opts) {
      clearTimeout(this.retry)
      this.retry = null
      this.attempt = 0
      this.open()
    }
  }

  private handle(msg: ServerMessage): void {
    switch (msg.type) {
      case 'hello': {
        const m = msg as { serverTime: string }
        this.offset = Date.parse(m.serverTime) - Date.now()
        break
      }
      case 'connect.devices': {
        const m = msg as { devices: ConnectDevice[], activeDeviceId: string | null }
        this.devices = m.devices
        this.activeId = m.activeDeviceId
        this.emit('devices')
        break
      }
      case 'connect.active': {
        const m = msg as { deviceId: string | null, state: PlaybackState | null }
        this.activeId = m.deviceId
        if (m.state) this.state = m.state
        this.emit('devices')
        this.emit('state')
        break
      }
      case 'connect.state': {
        const m = msg as { deviceId: string, state: PlaybackState }
        if (m.deviceId === this.activeId || !this.activeId) {
          this.activeId = m.deviceId
          this.state = m.state
          this.emit('state')
        }
        break
      }
      case 'connect.ack': {
        const m = msg as ConnectAck
        const p = this.pending.get(m.id)
        if (p) {
          clearTimeout(p.timer)
          this.pending.delete(m.id)
          p.resolve(m)
        }
        break
      }
      case 'library.changed': {
        this.emit('library', (msg as { entities: string[] }).entities)
        break
      }
      case 'resync':
        this.emit('library', ['likedTracks', 'playlists', 'playlistItems', 'wavePresets'])
        break
    }
  }
}
