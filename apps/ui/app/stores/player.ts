import { defineStore } from 'pinia'
import type { ConnectQueueItem, ContextType, EndReason, PlaybackState, RemoteCommand, Track, WaveFeedbackKind, WaveSeed } from '~/api/types'
import type { EngineTrack, MediaCommand, RepeatMode, PlaybackDevice, SourceResult, TrackSource, WaveMood, WaveSettings, WaveStart, WaveTrack } from '~/bridge/types'
import { getWaveEngine } from '~/lib/wave/engine'
import { coverColors } from '~/utils/cover-colors'
import { afterTrack, mixIn, nextRepeat, previousAction, restoreQueue, roundsNeeded, shuffleQueue, shuffled } from '~/utils/queue-order'
import { resamplePeaks, syntheticRhythm } from '~/utils/peaks'
import { sameSettings } from '~/utils/wave-options'

export type TransitionMode = 'gapless' | 'smooth' | 'smart' | 'dj'
export type QueueSource = 'manual' | 'context' | 'wave'

export interface QueueEntry {
  uid: string
  track: Track
  source: QueueSource
  reason?: string
  /** Повтор всей очереди: какой это круг списка (первый — 0) */
  round?: number
  /** «Подобрано» умным перемешиванием: не из списка, можно добавить в плейлист или убрать */
  suggested?: boolean
}

export interface PlayContext {
  type: ContextType
  id?: string | null
  title: string
}

/** Трек выбран, но файла на сервере ещё нет: сервер скачивает его или ждёт загрузчик */
export interface PendingState {
  uid: string
  status: 'acquiring' | 'waiting_for_downloader'
  queuePosition: number | null
}

/** Волна готовится: сервер скачивает первые треки под настройки — `ready` из `need` уже на месте */
export interface WavePrep {
  session: string
  ready: number
  need: number
}

// Длина наложения по режимам. «Диджей» на сайте работает как «Умный» — сведение умеют приложения
const FADE_MS: Record<TransitionMode, number> = { gapless: 0, smooth: 6000, smart: 4000, dj: 4000 }
const WAVE_BUFFER = 5
const PEAK_BARS = 120
/** Приложение: сколько треков очереди движок знает наперёд — доиграет их и в фоне, без интерфейса */
const ENGINE_AHEAD = 20
/** Подготовка волны: как часто спрашивать сервер, если событий о скачивании нет */
const PREP_POLL_MS = 2500
/** Сервер сам начинает с готового через полторы минуты; это — на случай, если он замолчал */
const PREP_GIVE_UP_MS = 110_000

interface Listen {
  entry: QueueEntry
  startedAt: string
  positionStartMs: number
  playedMs: number
  lastPos: number
}

export const usePlayerStore = defineStore('player', () => {
  const bridge = useBridge()
  const engine = bridge.audio
  // Приложение: звук в движке на Rust — он сам переходит к следующему треку и слушает пульт системы
  const native = engine.native
  const toast = useToast()
  const library = useLibraryStore()
  // Хранилище живёт и вне компонентов (горячие клавиши, системный плеер) — берём общий i18n
  const t = useNuxtApp().$i18n.t

  const current = ref<QueueEntry | null>(null)
  const manual = ref<QueueEntry[]>([])
  const upcoming = ref<QueueEntry[]>([])
  const history = ref<QueueEntry[]>([])
  const context = ref<PlayContext | null>(null)
  const playing = ref(false)
  const buffering = ref(false)
  /** Приложение: системный вывод звука не отвечает — движок ждёт и повторяет сам */
  const soundLost = ref(false)
  const positionMs = ref(0)
  const durationMs = ref(0)
  const source = ref<TrackSource | null>(null)
  const pending = ref<PendingState | null>(null)
  const volume = useLocalStorage('dw.volume', 0.72)
  const muted = ref(false)
  const transition = useLocalStorage<TransitionMode>('dw.transition', 'smart')
  const expressiveness = useLocalStorage('dw.expressiveness', 0.6)
  const normalize = useLocalStorage('dw.normalize', true)
  const wave = useLocalStorage<WaveSettings>('dw.wave', { mood: 'any', activity: 'any', character: 'mix', language: 'any' }, { mergeDefaults: true })
  const waveSessionId = ref<string | null>(null)
  const waveLoading = ref(false)
  const wavePrep = ref<WavePrep | null>(null)
  /** Волна или радио запускается (сервер подбирает и, если надо, докачивает начало): что именно — для подписи */
  const waveStarting = ref<string | null>(null)
  /** Форма волны текущего трека, 0…1 */
  const peaks = ref<number[]>([])
  const devices = ref<PlaybackDevice[]>([])

  let listen: Listen | null = null
  let transitioning = false
  let prefetched: { uid: string, result: SourceResult } | null = null
  /** Треки волны с явной реакцией («не нравится», «не сейчас»): их пропуск — уже не отдельный сигнал */
  const reacted = new Set<string>()
  /** Треки волны, которые уже пропускали вперёд, пока они докачивались: второй раз — мимо */
  const deferred = new Set<string>()
  /** Номер текущей подготовки волны: новая подготовка или «Отменить» делают прежнюю ненужной */
  let prepRun = 0

  const waveActive = computed(() => context.value?.type === 'wave')
  const effectiveMode = computed<TransitionMode>(() => (transition.value === 'dj' && bridge.kind === 'web' ? 'smart' : transition.value))
  const fadeMs = computed(() => FADE_MS[effectiveMode.value])
  const transitionAtMs = computed(() => (effectiveMode.value === 'gapless' ? durationMs.value : Math.max(0, durationMs.value - fadeMs.value - 400)))
  const progress = computed(() => (durationMs.value ? Math.min(1, positionMs.value / durationMs.value) : 0))
  const nextEntry = computed(() => manual.value[0] ?? upcoming.value[0] ?? null)

  const visual = () => (import.meta.client ? getWaveEngine() : null)

  // ---------- Внешний вид: настроение, обложка ----------

  function applyMood(mood: WaveMood, instant = false) {
    if (import.meta.client) document.documentElement.dataset.mood = mood
    visual()?.setMood(mood, { instant })
  }

  async function applyCover(track: Track) {
    const url = track.album?.image?.small
    const c = (await coverColors(url ? bridge.resolveUrl(url) : null)) ?? null
    if (!c || current.value?.track.id !== track.id) return
    const root = document.documentElement.style
    root.setProperty('--amb-1', c.vivid)
    root.setProperty('--amb-2', c.second)
    root.setProperty('--amb-3', c.deep)
    visual()?.setCover([c.vivid, c.deep])
  }

  // ---------- Прослушивания ----------

  function beginListen(entry: QueueEntry, startMs: number) {
    listen = { entry, startedAt: new Date().toISOString(), positionStartMs: startMs, playedMs: 0, lastPos: startMs }
  }

  function finishListen(reason: EndReason) {
    const l = listen
    listen = null
    if (!l || l.playedMs < 500) return
    bridge.events.play({
      trackId: l.entry.track.id,
      startedAt: l.startedAt,
      playedMs: Math.round(l.playedMs),
      positionStartMs: Math.round(l.positionStartMs),
      durationMs: Math.round(durationMs.value || l.entry.track.durationMs),
      endReason: reason,
      contextType: context.value?.type ?? null,
      contextId: context.value?.id ?? null,
      waveSessionId: waveActive.value ? waveSessionId.value : null,
      source: 'stream',
      transition: effectiveMode.value === 'smooth' ? 'crossfade' : effectiveMode.value,
    })
  }

  // ---------- Очередь ----------

  const entryOf = (track: Track, src: QueueSource, reason?: string): QueueEntry => ({ uid: crypto.randomUUID(), track, source: src, reason })

  function takeNext(): QueueEntry | null {
    return manual.value.shift() ?? upcoming.value.shift() ?? null
  }

  async function sourceFor(entry: QueueEntry): Promise<SourceResult> {
    if (prefetched?.uid === entry.uid && prefetched.result.kind === 'ready') return prefetched.result
    try {
      return await bridge.media.source(entry.track)
    }
    catch {
      return { kind: 'unavailable' }
    }
  }

  function prefetchNext() {
    const next = nextEntry.value
    if (!next || prefetched?.uid === next.uid) return
    const uid = next.uid
    void bridge.media.source(next.track).then((result) => {
      if (nextEntry.value?.uid !== uid) return
      prefetched = { uid, result }
      // Движок получит готовую ссылку и громкость следующего трека — меньше запросов в фоне
      syncUpcoming()
    }).catch(() => {})
  }

  async function refillWave() {
    const session = waveSessionId.value
    if (!waveActive.value || !session || waveLoading.value || upcoming.value.length >= WAVE_BUFFER) return
    waveLoading.value = true
    try {
      const exclude = [current.value, ...history.value.slice(-50), ...upcoming.value, ...manual.value].filter((e): e is QueueEntry => !!e).map(e => e.track.id)
      const more = await bridge.wave.next(session, wave.value, WAVE_BUFFER + 1 - upcoming.value.length, exclude)
      // Пока ждали ответ, настройки сменились: выдача прежней сессии уже не к месту
      if (session !== waveSessionId.value) return
      upcoming.value.push(...more.map((w: WaveTrack) => entryOf(w.track, 'wave', w.reason)))
      prefetchNext()
    }
    catch {
      // Волна продолжит с того, что уже в очереди
    }
    finally {
      waveLoading.value = false
    }
  }

  // ---------- Воспроизведение ----------

  function attachAudioTap() {
    const v = visual()
    if (!v) return
    const src = source.value
    // Приложение: анализатора нет — волна берёт уровень из событий движка
    if (native && current.value) v.setAudioTap({ levels: () => engine.levels(), time: () => engine.audioTime(), bpm: src?.bpm, phase: src?.beatPhase })
    else if (src && engine.analyser) v.setAudioTap({ analyser: engine.analyser, time: () => engine.audioTime(), bpm: src.bpm, phase: src.beatPhase })
    else v.setAudioTap(null)
  }

  function withGain(src: TrackSource | null): TrackSource | null {
    if (!src || normalize.value) return src
    return { ...src, gainDb: undefined, loudnessLufs: undefined, truePeakDb: undefined }
  }

  /** Трек очереди для движка приложения: метаданные для «Сейчас играет» и ссылка, если она уже есть */
  function engineTrack(entry: QueueEntry, src: TrackSource | null): EngineTrack {
    const tr = entry.track
    const art = tr.album?.image?.large ?? tr.album?.image?.medium ?? null
    return {
      uid: entry.uid,
      trackId: tr.id,
      title: tr.title,
      artist: tr.artists.map(a => a.name).join(', '),
      album: tr.album?.title ?? null,
      artworkUrl: art ? bridge.resolveUrl(art) : null,
      durationMs: src?.durationMs ?? tr.durationMs,
      source: withGain(src),
    }
  }

  /** Приложение: очередь после текущего — движку, чтобы он готовил следующий трек и переходил сам */
  function syncUpcoming() {
    if (!native || !current.value || pending.value || mirrored.value) return
    const ahead = [...manual.value, ...upcoming.value].slice(0, ENGINE_AHEAD)
    engine.setUpcoming(ahead.map((e) => {
      const known = prefetched?.uid === e.uid && prefetched.result.kind === 'ready' ? prefetched.result.source : null
      return engineTrack(e, known)
    }))
  }

  async function startEntry(entry: QueueEntry, opts: { startMs?: number, autoplay?: boolean, crossfade?: boolean, fadeMs?: number } = {}) {
    current.value = entry
    pending.value = null
    const startMs = opts.startMs ?? 0
    positionMs.value = startMs
    void applyCover(entry.track)
    const result = await sourceFor(entry)
    if (current.value?.uid !== entry.uid) return

    if (result.kind === 'unavailable') {
      toast.show({ text: t('player.unavailable', { title: entry.track.title }), icon: 'ph:cloud-slash', tone: 'error' })
      if (nextEntry.value || waveActive.value) {
        if (current.value) history.value.push(current.value)
        if (!nextEntry.value) await refillWave()
        const next = takeNext()
        if (next) return startEntry(next, { autoplay: opts.autoplay })
      }
      engine.load(null, { durationMs: entry.track.durationMs })
      pause()
      return
    }

    if (result.kind === 'pending') {
      // В волне не ждём: трек подобран под настройки и докачивается — он сыграет через пару треков,
      // а пока — следующий. Не успел и во второй раз — мимо
      if (waveActive.value && entry.source === 'wave' && (nextEntry.value || upcoming.value.length)) {
        if (deferred.has(entry.uid)) history.value.push(entry)
        else {
          deferred.add(entry.uid)
          upcoming.value.splice(Math.min(2, upcoming.value.length), 0, entry)
        }
        const next = takeNext()
        if (next) return startEntry(next, opts.crossfade ? { autoplay: true } : opts)
      }
      pending.value = { uid: entry.uid, status: result.status, queuePosition: result.queuePosition }
      source.value = null
      durationMs.value = entry.track.durationMs
      engine.load(null, { durationMs: entry.track.durationMs })
      engine.pause()
      playing.value = false
      visual()?.setPlaying(false)
      peaks.value = []
      publishNowPlaying()
      return
    }

    const src = withGain(result.source)
    source.value = src
    durationMs.value = src?.durationMs ?? entry.track.durationMs
    if (opts.crossfade) engine.crossfade(src, { durationMs: durationMs.value, fadeMs: opts.fadeMs ?? fadeMs.value, startMs })
    else engine.load(src, { durationMs: durationMs.value, startMs, track: engineTrack(entry, result.source) })
    syncUpcoming()
    beginListen(entry, startMs)
    const rhythm = syntheticRhythm(entry.track.id)
    const v = visual()
    v?.setBpm(src?.bpm ?? rhythm.bpm)
    v?.setTrackEnergy(rhythm.energy)
    if (result.peaks?.length) peaks.value = resamplePeaks(result.peaks, PEAK_BARS)
    else {
      void bridge.media.waveform(entry.track, PEAK_BARS).then((p) => {
        if (current.value?.uid === entry.uid) peaks.value = p
      }).catch(() => {})
    }
    if (opts.autoplay !== false && !opts.crossfade) await play()
    else if (opts.crossfade) attachAudioTap()
    publishNowPlaying()
    prefetchNext()
    void refillWave()
  }

  async function play() {
    if (routed({ action: 'play' })) return
    if (!current.value) return startWave()
    if (pending.value) return
    await engine.play()
    playing.value = engine.playing
    visual()?.setPlaying(playing.value)
    attachAudioTap()
    publishNowPlaying()
  }

  function pause() {
    if (routed({ action: 'pause' })) return
    engine.pause()
    playing.value = false
    visual()?.setPlaying(false)
    publishNowPlaying()
  }

  function toggle() {
    if (routed({ action: 'toggle' })) return
    return playing.value ? pause() : play()
  }

  async function next(reason: EndReason = 'skipped') {
    if (routed({ action: 'next' }, () => next(reason))) return
    if (transitioning) return
    if (reason === 'completed' && await replayIfRepeating()) return
    finishListen(reason)
    const prev = current.value
    waveOutcome(prev, reason, positionMs.value)
    let entry = takeNext()
    if (!entry && waveActive.value) {
      await refillWave()
      entry = takeNext()
    }
    if (!entry && wrapRepeat()) entry = takeNext()
    if (prev) history.value.push(prev)
    if (!entry) {
      pause()
      engine.seek(0)
      positionMs.value = 0
      return
    }
    if (reason === 'skipped') visual()?.skip()
    await startEntry(entry, { autoplay: true })
  }

  async function prev() {
    if (routed({ action: 'previous' }, prev)) return
    if (previousAction(positionMs.value, history.value.length > 0) === 'restart') {
      seek(0)
      return
    }
    finishListen('previous')
    const back = history.value.pop()!
    if (current.value) manual.value.unshift(current.value)
    visual()?.skip()
    await startEntry(back, { autoplay: true })
  }

  function seek(ms: number) {
    if (routed({ action: 'seek', positionMs: Math.max(0, Math.round(ms)) }, () => seek(ms))) return
    const clamped = Math.max(0, Math.min(durationMs.value || 0, ms))
    engine.seek(clamped)
    positionMs.value = clamped
    if (listen) listen.lastPos = clamped
    publishNowPlaying()
  }

  /** Играть список (альбом, плейлист, подборку) с позиции */
  async function playTracks(tracks: Track[], index: number, ctx: PlayContext, opts: { shuffle?: boolean } = {}) {
    if (!tracks.length) return
    // «Без нецензурного»: чистые версии вместо explicit, без них — мимо; нажатый скрытый — «Скрыто фильтром»
    const allowed = await useAccountSettings().clean(tracks, index, { strict: !opts.shuffle })
    if (!allowed) return
    tracks = allowed.tracks
    index = allowed.index
    // Пульт: где заиграет, решит сервер; «здесь» — этот же вызов, когда плеер снова свой
    if (routed(remotePlayTracks(tracks, index, ctx, opts), () => playTracks(tracks, index, ctx, opts))) return
    // «Перемешать» включает перемешивание насовсем, как в Spotify: со случайного трека
    if (opts.shuffle) shuffle.value = true
    const entries = tracks.map(tr => entryOf(tr, 'context'))
    const start = opts.shuffle ? Math.floor(Math.random() * entries.length) : Math.min(Math.max(0, index), entries.length - 1)
    finishListen('skipped')
    if (current.value) history.value.push(current.value)
    if (context.value?.type !== ctx.type || context.value?.id !== ctx.id) smartDropped.clear()
    context.value = ctx
    contextTracks = tracks.slice()
    round = 0
    if (shuffle.value) {
      // Выбранный трек — первым, остальное (и то, что было до него) — вразброс
      upcoming.value = shuffled(entries.filter((_, i) => i !== start))
      originalOrder = entries
    }
    else {
      upcoming.value = entries.slice(start + 1)
      originalOrder = null
    }
    prefetched = null
    smartRun++
    await startEntry(entries[start]!, { autoplay: true })
    void mixInSuggestions()
  }

  // ---------- Версии песни: оригинал, slowed, sped up ----------

  /** Наложение при смене версии: короче перехода между треками — это та же песня */
  const VERSION_FADE_MS = 1200

  /**
   * Заменить играющий трек другой версией той же песни: с той же доли песни (у slowed она длиннее),
   * наложением, если играет. Очередь и история не меняются, прослушивание прежней закрывается
   */
  async function swapCurrent(track: Track) {
    const cur = current.value
    if (!cur || cur.track.id === track.id || mirrored.value || previewing.value) return
    const share = durationMs.value ? Math.min(1, positionMs.value / durationMs.value) : 0
    const wasPlaying = playing.value
    finishListen('stopped')
    const entry: QueueEntry = { ...cur, uid: crypto.randomUUID(), track }
    const startMs = Math.max(0, Math.round(share * track.durationMs))
    await startEntry(entry, wasPlaying && !native
      ? { startMs, crossfade: true, fadeMs: VERSION_FADE_MS }
      : { startMs, autoplay: wasPlaying })
  }

  /** Трек в очереди (ещё не играл) заменить другой версией: выбранная у исполнителя версия */
  function replaceQueued(uid: string, track: Track) {
    for (const list of [manual, upcoming]) {
      const i = list.value.findIndex(e => e.uid === uid)
      if (i < 0) continue
      list.value.splice(i, 1, { ...list.value[i]!, uid: crypto.randomUUID(), track })
      if (prefetched?.uid === uid) prefetched = null
      prefetchNext()
      return
    }
  }

  // ---------- Подготовка волны ----------

  /** Пока волна готовится и ничего не играет, она «дышит» — медленно и тихо */
  function breathe(on: boolean) {
    const v = visual()
    if (!v || playing.value) return
    if (on) {
      v.setAudioTap(null)
      v.setBpm(66)
      v.setEnergy(0.28)
      v.setPlaying(true)
    }
    else {
      v.setEnergy(null)
      v.setPlaying(false)
    }
  }

  /** Ждёт, пока сервер подготовит начало: событие о скачанном треке очереди будит сразу, иначе — опрос.
   *  `show` — показывать «Готовим волну…» (не нужно, если музыка волны уже играет). null — подготовку
   *  отменили или начали новую */
  async function awaitPrepared(first: WaveStart, run: number, show = true): Promise<WaveStart | null> {
    let s = first
    if (s.state === 'ready') return s
    if (show) wavePrep.value = { session: s.id, ready: s.ready, need: s.need }
    const ids = new Set(s.queue.map(w => w.track.id))
    let wake: (() => void) | null = null
    const off = bridge.live.on('acquisition.updated', (e) => {
      if (ids.has(e.trackId) && e.audioStatus !== 'queued' && e.audioStatus !== 'acquiring') wake?.()
    })
    const began = Date.now()
    try {
      while (run === prepRun) {
        await new Promise<void>((resolve) => {
          wake = resolve
          window.setTimeout(resolve, PREP_POLL_MS)
        })
        wake = null
        if (run !== prepRun) return null
        try {
          s = await bridge.wave.session(s.id)
        }
        catch {
          // Сеть моргнула — спросим ещё раз
          continue
        }
        if (run !== prepRun) return null
        s.queue.forEach(w => ids.add(w.track.id))
        if (show) wavePrep.value = { session: s.id, ready: s.ready, need: s.need }
        if (s.state === 'ready' || Date.now() - began > PREP_GIVE_UP_MS) return s
      }
      return null
    }
    finally {
      off()
      if (show && run === prepRun) wavePrep.value = null
    }
  }

  /** «Отменить» на «Готовим волну…»: что играло — играет дальше */
  function cancelWavePrep() {
    if (!wavePrep.value && !waveLoading.value && !waveStarting.value) return
    prepRun++
    wavePrep.value = null
    waveLoading.value = false
    waveStarting.value = null
    breathe(false)
  }

  /** «Моя волна»: бесконечный поток под текущие настройки. Начало сервер подбирает под настройки
   *  и, если его ещё нет на сервере, сначала скачивает: до тех пор — «Готовим волну…», а то, что играло, играет */
  async function startWave(settings?: Partial<WaveSettings>, radio?: { seed: WaveSeed, title: string }) {
    // Радио от затравки пульт пока не передаёт — оно запускается здесь
    if (!radio && routed({ action: 'startWave', settings: settings ? { ...wave.value, ...settings } : null }, () => startWave(settings))) return
    if (settings) wave.value = { ...wave.value, ...settings }
    applyMood(wave.value.mood)
    const run = ++prepRun
    wavePrep.value = null
    waveLoading.value = true
    waveStarting.value = radio?.title ?? t('wave.title')
    let start: WaveStart | null
    try {
      // Сессия на сервере помнит выданное и реакции; её id — и в событиях прослушивания
      start = await bridge.wave.start(wave.value, radio?.seed)
      if (run !== prepRun) return
      if (start.state === 'preparing') {
        breathe(true)
        start = await awaitPrepared(start, run)
      }
    }
    catch {
      if (run === prepRun) toast.error(t('player.waveFailed'))
      start = null
    }
    finally {
      if (run === prepRun) {
        waveLoading.value = false
        wavePrep.value = null
        waveStarting.value = null
        breathe(false)
      }
    }
    if (!start || run !== prepRun) return
    if (!start.queue.length) {
      toast.show({ text: t('player.waveEmpty'), icon: 'ph:heart' })
      return
    }
    finishListen('skipped')
    if (current.value) history.value.push(current.value)
    waveSessionId.value = start.id
    resetOrder()
    context.value = { type: 'wave', id: start.id, title: radio?.title ?? t('wave.title') }
    upcoming.value = start.queue.slice(1).map(w => entryOf(w.track, 'wave', w.reason))
    prefetched = null
    visual()?.start()
    await startEntry(entryOf(start.queue[0]!.track, 'wave', start.queue[0]!.reason), { autoplay: true })
  }

  function playNext(track: Track) {
    // «Без нецензурного»: чистая версия вместо explicit, без неё — «Скрыто фильтром»
    const account = useAccountSettings()
    if (account.hideExplicit && track.explicit) return void account.clean([track], 0).then(r => r?.tracks[0] && playNext(r.tracks[0]))
    if (routed({ action: 'playNext', track }, () => playNext(track))) return
    manual.value.unshift(entryOf(track, 'manual'))
    prefetched = null
    prefetchNext()
    bridge.events.feedback({ kind: 'queue_add', trackId: track.id })
    toast.show({ text: t('player.playNextDone', { title: track.title }), icon: 'ph:arrow-bend-down-right', undo: () => removeTrack(track.id, 'manual') })
  }

  function enqueue(track: Track) {
    const account = useAccountSettings()
    if (account.hideExplicit && track.explicit) return void account.clean([track], 0).then(r => r?.tracks[0] && enqueue(r.tracks[0]))
    if (routed({ action: 'enqueue', track }, () => enqueue(track))) return
    manual.value.push(entryOf(track, 'manual'))
    bridge.events.feedback({ kind: 'queue_add', trackId: track.id })
    toast.show({ text: t('player.enqueued', { title: track.title }), icon: 'ph:list-plus' })
  }

  /** Альбом или плейлист целиком — в конец очереди, одним уведомлением */
  function enqueueMany(tracks: Track[], title: string) {
    const account = useAccountSettings()
    if (account.hideExplicit && tracks.some(tr => tr.explicit)) return void account.clean(tracks, 0, { strict: false }).then(r => r && enqueueMany(r.tracks, title))
    if (tracks.length <= 1) {
      if (tracks[0]) enqueue(tracks[0])
      return
    }
    for (const tr of tracks) {
      if (routed({ action: 'enqueue', track: tr })) continue
      manual.value.push(entryOf(tr, 'manual'))
      bridge.events.feedback({ kind: 'queue_add', trackId: tr.id })
    }
    toast.show({ text: t('player.enqueued', { title }), icon: 'ph:list-plus' })
  }

  function removeTrack(trackId: string, from: QueueSource) {
    const list = from === 'manual' ? manual : upcoming
    const i = list.value.findIndex(e => e.track.id === trackId)
    if (i >= 0) list.value.splice(i, 1)
  }

  function remove(uid: string) {
    if (routed({ action: 'remove', uid })) return
    for (const list of [manual, upcoming]) {
      const i = list.value.findIndex(e => e.uid === uid)
      if (i < 0) continue
      const [removed] = list.value.splice(i, 1)
      toast.show({ text: t('player.removed', { title: removed!.track.title }), icon: 'ph:trash', undo: () => list.value.splice(i, 0, removed!) })
      return
    }
  }

  function clearManual() {
    if (routed({ action: 'clearManual' })) return
    const was = manual.value
    manual.value = []
    toast.show({ text: t('player.manualCleared'), icon: 'ph:trash', undo: () => (manual.value = [...was, ...manual.value]) })
  }

  /** Перетаскивание в очереди «добавлено вручную» */
  function moveManual(from: number, to: number) {
    if (routed({ action: 'moveManual', from, to })) return
    const [e] = manual.value.splice(from, 1)
    if (e) manual.value.splice(to, 0, e)
  }

  /** Нажатие на трек в очереди: играть его сейчас. Пропущенное из волны или списка уходит */
  async function jumpTo(uid: string) {
    if (routed({ action: 'jumpTo', uid }, () => jumpTo(uid))) return
    let entry: QueueEntry | undefined
    const mi = manual.value.findIndex(e => e.uid === uid)
    if (mi >= 0) {
      entry = manual.value.splice(mi, 1)[0]
    }
    else {
      const ui = upcoming.value.findIndex(e => e.uid === uid)
      if (ui < 0) return
      entry = upcoming.value[ui]
      upcoming.value = upcoming.value.slice(ui + 1)
    }
    if (!entry) return
    finishListen('skipped')
    waveOutcome(current.value, 'skipped', positionMs.value)
    if (current.value) history.value.push(current.value)
    visual()?.skip()
    await startEntry(entry, { autoplay: true })
  }

  /** Приложение: движок сам перешёл к другому элементу очереди — догоняем очередь и историю */
  async function onEngineAdvance(uid: string, reason: 'auto' | 'user') {
    if (current.value?.uid === uid) {
      // Повтор одного трека: движок сам начал его заново
      if (reason === 'auto') onReplayed()
      return
    }
    const prevEntry = current.value
    let entry: QueueEntry | undefined
    const mi = manual.value.findIndex(e => e.uid === uid)
    const ui = upcoming.value.findIndex(e => e.uid === uid)
    const hi = history.value.findIndex(e => e.uid === uid)
    if (mi >= 0 || ui >= 0) {
      // Дальше по очереди: сам (без пауз, кроссфейдом) или с пульта. Пропущенные движком
      // треки (не открылись) из очереди уходят
      finishListen(reason === 'auto' ? 'completed' : 'skipped')
      waveOutcome(prevEntry, reason === 'auto' ? 'completed' : 'skipped', positionMs.value)
      if (prevEntry) history.value.push(prevEntry)
      if (mi >= 0) {
        entry = manual.value[mi]
        manual.value = manual.value.slice(mi + 1)
      }
      else {
        entry = upcoming.value[ui]
        manual.value = []
        upcoming.value = upcoming.value.slice(ui + 1)
      }
    }
    else if (hi >= 0) {
      // «Предыдущий» с пульта
      finishListen('previous')
      entry = history.value.splice(hi, 1)[0]
      if (prevEntry) manual.value.unshift(prevEntry)
    }
    if (!entry) return
    current.value = entry
    pending.value = null
    positionMs.value = 0
    durationMs.value = engine.durationMs || entry.track.durationMs
    source.value = null
    peaks.value = []
    beginListen(entry, 0)
    void applyCover(entry.track)
    const rhythm = syntheticRhythm(entry.track.id)
    const v = visual()
    v?.setBpm(rhythm.bpm)
    v?.setTrackEnergy(rhythm.energy)
    if (reason === 'user') v?.skip()
    attachAudioTap()
    // Форма волны и доли — из ответа сервера о файле (тот же, что у движка)
    const known = await sourceFor(entry)
    if (current.value?.uid === entry.uid && known.kind === 'ready') {
      source.value = withGain(known.source)
      if (known.source?.bpm) v?.setBpm(known.source.bpm)
      if (known.peaks?.length) peaks.value = resamplePeaks(known.peaks, PEAK_BARS)
      attachAudioTap()
    }
    if (current.value?.uid === entry.uid && !peaks.value.length) {
      void bridge.media.waveform(entry.track, PEAK_BARS).then((p) => {
        if (current.value?.uid === entry.uid) peaks.value = p
      }).catch(() => {})
    }
    prefetchNext()
    void refillWave()
  }

  // ---------- Устройства ----------

  // Список ведёт хранилище управления между устройствами (stores/connect.ts): оно же пишет сюда devices
  async function loadDevices() {
    const connect = useConnectStore()
    if (connect.started) return connect.refresh()
    devices.value = await bridge.devices().catch(() => devices.value)
  }

  function selectDevice(d: PlaybackDevice) {
    useConnectStore().select(d)
  }

  // ---------- Реакции волны ----------

  /** Трек волны в очереди: текущий или следующие */
  function waveEntryOf(track: Track): QueueEntry | null {
    return [current.value, ...upcoming.value].find(e => e?.source === 'wave' && e.track.id === track.id) ?? null
  }

  /** Реакция на трек волны — её сессии: сервер учтёт её в следующей выдаче и сам запишет событие */
  function waveFeedback(entry: QueueEntry, kind: WaveFeedbackKind, atMs: number = positionMs.value): boolean {
    const session = waveSessionId.value
    if (entry.source !== 'wave' || !session) return false
    void bridge.wave.feedback(session, entry.track.id, kind, atMs).catch(() => {})
    return true
  }

  /** Чем кончился трек волны: дослушан или пропущен (если до этого не было явной реакции) */
  function waveOutcome(entry: QueueEntry | null, reason: EndReason, atMs: number) {
    if (!entry || reacted.has(entry.uid)) return
    if (reason === 'completed') waveFeedback(entry, 'completed', atMs)
    else if (reason === 'skipped') waveFeedback(entry, 'skip', atMs)
  }

  function feedbackFor(kind: 'dislike' | 'undislike' | 'not_now' | 'more_like_this', track: Track) {
    if (kind !== 'undislike') {
      const entry = waveEntryOf(track)
      if (entry && waveFeedback(entry, kind)) {
        if (kind !== 'more_like_this') reacted.add(entry.uid)
        return
      }
    }
    bridge.events.feedback({ kind, trackId: track.id, contextType: context.value?.type ?? null, contextId: context.value?.id ?? null, waveSessionId: waveActive.value ? waveSessionId.value : null })
  }

  async function dislike(track: Track | null = current.value?.track ?? null) {
    if (!track) return
    if (mirrored.value && current.value?.track.id === track.id && routed({ action: 'dislike' })) return
    feedbackFor('dislike', track)
    toast.show({ text: t('player.disliked', { title: track.title }), icon: 'ph:prohibit', undo: () => feedbackFor('undislike', track) })
    if (current.value?.track.id === track.id) await next('skipped')
    else removeTrack(track.id, 'context')
  }

  async function notNow() {
    if (routed({ action: 'notNow' })) return
    const track = current.value?.track
    if (!track) return
    feedbackFor('not_now', track)
    toast.show({ text: t('player.notNow'), icon: 'ph:arrow-bend-up-right' })
    await next('skipped')
  }

  function moreLikeThis() {
    if (routed({ action: 'moreLikeThis' })) return
    const track = current.value?.track
    if (!track) return
    feedbackFor('more_like_this', track)
    visual()?.like()
    toast.show({ text: t('player.moreLikeThis', { title: track.title }), icon: 'ph:sparkle' })
  }

  async function toggleLike(track: Track | null = current.value?.track ?? null) {
    if (!track) return
    const willLike = !library.isLiked(track)
    if (willLike && current.value?.track.id === track.id) visual()?.like()
    await library.toggleLike(track)
    // Лайк в волне — ещё и сигнал её сессии: «что сейчас заходит» сдвигается к этому треку
    const entry = willLike ? waveEntryOf(track) : null
    if (entry) waveFeedback(entry, 'like')
  }

  // ---------- Настройки волны ----------

  function setWave(patch: Partial<WaveSettings>) {
    // Пульт: новые настройки — волне на активном устройстве (свои настройки волны не трогаем)
    if (mirrored.value && waveActive.value && routed({ action: 'setWave', settings: { ...wave.value, ...patch } })) return
    const before = { ...wave.value }
    wave.value = { ...wave.value, ...patch }
    if (patch.mood && patch.mood !== before.mood) applyMood(patch.mood)
    if ((wavePrep.value || waveLoading.value) && !waveActive.value && !sameSettings(before, wave.value)) {
      // Волна ещё готовится — готовим заново под новые настройки
      void startWave()
      return
    }
    if (waveActive.value && !sameSettings(before, wave.value)) {
      // Уже сыгранное остаётся, дальше волна идёт под новые настройки — новой сессией на сервере
      upcoming.value = []
      prefetched = null
      void retune()
      if (!patch.mood || Object.keys(patch).length > 1) toast.show({ text: t('player.waveRetuned'), icon: 'ph:waveform' })
    }
  }

  /** Новые настройки посреди волны: текущий трек доигрывает, а дальше — начало, подобранное под них.
   *  Пока оно докачивается, очередь держит то, что уже готово */
  async function retune() {
    const run = ++prepRun
    let start: WaveStart | null = null
    try {
      start = await bridge.wave.start(wave.value)
      if (run !== prepRun) return
      waveSessionId.value = start.id
      if (context.value?.type === 'wave') context.value = { ...context.value, id: start.id }
      upcoming.value = start.queue.map(w => entryOf(w.track, 'wave', w.reason))
      prefetched = null
      prefetchNext()
      if (start.state === 'preparing') start = await awaitPrepared(start, run, false)
    }
    catch {
      // Сервер не ответил: волна продолжит прежней сессией
    }
    if (run !== prepRun || !start || waveSessionId.value !== start.id) return
    // Готовое — вперёд; что из начала уже сыграно, не повторяется
    const played = new Set([current.value, ...history.value].filter((e): e is QueueEntry => !!e).map(e => e.track.id))
    const order = new Map(start.queue.map((w, i) => [w.track.id, i]))
    upcoming.value = upcoming.value
      .filter(e => !played.has(e.track.id))
      .sort((a, b) => (order.get(a.track.id) ?? 1e9) - (order.get(b.track.id) ?? 1e9))
    prefetched = null
    prefetchNext()
    await refillWave()
  }

  /** Сохранённое сочетание: волна играет — меняются настройки, нет — волна стартует с ними */
  async function applyPreset(set: WaveSettings) {
    if (waveActive.value) setWave(set)
    else await startWave(set)
  }

  // ---------- Громкость ----------

  function setVolume(v: number) {
    volume.value = Math.min(1, Math.max(0, v))
    if (volume.value > 0 && muted.value) muted.value = false
  }

  function toggleMute() {
    muted.value = !muted.value
  }

  // ---------- Управление между устройствами (stores/connect.ts) ----------
  // Пульт: в плеере — снимок активного устройства, звук здесь не играет, действия уходят командами.
  // Всё переключение — в функциях connect*; действия плеера только спрашивают routed()

  /** Плеер показывает другое устройство (пульт или «Продолжить здесь») */
  const mirrored = ref(false)
  let connectRoute: ((cmd: RemoteCommand, local?: () => unknown) => boolean) | null = null

  /** Хранилище управления подключает маршрут команд: true — действие ушло на другое устройство */
  function setConnectRoute(fn: ((cmd: RemoteCommand, local?: () => unknown) => boolean) | null) {
    connectRoute = fn
  }

  function routed(cmd: RemoteCommand, local?: () => unknown): boolean {
    return mirrored.value && !!connectRoute?.(cmd, local)
  }

  /** «Играть альбом там»: устройство загрузит треки по id (не больше 500 — окно от выбранного) */
  function remotePlayTracks(tracks: Track[], index: number, ctx: PlayContext, opts: { shuffle?: boolean }): RemoteCommand {
    const from = opts.shuffle ? 0 : Math.max(0, Math.min(index, tracks.length - 1))
    return {
      action: 'playTracks',
      trackIds: tracks.slice(from, from + 500).map(tr => tr.id),
      index: 0,
      context: { type: ctx.type, id: ctx.id ?? null, title: ctx.title },
      shuffle: !!opts.shuffle,
    }
  }

  const toItem = (e: QueueEntry): ConnectQueueItem => ({ uid: e.uid, track: e.track, source: e.source, reason: e.reason ?? null })
  const fromItem = (q: ConnectQueueItem): QueueEntry => ({ uid: q.uid, track: q.track, source: q.source, reason: q.reason ?? undefined })

  /** Снимок для пультов: очередь — окном (ближайшее и немного истории), чтобы сообщение было небольшим */
  function connectSnapshot(): PlaybackState {
    const ctx = context.value
    return {
      current: current.value ? toItem(current.value) : null,
      manual: manual.value.slice(0, 50).map(toItem),
      upcoming: upcoming.value.slice(0, 100).map(toItem),
      history: history.value.slice(-20).map(toItem),
      context: ctx ? { type: ctx.type, id: ctx.id ?? null, title: ctx.title } : null,
      playing: playing.value,
      pending: pending.value?.status ?? null,
      positionMs: Math.max(0, Math.round(positionMs.value)),
      durationMs: Math.max(0, Math.round(durationMs.value || current.value?.track.durationMs || 0)),
      volume: volume.value,
      muted: muted.value,
      transition: transition.value,
      shuffle: shuffle.value,
      repeat: repeat.value,
      waveSessionId: waveActive.value ? waveSessionId.value : null,
      wave: waveActive.value ? { ...wave.value } : null,
      peaks: peaks.value.map(p => Math.round(p * 100) / 100),
    }
  }

  /** Звук переходит на другое устройство: здесь всё останавливается, плеер становится пультом */
  function connectYield() {
    if (mirrored.value) return
    prepRun++
    wavePrep.value = null
    waveLoading.value = false
    finishListen('transferred')
    transitioning = false
    prefetched = null
    if (current.value) {
      engine.pause()
      // Приложение: очередь движка очищается — медиаклавиши здесь больше не играют старое
      engine.load(null, { durationMs: 0 })
    }
    mirrored.value = true
    playing.value = false
    buffering.value = false
    visual()?.setPlaying(false)
    if (import.meta.client) bridge.media.nowPlaying(null)
  }

  /** Пульт: показать снимок активного устройства; позиция — уже досчитанная на сейчас */
  function connectMirror(s: PlaybackState, atMs: number) {
    connectYield()
    const cur = s.current ? fromItem(s.current) : null
    const changed = current.value?.track.id !== cur?.track.id
    current.value = cur
    manual.value = (s.manual ?? []).map(fromItem)
    upcoming.value = (s.upcoming ?? []).map(fromItem)
    history.value = (s.history ?? []).map(fromItem)
    context.value = s.context ? { type: s.context.type, id: s.context.id ?? null, title: s.context.title } : null
    playing.value = !!s.playing
    pending.value = s.pending && cur ? { uid: cur.uid, status: s.pending, queuePosition: null } : null
    positionMs.value = atMs
    durationMs.value = s.durationMs || cur?.track.durationMs || 0
    waveSessionId.value = s.waveSessionId ?? null
    remoteModes.value = { shuffle: !!s.shuffle, repeat: s.repeat ?? 'off' }
    source.value = null
    if (s.peaks?.length) peaks.value = s.peaks
    else if (changed) peaks.value = []
    if (cur && changed) {
      void applyCover(cur.track)
      if (!s.peaks?.length) {
        void bridge.media.waveform(cur.track, PEAK_BARS).then((p) => {
          if (mirrored.value && current.value?.uid === cur.uid && !peaks.value.length) peaks.value = p
        }).catch(() => {})
      }
    }
  }

  /** Это устройство продолжает с того же места с той же очередью (перенос или «Продолжить здесь») */
  async function connectTakeOver(s: PlaybackState | null, atMs: number, autoplay: boolean) {
    mirrored.value = false
    adoptModes(s)
    const cur = s?.current ? fromItem(s.current) : null
    manual.value = (s?.manual ?? []).map(fromItem)
    upcoming.value = (s?.upcoming ?? []).map(fromItem)
    history.value = (s?.history ?? []).map(fromItem)
    context.value = s?.context ? { type: s.context.type, id: s.context.id ?? null, title: s.context.title } : null
    waveSessionId.value = s?.waveSessionId ?? null
    prefetched = null
    if (!cur) {
      current.value = null
      playing.value = false
      pending.value = null
      positionMs.value = 0
      durationMs.value = 0
      peaks.value = []
      return
    }
    await startEntry(cur, { startMs: Math.max(0, atMs), autoplay })
  }

  /** «Продолжить здесь» не нужно — человек сам включил здесь другое: плеер снова свой */
  function connectLeave() {
    mirrored.value = false
    remoteModes.value = null
    playing.value = false
    pending.value = null
  }

  // ---------- Таймер сна (stores/sound.ts) ----------
  // Приложение: таймер в движке. Сайт: «в конце трека» — трек доигрывает без перехода, следующий
  // встаёт в начало и ждёт на паузе

  const stopAtTrackEnd = ref(false)

  async function advancePaused() {
    stopAtTrackEnd.value = false
    finishListen('completed')
    const prevEntry = current.value
    waveOutcome(prevEntry, 'completed', positionMs.value)
    let entry = takeNext()
    if (!entry && waveActive.value) {
      await refillWave()
      entry = takeNext()
    }
    if (prevEntry) history.value.push(prevEntry)
    if (!entry) {
      pause()
      engine.seek(0)
      positionMs.value = 0
      return
    }
    await startEntry(entry, { autoplay: false })
    pause()
  }

  // ---------- Превью («Итоги года»): кусок трека через тот же движок, без истории ----------
  // Музыка историй ведёт utils/rewind-audio.ts. Пока она звучит, плеер стоит: прослушивания не пишутся,
  // очередь не меняется, события движка не трогают плеер. Пульт: звук только здесь, активное
  // устройство на это время встаёт на паузу и продолжает после

  /** Трек, который звучит превью; плеер в это время стоит, прослушивания не пишутся */
  const previewing = ref<Track | null>(null)
  /** Громкость превью поверх общей, 0…1: наложение в приложении, приглушение на паузе историй */
  const previewGain = ref(1)
  let previewSaved: { entry: QueueEntry, positionMs: number, playing: boolean } | null = null
  /** Пульт: активное устройство играло — после превью оно продолжит */
  let previewRemoteResume = false
  let previewRun = 0

  /**
   * Кусок готового на сервере трека с `startMs`: текущее прослушивание закрывается, очередь не меняется.
   * `fadeMs` — сменить звучащее превью наложением (сайт); `resolved` — источник, найденный заранее
   */
  async function startPreview(track: Track, startMs = 0, opts: { fadeMs?: number, resolved?: SourceResult } = {}): Promise<boolean> {
    const run = ++previewRun
    const switching = !!previewing.value
    if (!switching) {
      if (mirrored.value) {
        // Пульт: звук историй — на этом устройстве, активное ждёт на паузе
        previewRemoteResume = playing.value
        previewSaved = null
        if (playing.value) pause()
      }
      else {
        finishListen('stopped')
        previewSaved = current.value && !pending.value ? { entry: current.value, positionMs: positionMs.value, playing: playing.value } : null
        previewRemoteResume = false
        engine.pause()
        playing.value = false
        visual()?.setPlaying(false)
      }
      previewGain.value = 1
    }
    previewing.value = track
    let result = opts.resolved
    if (!result) {
      try {
        result = await bridge.media.source(track)
      }
      catch {
        result = { kind: 'unavailable' }
      }
    }
    if (run !== previewRun) return false
    if (result.kind !== 'ready' || !result.source) {
      engine.pause()
      return false
    }
    const dur = result.source.durationMs ?? track.durationMs
    const at = Math.max(0, Math.min(startMs, dur - 5000))
    if (switching && opts.fadeMs && !native) {
      engine.crossfade(withGain(result.source), { durationMs: dur, fadeMs: opts.fadeMs, startMs: at })
      return true
    }
    engine.load(withGain(result.source), {
      durationMs: dur,
      startMs: at,
      track: engineTrack({ uid: `preview:${track.id}`, track, source: 'manual' }, result.source),
    })
    await engine.play().catch(() => {})
    return true
  }

  /** Пауза и продолжение превью (страница историй скрыта) */
  function pausePreview() {
    if (previewing.value) engine.pause()
  }
  async function resumePreview() {
    if (previewing.value) await engine.play().catch(() => {})
  }

  /**
   * Превью кончилось: плеер возвращается к своему треку на том же месте и играет, если играл.
   * `resume: false` — человек сразу включает другое (волну года, подборку): прежнее встаёт на паузе
   */
  function stopPreview(resume = true) {
    if (!previewing.value) return
    previewRun++
    previewing.value = null
    previewGain.value = 1
    engine.pause()
    const saved = previewSaved
    previewSaved = null
    if (mirrored.value) {
      engine.load(null, { durationMs: 0 })
      if (previewRemoteResume && resume) void play()
    }
    else if (saved) void startEntry(saved.entry, { startMs: saved.positionMs, autoplay: saved.playing && resume })
    else engine.load(null, { durationMs: 0 })
    previewRemoteResume = false
  }

  // ---------- Перемешивание и повтор ----------
  // Свои у каждого устройства. Перемешивается только остаток альбома или плейлиста: добавленное
  // вручную («Далее») идёт первым и по порядку. Волна бесконечна — там их нет

  const shuffle = useLocalStorage('dw.shuffle', false)
  const repeat = useLocalStorage<RepeatMode>('dw.repeat', 'off')
  /** Пульт: режимы активного устройства */
  const remoteModes = ref<{ shuffle: boolean, repeat: RepeatMode } | null>(null)
  /** Порядок списка до перемешивания — по нему он вернётся */
  let originalOrder: QueueEntry[] | null = null
  /** Весь список (альбом, плейлист) — для повтора всей очереди новыми кругами */
  let contextTracks: Track[] = []
  /** Круг повтора текущего трека списка */
  let round = 0
  /** Повтор всей: сколько треков держать впереди — движок приложения доиграет их и в фоне */
  const REPEAT_AHEAD = 10

  /** Перемешивание и повтор всей — у альбома и плейлиста; у волны и радио только повтор трека */
  const modesAvailable = computed(() => !waveActive.value)
  const shuffleOn = computed(() => (mirrored.value && remoteModes.value ? remoteModes.value.shuffle : shuffle.value))
  const repeatMode = computed<RepeatMode>(() => {
    const mode = mirrored.value && remoteModes.value ? remoteModes.value.repeat : repeat.value
    // Волна бесконечна: «повторять всё» в ней — то же, что выключено
    return waveActive.value && mode === 'all' ? 'off' : mode
  })
  const effectiveRepeat = computed<RepeatMode>(() => (waveActive.value ? (repeat.value === 'one' ? 'one' : 'off') : repeat.value))

  // «Умное перемешивание» (как Smart Shuffle в Spotify): третье состояние кнопки у альбома и плейлиста —
  // примерно через три трека списка подмешивается похожее на весь список, с подписью «Подобрано»
  const smartShuffle = useLocalStorage('dw.smartShuffle', false)
  const smartAvailable = computed(() => !mirrored.value && !waveActive.value && (context.value?.type === 'album' || context.value?.type === 'playlist'))
  const shuffleMode = computed<'off' | 'on' | 'smart'>(() => (!shuffleOn.value ? 'off' : smartShuffle.value && smartAvailable.value ? 'smart' : 'on'))
  /** Убранное из подобранного — больше не предлагать в этом списке */
  const smartDropped = new Set<string>()
  let smartRun = 0
  const SMART_EVERY = 3

  /** Подмешать подобранное: по одному на каждые три трека, что ещё впереди */
  async function mixInSuggestions() {
    if (shuffleMode.value !== 'smart' || !contextTracks.length) return
    const run = ++smartRun
    const ctxId = context.value?.id
    const have = upcoming.value.filter(e => e.suggested).map(e => e.track.id)
    const own = upcoming.value.filter(e => !e.suggested).length
    const need = Math.min(30, Math.floor(own / SMART_EVERY) - have.length)
    if (need <= 0) return
    let picks: QueueEntry[]
    try {
      const r = await bridge.catalog.suggest(contextTracks.map(tr => tr.id), [...have, ...smartDropped], need)
      picks = r.items.map(w => ({ ...entryOf(w.track, 'context', t('player.suggested')), suggested: true }))
    }
    catch {
      return
    }
    if (run !== smartRun || shuffleMode.value !== 'smart' || context.value?.id !== ctxId || !picks.length) return
    upcoming.value = mixIn(upcoming.value, picks, SMART_EVERY)
    prefetched = null
    prefetchNext()
  }

  function dropSuggestions() {
    smartRun++
    if (upcoming.value.some(e => e.suggested)) upcoming.value = upcoming.value.filter(e => !e.suggested)
  }

  /** Кнопка перемешивания по кругу: выключено → перемешивать → умное (у альбома и плейлиста) → выключено */
  function cycleShuffle() {
    if (waveActive.value) return
    const mode = shuffleMode.value
    if (mode === 'off') {
      smartShuffle.value = false
      return setShuffle(true)
    }
    if (mode === 'on' && smartAvailable.value) {
      smartShuffle.value = true
      void mixInSuggestions()
      return
    }
    smartShuffle.value = false
    setShuffle(false)
  }

  /** Подобранное оставить в очереди как обычный трек (его добавили в плейлист) */
  function keepSuggestion(uid: string) {
    const i = upcoming.value.findIndex(e => e.uid === uid)
    if (i >= 0) upcoming.value.splice(i, 1, { ...upcoming.value[i]!, suggested: false, reason: undefined })
    if (current.value?.uid === uid) current.value = { ...current.value, suggested: false, reason: undefined }
  }

  /** «Убрать» подобранное: из очереди и больше не предлагать в этом списке */
  function dropSuggestion(uid: string) {
    const i = upcoming.value.findIndex(e => e.uid === uid)
    if (i < 0) return
    const [e] = upcoming.value.splice(i, 1)
    if (e) smartDropped.add(e.track.id)
    prefetched = null
    prefetchNext()
  }

  function resetOrder() {
    originalOrder = null
    contextTracks = []
    round = 0
    smartRun++
    smartDropped.clear()
  }

  function setShuffle(on: boolean) {
    if (routed({ action: 'setShuffle', shuffle: on })) return
    if (shuffle.value === on) return
    shuffle.value = on
    if (waveActive.value) return
    if (!on) dropSuggestions()
    if (on) {
      const r = shuffleQueue(current.value, upcoming.value)
      upcoming.value = r.upcoming
      originalOrder = r.original
    }
    else if (originalOrder) {
      upcoming.value = restoreQueue(current.value, upcoming.value, originalOrder)
      originalOrder = null
    }
    prefetched = null
    prefetchNext()
  }

  function setRepeat(mode: RepeatMode) {
    if (routed({ action: 'setRepeat', mode })) return
    repeat.value = mode
    // Повтор всей сняли — новые круги списка уходят, текущий доигрывает
    if (mode !== 'all') upcoming.value = upcoming.value.filter(e => (e.round ?? 0) <= round)
    else refillRepeat()
  }

  const toggleShuffle = () => cycleShuffle()
  // В волне повтор — только трека: выключен ↔ один трек
  const cycleRepeat = () => setRepeat(waveActive.value ? (repeatMode.value === 'one' ? 'off' : 'one') : nextRepeat(repeatMode.value))

  /** Новый круг списка в конец очереди (перемешанный, если перемешивание включено) */
  function appendRound() {
    const r = Math.max(round, ...upcoming.value.map(e => e.round ?? 0)) + 1
    const fresh = contextTracks.map(tr => ({ ...entryOf(tr, 'context'), round: r }))
    if (shuffle.value) {
      originalOrder = [...(originalOrder ?? []), ...fresh]
      upcoming.value.push(...shuffled(fresh))
    }
    else upcoming.value.push(...fresh)
  }

  /** Повтор всей: впереди всегда есть следующий круг — переход на начало списка без паузы */
  function refillRepeat() {
    if (effectiveRepeat.value !== 'all' || mirrored.value || !current.value) return
    const n = roundsNeeded(upcoming.value.length, contextTracks.length, REPEAT_AHEAD)
    for (let i = 0; i < n; i++) appendRound()
  }

  /** Очередь кончилась: повтор всей начинает список заново. true — есть что играть */
  function wrapRepeat(): boolean {
    if (afterTrack(effectiveRepeat.value, false, contextTracks.length > 0) !== 'wrap') return false
    appendRound()
    return upcoming.value.length > 0
  }

  /** Сайт: трек доиграл, а включён повтор одного — тот же элемент сначала (приложение повторяет само) */
  async function replayIfRepeating(): Promise<boolean> {
    const cur = current.value
    if (!cur || pending.value || afterTrack(effectiveRepeat.value, !!nextEntry.value, false) !== 'replay') return false
    onReplayed()
    engine.seek(0)
    await play()
    return true
  }

  /** Трек начался заново: прошлое прослушивание закрыто как дослушанное, новое — с нуля */
  function onReplayed() {
    const cur = current.value
    if (!cur) return
    finishListen('completed')
    waveOutcome(cur, 'completed', positionMs.value)
    positionMs.value = 0
    beginListen(cur, 0)
  }

  /** Перенос сюда: режимы — как были на прежнем устройстве */
  function adoptModes(s: PlaybackState | null) {
    resetOrder()
    if (typeof s?.shuffle === 'boolean') shuffle.value = s.shuffle
    if (s?.repeat) repeat.value = s.repeat
  }

  if (import.meta.client) {
    // Круг текущего трека: по нему снимаются лишние круги, когда повтор всей выключают
    watch(current, (e) => {
      if (e?.source === 'context') round = e.round ?? 0
    })
    watch(() => upcoming.value.length, () => refillRepeat())
    // Приложение: повтор одного трека ведёт движок, оба режима — в Пункте управления
    watch([shuffle, effectiveRepeat], ([sh, rep]) => engine.setModes?.(sh, rep), { immediate: true })
  }

  // ---------- Система: экран блокировки, медиаклавиши, «Сейчас играет» ----------

  // При смене трека, запуске, паузе и перемотке; позицию между ними система досчитывает сама
  function publishNowPlaying() {
    if (!import.meta.client) return
    const tr = current.value?.track
    if (!tr) return bridge.media.nowPlaying(null)
    const art = tr.album?.image?.large ?? tr.album?.image?.medium ?? null
    bridge.media.nowPlaying({
      title: tr.title,
      artist: tr.artists.map(a => a.name).join(', '),
      album: tr.album?.title ?? null,
      artworkUrl: art ? bridge.resolveUrl(art) : null,
      durationMs: durationMs.value || tr.durationMs,
      positionMs: positionMs.value,
      playing: playing.value,
    })
  }

  function onCommand(cmd: MediaCommand) {
    switch (cmd.action) {
      case 'play': return void play()
      case 'pause': return pause()
      case 'toggle': return void toggle()
      case 'next': return void next('skipped')
      case 'previous': return void prev()
      case 'stop': return pause()
      case 'seek': return seek(cmd.positionMs)
      case 'seekBy': return seek(positionMs.value + cmd.offsetMs)
      case 'setShuffle': return setShuffle(cmd.shuffle)
      case 'setRepeat': return setRepeat(cmd.mode)
    }
  }

  if (import.meta.client) {
    // Превью звучит со своей громкостью поверх общей: наложение в приложении, приглушение на паузе
    const engineVolume = () => volume.value * (previewing.value ? previewGain.value : 1)
    engine.setVolume(engineVolume())
    applyMood(wave.value.mood, true)
    watch(engineVolume, v => engine.setVolume(v))
    watch(muted, m => engine.setMuted(m))
    // Приложение: переход и выравнивание громкости делает движок; очередь он получает наперёд
    // «Диджей» (только приложения): сведение по долям в движке; выразительность — шагом 0,05, чтобы ползунок не дёргал движок
    watch([effectiveMode, normalize, () => Math.round(expressiveness.value * 20) / 20], ([mode, norm, expr]) => engine.configure({ transition: mode === 'gapless' ? 'gapless' : mode === 'dj' ? 'dj' : 'crossfade', fadeMs: FADE_MS[mode], normalize: norm, expressiveness: expr }), { immediate: true })
    watch(() => [...manual.value, ...upcoming.value].map(e => e.uid).join(), () => syncUpcoming())
    engine.on('advance', (uid, reason) => {
      if (!mirrored.value && !previewing.value) void onEngineAdvance(uid, reason)
    })

    engine.on('time', (ms) => {
      // Пульт: позицию ведёт снимок активного устройства; превью — не трек плеера
      if (mirrored.value || previewing.value) return
      positionMs.value = ms
      if (listen) {
        const d = ms - listen.lastPos
        if (d > 0 && d < 1500) listen.playedMs += d
        listen.lastPos = ms
      }
      // Наложение начинается заранее, чтобы следующий трек вошёл до конца текущего.
      // В приложении переход делает движок сам — точно по кадрам
      if (!native && playing.value && !transitioning && !stopAtTrackEnd.value && effectiveRepeat.value !== 'one' && effectiveMode.value !== 'gapless' && durationMs.value > fadeMs.value * 2 && ms >= transitionAtMs.value && nextEntry.value) {
        transitioning = true
        finishListen('completed')
        const prevEntry = current.value
        waveOutcome(prevEntry, 'completed', ms)
        const entry = takeNext()!
        if (prevEntry) history.value.push(prevEntry)
        void startEntry(entry, { crossfade: true }).finally(() => {
          window.setTimeout(() => (transitioning = false), fadeMs.value)
        })
      }
    })
    engine.on('ended', () => {
      if (!transitioning && !mirrored.value && !previewing.value) void (stopAtTrackEnd.value ? advancePaused() : next('completed'))
    })
    engine.on('state', (s) => {
      if (mirrored.value || previewing.value) return
      const changed = playing.value !== s.playing
      playing.value = s.playing
      buffering.value = s.buffering
      visual()?.setPlaying(s.playing)
      if (changed) publishNowPlaying()
    })
    engine.on('sound', (available) => {
      soundLost.value = !available
    })
    engine.on('error', (_e, uid) => {
      if (mirrored.value || previewing.value || !current.value || (uid && uid !== current.value.uid)) return
      toast.error(t('player.trackFailed', { title: current.value.track.title }))
    })

    bridge.media.onCommand(onCommand)

    // Сервер скачал трек, которого ждали: играем, если человек всё ещё на нём
    bridge.live.on('acquisition.updated', (e) => {
      library.setAudioStatus(e.trackId, e.audioStatus)
      const p = pending.value
      const cur = current.value
      // Пульт: трек ждёт активное устройство, не мы
      if (!p || !cur || cur.track.id !== e.trackId || mirrored.value) return
      if (e.audioStatus === 'ready') void startEntry(cur, { autoplay: true })
      else if (e.audioStatus === 'unavailable' || e.audioStatus === 'failed') {
        pending.value = null
        toast.show({ text: t('player.unavailable', { title: cur.track.title }), icon: 'ph:cloud-slash', tone: 'error' })
        void next('error')
      }
      else pending.value = { ...p, status: 'acquiring' }
    })

    window.addEventListener('pagehide', () => {
      finishListen('stopped')
      void bridge.events.flush()
    })
  }

  return {
    current, manual, upcoming, history, context, playing, buffering, soundLost, positionMs, durationMs, source, pending, volume, muted,
    transition, expressiveness, normalize, wave, waveSessionId, waveLoading, wavePrep, waveStarting, waveActive, effectiveMode, fadeMs,
    transitionAtMs, progress, nextEntry, peaks, devices,
    play, pause, toggle, next, prev, seek, playTracks, swapCurrent, replaceQueued, startWave, cancelWavePrep, playNext, enqueue, enqueueMany, remove, clearManual, moveManual,
    jumpTo, dislike, notNow, moreLikeThis, toggleLike, setWave, applyPreset, setVolume, toggleMute, loadDevices, selectDevice,
    previewing, previewGain, startPreview, pausePreview, resumePreview, stopPreview,
    mirrored, setConnectRoute, connectSnapshot, connectYield, connectMirror, connectTakeOver, connectLeave,
    stopAtTrackEnd,
    shuffle, repeat, shuffleOn, repeatMode, effectiveRepeat, modesAvailable, setShuffle, toggleShuffle, setRepeat, cycleRepeat,
    smartShuffle, smartAvailable, shuffleMode, cycleShuffle, keepSuggestion, dropSuggestion,
  }
})
