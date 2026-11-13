import type { Track } from '~/api/types'

/** «Клип» у трека (администратор): одно окно на приложение (MusicClipSheet в app.vue), открывается
 *  из ⋯ трека в плеере и на странице трека. Слушателю вместо него — «Скрыть клип» ([`hide`]) */
export function useClipSheet() {
  const track = useState<Track | null>('clip-sheet-track', () => null)
  const open = useState('clip-sheet-open', () => false)
  function show(tr: Track) {
    track.value = tr
    open.value = true
  }

  /** Скрыть клип у себя (и сигнал администратору) — с отменой в тосте */
  async function hide(tr: Track) {
    const bridge = useBridge()
    const toast = useToast()
    const motion = useMotionStore()
    const t = useNuxtApp().$i18n.t
    try {
      await bridge.video.hideClip(tr.id)
    }
    catch {
      toast.error(t('track.clipFailed'))
      return
    }
    motion.refresh(tr.id)
    toast.show({
      text: t('track.clipHidden'),
      icon: 'ph:eye-slash',
      undo: () => void bridge.video.unhideClip(tr.id).then(() => motion.refresh(tr.id)).catch(() => toast.error(t('track.clipFailed'))),
    })
  }

  return { track, open, show, hide }
}
