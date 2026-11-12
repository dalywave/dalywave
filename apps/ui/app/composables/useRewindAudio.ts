import type { Track } from '~/api/types'
import { RewindAudio, type RewindAudioPort } from '~/utils/rewind-audio'

/** Сайт: сколько файлов следующих слайдов держать подгруженными */
const WARM_KEEP = 2

/**
 * Музыка историй «Итогов года» (utils/rewind-audio.ts) через плеер в режиме превью: звук только на
 * этом устройстве, без истории, очередь не трогается; после закрытия плеер продолжает своё. Трек,
 * который сервер докачал, вступает сам, если его слайд на экране
 */
export function useRewindAudio() {
  const bridge = useBridge()
  const player = usePlayerStore()
  const muted = ref(false)
  const current = shallowRef<Track | null>(null)
  // Сайт: файл следующего слайда начинает грузиться заранее, чтобы наложение не ждало сеть
  const warmed: HTMLAudioElement[] = []

  const port: RewindAudioPort = {
    native: bridge.audio.native,
    resolve: track => bridge.media.source(track),
    async start(track, startMs, opts) {
      const ok = await player.startPreview(track, startMs, opts)
      if (ok) current.value = track
      return ok
    },
    pause: () => player.pausePreview(),
    resume: () => void player.resumePreview(),
    setGain: (gain) => {
      player.previewGain = gain
    },
    end: resume => player.stopPreview(resume),
    warm: bridge.audio.native
      ? undefined
      : (url, startMs) => {
          if (warmed.some(a => a.src === new URL(url, location.href).href)) return
          const el = new Audio()
          el.preload = 'auto'
          el.crossOrigin = 'anonymous'
          el.muted = true
          el.src = url
          el.currentTime = startMs / 1000
          warmed.push(el)
          while (warmed.length > WARM_KEEP) warmed.shift()?.removeAttribute('src')
        },
    schedule: (fn, ms) => window.setTimeout(fn, ms),
    cancel: handle => window.clearTimeout(handle as number),
  }

  const audio = new RewindAudio(port)
  const off = bridge.live.on('acquisition.updated', (e) => {
    if (e.audioStatus === 'ready') audio.trackReady(e.trackId)
  })

  function toggleMute() {
    muted.value = !muted.value
    audio.setMuted(muted.value)
  }

  /** `resume: false` — человек сразу включает другое: прежняя музыка не продолжается */
  function close(resume = true) {
    audio.close(resume)
    current.value = null
    for (const el of warmed.splice(0)) el.removeAttribute('src')
  }

  onBeforeUnmount(() => {
    off()
    close()
  })

  return { audio, muted: readonly(muted), current: readonly(current), toggleMute, close }
}
