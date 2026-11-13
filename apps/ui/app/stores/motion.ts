import type { AlbumMotion, Track, TrackClip } from '~/api/types'

/** Ссылки подписаны на несколько часов: через час спрашиваем заново */
const FRESH_MS = 60 * 60 * 1000
/** Видео ищется — спросить снова через столько (дальше реже): клип загрузчик режет за 10–20 секунд,
 *  но перед ним может быть очередь */
const RETRY_MS = [8_000, 15_000, 30_000, 60_000, 120_000]

interface Entry {
  clip: TrackClip | null
  at: number
  tries: number
}

/**
 * Видео вместо обложки, как Canvas у Spotify: зацикленный кусок официального клипа трека без звука
 * (тот же, что в ленте «Открытия»), а нет клипа — живая обложка альбома, если Apple её отдаст.
 * Одна загрузка на трек; «ищется» перепрашивается несколько раз — сервер в это время заказывает
 * клип загрузчику. Показывается, только если разрешают настройки устройства, сеть и «уменьшение
 * движения» ([`allowed`])
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

  function pending(c: TrackClip | null) {
    return !c || c.state === 'pending' || c.motion?.state === 'pending'
  }

  async function load(trackId: string) {
    const prev = entries.get(trackId)
    const tries = (prev?.tries ?? 0) + 1
    try {
      const [clip] = await bridge.video.clips([trackId])
      entries.set(trackId, { clip: clip ?? null, at: Date.now(), tries })
      if (clip && pending(clip) && tries <= RETRY_MS.length) {
        clearTimeout(timers.get(trackId))
        timers.set(trackId, setTimeout(() => void load(trackId), RETRY_MS[tries - 1]))
      }
    }
    catch {
      // Сервер не ответил — останется обычная обложка; спросим при следующем показе через час
      entries.set(trackId, { clip: prev?.clip ?? null, at: Date.now(), tries })
    }
  }

  /** Заказать видео трека заранее (началось воспроизведение): к раскрытию плеера оно уже будет */
  function prefetch(trackId: string | null | undefined) {
    if (!trackId || !allowed.value) return
    const e = entries.get(trackId)
    if (e && Date.now() - e.at < FRESH_MS) return
    entries.set(trackId, { clip: e?.clip ?? null, at: Date.now(), tries: 0 })
    void load(trackId)
  }

  /** Видео для обложки трека: клип, иначе живая обложка альбома; `tall` — для вертикального фона */
  function src(track: Pick<Track, 'id'> | null | undefined, shape: 'square' | 'tall' = 'square'): string | null {
    if (!track || !allowed.value) return null
    // Заказ — не во время вычисления: состояние меняется уже после него
    if (!entries.has(track.id)) queueMicrotask(() => prefetch(track.id))
    const c = entries.get(track.id)?.clip
    if (c?.state === 'ready' && c.url) return c.url
    const m: AlbumMotion | null | undefined = c?.motion?.state === 'ready' ? c.motion : null
    if (!m) return null
    return shape === 'tall' ? (m.tall ?? m.square) : (m.square ?? m.tall)
  }

  /** Видео, которые пришли вместе с лентой: лишний запрос не нужен */
  function remember(clip: TrackClip) {
    if (!pending(clip)) entries.set(clip.trackId, { clip, at: Date.now(), tries: 0 })
  }

  /** У трека показывается клип (не живая обложка) — для пунктов «Клип» и «Скрыть клип» в меню */
  function hasClip(trackId: string | null | undefined): boolean {
    const c = trackId ? entries.get(trackId)?.clip : null
    return c?.state === 'ready' && !!c.url
  }

  /** Клип трека сменили (выбор администратора, «Скрыть клип»): спросить заново, без старой ссылки */
  function refresh(trackId: string) {
    clearTimeout(timers.get(trackId))
    entries.set(trackId, { clip: null, at: Date.now(), tries: 0 })
    void load(trackId)
  }

  return { allowed, cellular, src, prefetch, remember, hasClip, refresh }
})
