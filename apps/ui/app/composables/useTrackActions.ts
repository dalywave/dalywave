import type { Track } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

/** Одинаковое меню трека везде: строка, плеер, очередь, правый клик */
export function useTrackActions() {
  const player = usePlayerStore()
  const library = useLibraryStore()
  const ui = useUiStore()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t

  /** `nowPlaying` — меню играющего трека в плеере: «играть следующим» и «в очередь» там ни к чему */
  function menuFor(track: Track, opts: { inQueue?: string, nowPlaying?: boolean } = {}): MenuEntry[] {
    const liked = library.isLiked(track)
    const artist = track.artists[0]
    const entries: MenuEntry[] = [
      { kind: 'heading', label: track.title, hint: track.artists.map(a => a.name).join(', '), image: track.album?.image ?? null },
      ...(opts.nowPlaying
        ? []
        : [
            { label: t('track.playNext'), icon: 'ph:arrow-bend-down-right', kbd: ['⌘', '↵'], onSelect: () => player.playNext(track) },
            { label: t('track.enqueue'), icon: 'ph:list-plus', onSelect: () => player.enqueue(track) },
          ]),
      { label: t('track.radio'), icon: 'ph:waveform', onSelect: () => toast.show({ text: `Волна по треку «${track.title}» появится вместе с рекомендациями`, icon: 'ph:waveform' }) },
      { label: liked ? t('player.unlike') : t('player.like'), icon: liked ? 'ph:heart-break' : 'ph:heart', kbd: ['L'], onSelect: () => player.toggleLike(track) },
      { kind: 'separator' },
    ]
    // «Скачать на это устройство» появится там, где мост научится хранить треки офлайн:
    // пока такого нет ни на сайте, ни в приложениях — пункт не показываем вовсе.
    // Переходы: плеер на телефоне сначала сворачивается, иначе страница откроется под ним
    if (artist) entries.push({ label: t('track.goArtist'), icon: 'ph:user', onSelect: () => void ui.navigate(`/artist/${artist.id}`) })
    if (track.album) entries.push({ label: t('track.goAlbum'), icon: 'ph:disc', onSelect: () => void ui.navigate(`/album/${track.album!.id}`) })
    if (opts.inQueue) entries.push({ label: 'Убрать из очереди', icon: 'ph:minus-circle', danger: true, onSelect: () => player.remove(opts.inQueue!) })
    entries.push(
      { kind: 'separator' },
      { label: t('track.block'), icon: 'ph:prohibit', danger: true, onSelect: () => player.dislike(track) },
      { label: t('track.report'), icon: 'ph:warning-circle', onSelect: () => toast.show({ text: 'Отметили: запись проверят в админке, раздел «Спорные»', icon: 'ph:flag' }) },
    )
    return entries
  }

  return { menuFor }
}
