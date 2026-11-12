import type { Track } from '~/api/types'

/** «Неверная запись»: одно окно на приложение (MusicReportRecording в app.vue), открывается из меню трека */
export function useReportRecording() {
  const track = useState<Track | null>('report-recording-track', () => null)
  const open = useState('report-recording-open', () => false)
  function show(tr: Track) {
    track.value = tr
    open.value = true
  }
  return { track, open, show }
}
