import type { Track } from '~/api/types'

/** «Поделиться историей»: одно окно на приложение (MusicStoryShare в app.vue), открывается из меню трека */
export function useStory() {
  const track = useState<Track | null>('story-track', () => null)
  const open = useState('story-open', () => false)
  function show(tr: Track) {
    track.value = tr
    open.value = true
  }
  return { track, open, show }
}
