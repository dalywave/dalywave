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
  const story = useStory()
  const hookPreview = useHookPreviewStore()
  const why = useWhyTrack()
  const versions = useVersionsStore()
  const auth = useAuthStore()
  const motion = useMotionStore()
  const clipSheet = useClipSheet()
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
   *  `about` — меню на странице трека. `extra` — пункты места, где трек показан («Убрать из плейлиста») */
  function menuFor(track: Track, opts: { inQueue?: string, nowPlaying?: boolean, about?: boolean, extra?: MenuEntry[] } = {}): MenuEntry[] {
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
      // Отрывок с припева перед включением — не тому, что уже играет
      ...(opts.nowPlaying || player.current?.track.id === track.id ? [] : [{ label: t('hook.listen'), icon: 'ph:ear', onSelect: () => void hookPreview.start(track) }]),
      { label: liked ? t('player.unlike') : t('player.like'), icon: liked ? 'ph:heart-break' : 'ph:heart', kbd: ['L'], onSelect: () => player.toggleLike(track) },
    ]
    // Играющий трек: другие версии той же песни — «Оригинал», «Slowed», «Sped up»
    const own = opts.nowPlaying && versions.available && versions.current?.trackId === track.id ? versions.current : null
    if (own) {
      const selected = versions.waiting?.version.kind ?? own.kind
      entries.push({
        label: t('versions.label'),
        icon: 'ph:gauge',
        hint: versions.label(selected),
        children: versions.options.map(o => ({ label: versions.label(o.kind), checked: o.kind === selected, onSelect: () => void versions.switchTo(o.kind) })),
      })
    }
    entries.push({ kind: 'separator' })
    // «Скачать на это устройство» — только в приложении: сайт файлов не хранит
    // (скачанный в составе альбома или запаса трек отдельно не открепить — пункта нет)
    if (offline.available) {
      if (offline.isPinned('track', track.id)) entries.push({ label: t('offline.remove'), icon: 'ph:x-circle', onSelect: () => offline.unpin('track', track.id, track.title) })
      else if (!offline.isDownloaded(track.id)) entries.push({ label: t('track.download'), icon: 'ph:arrow-circle-down', onSelect: () => offline.pin('track', track.id, track.title) })
    }
    // Переходы: плеер на телефоне сначала сворачивается, иначе страница откроется под ним
    if (artist) entries.push({ label: t('track.goArtist'), icon: 'ph:user', onSelect: () => void ui.navigate(`/artist/${artist.id}`) })
    if (track.album) entries.push({ label: t('track.goAlbum'), icon: 'ph:disc', onSelect: () => void ui.navigate(`/album/${track.album!.id}`) })
    // «О треке»: кто сделал трек — авторы, продюсеры, лейбл, год
    entries.push({ label: t('track.about'), icon: 'ph:info', onSelect: () => void ui.navigate(`/track/${track.id}`) })
    // «Почему этот трек» — только у того, что подобрала волна (у записи очереди есть подпись)
    const entry = opts.nowPlaying ? player.current : opts.inQueue ? player.upcoming.find(e => e.uid === opts.inQueue) : null
    if (entry?.source === 'wave' && entry.reason && entry.track.id === track.id) {
      entries.push({ label: t('track.why'), icon: 'ph:question', onSelect: () => why.show(track, player.waveSessionId) })
    }
    if (opts.inQueue) entries.push({ label: t('track.removeFromQueue'), icon: 'ph:minus-circle', danger: true, onSelect: () => player.remove(opts.inQueue!) })
    // Играющий трек — «Слушать вместе»: позвать друзей добавлять треки в эту очередь
    if (opts.nowPlaying) entries.push({ label: t('jam.title'), icon: 'ph:users-three', onSelect: () => void useJamStore().open() })
    entries.push(share.entry('track', track.id, track.title))
    // Видео-история 9:16 для Instagram и TikTok: обложка, волна, строка текста и 15 с с припева
    entries.push({ label: t('story.action'), icon: 'ph:film-strip', onSelect: () => story.show(track) })
    // «Неверная запись» — только у того, что уже на сервере: жалуются на услышанное
    if (opts.nowPlaying || library.statusOf(track) === 'ready') entries.push({ label: t('track.wrongRecording'), icon: 'ph:flag', onSelect: () => report.show(track) })
    // Клип вместо обложки — в плеере и на странице трека: администратор выбирает его сам,
    // слушатель может скрыть у себя (это же жалоба в «Спорные»)
    if (opts.nowPlaying || opts.about) {
      if (auth.isAdmin) entries.push({ label: t('track.clip'), icon: 'ph:film-slate', onSelect: () => clipSheet.show(track) })
      else if (motion.hasClip(track.id)) entries.push({ label: t('track.hideClip'), icon: 'ph:eye-slash', onSelect: () => void clipSheet.hide(track) })
    }
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
