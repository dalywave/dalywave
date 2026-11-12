// Только разработка: загрузки на устройство изображает мок — для снимков экранов в браузере.
// Включается `localStorage['dw.offlineMock'] = JSON.stringify({ albumId, trackIds, playlistId? })`:
// альбом закреплён и качается (первые треки уже на устройстве, один — на 40 %), все лайки — тоже.
// `reserveIds` — умный запас: всё на устройстве, кроме двух последних (одно качается, одно ждёт).
import type { DownloadKind, OfflineBridge, OfflinePin, OfflineSettings, OfflineStatus, TrackDownload } from '../types'

interface Scenario {
  albumId?: string
  trackIds?: string[]
  playlistId?: string
  reserveIds?: string[]
}

const MB = 1024 * 1024
const GB = 1024 * MB

export function createDevOffline(raw: string): OfflineBridge {
  let sc: Scenario = {}
  try {
    sc = JSON.parse(raw) as Scenario
  }
  catch {
    // Пустой сценарий — ничего не закреплено
  }
  const ids = sc.trackIds ?? []
  const downloads: TrackDownload[] = ids.map((trackId, i) => {
    const total = 5 * MB + i * 137_000
    if (i < Math.ceil(ids.length / 2)) return { trackId, state: 'ready', received: total, total }
    if (i === Math.ceil(ids.length / 2)) return { trackId, state: 'downloading', received: Math.round(total * 0.4), total }
    return { trackId, state: 'queued', received: 0, total }
  })
  const ready = downloads.filter(d => d.state === 'ready').length
  const reserveIds = sc.reserveIds ?? []
  const reserveReady = reserveIds.slice(0, Math.max(0, reserveIds.length - 2))
  reserveIds.forEach((trackId, i) => {
    if (downloads.some(d => d.trackId === trackId)) return
    const total = 4 * MB + i * 91_000
    if (i < reserveReady.length) downloads.push({ trackId, state: 'ready', received: total, total })
    else if (i === reserveReady.length) downloads.push({ trackId, state: 'downloading', received: Math.round(total * 0.6), total })
    else downloads.push({ trackId, state: 'queued', received: 0, total })
  })
  const pins: OfflinePin[] = []
  if (sc.albumId) pins.push({ kind: 'album', refId: sc.albumId, tracksTotal: ids.length, tracksReady: ready, bytes: ready * 5 * MB })
  if (sc.playlistId) pins.push({ kind: 'playlist', refId: sc.playlistId, tracksTotal: 24, tracksReady: 24, bytes: 118 * MB })
  pins.push({ kind: 'liked', refId: null, tracksTotal: 183, tracksReady: 171, bytes: 842 * MB })
  let settings: OfflineSettings = { limitBytes: 8 * GB, wifiOnly: true, reserveEnabled: true, forceOffline: false }
  const statusCbs = new Set<(s: OfflineStatus) => void>()

  const reserveBytes = () => (!settings.reserveEnabled ? 0 : reserveIds.length ? reserveReady.length * Math.round(4.4 * MB) : Math.round(2.4 * GB))
  const status = (): OfflineStatus => ({
    online: !settings.forceOffline,
    network: 'wifi',
    synced: true,
    syncedAt: new Date().toISOString(),
    syncing: false,
    settings,
    usage: { pinnedBytes: 960 * MB + ready * 5 * MB, reserveBytes: reserveBytes(), cacheBytes: 310 * MB, totalBytes: 960 * MB + ready * 5 * MB + reserveBytes() + 310 * MB, files: 712 },
    downloads: { active: 1, queued: Math.max(0, ids.length - ready - 1), failed: 0, overLimit: 0, waitingForWifi: false },
    pins: [...pins],
    reserve: reserveIds.length && settings.reserveEnabled
      ? { tracks: reserveIds.length, ready: reserveReady.length, builtAt: new Date().toISOString() }
      : { tracks: settings.reserveEnabled ? 480 : 0, ready: settings.reserveEnabled ? 452 : 0, builtAt: new Date().toISOString() },
  })
  const emit = () => {
    const s = status()
    statusCbs.forEach(cb => cb(s))
    return s
  }
  const find = (kind: DownloadKind, refId: string | null) => pins.findIndex(p => p.kind === kind && p.refId === refId)

  return {
    status: async () => status(),
    onStatus(cb) {
      statusCbs.add(cb)
      return () => statusCbs.delete(cb)
    },
    async setSettings(patch) {
      settings = { ...settings, ...patch }
      return emit()
    },
    async sync() {},
    onSynced: () => () => {},
    async pin(kind, refId) {
      if (find(kind, refId) < 0) pins.push({ kind, refId: kind === 'liked' ? null : refId, tracksTotal: 10, tracksReady: 0, bytes: 0 })
      return emit()
    },
    async unpin(kind, refId) {
      const i = find(kind, refId)
      if (i >= 0) pins.splice(i, 1)
      return emit()
    },
    async clear() {
      pins.splice(0)
      return emit()
    },
    downloads: async () => downloads,
    onProgress: () => () => {},
    reserve: async () => (settings.reserveEnabled
      ? { trackIds: [...reserveIds], readyIds: [...reserveReady], builtAt: new Date().toISOString() }
      : { trackIds: [], readyIds: [], builtAt: null }),
  }
}
