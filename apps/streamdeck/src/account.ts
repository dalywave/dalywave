// Подключение плагина к аккаунту — как у приложений для компьютера: «Подключить» открывает браузер
// (`/api/auth/app/start` → вход на daly.rest), одноразовый код возвращается на свой порт плагина
// `http://127.0.0.1:<порт>/callback` и меняется на токен устройства `dwd_…` с PKCE. Своего входа нет;
// токен — отдельная строка в «Доступе», её можно отозвать. Хранится в глобальных настройках плагина.
import { createHash, randomBytes, randomUUID } from 'node:crypto'
import { type IncomingMessage, type Server, type ServerResponse, createServer } from 'node:http'
import type { AddressInfo } from 'node:net'
import { hostname } from 'node:os'
import streamDeck from '@elgato/streamdeck'
import { ApiError, DEFAULT_SERVER, api, normalizeServer } from './api'
import { isRussian } from './i18n'
import { player } from './player'
import type { AppToken } from './types'

export interface GlobalSettings {
  [key: string]: string | number | boolean | null | undefined
  server?: string
  token?: string | null
  /** Постоянный id пульта в «Где играть» */
  deviceId?: string
  /** Кто вошёл — для инспектора */
  userName?: string | null
  /** Таймер сна: когда остановить (мс, часы компьютера) и на сколько ставили */
  sleepUntil?: number | null
  sleepTotal?: number | null
}

export type PairingState =
  | { phase: 'idle' }
  /** Браузер открыт, ждём возврата */
  | { phase: 'waiting', url: string }
  | { phase: 'error', message: string }

/** Сколько ждать входа в браузере */
const LOGIN_TTL_MS = 10 * 60_000

type Listener = () => void

function base64url(buf: Buffer): string {
  return buf.toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

/** Страница, которую браузер видит после входа: короткая, в цветах «Полночи» */
function page(ok: boolean): string {
  const ru = isRussian()
  const title = ok ? (ru ? 'Stream Deck подключён' : 'Stream Deck connected') : (ru ? 'Не получилось подключить' : 'Couldn’t connect')
  const hint = ok
    ? (ru ? 'Кнопки уже управляют музыкой. Вкладку можно закрыть.' : 'Your keys control the music now. You can close this tab.')
    : (ru ? 'Вернитесь в Stream Deck и нажмите «Подключить» ещё раз.' : 'Go back to Stream Deck and press Connect again.')
  return `<!doctype html><html lang="${ru ? 'ru' : 'en'}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>DalyWave</title>
<style>:root{color-scheme:dark}body{margin:0;min-height:100vh;display:grid;place-items:center;background:#08090c;color:#f3f5f9;font:16px/1.5 -apple-system,'Segoe UI',system-ui,sans-serif}
main{max-width:360px;padding:24px;text-align:center}svg{width:56px;height:56px}h1{margin:16px 0 6px;font-size:22px;font-weight:700;letter-spacing:-.01em}p{margin:0;color:rgba(232,236,244,.68)}</style></head>
<body><main><svg viewBox="0 0 100 100"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#8e6bff"/><stop offset=".45" stop-color="#d45bef"/><stop offset=".75" stop-color="#ff6f95"/><stop offset="1" stop-color="#ffa25c"/></linearGradient></defs><path d="M16 31C18.83 37.33 27.33 67.33 33 69C38.67 70.67 44.33 41 50 41C55.67 41 61.33 70.67 67 69C72.67 67.33 81.17 37.33 84 31" fill="none" stroke="url(#g)" stroke-width="11.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
<h1>${title}</h1><p>${hint}</p></main></body></html>`
}

class Account {
  settings: GlobalSettings = {}
  pairing: PairingState = { phase: 'idle' }
  private listeners = new Set<Listener>()
  private server: Server | null = null
  private timeout: NodeJS.Timeout | null = null

  onChange(fn: Listener): () => void {
    this.listeners.add(fn)
    return () => this.listeners.delete(fn)
  }

  private notify(): void {
    for (const fn of this.listeners) fn()
  }

  get serverUrl(): string {
    return normalizeServer(this.settings.server ?? DEFAULT_SERVER)
  }

  get connected(): boolean {
    return !!this.settings.token
  }

  async load(): Promise<void> {
    this.settings = await streamDeck.settings.getGlobalSettings<GlobalSettings>()
    if (!this.settings.deviceId) await this.save({ deviceId: `streamdeck-${randomUUID()}` })
    this.apply()
  }

  /** Глобальные настройки пришли от Stream Deck (их мог поменять инспектор) */
  received(next: GlobalSettings): void {
    const before = `${this.serverUrl}|${this.settings.token ?? ''}`
    this.settings = next
    if (`${this.serverUrl}|${this.settings.token ?? ''}` !== before) this.apply()
    this.notify()
  }

  async save(patch: Partial<GlobalSettings>): Promise<void> {
    this.settings = { ...this.settings, ...patch }
    await streamDeck.settings.setGlobalSettings(this.settings)
    this.notify()
  }

  apply(): void {
    const token = this.settings.token ?? null
    api.configure(this.serverUrl, token)
    player.paired = !!token
    player.link.configure(token
      ? { server: this.serverUrl, token, deviceId: this.settings.deviceId ?? 'streamdeck', name: deviceName(), version: api.version }
      : null)
    player.changed()
  }

  async setServer(raw: string): Promise<void> {
    const server = normalizeServer(raw)
    if (server === this.serverUrl) return
    this.cancel()
    // Токен другого сервера здесь не годится
    await this.save({ server, token: null, userName: null })
    this.apply()
  }

  /** Начать вход: свой порт для возврата и страница входа в браузере */
  async start(): Promise<void> {
    // Вход уже идёт — просто открыть браузер снова
    if (this.pairing.phase === 'waiting') {
      await streamDeck.system.openUrl(this.pairing.url)
      return
    }
    this.cancel()
    const verifier = base64url(randomBytes(48))
    const challenge = base64url(createHash('sha256').update(verifier).digest())
    const state = base64url(randomBytes(24))
    const server = this.serverUrl
    try {
      const port = await this.listen((req, res) => void this.callback(req, res, { verifier, state, server }))
      const redirect = `http://127.0.0.1:${port}/callback`
      const params = new URLSearchParams({
        redirect_uri: redirect,
        code_challenge: challenge,
        code_challenge_method: 'S256',
        state,
        device_name: deviceName(),
        platform: 'streamdeck',
      })
      const url = `${server}/api/auth/app/start?${params}`
      this.pairing = { phase: 'waiting', url }
      this.timeout = setTimeout(() => this.cancel(), LOGIN_TTL_MS)
      this.notify()
      await streamDeck.system.openUrl(url)
    }
    catch (e) {
      this.cancel()
      this.fail(e instanceof Error ? e.message : String(e))
    }
  }

  private listen(handler: (req: IncomingMessage, res: ServerResponse) => void): Promise<number> {
    return new Promise((resolve, reject) => {
      const srv = createServer(handler)
      srv.once('error', reject)
      // Только эта машина: порт выбирает система
      srv.listen(0, '127.0.0.1', () => {
        this.server = srv
        resolve((srv.address() as AddressInfo).port)
      })
    })
  }

  private async callback(req: IncomingMessage, res: ServerResponse, flow: { verifier: string, state: string, server: string }): Promise<void> {
    const url = new URL(req.url ?? '/', 'http://127.0.0.1')
    if (url.pathname !== '/callback') {
      res.writeHead(404).end()
      return
    }
    const code = url.searchParams.get('code')
    const ok = !!code && url.searchParams.get('state') === flow.state
    if (!ok) {
      res.writeHead(400, { 'Content-Type': 'text/html; charset=utf-8' }).end(page(false))
      this.cancel()
      this.fail(url.searchParams.get('error') ?? 'state')
      return
    }
    try {
      const token = await api.post<AppToken>('/api/auth/app/token', { code, codeVerifier: flow.verifier, appVersion: api.version }, false)
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }).end(page(true))
      this.cancel()
      await this.save({ server: flow.server, token: token.token, userName: token.user.name })
      this.apply()
    }
    catch (e) {
      res.writeHead(502, { 'Content-Type': 'text/html; charset=utf-8' }).end(page(false))
      this.cancel()
      this.fail(e instanceof ApiError ? e.message : 'network')
    }
  }

  private fail(message: string): void {
    streamDeck.logger.warn(`sign-in failed: ${message}`)
    this.pairing = { phase: 'error', message }
    this.notify()
  }

  cancel(): void {
    if (this.timeout) clearTimeout(this.timeout)
    this.timeout = null
    if (this.server) {
      this.server.close()
      this.server.closeAllConnections?.()
    }
    this.server = null
    if (this.pairing.phase !== 'idle') {
      this.pairing = { phase: 'idle' }
      this.notify()
    }
  }

  /** Отключить: сессия на сервере закрывается, токен забывается */
  async disconnect(): Promise<void> {
    this.cancel()
    if (this.settings.token) {
      try {
        await api.post('/api/auth/logout')
      }
      catch {
        // сессию можно отозвать и в «Доступе»
      }
    }
    await this.save({ token: null, userName: null })
    this.apply()
  }

  /** Сервер не принял токен: отозвали в «Доступе» */
  async revoked(): Promise<void> {
    if (!this.settings.token) return
    await this.save({ token: null, userName: null })
    this.apply()
  }
}

/** «Stream Deck · MacBook Pro» — имя для «Доступа» и «Где играть» */
export function deviceName(): string {
  const host = hostname().replace(/\.local$/i, '').replace(/[-_]+/g, ' ').trim()
  return host ? `Stream Deck · ${host}`.slice(0, 64) : 'Stream Deck'
}

export const account = new Account()
