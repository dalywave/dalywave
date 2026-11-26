// Мост сайта: HTTP к нашему API, звук — WebAudioEngine, события — пачками, живые события — WebSocket.
// Чего сервер ещё не умеет (волна, звук этапа 1b, устройства), в разработке отдаёт мок; в сборке мока нет.
import type { EventResult, ImportItem, ImportSummary, LikedTrackPage, NetworkKind, PlaylistSummary, Track, WaveSession } from '~/api/types'
import { syntheticPeaks } from '~/utils/peaks'
import type { AppPush, AppServer, AppServerRecent, AppUpdateState, AudioEngine, AudioRoutes, BeatHaptics, Bridge, Downloader, HapticKind, MediaCommand, NowPlayingInfo, OfflineBridge, PlaybackDevice, Platform, SaveResult, ScreensaverBridge, Section, SourceResult, StoryMoment, StreamResponse, SystemNotify, Trainer, WaveStart, WaveTrack, WindowChrome } from '../types'
import type { DevMock } from '../mock'
import { createDevOffline } from '../mock/offline'
import { lazyDownloader } from '../tauri/downloader'
import { createDevTrainer, lazyTrainer } from '../tauri/trainer'
import { WebAudioEngine } from './audio'
import { EventQueue } from './events'
import { createHttp, done, query, toApiError, unwrap, type Http, type HttpOptions } from './http'
import { createJam } from './jam'
import { createLive } from './live'
import { createParty } from './party'
import { devOutputs, safariOutputs } from './outputs'
import { devRecognizer } from '../mock/recognize'
import { uuid } from '~/utils/web-api'

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

/** Момент истории в адресе: `?startMs=&lengthMs=` */
const momentQuery = (m?: StoryMoment) => query({ startMs: m?.startMs != null ? Math.round(m.startMs) : null, lengthMs: m?.lengthMs })

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
  /** Звук приложения — движок на Rust; по умолчанию — WebAudio сайта */
  audio?: () => AudioEngine
  /** Загрузчик на компьютере (приложение); у сайта его нет */
  downloader?: Downloader | null
  /** Обучение рекомендаций на компьютере (приложение); у сайта его нет */
  trainer?: Trainer | null
  appServer?: AppServer | null
  /** Вывод звука приложения; на сайте — окно AirPlay Safari, если оно есть */
  outputs?: AudioRoutes | null
  systemIdle?: (() => Promise<number | null>) | null
  /** Загрузки на устройство (приложение) */
  offline?: OfflineBridge | null
  /** Тактильный отклик (приложение на iPhone) */
  haptic?: (kind: HapticKind) => void
  /** Push приложения (iPhone и iPad) */
  appPush?: AppPush | null
  /** Системные уведомления приложения для компьютера */
  systemNotify?: SystemNotify | null
  /** «Почувствуй бит» (приложение на iPhone) */
  beat?: BeatHaptics | null
  /** Заставка для Mac (приложение на macOS) */
  screensaver?: ScreensaverBridge | null
  /** Сохранить выгрузку (приложение): окно «Сохранить как» или «Поделиться»; по умолчанию — загрузка браузера */
  saveFile?: (path: string, name: string, http: Http) => Promise<SaveResult>
}

/** Сайт: загрузка браузера по прямой ссылке — файл пишется на диск потоком, большая медиатека не
 *  держится в памяти страницы. Имя файла даёт сервер (`Content-Disposition`) */
function browserDownload(href: string, name: string): SaveResult {
  const a = document.createElement('a')
  a.href = href
  a.download = name
  a.rel = 'noopener'
  document.body.appendChild(a)
  a.click()
  a.remove()
  return 'saved'
}

/** Готовый файл: системное «Поделиться» (iPhone и iPad — «Сохранить в Файлы», отправить), где его нет —
 *  загрузка браузера */
export async function shareOrSaveBlob(blob: Blob, name: string): Promise<SaveResult> {
  const file = new File([blob], name, { type: blob.type || 'application/octet-stream' })
  if (typeof navigator.share === 'function' && navigator.canShare?.({ files: [file] })) {
    try {
      await navigator.share({ files: [file] })
      return 'shared'
    }
    catch (e) {
      if (e instanceof DOMException && e.name === 'AbortError') return 'cancelled'
    }
  }
  const url = URL.createObjectURL(blob)
  browserDownload(url, name)
  setTimeout(() => URL.revokeObjectURL(url), 30_000)
  return 'saved'
}

/** Только разработка: экран «Адрес сервера» приложения в браузере — `?appServer` (`?appServer=fresh` —
 *  первый запуск). Проверка адреса изображается: `demo…` и `wave…` — DalyWave, `down…` — без базы,
 *  `example.com` — не DalyWave, остальное не отвечает */
function devAppServer(): AppServer | null {
  if (!import.meta.dev || typeof location === 'undefined') return null
  const q = new URLSearchParams(location.search).get('appServer')
  if (q === null) return null
  const recent: AppServerRecent[] = q === 'fresh' ? [] : [{ url: 'https://wave.daly.rest', name: 'DalyWave', usedAt: 1 }]
  const fail = (code: string) => Object.assign(new Error(code), { code, message: code })
  return {
    state: async () => ({ url: 'https://wave.daly.rest', chosen: q !== 'fresh', recent, demo: 'https://demo.wave.daly.rest' }),
    async check(url) {
      await new Promise(r => setTimeout(r, 600))
      const raw = url.trim()
      if (!/^(https?:\/\/)?[\w.-]+(:\d+)?\/?$/.test(raw)) throw fail('invalid')
      const host = raw.replace(/^https?:\/\//, '').replace(/\/$/, '')
      if (host.startsWith('down')) throw fail('down')
      if (host === 'example.com') throw fail('not_dalywave')
      if (!host.startsWith('demo') && !host.startsWith('wave')) throw fail('unreachable')
      return { url: `https://${host}`, name: host.startsWith('demo') ? 'DalyWave Demo' : 'DalyWave', version: '0.1.17' }
    },
    async use() {},
    async forget() {},
    onLink: () => () => {},
  }
}

/** Только разработка: обучение на компьютере изображает мок — `?trainer=idle|active|kept|on-battery|…` */
function devTrainer(): Trainer | null {
  if (import.meta.dev) {
    const scenario = typeof location !== 'undefined' ? new URLSearchParams(location.search).get('trainer') : null
    if (scenario) return lazyTrainer(async () => createDevTrainer(scenario))
  }
  return null
}

/** Только разработка: загрузчик изображает мок — `?downloader=idle|working|paused|offline|…` */
function devDownloader(): Downloader | null {
  if (import.meta.dev) {
    const scenario = typeof location !== 'undefined' ? new URLSearchParams(location.search).get('downloader') : null
    if (scenario) return lazyDownloader(() => import('../mock/downloader').then(m => m.createDevDownloader(scenario)))
  }
  return null
}

/** Только разработка: строка обновления в настройках — `?update=idle|checking|downloading|ready|installing|failed` */
function devUpdates(): Bridge['updates'] | null {
  if (!import.meta.dev || typeof location === 'undefined') return null
  const status = new URLSearchParams(location.search).get('update') as AppUpdateState['status'] | null
  if (!status) return null
  const found = status !== 'idle' && status !== 'checking'
  const state: AppUpdateState = {
    status,
    currentVersion: '0.1.0',
    update: found ? { version: '0.2.0', currentVersion: '0.1.0', notes: null } : null,
    progress: status === 'downloading' ? 0.42 : null,
    error: status === 'failed' ? 'network' : null,
    installOnQuit: true,
  }
  return { onAvailable: () => () => {}, state: async () => state, onState: () => () => {}, check: async () => {}, install: async () => {} }
}

/** Только разработка: загрузки на устройство изображает мок — `localStorage['dw.offlineMock']` (снимки экранов) */
function devOffline(): OfflineBridge | null {
  if (!import.meta.dev || typeof localStorage === 'undefined') return null
  try {
    const raw = localStorage.getItem('dw.offlineMock')
    return raw ? createDevOffline(raw) : null
  }
  catch {
    return null
  }
}

/** Только разработка: вывод звука изображает мок — `?outputs=picker|list` */
function devOutputsFromQuery(): AudioRoutes | null {
  if (import.meta.dev && typeof location !== 'undefined') {
    const scenario = new URLSearchParams(location.search).get('outputs')
    if (scenario) return devOutputs(scenario)
  }
  return null
}

function waveStart(s: WaveSession): WaveStart {
  return {
    id: s.id,
    state: s.state,
    ready: s.ready,
    need: s.need,
    queue: s.queue.map((i): WaveTrack => ({ track: i.track, reason: i.reason.text })),
  }
}

/** Сколько раз переспросить, пока сервер готовит копию файла (раз в ~2 с) */
const PREPARE_TRIES = 30

export function createWebBridge(opts: WebBridgeOptions = {}): Bridge {
  const audio = opts.audio?.() ?? new WebAudioEngine()
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

  /** Загрузка файла телом запроса с прогрессом: у fetch его нет, поэтому XMLHttpRequest */
  async function upload<T>(method: 'POST' | 'PUT', path: string, file: Blob, type: string, onProgress?: (share: number) => void, signal?: AbortSignal): Promise<T> {
    const token = opts.token ? await opts.token() : null
    return new Promise<T>((resolve, reject) => {
      const xhr = new XMLHttpRequest()
      xhr.open(method, http.url(path))
      xhr.withCredentials = !http.baseUrl
      xhr.setRequestHeader('Content-Type', type)
      xhr.setRequestHeader('Accept', 'application/json')
      if (token) xhr.setRequestHeader('Authorization', `Bearer ${token}`)
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable && onProgress) onProgress(e.loaded / e.total)
      }
      xhr.onload = () => {
        let body: unknown
        try {
          body = xhr.responseText ? JSON.parse(xhr.responseText) : null
        }
        catch {
          body = null
        }
        if (xhr.status >= 200 && xhr.status < 300) resolve(body as T)
        else {
          if (xhr.status === 401) {
            opts.onUnauthorized?.()
            endSession()
          }
          reject(toApiError(xhr.status, body))
        }
      }
      xhr.onerror = () => reject(toApiError(0, null))
      xhr.onabort = () => reject(new DOMException('Загрузка отменена', 'AbortError'))
      signal?.addEventListener('abort', () => xhr.abort(), { once: true })
      xhr.send(file)
    })
  }

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
      setTheme: opts.window?.setTheme ?? (() => {}),
      setBackSwipe: opts.window?.setBackSwipe ?? (() => {}),
    },
    updates: opts.updates ?? devUpdates() ?? { onAvailable: () => () => {}, state: async () => null, onState: () => () => {}, check: noWindow, install: noWindow },
    setLocale: opts.setLocale ?? (() => {}),
    desktopApps: () => unwrap(api.GET('/api/downloads')),
    serverConfig: () => unwrap(api.GET('/api/config')),
    downloader: opts.downloader !== undefined ? opts.downloader : devDownloader(),
    trainer: opts.trainer !== undefined ? opts.trainer : devTrainer(),
    systemIdle: opts.systemIdle ?? null,
    appServer: opts.appServer !== undefined ? opts.appServer : devAppServer(),
    outputs: opts.outputs !== undefined ? opts.outputs : devOutputsFromQuery() ?? (audio instanceof WebAudioEngine ? safariOutputs(audio) : null),
    offline: opts.offline !== undefined ? opts.offline : devOffline(),
    haptic: opts.haptic ?? (() => {}),
    exports: {
      save: (path, name) => (opts.saveFile ? opts.saveFile(path, name, http) : Promise.resolve(browserDownload(http.url(path), name))),
    },
    appPush: opts.appPush ?? null,
    systemNotify: opts.systemNotify ?? null,
    beat: opts.beat ?? null,
    screensaver: opts.screensaver ?? null,

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
      providerUrl: (id, o = {}) => `/api/auth/oidc/${encodeURIComponent(id)}/start${query({
        next: o.next && o.next !== '/' ? o.next : undefined,
        app: o.app ? 1 : undefined,
        link: o.link ? 1 : undefined,
        invite: o.invite,
      })}`,
      localLogin: (username, password, app = false) => unwrap(api.POST('/api/auth/local/login', { body: { username, password, app } })),
      demoLogin: (app = false) => unwrap(api.POST('/api/auth/demo', { body: { app } })),
      setup: account => unwrap(api.POST('/api/auth/setup', { body: account })),
      inviteInfo: token => unwrap(api.GET('/api/auth/invites/{token}', { params: { path: { token } } })),
      acceptInvite: (token, account) => unwrap(api.POST('/api/auth/invites/{token}', { params: { path: { token } }, body: account })),
      resetInfo: token => unwrap(api.GET('/api/auth/reset/{token}', { params: { path: { token } } })),
      resetPassword: (token, body) => unwrap(api.POST('/api/auth/reset/{token}', { params: { path: { token } }, body })),
      account: () => unwrap(api.GET('/api/auth/account')),
      deleteAccount: () => done(api.DELETE('/api/auth/me')),
      changePassword: body => done(api.POST('/api/auth/account/password', { body })),
      unlinkIdentity: id => done(api.DELETE('/api/auth/account/identities/{id}', { params: { path: { id } } })),
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
      soundSearch: (text, limit = 12) => unwrap(api.POST('/api/search/sound', { body: { text, limit } })),
      lyrics: trackId => unwrap(api.GET('/api/tracks/{id}/lyrics', { params: { path: { id: trackId } } })),
      searchLyrics: (q, limit = 20) => unwrap(api.GET('/api/lyrics/search', { params: { query: { q, limit } } })),
      versions: trackId => unwrap(api.GET('/api/tracks/{id}/versions', { params: { path: { id: trackId } } })),
      chooseVersion: (trackId, kind) => done(api.PUT('/api/tracks/{id}/versions/choice', { params: { path: { id: trackId } }, body: { kind } })),
      versionChoices: async () => (await unwrap(api.GET('/api/versions/choices'))).items,
      similar: (id, limit = 20) => unwrap(api.GET('/api/tracks/{id}/similar', { params: { path: { id }, query: { limit } } })),
      suggest: (trackIds, exclude = [], limit = 20) => unwrap(api.POST('/api/tracks/suggest', { body: { trackIds: trackIds.slice(0, 500), exclude: exclude.slice(0, 500), limit } })),
      resolveLink: url => unwrap(api.POST('/api/resolve-link', { body: { url } })),
      charts: () => unwrap(api.GET('/api/charts')),
      chart: id => unwrap(api.GET('/api/charts/{id}', { params: { path: { id } } })),
      hook: id => unwrap(api.GET('/api/tracks/{id}/hook', { params: { path: { id } } })),
      storyAudio: async (id, moment) => {
        const res = await http.raw('GET', `/api/tracks/${id}/story/audio${momentQuery(moment)}`)
        if (!res.ok) throw toApiError(res.status, await res.json().catch(() => null))
        return await res.arrayBuffer()
      },
      storyVideo: async (id, video, moment) => {
        const res = await http.raw('POST', `/api/tracks/${id}/story${momentQuery(moment)}`, { body: video, headers: { 'Content-Type': video.type || 'video/mp4' } })
        if (!res.ok) throw toApiError(res.status, await res.json().catch(() => null))
        return new Blob([await res.arrayBuffer()], { type: 'video/mp4' })
      },
      credits: id => unwrap(api.GET('/api/tracks/{id}/credits', { params: { path: { id } } })),
      about: id => unwrap(api.GET('/api/artists/{id}/about', { params: { path: { id } } })),
    },

    home: () => unwrap(api.GET('/api/home')),

    video: {
      albumMotion: id => unwrap(api.GET('/api/albums/{id}/motion', { params: { path: { id } } })),
      clips: async (trackIds) => {
        if (!trackIds.length) return []
        return (await unwrap(api.POST('/api/clips', { body: { trackIds: trackIds.slice(0, 20) } }))).items
      },
      clip: id => unwrap(api.GET('/api/tracks/{id}/clip', { params: { path: { id } } })),
      chooseClip: (id, choice) => unwrap(api.PUT('/api/tracks/{id}/clip', { params: { path: { id } }, body: choice })),
      hideClip: id => done(api.POST('/api/tracks/{id}/clip/hide', { params: { path: { id } } })),
      unhideClip: id => done(api.DELETE('/api/tracks/{id}/clip/hide', { params: { path: { id } } })),
    },

    taste: {
      overview: () => unwrap(api.GET('/api/taste')),
      less: facetId => unwrap(api.POST('/api/taste/facets/{id}/less', { params: { path: { id: facetId } } })),
      undoLess: damperId => done(api.DELETE('/api/taste/dampers/{id}', { params: { path: { id: damperId } } })),
      reset: () => done(api.POST('/api/taste/reset', { body: { confirm: true } })),
    },

    describe: {
      preview: (text, exclude = []) => unwrap(api.POST('/api/describe', { body: { text, exclude: exclude.slice(0, 500) } })),
      more: (brief, exclude, afterTrackId, count = 10) => unwrap(api.POST('/api/describe/more', { body: { brief, exclude: exclude.slice(0, 500), afterTrackId, count } })),
      save: body => unwrap(api.POST('/api/describe/save', { body })),
      get: async (id) => {
        try {
          return await unwrap(api.GET('/api/playlists/{id}/describe', { params: { path: { id } } }))
        }
        catch (e) {
          if ((e as { status?: number }).status === 404) return null
          throw e
        }
      },
      setWeekly: (id, weekly) => unwrap(api.PATCH('/api/playlists/{id}/describe', { params: { path: { id } }, body: { weekly } })),
      extend: async (id, count = 10) => (await unwrap(api.POST('/api/playlists/{id}/describe/more', { params: { path: { id } }, body: { count } }))).trackIds,
    },

    rewind: {
      years: () => unwrap(api.GET('/api/rewind')),
      get: year => unwrap(api.GET('/api/rewind/{year}', { params: { path: { year } } })),
      savePlaylist: async year => (await unwrap(api.POST('/api/rewind/{year}/playlist', { params: { path: { year } } }))).id,
      soundtrack: year => unwrap(api.GET('/api/rewind/{year}/soundtrack', { params: { path: { year } } })),
      more: year => unwrap(api.GET('/api/rewind/{year}/more', { params: { path: { year } } })),
      saveMore: async (year, trackIds) => (await unwrap(api.POST('/api/rewind/{year}/more', { params: { path: { year } }, body: { trackIds } }))).id,
      seen: year => done(api.POST('/api/rewind/{year}/seen', { params: { path: { year } } })),
    },

    // Распознать песню умеют только приложения (bridge.recognizer); трек и история — везде
    recognizer: devRecognizer(),
    recognize: {
      resolve: match => unwrap(api.POST('/api/recognize/resolve', { body: match })),
      history: (cursor, limit = 20) => unwrap(api.GET('/api/recognize/history', { params: { query: { cursor: cursor ?? undefined, limit } } })),
      remove: id => done(api.DELETE('/api/recognize/history/{id}', { params: { path: { id } } })),
    },

    library: {
      overview: () => unwrap(api.GET('/api/library')),
      liked: (cursor, limit = 200) => unwrap(api.GET('/api/library/tracks', { params: { query: { cursor: cursor ?? undefined, limit } } })),
      savedAlbums: cursor => unwrap(api.GET('/api/library/albums', { params: { query: { cursor: cursor ?? undefined, limit: 200 } } })),
      followed: cursor => unwrap(api.GET('/api/library/artists', { params: { query: { cursor: cursor ?? undefined, limit: 200 } } })),
      like: id => done(api.PUT('/api/library/tracks/{id}', { params: { path: { id } } })),
      unlike: id => done(api.DELETE('/api/library/tracks/{id}', { params: { path: { id } } })),
      saveAlbum: id => done(api.PUT('/api/library/albums/{id}', { params: { path: { id } } })),
      unsaveAlbum: id => done(api.DELETE('/api/library/albums/{id}', { params: { path: { id } } })),
      upcoming: () => unwrap(api.GET('/api/library/upcoming')),
      presave: id => unwrap(api.PUT('/api/library/presaves/{id}', { params: { path: { id } } })),
      unpresave: id => done(api.DELETE('/api/library/presaves/{id}', { params: { path: { id } } })),
      follow: (id, request) => done(api.PUT('/api/library/artists/{id}', { params: { path: { id } }, body: request ?? {} })),
      unfollow: id => done(api.DELETE('/api/library/artists/{id}', { params: { path: { id } } })),
      blocked: () => unwrap(api.GET('/api/library/blocked')),
      block: (kind, id) => done(api.PUT('/api/library/blocked/{kind}/{id}', { params: { path: { kind, id } } })),
      unblock: (kind, id) => done(api.DELETE('/api/library/blocked/{kind}/{id}', { params: { path: { kind, id } } })),
      history: (cursor, opts = {}) => unwrap(api.GET('/api/me/history', { params: { query: { cursor: cursor ?? undefined, q: opts.q || undefined, before: opts.before, limit: opts.limit } } })),
      historyDays: () => unwrap(api.GET('/api/me/history/days', { params: { query: { offsetMin: -new Date().getTimezoneOffset() } } })),
      hidePlay: id => done(api.DELETE('/api/me/history/{id}', { params: { path: { id } } })),
      clearHistory: () => done(api.DELETE('/api/me/history')),
      reportRecording: (trackId, report) => unwrap(api.POST('/api/tracks/{id}/wrong-recording', { params: { path: { id: trackId } }, body: report })),
    },

    imports: {
      list: () => unwrap(api.GET('/api/imports')),
      get: id => unwrap(api.GET('/api/imports/{id}', { params: { path: { id } } })),
      items: (id, status, cursor, limit = 50) => unwrap(api.GET('/api/imports/{id}/items', { params: { path: { id }, query: { status: status ?? undefined, cursor: cursor ?? undefined, limit } } })),
      fix: (id, itemId, fix) => unwrap(api.PUT('/api/imports/{id}/items/{itemId}', { params: { path: { id, itemId } }, body: fix })),
      remove: id => done(api.DELETE('/api/imports/{id}', { params: { path: { id } } })),
      spotify: (file, onProgress, signal) => upload<ImportSummary>('POST', `/api/imports/spotify${query({ name: file.name })}`, file, 'application/zip', onProgress, signal),
      fromList: body => unwrap(api.POST('/api/imports/list', { body })),
      yandex: body => unwrap(api.POST('/api/imports/yandex', { body })),
      youtube: url => unwrap(api.POST('/api/imports/youtube', { body: { url } })),
      takeout: (file, onProgress, signal) => upload<ImportSummary>('POST', `/api/imports/youtube/takeout${query({ name: file.name })}`, file, 'application/zip', onProgress, signal),
      startFiles: title => unwrap(api.POST('/api/imports/files', { body: { title } })),
      file: (id, file, onProgress, signal) => upload<ImportItem>('PUT', `/api/imports/${encodeURIComponent(id)}/files${query({ name: file.name })}`, file, file.type || 'application/octet-stream', onProgress, signal),
      finish: id => unwrap(api.POST('/api/imports/{id}/finish', { params: { path: { id } } })),
    },

    notifications: {
      list: (before, limit = 30) => unwrap(api.GET('/api/notifications', { params: { query: { before: before ?? undefined, limit } } })),
      read: ids => unwrap(api.POST('/api/notifications/read', { body: { ids } })),
      settings: () => unwrap(api.GET('/api/notifications/settings')),
      updateSettings: patch => unwrap(api.PATCH('/api/notifications/settings', { body: patch })),
      push: () => unwrap(api.GET('/api/notifications/push')),
      subscribe: reg => unwrap(api.POST('/api/notifications/push', { body: reg })),
      unsubscribe: body => done(api.DELETE('/api/notifications/push', { body })),
      test: () => unwrap(api.POST('/api/notifications/test')),
    },

    settings: {
      get: () => unwrap(api.GET('/api/settings')),
      update: patch => unwrap(api.PATCH('/api/settings', { body: patch })),
    },

    overlay: {
      get: () => unwrap(api.GET('/api/overlay')),
      rotate: () => unwrap(api.POST('/api/overlay/link')),
      update: patch => unwrap(api.PATCH('/api/overlay', { body: patch })),
      disable: () => done(api.DELETE('/api/overlay')),
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
      addItems: (id, body) => unwrap(api.POST('/api/playlists/{id}/items', { params: { path: { id } }, body })),
      removeItems: (id, itemIds) => done(api.POST('/api/playlists/{id}/items/remove', { params: { path: { id } }, body: { itemIds } })),
      setCover: (id, file) => upload<PlaylistSummary>('PUT', `/api/playlists/${encodeURIComponent(id)}/cover`, file, file.type || 'image/jpeg'),
      removeCover: id => unwrap(api.DELETE('/api/playlists/{id}/cover', { params: { path: { id } } })),
      duplicate: (id, title) => unwrap(api.POST('/api/playlists/{id}/duplicate', { params: { path: { id } }, body: title ? { title } : {} })),
      move: (id, body) => unwrap(api.POST('/api/playlists/{id}/move', { params: { path: { id } }, body })),
      createFolder: body => unwrap(api.POST('/api/playlist-folders', { body })),
      updateFolder: (id, patch) => unwrap(api.PATCH('/api/playlist-folders/{id}', { params: { path: { id } }, body: patch })),
      removeFolder: id => unwrap(api.DELETE('/api/playlist-folders/{id}', { params: { path: { id } } })),
      moveFolder: (id, body) => unwrap(api.POST('/api/playlist-folders/{id}/move', { params: { path: { id } }, body })),
      removeItem: (id, itemId) => done(api.DELETE('/api/playlists/{id}/items/{itemId}', { params: { path: { id, itemId } } })),
      moveItem: (id, itemId, afterItemId) => done(api.POST('/api/playlists/{id}/items/{itemId}/move', { params: { path: { id, itemId } }, body: { afterItemId } })),
      setPreview: (id, opts) => unwrap(api.POST('/api/playlists/{id}/set/preview', { params: { path: { id } }, body: opts })),
      applySet: (id, opts) => unwrap(api.POST('/api/playlists/{id}/set', { params: { path: { id } }, body: opts })),
      setOrder: (id, itemIds) => done(api.PUT('/api/playlists/{id}/order', { params: { path: { id } }, body: { itemIds } })),
      pin: (id, pinned) => (pinned
        ? done(api.PUT('/api/playlists/{id}/pin', { params: { path: { id } } }))
        : done(api.DELETE('/api/playlists/{id}/pin', { params: { path: { id } } }))),
      setCollaborative: (id, on) => (on
        ? unwrap(api.PUT('/api/playlists/{id}/collaborative', { params: { path: { id } } }))
        : unwrap(api.DELETE('/api/playlists/{id}/collaborative', { params: { path: { id } } }))),
      setVisibility: (id, friendsCanSee) => unwrap(api.PUT('/api/playlists/{id}/visibility', { params: { path: { id } }, body: { friendsCanSee } })),
      embed: async id => (await unwrap(api.GET('/api/playlists/{id}/embed', { params: { path: { id } } }))).embeddable,
      setEmbed: async (id, embeddable) => (await unwrap(api.PUT('/api/playlists/{id}/embed', { params: { path: { id } }, body: { embeddable } }))).embeddable,
      invite: (id, reset) => (reset
        ? unwrap(api.POST('/api/playlists/{id}/invite', { params: { path: { id } } }))
        : unwrap(api.GET('/api/playlists/{id}/invite', { params: { path: { id } } }))),
      addMembers: (id, userIds) => unwrap(api.POST('/api/playlists/{id}/members', { params: { path: { id } }, body: { userIds } })),
      removeMember: (id, userId) => done(api.DELETE('/api/playlists/{id}/members/{userId}', { params: { path: { id, userId } } })),
      joinPreview: code => unwrap(api.GET('/api/playlists/join/{code}', { params: { path: { code } } })),
      join: code => unwrap(api.POST('/api/playlists/join/{code}', { params: { path: { code } } })),
    },

    friends: {
      list: () => unwrap(api.GET('/api/friends')),
      people: q => unwrap(api.GET('/api/friends/people', { params: { query: { q: q || undefined } } })),
      request: userId => unwrap(api.POST('/api/friends/requests', { body: { userId } })),
      cancel: userId => done(api.DELETE('/api/friends/requests/{userId}', { params: { path: { userId } } })),
      accept: userId => unwrap(api.POST('/api/friends/requests/{userId}/accept', { params: { path: { userId } } })),
      decline: userId => done(api.POST('/api/friends/requests/{userId}/decline', { params: { path: { userId } } })),
      remove: userId => done(api.DELETE('/api/friends/{userId}', { params: { path: { userId } } })),
      profile: userId => unwrap(api.GET('/api/friends/{userId}', { params: { path: { userId } } })),
      invite: reset => (reset ? unwrap(api.POST('/api/friends/invite')) : unwrap(api.GET('/api/friends/invite'))),
      invitePreview: code => unwrap(api.GET('/api/friends/invite/{code}', { params: { path: { code } } })),
      useInvite: code => unwrap(api.POST('/api/friends/invite/{code}', { params: { path: { code } } })),
      privacy: () => unwrap(api.GET('/api/friends/privacy')),
      updatePrivacy: patch => unwrap(api.PATCH('/api/friends/privacy', { body: patch })),
    },

    wave: {
      async start(settings, seed) {
        await ready()
        if (mock && mocked('wave')) {
          const queue = await mock.waveNext(settings, 6, [])
          return { id: uuid(), state: 'ready', ready: queue.length, need: 0, queue }
        }
        return waveStart(await unwrap(api.POST('/api/wave/sessions', { body: { settings: seed ? { ...settings, seed } : settings } })))
      },
      async session(id) {
        await ready()
        if (mock && mocked('wave')) return { id, state: 'ready', ready: 0, need: 0, queue: [] }
        return waveStart(await unwrap(api.GET('/api/wave/sessions/{id}', { params: { path: { id } } })))
      },
      async next(session, settings, count, exclude) {
        await ready()
        if (mock && mocked('wave')) return mock.waveNext(settings, count, exclude)
        // Выданное в сессии сервер не повторяет сам; сыгранное до смены настроек отсеиваем здесь
        const skip = new Set(exclude)
        const page = await unwrap(api.GET('/api/wave/sessions/{id}/next', {
          params: { path: { id: session }, query: { count: Math.min(30, count + 2) } },
        }))
        return page.items
          .filter(i => !skip.has(i.track.id))
          .slice(0, count)
          .map((i): WaveTrack => ({ track: i.track, reason: i.reason.text }))
      },
      async feedback(session, trackId, kind, positionMs) {
        await ready()
        if (mock && mocked('wave')) return
        await done(api.POST('/api/wave/sessions/{id}/feedback', {
          params: { path: { id: session } },
          body: { trackId, kind, positionMs: Math.max(0, Math.round(positionMs)) },
        }))
      },
      async why(trackId, session) {
        await ready()
        return unwrap(api.GET('/api/wave/why', { params: { query: { trackId, sessionId: session ?? undefined } } }))
      },
      async explain(trackId, session) {
        await ready()
        return unwrap(api.GET('/api/wave/explain', { params: { query: { trackId, sessionId: session ?? undefined } } }))
      },
      presets: {
        list: async () => (await unwrap(api.GET('/api/wave/presets'))).items,
        create: body => unwrap(api.POST('/api/wave/presets', { body })),
        update: (id, patch) => unwrap(api.PATCH('/api/wave/presets/{id}', { params: { path: { id } }, body: patch })),
        remove: id => done(api.DELETE('/api/wave/presets/{id}', { params: { path: { id } } })),
        reorder: async ids => (await unwrap(api.PUT('/api/wave/presets/order', { body: { ids } }))).items,
      },
    },

    media: {
      async source(track: Track, network?: NetworkKind): Promise<SourceResult> {
        await ready()
        if (mock && mocked('stream')) return mock.source(track)
        if (track.audioStatus === 'unavailable') return { kind: 'unavailable' }
        // Качество выбирает сервер по сети и настройкам слушателя; сайт считается подключённым по Wi-Fi
        const url = `/api/tracks/${track.id}/stream${network ? `?network=${network}` : ''}`
        let res = await http.raw('GET', url)
        let body = await res.json().catch(() => null) as StreamResponse | null
        // Своя музыка в формате, который плеер не играет: сервер делает копию за пару секунд
        for (let i = 0; i < PREPARE_TRIES && body?.status === 'preparing'; i++) {
          const wait = body.retryAfterMs ?? 2000
          await new Promise(r => setTimeout(r, Math.min(Math.max(wait, 500), 5000)))
          res = await http.raw('GET', url)
          body = await res.json().catch(() => null) as StreamResponse | null
        }
        if (res.status === 404 || !body || body.status === 'preparing') return { kind: 'unavailable' }
        if (!res.ok) throw toApiError(res.status, body)
        if (body.status === 'ready') {
          return {
            kind: 'ready',
            source: {
              url: http.url(body.url),
              durationMs: body.durationMs ?? undefined,
              gainDb: gainFor(body.loudnessLufs, body.truePeakDb),
              loudnessLufs: body.loudnessLufs ?? undefined,
              truePeakDb: body.truePeakDb ?? undefined,
              bpm: body.bpm ?? undefined,
              codec: body.codec ?? undefined,
              bitrateKbps: body.bitrateKbps ?? undefined,
            },
            // Сервер отдаёт огибающую в 0..1000, полоса прокрутки рисует 0..1
            peaks: body.waveform ? body.waveform.map(v => v / 1000) : null,
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

    updateDevice: (id, patch) => unwrap(api.PATCH('/api/devices/{id}', { params: { path: { id } }, body: patch })),

    jam: createJam(http),

    party: createParty(http),

    admin: {
      health: () => unwrap(api.GET('/api/healthz')),
      overview: () => unwrap(api.GET('/api/admin/overview')),
      // Все люди одной страницей, ждущие доступа — первыми
      users: () => unwrap(api.GET('/api/admin/users')),
      updateUser: (id, patch) => done(api.PATCH('/api/admin/users/{id}', { params: { path: { id } }, body: patch })),
      userTable: () => unwrap(api.GET('/api/admin/users/table')),
      userDetail: id => unwrap(api.GET('/api/admin/users/{id}', { params: { path: { id } } })),
      userHistory: (id, cursor, limit = 50) => unwrap(api.GET('/api/admin/users/{id}/history', { params: { path: { id }, query: { cursor: cursor ?? undefined, limit } } })),
      revokeUserSessions: async id => (await unwrap(api.DELETE('/api/admin/users/{id}/sessions', { params: { path: { id } } }))).revoked,
      deleteUser: (id, confirmName) => done(api.DELETE('/api/admin/users/{id}', { params: { path: { id } }, body: { confirmName } })),
      live: () => unwrap(api.GET('/api/admin/live')),
      envSettings: () => unwrap(api.GET('/api/admin/settings/env')),
      serverHealth: () => unwrap(api.GET('/api/admin/health')),
      backups: () => unwrap(api.GET('/api/admin/backups')),
      checkBackup: name => unwrap(api.POST('/api/admin/backups/check', { body: { name: name ?? null } })),
      releases: () => unwrap(api.GET('/api/admin/releases')),
      authOverview: () => unwrap(api.GET('/api/admin/auth')),
      createUser: user => unwrap(api.POST('/api/admin/users', { body: user })),
      resetLink: id => unwrap(api.POST('/api/admin/users/{id}/reset', { params: { path: { id } } })),
      invites: () => unwrap(api.GET('/api/admin/invites')),
      createInvite: invite => unwrap(api.POST('/api/admin/invites', { body: invite })),
      revokeInvite: id => done(api.DELETE('/api/admin/invites/{id}', { params: { path: { id } } })),
      sessions: userId => unwrap(api.GET('/api/admin/sessions', { params: { query: { userId } } })),
      revokeSession: id => done(api.DELETE('/api/admin/sessions/{id}', { params: { path: { id } } })),
      audit: (cursor, limit = 50, filter = {}) => unwrap(api.GET('/api/admin/audit', { params: { query: { cursor: cursor ?? undefined, limit, kind: filter.kind || undefined, actor: filter.actor || undefined } } })),
      jobs: (q = {}) => unwrap(api.GET('/api/admin/jobs', { params: { query: { lane: q.lane, status: q.status, kind: q.kind, cursor: q.cursor ?? undefined, limit: q.limit } } })),
      retryJob: id => done(api.POST('/api/admin/jobs/{id}/retry', { params: { path: { id } } })),
      cancelJob: id => done(api.POST('/api/admin/jobs/{id}/cancel', { params: { path: { id } } })),
      lanes: () => unwrap(api.GET('/api/admin/lanes')),
      updateLane: (lane, patch) => done(api.PATCH('/api/admin/lanes/{lane}', { params: { path: { lane } }, body: patch })),
      acquisitions: (q = {}) => unwrap(api.GET('/api/admin/acquisitions', { params: { query: { status: q.status, active: q.active, cursor: q.cursor ?? undefined, limit: q.limit } } })),
      acquisition: id => unwrap(api.GET('/api/admin/acquisitions/{id}', { params: { path: { id } } })),
      resolve: (id, req) => unwrap(api.POST('/api/admin/acquisitions/{id}/resolve', { params: { path: { id } }, body: req })),
      preview: trackId => unwrap(api.GET('/api/admin/tracks/{id}/preview', { params: { path: { id: trackId } } })),
      clipReports: () => unwrap(api.GET('/api/admin/clips/reports')),
      sources: () => unwrap(api.GET('/api/admin/sources')),
      checkDownload: trackId => unwrap(api.POST('/api/admin/sources/check', { body: { trackId } })),
      createDownloader: name => unwrap(api.POST('/api/admin/downloaders', { body: { name } })),
      revokeDownloader: id => done(api.DELETE('/api/admin/downloaders/{id}', { params: { path: { id } } })),
      recordings: (q = {}) => unwrap(api.GET('/api/admin/recordings', { params: { query: { status: q.status, source: q.source || undefined, verified: q.verified, q: q.q || undefined, cursor: q.cursor ?? undefined, limit: q.limit } } })),
      reacquire: (id, url) => unwrap(api.POST('/api/admin/recordings/{id}/reacquire', { params: { path: { id } }, body: { url } })),
      storage: () => unwrap(api.GET('/api/admin/storage')),
      library: () => unwrap(api.GET('/api/admin/library')),
      rescanLibrary: () => done(api.POST('/api/admin/library/scan')),
      imports: () => unwrap(api.GET('/api/admin/imports')),
      settings: () => unwrap(api.GET('/api/admin/settings')),
      updateSettings: patch => unwrap(api.PATCH('/api/admin/settings', { body: patch })),
      rewindEvent: () => unwrap(api.GET('/api/admin/rewind')),
      updateRewindEvent: patch => unwrap(api.PATCH('/api/admin/rewind', { body: patch })),
      reco: userId => unwrap(api.GET('/api/admin/reco', { params: { query: { userId: userId || undefined } } })),
      updateReco: patch => unwrap(api.PATCH('/api/admin/reco/settings', { body: patch })),
      runReco: (task, userId) => unwrap(api.POST('/api/admin/reco/run', { body: { task, userId: userId || undefined } })),
    },

    audio,
  }
  return bridge
}
