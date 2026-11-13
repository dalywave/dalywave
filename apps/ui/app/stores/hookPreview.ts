import type { Track } from '~/api/types'
import type { PlayContext } from '~/stores/player'

/** Откуда отрывок: «Слушать» включает весь список с этого трека, а не один трек */
export interface HookOrigin {
  tracks: Track[]
  index: number
  context: PlayContext
}

/** Нарастание в начале и затухание в конце отрывка, мс */
const FADE_IN_MS = 250
const FADE_OUT_MS = 1200

/**
 * «Послушать припев» перед включением: ~15 секунд с припева через плеер в режиме превью
 * (`player.startPreview`): очередь и история не трогаются, играющее встаёт на паузу и потом
 * продолжается. Трек на сервере звучит с места припева по анализу, остальные — превью Deezer.
 * Любое обычное включение сразу заканчивает отрывок
 */
export const useHookPreviewStore = defineStore('hookPreview', () => {
  const bridge = useBridge()
  const player = usePlayerStore()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t

  /** Трек отрывка; null — отрывок не звучит */
  const track = shallowRef<Track | null>(null)
  const origin = shallowRef<HookOrigin | null>(null)
  /** Ищем место и звук */
  const loading = ref(false)
  /** Сыграно, 0…1 */
  const progress = ref(0)
  const lengthMs = ref(15_000)
  let run = 0
  let raf = 0
  let startedAt = 0

  function stopClock() {
    if (raf) cancelAnimationFrame(raf)
    raf = 0
  }

  // Громкость: короткое нарастание, затухание в конце, потом — конец отрывка
  function tick() {
    const elapsed = performance.now() - startedAt
    const len = lengthMs.value
    progress.value = Math.min(1, elapsed / len)
    const fadeIn = Math.min(1, elapsed / FADE_IN_MS)
    const fadeOut = Math.min(1, Math.max(0, (len - elapsed) / FADE_OUT_MS))
    player.previewGain = Math.min(fadeIn, fadeOut)
    if (elapsed >= len) {
      stop()
      return
    }
    raf = requestAnimationFrame(tick)
  }

  async function start(tr: Track, from: HookOrigin | null = null) {
    const id = ++run
    if (track.value?.id === tr.id && !loading.value) return
    stopClock()
    track.value = tr
    origin.value = from
    loading.value = true
    progress.value = 0
    bridge.haptic('medium')
    try {
      const hook = await bridge.catalog.hook(tr.id)
      if (id !== run) return
      lengthMs.value = hook.lengthMs
      let ok = false
      if (hook.ready) {
        ok = await player.startPreview(tr, hook.startMs)
      }
      else if (hook.previewUrl) {
        ok = await player.startPreview(tr, hook.startMs, {
          resolved: { kind: 'ready', source: { url: hook.previewUrl, durationMs: 30_000, preview: true }, peaks: null },
        })
      }
      if (id !== run) return
      if (!ok) {
        finish(true)
        toast.show({ text: t('hook.unavailable'), icon: 'ph:speaker-simple-slash' })
        return
      }
      player.previewGain = 0
      startedAt = performance.now()
      loading.value = false
      raf = requestAnimationFrame(tick)
    }
    catch {
      if (id !== run) return
      finish(true)
      toast.error(t('hook.failed'))
    }
  }

  function finish(resume: boolean) {
    run++
    stopClock()
    if (player.previewing) player.stopPreview(resume)
    track.value = null
    origin.value = null
    loading.value = false
    progress.value = 0
  }

  /** Отрывок кончился или закрыт: играющее до него продолжается */
  function stop() {
    if (track.value) finish(true)
  }

  /** «Слушать»: трек целиком (со списком, откуда он) — прежнее не продолжается */
  async function listen() {
    const tr = track.value
    const from = origin.value
    if (!tr) return
    finish(false)
    if (from) await player.playTracks(from.tracks, from.index, from.context)
    else await player.playTracks([tr], 0, { type: 'search', id: null, title: tr.title })
  }

  // Любое обычное включение заканчивает отрывок раньше, чем начнёт своё
  const TAKEOVER = new Set(['playTracks', 'play', 'toggle', 'next', 'prev', 'startWave', 'seek', 'jumpTo', 'applyPreset'])
  player.$onAction(({ name }) => {
    if (track.value && TAKEOVER.has(name)) finish(false)
  })

  return { track, origin, loading, progress, lengthMs, start, stop, listen }
})
