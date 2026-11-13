// HTTP к серверу DalyWave токеном устройства `dwd_…` (его выдаёт вход кодом, см. pairing.ts).

export class ApiError extends Error {
  constructor(readonly status: number, readonly code: string | null, message: string) {
    super(message)
  }
}

/** Сервер не ответил (нет сети, сервер перезапускается) */
export class NetworkError extends Error {}

export const DEFAULT_SERVER = 'https://wave.daly.rest'

export function normalizeServer(raw: string | undefined | null): string {
  const s = (raw ?? '').trim().replace(/\/+$/, '')
  if (!s) return DEFAULT_SERVER
  return /^https?:\/\//.test(s) ? s : `https://${s}`
}

type Listener = () => void

export class Api {
  server = DEFAULT_SERVER
  token: string | null = null
  version = '0.0.0'
  private unauthorized = new Set<Listener>()
  private covers = new Map<string, Promise<string | null>>()

  onUnauthorized(fn: Listener): void {
    this.unauthorized.add(fn)
  }

  configure(server: string, token: string | null): void {
    const changed = server !== this.server || token !== this.token
    this.server = server
    this.token = token
    if (changed) this.covers.clear()
  }

  url(path: string): string {
    return /^https?:\/\//.test(path) ? path : `${this.server}${path}`
  }

  async request<T>(method: string, path: string, body?: unknown, auth = true): Promise<T> {
    const headers: Record<string, string> = { 'Accept': 'application/json', 'User-Agent': `DalyWave-StreamDeck/${this.version}` }
    if (auth && this.token) headers.Authorization = `Bearer ${this.token}`
    if (body !== undefined) headers['Content-Type'] = 'application/json'
    let res: Response
    try {
      res = await fetch(this.url(path), {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(15_000),
      })
    }
    catch (e) {
      throw new NetworkError(e instanceof Error ? e.message : String(e))
    }
    if (res.status === 401 && auth && this.token) {
      for (const fn of this.unauthorized) fn()
    }
    if (!res.ok) {
      let code: string | null = null
      let message = res.statusText
      try {
        const err = await res.json() as { error?: { code?: string, message?: string } }
        code = err.error?.code ?? null
        message = err.error?.message ?? message
      }
      catch {
        // тело без JSON
      }
      throw new ApiError(res.status, code, message)
    }
    if (res.status === 204) return undefined as T
    const type = res.headers.get('content-type') ?? ''
    return (type.includes('json') ? await res.json() : undefined) as T
  }

  get<T>(path: string): Promise<T> {
    return this.request<T>('GET', path)
  }

  post<T>(path: string, body?: unknown, auth = true): Promise<T> {
    return this.request<T>('POST', path, body ?? {}, auth)
  }

  put<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>('PUT', path, body)
  }

  del<T>(path: string): Promise<T> {
    return this.request<T>('DELETE', path)
  }

  /** Обложка как data URI (JPEG); кешируется на время работы плагина. Нет — null */
  cover(path: string | null | undefined): Promise<string | null> {
    if (!path) return Promise.resolve(null)
    const hit = this.covers.get(path)
    if (hit) return hit
    const p = (async () => {
      try {
        const res = await fetch(this.url(path), { signal: AbortSignal.timeout(15_000) })
        if (!res.ok) return null
        const type = res.headers.get('content-type')?.split(';')[0] || 'image/jpeg'
        const buf = Buffer.from(await res.arrayBuffer())
        return `data:${type};base64,${buf.toString('base64')}`
      }
      catch {
        return null
      }
    })()
    this.covers.set(path, p)
    // Неудачу не помним: сеть могла моргнуть
    void p.then((v) => {
      if (v === null) this.covers.delete(path)
    })
    // Сотни обложек незачем держать: старые выбрасываются
    if (this.covers.size > 200) {
      const first = this.covers.keys().next().value
      if (first !== undefined) this.covers.delete(first)
    }
    return p
  }
}

export const api = new Api()
