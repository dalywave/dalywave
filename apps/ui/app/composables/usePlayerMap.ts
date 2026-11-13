import type { ComputedRef } from 'vue'
import type { WaveExplain } from '~/api/types'
import type { PlayerView } from '~/stores/prefs'

/** «Как подобран» в полном плеере: карта вкуса вместо обложки — только у трека, который подобрала
 *  волна. Выбор «Обложка / Как подобран» запоминается на устройстве (prefs.playerView). Карта
 *  загружается, когда её видно; не нашлась (трек без анализа звучания, показ давно удалён) —
 *  тихо остаётся обложка, а переключатель у этого трека прячется */
export function usePlayerMap(opts: { open: ComputedRef<boolean> }) {
  const player = usePlayerStore()
  const prefs = usePrefsStore()
  const bridge = useBridge()

  const entry = computed(() => player.current)
  const trackId = computed(() => entry.value?.track.id ?? null)
  /** Тот же признак, что у «Почему этот трек» в меню: запись волны с подписью */
  const eligible = computed(() => !!entry.value && entry.value.source === 'wave' && !!entry.value.reason)

  const data = shallowRef<WaveExplain | null>(null)
  const loading = ref(false)
  /** Треки, для которых карты нет: переключатель у них прячется */
  const missing = reactive(new Set<string>())
  const cache = new Map<string, WaveExplain>()

  const available = computed(() => eligible.value && !!trackId.value && !missing.has(trackId.value))
  const view = computed<PlayerView>({
    get: () => prefs.playerView,
    set: v => (prefs.playerView = v),
  })
  /** Карта на месте обложки (пока грузится — тоже: без прыжка обложка → карта) */
  const on = computed(() => available.value && view.value === 'map')

  let run = 0
  async function load(id: string) {
    const hit = cache.get(id)
    if (hit) {
      data.value = hit
      return
    }
    const mine = ++run
    loading.value = true
    data.value = null
    try {
      const res = await bridge.wave.explain(id, player.waveSessionId)
      if (cache.size > 20) cache.delete(cache.keys().next().value!)
      cache.set(id, res)
      if (mine === run) data.value = res
    }
    catch {
      if (mine === run) missing.add(id)
    }
    finally {
      if (mine === run) loading.value = false
    }
  }

  watch([trackId, on, opts.open], ([id, isOn, isOpen]) => {
    if (!id || !isOn || !isOpen) return
    if (data.value?.track.id === id) return
    void load(id)
  }, { immediate: true })

  // Свежая выдача — свежие следы: старое из кэша не нужно
  watch(() => player.waveSessionId, () => cache.clear())

  return reactive({ available, on, view, data, loading })
}

export type PlayerMap = ReturnType<typeof usePlayerMap>
