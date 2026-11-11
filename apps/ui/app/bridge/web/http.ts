// HTTP к нашему API. Типизированные вызовы — через openapi-fetch по сгенерированной схеме.
// Сайт ходит на свой адрес с cookie; приложение — на адрес сервера с токеном устройства.
import createClient from 'openapi-fetch'
import type { paths } from '~/api/schema'
import { ApiError, type ErrorBody } from '~/api/types'

function isErrorBody(v: unknown): v is ErrorBody {
  return typeof v === 'object' && v !== null && 'error' in v && typeof (v as ErrorBody).error?.code === 'string'
}

export function toApiError(status: number, body: unknown): ApiError {
  if (isErrorBody(body)) return new ApiError(status, body.error.code, body.error.message)
  if (status === 0) return new ApiError(0, 'network', 'Нет связи с сервером. Проверьте сеть и попробуйте ещё раз.')
  if (status === 404) return new ApiError(404, 'not_found', 'Не нашли — возможно, это уже удалили.')
  return new ApiError(status, 'http_' + status, 'Сервер ответил ошибкой. Попробуйте ещё раз чуть позже.')
}

/** Распаковать ответ openapi-fetch: данные или ApiError. 204 — undefined */
export async function unwrap<T>(call: Promise<{ data?: T, error?: unknown, response: Response }>): Promise<T> {
  let result: { data?: T, error?: unknown, response: Response }
  try {
    result = await call
  }
  catch {
    throw toApiError(0, null)
  }
  if (result.response.ok) return result.data as T
  throw toApiError(result.response.status, result.error)
}

/** Ответ без тела, которое нам нужно */
export async function done(call: Promise<{ error?: unknown, response: Response }>): Promise<void> {
  await unwrap(call as Promise<{ data?: unknown, error?: unknown, response: Response }>)
}

export interface HttpOptions {
  /** Адрес сервера; пусто — тот же, что у страницы (сайт) */
  baseUrl?: string
  /** Токен устройства (приложение); без него — cookie сайта */
  token?: () => Promise<string | null>
  /** Сервер ответил 401 там, где вход должен быть: сессия кончилась */
  onUnauthorized?: () => void
}

// Эти пути отвечают 401 штатно — это не «сессия кончилась»
const AUTH_PATHS = ['/api/auth/me', '/api/auth/password', '/api/auth/providers']

export function query(params: Record<string, string | number | boolean | null | undefined>): string {
  const q = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) if (v !== null && v !== undefined && v !== '') q.set(k, String(v))
  const s = q.toString()
  return s ? `?${s}` : ''
}

export function createHttp(opts: HttpOptions = {}) {
  const baseUrl = (opts.baseUrl ?? '').replace(/\/$/, '')
  const credentials: RequestCredentials = baseUrl ? 'omit' : 'include'
  const api = createClient<paths>({ baseUrl, credentials })

  const expired = (url: string, status: number) => {
    if (status !== 401 || !opts.onUnauthorized) return
    const path = new URL(url, 'http://x').pathname
    if (!AUTH_PATHS.includes(path)) opts.onUnauthorized()
  }

  if (opts.token) {
    const token = opts.token
    api.use({
      async onRequest({ request }) {
        const t = await token()
        if (t) request.headers.set('Authorization', `Bearer ${t}`)
        return request
      },
    })
  }
  api.use({
    onResponse({ request, response }) {
      expired(request.url, response.status)
      return response
    },
  })

  /** Сырой запрос к пути, которого ещё нет в схеме (договор следующего этапа сервера) */
  async function raw(method: string, path: string, init: RequestInit = {}): Promise<Response> {
    const headers = new Headers(init.headers)
    headers.set('Accept', 'application/json')
    if (opts.token) {
      const t = await opts.token()
      if (t) headers.set('Authorization', `Bearer ${t}`)
    }
    let res: Response
    try {
      res = await fetch(baseUrl + path, { ...init, method, headers, credentials })
    }
    catch {
      throw toApiError(0, null)
    }
    expired(baseUrl + path, res.status)
    return res
  }

  return { api, raw, baseUrl, url: (path: string) => (path.startsWith('/') ? baseUrl + path : path) }
}

export type Http = ReturnType<typeof createHttp>
