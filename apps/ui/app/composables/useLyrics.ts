import type { LyricsLine, TrackLyrics } from '~/api/types'

// Тексты песен: один кеш на всё приложение — панель, лист и плеер не спрашивают сервер дважды.
// Найденный текст не меняется; «не нашли» и ошибка через несколько минут спрашиваются снова
const cache = new Map<string, { at: number, data: TrackLyrics }>()
const inflight = new Map<string, Promise<TrackLyrics>>()
const MAX_CACHED = 60
const RETRY_NONE_MS = 10 * 60_000

export type LyricsState =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'ready', data: TrackLyrics }

function fresh(id: string): TrackLyrics | null {
  const hit = cache.get(id)
  if (!hit) return null
  if (hit.data.kind === 'none' && Date.now() - hit.at > RETRY_NONE_MS) return null
  return hit.data
}

function fetchLyrics(bridge: ReturnType<typeof useBridge>, id: string): Promise<TrackLyrics> {
  const known = fresh(id)
  if (known) return Promise.resolve(known)
  let p = inflight.get(id)
  if (!p) {
    p = bridge.catalog.lyrics(id).then((data) => {
      cache.set(id, { at: Date.now(), data })
      if (cache.size > MAX_CACHED) cache.delete(cache.keys().next().value!)
      return data
    }).finally(() => inflight.delete(id))
    inflight.set(id, p)
  }
  return p
}

/** Текст трека по id; смена id — новый запрос (или кеш) */
export function useTrackLyrics(trackId: Ref<string | null | undefined>) {
  const bridge = useBridge()
  const state = shallowRef<LyricsState>({ status: 'idle' })
  let run = 0

  async function load(id: string | null | undefined) {
    const mine = ++run
    if (!id) {
      state.value = { status: 'idle' }
      return
    }
    const known = fresh(id)
    if (known) {
      state.value = { status: 'ready', data: known }
      return
    }
    state.value = { status: 'loading' }
    try {
      const data = await fetchLyrics(bridge, id)
      if (mine === run) state.value = { status: 'ready', data }
    }
    catch {
      if (mine === run) state.value = { status: 'error' }
    }
  }

  watch(trackId, id => void load(id), { immediate: true })
  return { state, retry: () => load(trackId.value) }
}

/** Индекс текущей строки: последняя, что началась к `ms`; -1 — до первой */
export function lineAt(lines: LyricsLine[], ms: number): number {
  let lo = 0
  let hi = lines.length - 1
  let found = -1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (lines[mid]!.tMs <= ms) {
      found = mid
      lo = mid + 1
    }
    else hi = mid - 1
  }
  return found
}

/** Позиция плеера с ходом между обновлениями: строка меняется вовремя, а не раз в четверть секунды
 *  (у пульта позиция приходит ещё реже) */
export function usePlaybackClock(active: Ref<boolean>) {
  const player = usePlayerStore()
  const ms = ref(player.positionMs)
  let base = player.positionMs
  let baseAt = performance.now()
  let raf = 0

  watch(() => player.positionMs, (v) => {
    base = v
    baseAt = performance.now()
    ms.value = v
  })

  function tick() {
    raf = 0
    if (!active.value) return
    ms.value = player.playing && !player.pending ? base + (performance.now() - baseAt) : base
    raf = requestAnimationFrame(tick)
  }

  watch([active, () => player.playing], ([on]) => {
    base = player.positionMs
    baseAt = performance.now()
    ms.value = base
    if (on && !raf) raf = requestAnimationFrame(tick)
  }, { immediate: true })

  onBeforeUnmount(() => {
    if (raf) cancelAnimationFrame(raf)
  })
  return ms
}
