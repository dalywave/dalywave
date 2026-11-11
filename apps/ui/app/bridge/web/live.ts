// Живые события сервера по WebSocket /api/ws: загрузки, изменения медиатеки, задачи.
// Переподключается сам с нарастающей паузой; после обрыва сообщает «данные могли устареть».
import type { ServerEvent, ServerEventType } from '~/api/types'
import type { Bridge, LiveState, ServerEventOf } from '../types'

export interface LiveOptions {
  /** Адрес WebSocket: ws(s)://…/api/ws */
  url: () => Promise<string | null>
}

type Listener = (e: ServerEvent) => void

export function createLive(opts: LiveOptions): Bridge['live'] {
  let ws: WebSocket | null = null
  let state: LiveState = 'idle'
  let wanted = false
  let attempt = 0
  let retry: ReturnType<typeof setTimeout> | undefined
  let ping: ReturnType<typeof setInterval> | undefined
  let everOpened = false
  const listeners = new Map<ServerEventType, Set<Listener>>()
  const reconnectListeners = new Set<() => void>()

  function emit(e: ServerEvent) {
    listeners.get(e.type)?.forEach(cb => cb(e))
  }

  function schedule() {
    if (!wanted) return
    clearTimeout(retry)
    const delay = Math.min(30_000, 1000 * 2 ** Math.min(attempt, 5)) * (0.75 + Math.random() * 0.5)
    attempt++
    retry = setTimeout(() => void open(), delay)
  }

  async function open() {
    if (!wanted || ws) return
    const url = await opts.url().catch(() => null)
    if (!url) {
      state = 'closed'
      schedule()
      return
    }
    state = 'connecting'
    let socket: WebSocket
    try {
      socket = new WebSocket(url)
    }
    catch {
      state = 'closed'
      schedule()
      return
    }
    ws = socket
    socket.onopen = () => {
      state = 'open'
      attempt = 0
      if (everOpened) reconnectListeners.forEach(cb => cb())
      everOpened = true
    }
    socket.onmessage = (m) => {
      if (typeof m.data !== 'string') return
      let e: ServerEvent
      try {
        e = JSON.parse(m.data) as ServerEvent
      }
      catch {
        return
      }
      if (e.type === 'hello') {
        clearInterval(ping)
        const every = Math.max(10_000, e.heartbeatMs || 30_000)
        ping = setInterval(() => {
          if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: 'ping', id: String(Date.now()) }))
        }, every)
      }
      if (e.type === 'resync') reconnectListeners.forEach(cb => cb())
      emit(e)
    }
    socket.onclose = () => {
      clearInterval(ping)
      ws = null
      state = 'closed'
      schedule()
    }
    socket.onerror = () => socket.close()
  }

  const onVisible = () => {
    if (document.visibilityState === 'visible' && wanted && !ws) {
      attempt = 0
      void open()
    }
  }

  return {
    get state() {
      return state
    },
    connect() {
      if (wanted) return
      wanted = true
      document.addEventListener('visibilitychange', onVisible)
      window.addEventListener('online', onVisible)
      void open()
    },
    disconnect() {
      wanted = false
      clearTimeout(retry)
      clearInterval(ping)
      document.removeEventListener('visibilitychange', onVisible)
      window.removeEventListener('online', onVisible)
      ws?.close()
      ws = null
      state = 'idle'
    },
    on<T extends ServerEventType>(type: T, cb: (event: ServerEventOf<T>) => void) {
      const set = listeners.get(type) ?? new Set<Listener>()
      listeners.set(type, set)
      const fn = cb as unknown as Listener
      set.add(fn)
      return () => set.delete(fn)
    },
    onReconnect(cb: () => void) {
      reconnectListeners.add(cb)
      return () => reconnectListeners.delete(cb)
    },
  }
}
