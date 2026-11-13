import { defineStore } from 'pinia'
import type { Track, TrackVersion, TrackVersions, VersionKind } from '~/api/types'

/** Порядок переключателя: оригинал, затем медленнее, затем быстрее */
const ORDER: VersionKind[] = ['original', 'slowed', 'super_slowed', 'sped_up']

/**
 * Версии играющей песни в один клик: «Оригинал», «Slowed», «Super slowed», «Sped up» — только те,
 * что выпустил сам исполнитель (сервер их находит, звук никто не обрабатывает). Переключение — с той
 * же доли песни, наложением. Версии, которой ещё нет на сервере, сервер качает вне очереди — плеер
 * переключится, когда она будет готова, а пока играет прежняя.
 *
 * Выбор запоминается по исполнителю: у него следующий трек в очереди заранее заменяется той же
 * версией (если она уже на сервере).
 */
export const useVersionsStore = defineStore('versions', () => {
  const bridge = useBridge()
  const player = usePlayerStore()
  const library = useLibraryStore()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t

  /** Найденные версии по id трека (у всех треков семьи — одна и та же запись) */
  const byTrack = shallowReactive(new Map<string, TrackVersions>())
  const loading = new Map<string, Promise<TrackVersions | null>>()
  /** Выбор слушателя: исполнитель → версия (оригинал — нет записи) */
  const choices = ref<Map<string, VersionKind> | null>(null)
  /** Версия качается, плеер переключится на неё, когда будет готова */
  const waiting = ref<{ from: string, version: TrackVersion } | null>(null)

  const label = (kind: VersionKind) => t(`versions.${kind}`)

  function remember(v: TrackVersions) {
    for (const item of v.items) {
      byTrack.set(item.track.id, { ...v, trackId: item.track.id, kind: item.kind })
    }
  }

  async function load(trackId: string): Promise<TrackVersions | null> {
    const known = byTrack.get(trackId)
    if (known) return known
    let run = loading.get(trackId)
    if (!run) {
      run = bridge.catalog.versions(trackId).then((v) => {
        remember(v)
        return byTrack.get(trackId) ?? v
      }).catch(() => null).finally(() => loading.delete(trackId))
      loading.set(trackId, run)
    }
    return run
  }

  async function loadChoices(): Promise<Map<string, VersionKind>> {
    if (choices.value) return choices.value
    try {
      const list = await bridge.catalog.versionChoices()
      choices.value = new Map(list.map(c => [c.artistId, c.kind]))
    }
    catch {
      choices.value = new Map()
    }
    return choices.value
  }

  const currentTrack = computed(() => player.current?.track ?? null)
  /** Версии играющей песни; одна (только сам трек) — переключателя нет */
  const current = computed<TrackVersions | null>(() => {
    const tr = currentTrack.value
    const v = tr ? byTrack.get(tr.id) : undefined
    return v && v.items.length > 1 ? v : null
  })
  const available = computed(() => !!current.value && !player.mirrored && !player.previewing)
  const options = computed(() => [...(current.value?.items ?? [])].sort((a, b) => ORDER.indexOf(a.kind) - ORDER.indexOf(b.kind)))

  function ready(track: Track) {
    return library.statusOf(track) === 'ready'
  }

  /** Переключить играющий трек на версию */
  async function switchTo(kind: VersionKind) {
    const v = current.value
    const from = currentTrack.value
    const version = v?.items.find(i => i.kind === kind)
    if (!v || !from || !version || version.track.id === from.id) {
      waiting.value = null
      return
    }
    const artist = from.artists[0]?.id
    void bridge.catalog.chooseVersion(from.id, kind).catch(() => {})
    if (artist && choices.value) {
      if (kind === 'original') choices.value.delete(artist)
      else choices.value.set(artist, kind)
    }
    for (const item of v.items) {
      const known = byTrack.get(item.track.id)
      if (known) byTrack.set(item.track.id, { ...known, preferred: kind === 'original' ? null : kind })
    }
    if (ready(version.track)) {
      waiting.value = null
      await player.swapCurrent(version.track)
      return
    }
    // Нет на сервере: сервер качает её вне очереди (как при обычном включении), а пока играет эта
    waiting.value = { from: from.id, version }
    const result = await bridge.media.source(version.track).catch(() => ({ kind: 'unavailable' as const }))
    if (waiting.value?.version.track.id !== version.track.id) return
    if (result.kind === 'ready') {
      library.setAudioStatus(version.track.id, 'ready')
      finishWaiting()
    }
    else if (result.kind === 'unavailable') {
      waiting.value = null
      toast.show({ text: t('versions.unavailable', { kind: label(kind) }), icon: 'ph:cloud-slash', tone: 'error' })
    }
    else toast.show({ text: t('versions.soon', { kind: label(kind) }), icon: 'ph:hourglass-medium' })
  }

  function finishWaiting() {
    const w = waiting.value
    waiting.value = null
    // Человек уже ушёл с этой песни — не трогаем
    if (!w || currentTrack.value?.id !== w.from) return
    void player.swapCurrent(w.version.track)
  }

  if (import.meta.client) {
    bridge.live.on('acquisition.updated', (e) => {
      const w = waiting.value
      if (!w || w.version.track.id !== e.trackId) return
      if (e.audioStatus === 'ready') finishWaiting()
      else if (e.audioStatus === 'unavailable' || e.audioStatus === 'failed') {
        waiting.value = null
        toast.show({ text: t('versions.unavailable', { kind: label(w.version.kind) }), icon: 'ph:cloud-slash', tone: 'error' })
      }
    })

    // Играющая песня: версии — для переключателя
    watch(() => currentTrack.value?.id, (id) => {
      if (waiting.value && waiting.value.from !== id && waiting.value.version.track.id !== id) waiting.value = null
      if (id) void load(id)
    }, { immediate: true })

    // Следующий в очереди: у исполнителя выбрана другая версия — ставим её заранее, если она готова
    watch(() => player.nextEntry?.uid, async () => {
      const entry = player.nextEntry
      const artist = entry?.track.artists[0]?.id
      if (!entry || !artist || entry.source === 'manual') return
      const wanted = (await loadChoices()).get(artist)
      if (!wanted) return
      const v = await load(entry.track.id)
      const version = v?.items.find(i => i.kind === wanted)
      if (!v || v.kind === wanted || !version || !ready(version.track)) return
      if (player.nextEntry?.uid !== entry.uid) return
      player.replaceQueued(entry.uid, version.track)
    })
  }

  return { byTrack, current, available, options, waiting, label, load, switchTo }
})
