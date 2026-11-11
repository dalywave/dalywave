// Мост сайта: HTTP к нашему API, звук — WebAudioEngine, события — пачками, живые события — WebSocket.
// Чего сервер ещё не умеет (волна, звук этапа 1b, устройства), в разработке отдаёт мок; в сборке мока нет.
import type { EventResult, LikedTrackPage, Track } from '~/api/types'
import { syntheticPeaks } from '~/utils/peaks'
import type { Bridge, MediaCommand, NowPlayingInfo, PlaybackDevice, Platform, Section, SourceResult, StreamResponse, WaveTrack, WindowChrome } from '../types'
import type { DevMock } from '../mock'
import { WebAudioEngine } from './audio'
import { EventQueue } from './events'
import { createHttp, done, toApiError, unwrap, type HttpOptions } from './http'
import { createLive } from './live'

// По каким путям OpenAPI видно, что раздел уже на сервере
const SECTION_READY: Record<Section, (paths: string[]) => boolean> = {
  catalog: p => p.includes('/api/search'),
  home: p => p.includes('/api/home'),
  library: p => p.some(x => x.startsWith('/api/library')),
  events: p => p.includes('/api/events'),
  stream: p => p.some(x => /^\/api\/tracks\/\{[^}]+\}\/stream$/.test(x)),
  wave: p => p.some(x => x.startsWith('/api/wave')),
  devices: p => p.includes('/api/devices'),
}

/** Громкость как у остальных треков: цель −14 LUFS, без клиппинга по истинному пику */
export function gainFor(lufs: number | null | undefined, truePeak: number | null | undefined): number | undefined {
  if (lufs == null) return undefined
  let gain = Math.max(-12, Math.min(6, -14 - lufs))
  if (truePeak != null) gain = Math.min(gain, -1 - truePeak)
  return gain
}

function detectPlatform(): Platform {
  const ua = navigator.userAgent
  if (/iPad/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) return 'ipados'
  if (/iPhone|iPod/.test(ua)) return 'ios'
  if (/Android/.test(ua)) return 'android'
  return 'web'
}

export interface WebBridgeOptions extends HttpOptions {
  kind?: 'web' | 'tauri'
  platform?: Platform
  isDesktopApp?: boolean
  chrome?: WindowChrome | null
  window?: Partial<Bridge['window']>
  auth?: Partial<Pick<Bridge['auth'], 'login' | 'logout' | 'me' | 'cancelLogin'>>
  media?: Partial<Pick<Bridge['media'], 'nowPlaying' | 'onCommand'>>
  updates?: Bridge['updates']
  setLocale?: Bridge['setLocale']
  /** Подписка на «вышли в другом окне» (приложение) */
  onSessionEnded?: (cb: () => void) => void
}

export function createWebBridge(opts: WebBridgeOptions = {}): Bridge {
  const signedOut = new Set<() => void>()
  const endSession = () => signedOut.forEach(cb => cb())
  opts.onSessionEnded?.(endSession)
  const http = createHttp({ ...opts, onUnauthorized: () => {
    opts.onUnauthorized?.()
    endSession()
  } })
  const { api } = http
  let paths: string[] | null = null
  let mock: DevMock | null = null
  let readyJob: Promise<void> | null = null

  async function likedTracks(limit = 200): Promise<Track[]> {
    const page: LikedTrackPage = await unwrap(api.GET('/api/library/tracks', { params: { query: { limit } } }))
    return page.items.map(i => i.track)
  }

  function ready(): Promise<void> {
    readyJob ??= (async () => {
      if (!import.meta.dev) return
      try {
        const res = await http.raw('GET', '/api/openapi.json')
        const spec = await res.json() as { paths?: Record<string, unknown> }
        paths = Object.keys(spec.paths ?? {})
      }
      catch {
        paths = []
      }
      const known = paths
      const missing = (Object.keys(SECTION_READY) as Section[]).filter(s => !SECTION_READY[s](known))
      if (missing.length) {
        const m = await import('../mock')
        mock = m.createMock({
          deezerTrack: async (deezerId) => {
            const e = await unwrap(api.GET('/api/catalog/deezer/{kind}/{deezerId}', { params: { path: { kind: 'track', deezerId } } }))
            return e.type === 'track' ? e : null
          },
          liked: () => likedTracks(),
          followedNames: async () => {
            const o = await unwrap(api.GET('/api/library'))
            return o.followedArtists.map(f => f.artist.name)
          },
        })
      }
    })()
    return readyJob
  }

  const mocked = (section: Section) => import.meta.dev && !!mock && !!paths && !SECTION_READY[section](paths)

  const events = new EventQueue(async (batch, keepalive) => {
    if (keepalive) {
      // Закрытие вкладки: fetch с keepalive переживает выгрузку страницы
      const res = await http.raw('POST', '/api/events', { body: JSON.stringify(batch), keepalive, headers: { 'Content-Type': 'application/json' } })
      const body: unknown = await res.json().catch(() => null)
      if (!res.ok) throw toApiError(res.status, body)
      return body as EventResult
    }
    return unwrap(api.POST('/api/events', { body: batch }))
  })

  const live = createLive({
    url: async () => {
      if (http.baseUrl) {
        const t = await opts.token?.()
        return `${http.baseUrl.replace(/^http/, 'ws')}/api/ws${t ? `?token=${encodeURIComponent(t)}` : ''}`
      }
      return `${location.origin.replace(/^http/, 'ws')}/api/ws`
    },
  })

  // Системный плеер браузера: экран блокировки, медиаклавиши, наушники
  const commandListeners = new Set<(cmd: MediaCommand) => void>()
  let sessionBound = false
  function bindMediaSession() {
    if (sessionBound || !('mediaSession' in navigator)) return
    sessionBound = true
    const ms = navigator.mediaSession
    const send = (cmd: MediaCommand) => commandListeners.forEach(cb => cb(cmd))
    const handlers: [MediaSessionAction, MediaSessionActionHandler][] = [
      ['play', () => send({ action: 'play' })],
      ['pause', () => send({ action: 'pause' })],
      ['previoustrack', () => send({ action: 'previous' })],
      ['nexttrack', () => send({ action: 'next' })],
      ['stop', () => send({ action: 'stop' })],
      ['seekto', d => d.seekTime != null && send({ action: 'seek', positionMs: d.seekTime * 1000 })],
      ['seekbackward', d => send({ action: 'seekBy', offsetMs: -(d.seekOffset ?? 10) * 1000 })],
      ['seekforward', d => send({ action: 'seekBy', offsetMs: (d.seekOffset ?? 10) * 1000 })],
    ]
    for (const [action, fn] of handlers) {
      try {
        ms.setActionHandler(action, fn)
      }
      catch {
        // Не все браузеры знают все действия
      }
    }
  }

  function webNowPlaying(info: NowPlayingInfo | null) {
    if (!('mediaSession' in navigator)) return
    const ms = navigator.mediaSession
    if (!info) {
      ms.metadata = null
      ms.playbackState = 'none'
      return
    }
    const art = info.artworkUrl ? new URL(info.artworkUrl, location.href).href : null
    if (ms.metadata?.title !== info.title || ms.metadata?.artist !== info.artist) {
      ms.metadata = new MediaMetadata({ title: info.title, artist: info.artist, album: info.album ?? '', artwork: art ? [{ src: art, sizes: '500x500', type: 'image/jpeg' }] : [] })
    }
    ms.playbackState = info.playing ? 'playing' : 'paused'
    if (info.durationMs && info.positionMs != null) {
      try {
        ms.setPositionState({ duration: info.durationMs / 1000, position: Math.min(info.positionMs, info.durationMs) / 1000, playbackRate: 1 })
      }
      catch {
        // Браузер не принял позицию во время смены трека — следующая придёт через секунду
      }
    }
  }

  const noWindow = async () => {}

  const bridge: Bridge = {
    kind: opts.kind ?? 'web',
    platform: opts.platform ?? detectPlatform(),
    isDesktopApp: opts.isDesktopApp ?? false,
    chrome: opts.chrome ?? null,
    ready,
    mocked,
    resolveUrl: url => http.url(url),
    window: {
      minimize: opts.window?.minimize ?? noWindow,
      toggleMaximize: opts.window?.toggleMaximize ?? (async () => false),
      close: opts.window?.close ?? noWindow,
      onState: opts.window?.onState ?? (() => () => {}),
      trackSnap: opts.window?.trackSnap ?? (() => () => {}),
    },
    updates: opts.updates ?? { onAvailable: () => () => {}, install: noWindow },
    setLocale: opts.setLocale ?? (() => {}),

    auth: {
      providers: () => unwrap(api.GET('/api/auth/providers')),
      me: opts.auth?.me ?? (async () => {
        const res = await api.GET('/api/auth/me').catch(() => null)
        if (!res) throw toApiError(0, null)
        if (res.response.status === 401) return null
        if (!res.response.ok || !res.data) throw toApiError(res.response.status, res.error)
        return res.data
      }),
      login: opts.auth?.login ?? (async (next) => {
        location.assign(`/api/auth/oidc/start?next=${encodeURIComponent(next)}`)
        return null
      }),
      cancelLogin: opts.auth?.cancelLogin ?? noWindow,
      passwordAllowed: (opts.kind ?? 'web') === 'web',
      passwordLogin: password => unwrap(api.POST('/api/auth/password', { body: { password } })),
      logout: opts.auth?.logout ?? (() => done(api.POST('/api/auth/logout'))),
      loginUrl: next => `/api/auth/oidc/start?next=${encodeURIComponent(next)}`,
      onSignedOut(cb) {
        signedOut.add(cb)
        return () => signedOut.delete(cb)
      },
    },

    catalog: {
      search: (q, scope, limit = 20) => unwrap(api.GET('/api/search', { params: { query: { q, scope, limit } } })),
      artist: id => unwrap(api.GET('/api/artists/{id}', { params: { path: { id } } })),
      album: id => unwrap(api.GET('/api/albums/{id}', { params: { path: { id } } })),
      track: id => unwrap(api.GET('/api/tracks/{id}', { params: { path: { id } } })),
      lookup: async (ids) => {
        if (!ids.length) return []
        const list = await unwrap(api.POST('/api/tracks/lookup', { body: { ids } }))
        return list.items
      },
    },

    home: () => unwrap(api.GET('/api/home')),

    library: {
      overview: () => unwrap(api.GET('/api/library')),
      liked: (cursor, limit = 200) => unwrap(api.GET('/api/library/tracks', { params: { query: { cursor: cursor ?? undefined, limit } } })),
      savedAlbums: cursor => unwrap(api.GET('/api/library/albums', { params: { query: { cursor: cursor ?? undefined, limit: 200 } } })),
      followed: cursor => unwrap(api.GET('/api/library/artists', { params: { query: { cursor: cursor ?? undefined, limit: 200 } } })),
      like: id => done(api.PUT('/api/library/tracks/{id}', { params: { path: { id } } })),
      unlike: id => done(api.DELETE('/api/library/tracks/{id}', { params: { path: { id } } })),
      saveAlbum: id => done(api.PUT('/api/library/albums/{id}', { params: { path: { id } } })),
      unsaveAlbum: id => done(api.DELETE('/api/library/albums/{id}', { params: { path: { id } } })),
      follow: (id, request) => done(api.PUT('/api/library/artists/{id}', { params: { path: { id } }, body: request ?? {} })),
      unfollow: id => done(api.DELETE('/api/library/artists/{id}', { params: { path: { id } } })),
      blocked: () => unwrap(api.GET('/api/library/blocked')),
      block: (kind, id) => done(api.PUT('/api/library/blocked/{kind}/{id}', { params: { path: { kind, id } } })),
      unblock: (kind, id) => done(api.DELETE('/api/library/blocked/{kind}/{id}', { params: { path: { kind, id } } })),
      history: cursor => unwrap(api.GET('/api/me/history', { params: { query: { cursor: cursor ?? undefined } } })),
    },

    playlists: {
      list: () => unwrap(api.GET('/api/playlists')),
      get: id => unwrap(api.GET('/api/playlists/{id}', { params: { path: { id } } })),
      create: async (body) => {
        const d = await unwrap(api.POST('/api/playlists', { body }))
        return d.playlist
      },
      update: (id, patch) => unwrap(api.PATCH('/api/playlists/{id}', { params: { path: { id } }, body: patch })),
      remove: id => done(api.DELETE('/api/playlists/{id}', { params: { path: { id } } })),
      addItems: (id, body) => done(api.POST('/api/playlists/{id}/items', { params: { path: { id } }, body })),
      removeItem: (id, itemId) => done(api.DELETE('/api/playlists/{id}/items/{itemId}', { params: { path: { id, itemId } } })),
      moveItem: (id, itemId, afterItemId) => done(api.POST('/api/playlists/{id}/items/{itemId}/move', { params: { path: { id, itemId } }, body: { afterItemId } })),
      pin: (id, pinned) => (pinned
        ? done(api.PUT('/api/playlists/{id}/pin', { params: { path: { id } } }))
        : done(api.DELETE('/api/playlists/{id}/pin', { params: { path: { id } } }))),
    },

    wave: {
      async next(settings, count, exclude) {
        await ready()
        if (mock && mocked('wave')) return mock.waveNext(settings, count, exclude)
        // Пока сервер не собирает волну: любимое вперемешку, без уже сыгранного
        const skip = new Set(exclude)
        return (await likedTracks())
          .filter(t => !skip.has(t.id))
          .sort(() => Math.random() - 0.5)
          .slice(0, count)
          .map((track): WaveTrack => ({ track, reason: 'Из любимого' }))
      },
    },

    media: {
      async source(track: Track): Promise<SourceResult> {
        await ready()
        if (mock && mocked('stream')) return mock.source(track)
        if (track.audioStatus === 'unavailable') return { kind: 'unavailable' }
        const res = await http.raw('GET', `/api/tracks/${track.id}/stream`)
        const body = await res.json().catch(() => null) as StreamResponse | null
        if (res.status === 404 || !body) return { kind: 'unavailable' }
        if (!res.ok) throw toApiError(res.status, body)
        if (body.status === 'ready') {
          return {
            kind: 'ready',
            source: { url: http.url(body.url), durationMs: body.durationMs ?? undefined, gainDb: gainFor(body.loudnessLufs, body.truePeakDb), bpm: body.bpm ?? undefined },
            peaks: body.waveform ?? null,
          }
        }
        return { kind: 'pending', status: body.status, acquisitionId: body.acquisitionId ?? null, queuePosition: body.queuePosition ?? null }
      },
      async waveform(track, bars) {
        await ready()
        if (mock && mocked('stream')) return mock.waveform(track, bars)
        return syntheticPeaks(track.id, bars)
      },
      nowPlaying: opts.media?.nowPlaying ?? webNowPlaying,
      onCommand: opts.media?.onCommand ?? ((cb) => {
        bindMediaSession()
        commandListeners.add(cb)
        return () => commandListeners.delete(cb)
      }),
    },

    events: {
      play: e => events.play(e),
      feedback: e => events.feedback(e),
      flush: () => events.flush(),
    },

    live,

    async devices(): Promise<PlaybackDevice[]> {
      await ready()
      if (mock && mocked('devices')) return mock.devices()
      return [{ id: 'this', name: opts.kind === 'tauri' ? 'Это устройство' : 'Этот браузер', kind: opts.isDesktopApp ? 'laptop' : 'web', current: true, online: true, lastSeenAt: new Date().toISOString() }]
    },

    admin: {
      health: () => unwrap(api.GET('/api/healthz')),
      overview: () => unwrap(api.GET('/api/admin/overview')),
      // Все люди одной страницей, ждущие доступа — первыми
      users: () => unwrap(api.GET('/api/admin/users')),
      updateUser: (id, patch) => done(api.PATCH('/api/admin/users/{id}', { params: { path: { id } }, body: patch })),
      sessions: userId => unwrap(api.GET('/api/admin/sessions', { params: { query: { userId } } })),
      revokeSession: id => done(api.DELETE('/api/admin/sessions/{id}', { params: { path: { id } } })),
      audit: (cursor, limit = 50) => unwrap(api.GET('/api/admin/audit', { params: { query: { cursor: cursor ?? undefined, limit } } })),
      jobs: (q = {}) => unwrap(api.GET('/api/admin/jobs', { params: { query: { lane: q.lane, status: q.status, kind: q.kind, cursor: q.cursor ?? undefined, limit: q.limit } } })),
      retryJob: id => done(api.POST('/api/admin/jobs/{id}/retry', { params: { path: { id } } })),
      cancelJob: id => done(api.POST('/api/admin/jobs/{id}/cancel', { params: { path: { id } } })),
      lanes: () => unwrap(api.GET('/api/admin/lanes')),
      updateLane: (lane, patch) => done(api.PATCH('/api/admin/lanes/{lane}', { params: { path: { lane } }, body: patch })),
    },

    audio: new WebAudioEngine(),
  }
  return bridge
}
