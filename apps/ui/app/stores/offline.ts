import { defineStore } from 'pinia'
import type { DownloadKind, OfflinePin, OfflineSettings, OfflineStatus, TrackDownload } from '~/bridge'

// Загрузки на устройство (приложение): что закреплено, ход загрузки каждого трека, место и настройки.
// На сайте `available` — false: пунктов «Скачать» и раздела «Загрузки» там нет
export const useOfflineStore = defineStore('offline', () => {
  const bridge = useBridge()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t
  const available = !!bridge.offline
  const status = ref<OfflineStatus | null>(null)
  const tracks = ref(new Map<string, TrackDownload>())
  let started = false

  async function refresh() {
    const off = bridge.offline
    if (!off) return
    try {
      const [s, list] = await Promise.all([off.status(), off.downloads()])
      status.value = s
      tracks.value = new Map(list.map(d => [d.trackId, d]))
    }
    catch {
      // Оболочка ответит в следующий раз — состояние придёт событием
    }
  }

  /** Подписаться на оболочку один раз: ход загрузок и состояние приходят событиями */
  function start() {
    const off = bridge.offline
    if (!off || started || !import.meta.client) return
    started = true
    void refresh()
    off.onStatus(s => (status.value = s))
    off.onProgress((d) => {
      tracks.value.set(d.trackId, d)
    })
    // iPhone: пока приложение спало, фоновая сессия докачала — показать сразу при возвращении
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') void refresh()
    })
  }

  const online = computed(() => status.value?.online ?? true)

  function pinOf(kind: DownloadKind, refId: string | null): OfflinePin | undefined {
    return status.value?.pins.find(p => p.kind === kind && p.refId === (kind === 'liked' ? null : refId))
  }

  function isPinned(kind: DownloadKind, refId: string | null): boolean {
    return !!pinOf(kind, refId)
  }

  function trackState(trackId: string): TrackDownload | undefined {
    return tracks.value.get(trackId)
  }

  /** Трек на устройстве целиком */
  function isDownloaded(trackId: string): boolean {
    return tracks.value.get(trackId)?.state === 'ready'
  }

  async function pin(kind: DownloadKind, refId: string | null, title?: string) {
    const off = bridge.offline
    if (!off) return
    try {
      status.value = await off.pin(kind, refId)
      toast.show({ text: title ? t('offline.pinned', { title }) : t('offline.pinnedPlain'), icon: 'ph:arrow-circle-down' })
    }
    catch {
      toast.error(t('offline.failed'))
    }
  }

  async function unpin(kind: DownloadKind, refId: string | null, title?: string) {
    const off = bridge.offline
    if (!off) return
    try {
      status.value = await off.unpin(kind, refId)
      toast.show({
        text: title ? t('offline.unpinned', { title }) : t('offline.unpinnedPlain'),
        icon: 'ph:trash',
        undo: () => void pin(kind, refId),
      })
    }
    catch {
      toast.error(t('offline.failed'))
    }
  }

  function toggle(kind: DownloadKind, refId: string | null, title?: string) {
    return isPinned(kind, refId) ? unpin(kind, refId, title) : pin(kind, refId, title)
  }

  async function setSettings(patch: Partial<OfflineSettings>) {
    const off = bridge.offline
    if (!off) return
    try {
      status.value = await off.setSettings(patch)
    }
    catch {
      toast.error(t('offline.failed'))
    }
  }

  async function clear() {
    const off = bridge.offline
    if (!off) return
    try {
      status.value = await off.clear()
      tracks.value = new Map()
      toast.show({ text: t('offline.cleared'), icon: 'ph:trash' })
    }
    catch {
      toast.error(t('offline.failed'))
    }
  }

  return {
    available, status, tracks, online, start, refresh, pinOf, isPinned, trackState, isDownloaded, pin, unpin, toggle,
    setSettings, clear,
  }
})
