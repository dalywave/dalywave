// Умный офлайн приложения (apps/shell/src/offline, BRIDGE.md «Умный офлайн»): локальная база медиатеки,
// загрузки на устройство, волна без сети, тактильный отклик.
//
// `withOffline` оборачивает мост приложения: медиатека читается из локальной базы (она синхронизирована
// с сервером в фоне), каталог и поиск без сети — тоже из неё, звук скачанного трека — из файла, волна без
// сети — из скачанного запаса. После изменений медиатеки база синхронизируется сразу, до ответа экрану.
import { ApiError } from '~/api/types'
import type {
  AlbumDetail, ArtistDetail, FollowedArtistPage, LibraryOverview, LikedTrackPage, PlaylistDetail, PlaylistList,
  SavedAlbumPage, SearchResults, Track,
} from '~/api/types'
import type {
  AppPush, Bridge, DownloadKind, HapticKind, OfflineBridge, OfflineReserve, OfflineSettings, OfflineStatus, PushPermission, SourceResult, SystemNotify, TrackDownload,
  WaveSettings, WaveStart, WaveTrack,
} from '../types'
import { gainFor } from '../web'

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
type Listen = <T>(event: string, handler: (e: { payload: T }) => void) => Promise<() => void>

let core: Promise<{ invoke: Invoke, listen: Listen }> | null = null
function tauri() {
  core ??= Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')])
    .then(([c, e]) => ({ invoke: c.invoke as Invoke, listen: e.listen as unknown as Listen }))
  return core
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return (await tauri()).invoke<T>(cmd, args)
}

/** Подписка на событие оболочки с отпиской синхронно */
function on<T>(event: string, cb: (payload: T) => void): () => void {
  let off: (() => void) | null = null
  let stopped = false
  void tauri().then(t => t.listen<T>(event, e => cb(e.payload))).then((unlisten) => {
    if (stopped) unlisten()
    else off = unlisten
  }).catch(() => {})
  return () => {
    stopped = true
    off?.()
  }
}

export function createOfflineBridge(): OfflineBridge {
  return {
    status: () => call<OfflineStatus>('offline_status'),
    onStatus: cb => on<OfflineStatus>('offline:state', cb),
    setSettings: (patch: Partial<OfflineSettings>) => call<OfflineStatus>('offline_set_settings', { patch }),
    sync: async () => {
      await call('offline_sync')
    },
    onSynced: cb => on<null>('offline:synced', () => cb()),
    pin: (kind: DownloadKind, refId: string | null) => call<OfflineStatus>('offline_pin', { pin: { kind, refId } }),
    unpin: (kind: DownloadKind, refId: string | null) => call<OfflineStatus>('offline_unpin', { pin: { kind, refId } }),
    clear: () => call<OfflineStatus>('offline_clear'),
    downloads: () => call<TrackDownload[]>('offline_downloads'),
    onProgress: cb => on<TrackDownload>('downloads:progress', cb),
    reserve: () => call<OfflineReserve>('offline_reserve'),
  }
}

/** iPhone: тактильный отклик; на компьютере оболочка ничего не делает */
export function haptic(kind: HapticKind) {
  void call('haptic', { kind }).catch(() => {})
}

/** Push приложения на iPhone и iPad (APNs): разрешение, токен устройства, число на значке */
export const appPush: AppPush = {
  state: () => call<{ supported: boolean, permission: PushPermission }>('push_state').catch(() => ({ supported: false, permission: 'default' as const })),
  register: prompt => call<{ token: string, environment: 'sandbox' | 'production' } | null>('push_register', { prompt }).catch(() => null),
  setBadge: (count) => {
    void call('push_set_badge', { count }).catch(() => {})
  },
}

/** Приложение для компьютера: системное уведомление, пока окно не в фокусе */
export const systemNotify: SystemNotify = {
  show: n => call<boolean>('notify_show', { id: n.id, title: n.title, body: n.body, link: n.link }).catch(() => false),
}

interface LocalSource {
  durationMs: number | null
  loudnessLufs: number | null
  truePeakDb: number | null
  waveform: number[] | null
}

/** Ошибка сети: без неё сервер ответил, и ответ надо показать как есть */
function isNetwork(e: unknown): boolean {
  return e instanceof ApiError ? e.status === 0 : e instanceof TypeError
}

const OFFLINE_WAVE = 'offline:'

export function withOffline(bridge: Bridge, offline: OfflineBridge): Bridge {
  let status: OfflineStatus | null = null
  void offline.status().then(s => (status = s)).catch(() => {})
  offline.onStatus(s => (status = s))
  const isOffline = () => !!status && !status.online
  // Чужие изменения медиатеки (другое устройство) — сначала в базу, экраны перечитают по offline:synced
  bridge.live.on('library.changed', () => void offline.sync().catch(() => {}))

  const read = <T>(what: string, args: Record<string, unknown> = {}) =>
    call<T | null>('offline_read', { query: { what, ...args } }).catch(() => null)

  /** Медиатека: из базы, если она уже синхронизирована, иначе — с сервера */
  async function local<T>(what: string, args: Record<string, unknown>, net: () => Promise<T>): Promise<T> {
    const l = await read<T>(what, args)
    return l ?? net()
  }

  /** Каталог: с сервера (там полнее), без сети или при сбое сети — из базы */
  async function network<T>(what: string, args: Record<string, unknown>, net: () => Promise<T>): Promise<T> {
    if (isOffline()) {
      const l = await read<T>(what, args)
      if (l) return l
      throw new ApiError(0, 'network', 'Нет связи с сервером — это не скачано')
    }
    try {
      return await net()
    }
    catch (e) {
      if (!isNetwork(e)) throw e
      const l = await read<T>(what, args)
      if (l) return l
      throw e
    }
  }

  /** После изменения медиатеки — синхронизировать базу, чтобы экраны сразу видели перемену */
  function synced<A extends unknown[], R>(f: (...a: A) => Promise<R>): (...a: A) => Promise<R> {
    return async (...a: A) => {
      const r = await f(...a)
      await offline.sync().catch(() => {})
      return r
    }
  }

  const lib = bridge.library
  const pl = bridge.playlists
  const cat = bridge.catalog
  const wave = bridge.wave
  const media = bridge.media
  const events = bridge.events

  async function offlineWave(session: string, settings: WaveSettings, count: number, exclude: string[]): Promise<WaveTrack[]> {
    return call<WaveTrack[]>('offline_wave_next', { session, settings, count, exclude })
  }

  return {
    ...bridge,
    catalog: {
      ...cat,
      album: id => network<AlbumDetail>('album', { id }, () => cat.album(id)),
      artist: id => network<ArtistDetail>('artist', { id }, () => cat.artist(id)),
      track: async (id) => {
        const tracks = await network<Track[]>('tracks', { ids: [id] }, async () => [await cat.track(id)])
        if (!tracks[0]) throw new ApiError(404, 'not_found', 'Трека нет')
        return tracks[0]
      },
      lookup: ids => network<Track[]>('tracks', { ids }, () => cat.lookup(ids)),
      search: (q, scope, limit) => network<SearchResults>('search', { q, limit }, () => cat.search(q, scope, limit)),
    },
    library: {
      ...lib,
      overview: () => local<LibraryOverview>('overview', {}, () => lib.overview()),
      liked: (cursor, limit) => local<LikedTrackPage>('liked', { cursor, limit }, () => lib.liked(cursor, limit)),
      savedAlbums: cursor => (cursor ? lib.savedAlbums(cursor) : local<SavedAlbumPage>('albums', {}, () => lib.savedAlbums(cursor))),
      followed: cursor => (cursor ? lib.followed(cursor) : local<FollowedArtistPage>('artists', {}, () => lib.followed(cursor))),
      like: synced(lib.like),
      unlike: synced(lib.unlike),
      saveAlbum: synced(lib.saveAlbum),
      unsaveAlbum: synced(lib.unsaveAlbum),
      follow: synced(lib.follow),
      unfollow: synced(lib.unfollow),
      block: synced(lib.block),
      unblock: synced(lib.unblock),
    },
    playlists: {
      ...pl,
      list: () => local<PlaylistList>('playlists', {}, () => pl.list()),
      get: id => local<PlaylistDetail>('playlist', { id }, () => pl.get(id)),
      create: synced(pl.create),
      update: synced(pl.update),
      remove: synced(pl.remove),
      addItems: synced(pl.addItems),
      removeItem: synced(pl.removeItem),
      removeItems: synced(pl.removeItems),
      setCover: synced(pl.setCover),
      removeCover: synced(pl.removeCover),
      duplicate: synced(pl.duplicate),
      moveItem: synced(pl.moveItem),
      applySet: synced(pl.applySet),
      setOrder: synced(pl.setOrder),
      pin: synced(pl.pin),
      move: synced(pl.move),
      createFolder: synced(pl.createFolder),
      updateFolder: synced(pl.updateFolder),
      removeFolder: synced(pl.removeFolder),
      moveFolder: synced(pl.moveFolder),
    },
    wave: {
      ...wave,
      // Без сети — волна из скачанного запаса; сессия живёт в оболочке
      async start(settings, seed) {
        const startOffline = async (): Promise<WaveStart> => {
          const id = `${OFFLINE_WAVE}${crypto.randomUUID()}`
          const queue = await offlineWave(id, settings, 6, [])
          return { id, state: 'ready', ready: queue.length, need: 0, queue }
        }
        if (isOffline()) return startOffline()
        try {
          return await wave.start(settings, seed)
        }
        catch (e) {
          if (isNetwork(e)) return startOffline()
          throw e
        }
      },
      async session(id) {
        if (id.startsWith(OFFLINE_WAVE)) return { id, state: 'ready', ready: 0, need: 0, queue: [] }
        return wave.session(id)
      },
      // Сеть пропала посреди волны — она продолжается из скачанного, не повторяя сыгранное
      async next(session, settings, count, exclude) {
        if (session.startsWith(OFFLINE_WAVE) || isOffline()) return offlineWave(session, settings, count, exclude)
        try {
          return await wave.next(session, settings, count, exclude)
        }
        catch (e) {
          if (isNetwork(e)) return offlineWave(session, settings, count, exclude)
          throw e
        }
      },
      async feedback(session, trackId, kind, positionMs) {
        void call('offline_wave_react', { session, trackId, kind, positionMs: Math.round(positionMs) }).catch(() => {})
        if (session.startsWith(OFFLINE_WAVE) || isOffline()) return
        await wave.feedback(session, trackId, kind, positionMs)
      },
    },
    media: {
      ...media,
      // Скачанный трек играет из файла: ссылка не нужна, громкость и огибающая — из базы
      async source(track): Promise<SourceResult> {
        const l = await call<LocalSource | null>('offline_source', { trackId: track.id }).catch(() => null)
        if (l) {
          return {
            kind: 'ready',
            source: {
              url: '',
              durationMs: l.durationMs ?? undefined,
              gainDb: gainFor(l.loudnessLufs, l.truePeakDb),
              loudnessLufs: l.loudnessLufs ?? undefined,
              truePeakDb: l.truePeakDb ?? undefined,
            },
            peaks: l.waveform ? l.waveform.map(v => v / 1000) : null,
          }
        }
        if (isOffline()) return { kind: 'unavailable' }
        // На сотовой — качество «По сотовой» (с «Экономией» — Opus 64 кбит/с)
        return media.source(track, status?.network === 'cellular' ? 'cellular' : 'wifi')
      },
    },
    events: {
      ...events,
      play(e) {
        events.play(e)
        void call('offline_played', {
          play: { trackId: e.trackId, playedMs: e.playedMs, durationMs: e.durationMs, endReason: e.endReason, wave: !!e.waveSessionId },
        }).catch(() => {})
      },
    },
    offline,
    haptic,
  }
}
