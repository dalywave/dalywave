import type { BlockedKind, Track } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

/** Одинаковое меню трека везде: строка, плеер, очередь, правый клик */
export function useTrackActions() {
  const player = usePlayerStore()
  const library = useLibraryStore()
  const ui = useUiStore()
  const toast = useToast()
  const bridge = useBridge()
  const offline = useOfflineStore()
  const pui = usePlaylistUi()
  const share = useShare()
  const report = useReportRecording()
  const t = useNuxtApp().$i18n.t

  /** «Не рекомендовать» трек или исполнителя — в список, который ведёт сам человек («Мой вкус»), с отменой */
  async function block(kind: BlockedKind, id: string, text: string) {
    try {
      await bridge.library.block(kind, id)
    }
    catch {
      toast.error(t('track.blockFailed'))
      return
    }
    toast.show({ text, icon: 'ph:prohibit', undo: () => void bridge.library.unblock(kind, id).catch(() => toast.error(t('track.blockFailed'))) })
  }

  /** `nowPlaying` — меню играющего трека в плеере: «играть следующим» и «в очередь» там ни к чему.
   *  `extra` — пункты места, где трек показан («Убрать из плейлиста») */
  function menuFor(track: Track, opts: { inQueue?: string, nowPlaying?: boolean, extra?: MenuEntry[] } = {}): MenuEntry[] {
    const liked = library.isLiked(track)
    const artist = track.artists[0]
    const entries: MenuEntry[] = [
      { kind: 'heading', label: track.title, hint: track.artists.map(a => a.name).join(', '), image: track.album?.image ?? null },
      ...(opts.nowPlaying
        ? []
        : [
            { label: t('track.playNext'), icon: 'ph:arrow-bend-down-right', onSelect: () => player.playNext(track) },
            { label: t('track.enqueue'), icon: 'ph:list-plus', onSelect: () => player.enqueue(track) },
          ]),
      { label: t('pl.add'), icon: 'ph:plus-circle', onSelect: () => pui.pick([track]) },
      { label: t('track.radio'), icon: 'ph:waveform', onSelect: () => void player.startWave(undefined, { seed: { type: 'track', id: track.id }, title: track.title }) },
      { label: liked ? t('player.unlike') : t('player.like'), icon: liked ? 'ph:heart-break' : 'ph:heart', kbd: ['L'], onSelect: () => player.toggleLike(track) },
      { kind: 'separator' },
    ]
    // «Скачать на это устройство» — только в приложении: сайт файлов не хранит
    // (скачанный в составе альбома или запаса трек отдельно не открепить — пункта нет)
    if (offline.available) {
      if (offline.isPinned('track', track.id)) entries.push({ label: t('offline.remove'), icon: 'ph:x-circle', onSelect: () => offline.unpin('track', track.id, track.title) })
      else if (!offline.isDownloaded(track.id)) entries.push({ label: t('track.download'), icon: 'ph:arrow-circle-down', onSelect: () => offline.pin('track', track.id, track.title) })
    }
    // Переходы: плеер на телефоне сначала сворачивается, иначе страница откроется под ним
    if (artist) entries.push({ label: t('track.goArtist'), icon: 'ph:user', onSelect: () => void ui.navigate(`/artist/${artist.id}`) })
    if (track.album) entries.push({ label: t('track.goAlbum'), icon: 'ph:disc', onSelect: () => void ui.navigate(`/album/${track.album!.id}`) })
    if (opts.inQueue) entries.push({ label: t('track.removeFromQueue'), icon: 'ph:minus-circle', danger: true, onSelect: () => player.remove(opts.inQueue!) })
    // Играющий трек — «Слушать вместе»: позвать друзей добавлять треки в эту очередь
    if (opts.nowPlaying) entries.push({ label: t('jam.title'), icon: 'ph:users-three', onSelect: () => void useJamStore().open() })
    entries.push(share.entry('track', track.id, track.title))
    // «Неверная запись» — только у того, что уже на сервере: жалуются на услышанное
    if (opts.nowPlaying || library.statusOf(track) === 'ready') entries.push({ label: t('track.wrongRecording'), icon: 'ph:flag', onSelect: () => report.show(track) })
    if (opts.extra?.length) entries.push(...opts.extra)
    entries.push(
      { kind: 'separator' },
      { label: t('track.block'), icon: 'ph:prohibit', danger: true, onSelect: () => void block('track', track.id, t('track.blocked', { title: track.title })) },
      ...(artist ? [{ label: t('track.blockArtist'), icon: 'ph:user-minus', danger: true, onSelect: () => void block('artist', artist.id, t('track.blockedArtist', { name: artist.name })) }] : []),
    )
    return entries
  }

  return { menuFor }
}
