import type { Track } from '~/api/types'

/** «Почему этот трек»: одно окно на приложение (MusicWhyTrack в app.vue), открывается из меню трека,
 *  который подобрала волна */
export function useWhyTrack() {
  const track = useState<Track | null>('why-track', () => null)
  const session = useState<string | null>('why-track-session', () => null)
  const open = useState('why-track-open', () => false)
  function show(tr: Track, waveSession: string | null) {
    track.value = tr
    session.value = waveSession
    open.value = true
  }
  return { track, session, open, show }
}
