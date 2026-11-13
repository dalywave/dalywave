import type { AlbumMotion } from '~/api/types'

/** Ссылки подписаны на несколько часов: через час спрашиваем заново */
const FRESH_MS = 60 * 60 * 1000
/** Обложка ищется — спросить снова через столько (дальше реже), не больше нескольких раз */
const RETRY_MS = [6_000, 15_000, 40_000]

interface Entry {
  motion: AlbumMotion | null
  at: number
  tries: number
}

/**
 * Живые обложки альбомов (Apple Music): одна загрузка на альбом, ответ «ищется» перепрашивается
 * несколько раз. Видео показывается, только если его разрешают настройки устройства, сеть
 * и «уменьшение движения» ([`allowed`])
 */
export const useMotionStore = defineStore('motion', () => {
  const bridge = useBridge()
  const prefs = usePrefsStore()
  const ui = useUiStore()
  const offline = useOfflineStore()

  const entries = shallowReactive(new Map<string, Entry>())
  const timers = new Map<string, ReturnType<typeof setTimeout>>()

  /** Сеть сотовая: приложение знает точно, браузер — иногда (Network Information API) */
  const cellular = computed(() => {
    if (offline.status?.network === 'cellular') return true
    if (!import.meta.client) return false
    const conn = (navigator as Navigator & { connection?: { type?: string } }).connection
    return conn?.type === 'cellular'
  })

  /** Видео без звука можно показывать: включено, не «только по Wi-Fi» на сотовой, движение не уменьшено */
  const allowed = computed(() => prefs.motion && !ui.reducedMotion && !(prefs.motionWifiOnly && cellular.value))

  async function load(albumId: string) {
    const prev = entries.get(albumId)
    try {
      const motion = await bridge.video.albumMotion(albumId)
      const tries = (prev?.tries ?? 0) + 1
      entries.set(albumId, { motion, at: Date.now(), tries })
      if (motion.state === 'pending' && tries <= RETRY_MS.length) {
        clearTimeout(timers.get(albumId))
        timers.set(albumId, setTimeout(() => void load(albumId), RETRY_MS[tries - 1]))
      }
    }
    catch {
      // Сервер не ответил — останется обычная обложка; спросим при следующем показе через час
      entries.set(albumId, { motion: null, at: Date.now(), tries: prev?.tries ?? 0 })
    }
  }

  /** Живая обложка альбома, если она есть и готова; запрос — при первом обращении */
  function get(albumId: string | null | undefined): AlbumMotion | null {
    if (!albumId || !allowed.value) return null
    const e = entries.get(albumId)
    if (!e || Date.now() - e.at > FRESH_MS) {
      if (!e) entries.set(albumId, { motion: null, at: Date.now(), tries: 0 })
      void load(albumId)
    }
    const m = e?.motion
    return m?.state === 'ready' ? m : null
  }

  /** Обложки, которые пришли вместе с клипами ленты: лишний запрос не нужен */
  function remember(motion: AlbumMotion) {
    entries.set(motion.albumId, { motion, at: Date.now(), tries: 0 })
  }

  return { allowed, cellular, get, remember }
})
