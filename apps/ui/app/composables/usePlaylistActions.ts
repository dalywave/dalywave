import type { PlaylistSummary, Track } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

/** Сколько ждёт «Отменить» у удалённого плейлиста, прежде чем он сотрётся на сервере */
const DELETE_DELAY = 6000

/**
 * Всё, что делают с плейлистом, в одном месте: одно меню для боковой панели, медиатеки и шапки
 * плейлиста; добавление треков с «Отменить»; копия, «Сохранить как плейлист», удаление
 */
export function usePlaylistActions() {
  const bridge = useBridge()
  const library = useLibraryStore()
  const player = usePlayerStore()
  const offline = useOfflineStore()
  const ui = useUiStore()
  const pui = usePlaylistUi()
  const toast = useToast()
  const route = useRoute()
  const { playPlaylist } = usePlayActions()
  const t = useNuxtApp().$i18n.t
  const lang = useLang()

  /** Плейлисты, которые человек ведёт сам: в них можно добавлять */
  const own = computed(() => (library.playlists ?? []).filter(p => p.kind === 'user' && !pui.hidden.has(p.id)))

  const editable = (p: Pick<PlaylistSummary, 'kind'>) => p.kind === 'user'

  /** Подпись под названием: «Плейлист · 12 треков», пустой — «Пустой плейлист» */
  function subOf(p: PlaylistSummary): string {
    if (!p.tracksCount && editable(p)) return t('pl.empty')
    const kind = p.kind === 'mix' ? t('pl.kind.mix') : p.kind === 'smart' ? t('pl.kind.smart') : t('pl.kind.user')
    return `${kind} · ${formatCount(p.tracksCount, 'track', lang.value)}`
  }

  function refresh() {
    return Promise.all([library.loadPlaylists(true), library.loadOverview(true)]).catch(() => {})
  }

  /** Треки — в плейлист, с «Отменить» */
  async function addTo(p: PlaylistSummary, tracks: Track[]) {
    if (!tracks.length) return
    try {
      const added = await bridge.playlists.addItems(p.id, { trackIds: tracks.map(tr => tr.id) })
      void refresh()
      for (const tr of tracks) bridge.events.feedback({ kind: 'playlist_add', trackId: tr.id })
      const ids = added.items.map(i => i.id)
      const text = tracks.length === 1
        ? t('pl.added', { title: tracks[0]!.title, playlist: p.title })
        : t('pl.addedMany', { n: formatCount(tracks.length, 'track', lang.value), playlist: p.title })
      toast.show({
        text,
        icon: 'ph:check-circle-fill',
        undo: () => void bridge.playlists.removeItems(p.id, ids).then(refresh).catch(() => toast.error(t('pl.failed'))),
      })
      return added
    }
    catch {
      toast.error(t('pl.addFailed'))
    }
  }

  /** Новый плейлист сразу с треками (или пустой) — и переход в него */
  async function createWith(title: string, tracks: Track[] = [], opts: { open?: boolean } = {}) {
    const p = await bridge.playlists.create({ title, trackIds: tracks.map(tr => tr.id) })
    void refresh()
    toast.show({
      text: tracks.length ? t('pl.createdWith', { title }) : t('pl.created', { title }),
      icon: 'ph:playlist',
      action: opts.open === false ? { label: t('pl.open'), onClick: () => void ui.navigate(`/playlist/${p.id}`) } : undefined,
    })
    if (opts.open !== false) await ui.navigate(`/playlist/${p.id}`)
    return p
  }

  async function duplicate(p: PlaylistSummary) {
    try {
      const copy = await bridge.playlists.duplicate(p.id, p.kind === 'user' ? t('pl.copyTitle', { title: p.title }) : undefined)
      void refresh()
      toast.show({ text: p.kind === 'user' ? t('pl.duplicated', { title: copy.title }) : t('pl.saved', { title: copy.title }), icon: 'ph:copy', action: { label: t('pl.open'), onClick: () => void ui.navigate(`/playlist/${copy.id}`) } })
      return copy
    }
    catch {
      toast.error(t('pl.failed'))
    }
  }

  /** Удаление: подтверждение, затем плейлист пропадает сразу, а на сервере стирается, только
   *  если за несколько секунд не нажали «Отменить» */
  async function remove(p: PlaylistSummary) {
    const ok = await pui.confirm({ title: t('pl.deleteTitle', { title: p.title }), text: t('pl.deleteText'), action: t('pl.delete'), danger: true })
    if (!ok) return
    pui.hidden.add(p.id)
    if (route.path === `/playlist/${p.id}`) await ui.navigate('/library')
    let undone = false
    toast.show({
      text: t('pl.deleted', { title: p.title }),
      icon: 'ph:trash',
      duration: DELETE_DELAY,
      undo: () => {
        undone = true
        pui.hidden.delete(p.id)
      },
    })
    window.setTimeout(async () => {
      if (undone) return
      try {
        await bridge.playlists.remove(p.id)
        await refresh()
      }
      catch {
        toast.error(t('pl.failed'))
      }
      finally {
        pui.hidden.delete(p.id)
      }
    }, DELETE_DELAY + 300)
  }

  async function enqueue(p: PlaylistSummary) {
    try {
      const d = await bridge.playlists.get(p.id)
      player.enqueueMany(d.items.map(i => i.track), p.title)
    }
    catch {
      toast.error(t('pl.failed'))
    }
  }

  function edit(p: PlaylistSummary) {
    return ui.navigate(`/playlist/${p.id}?edit=1`)
  }

  /** Меню плейлиста — правый щелчок, долгое нажатие или «⋯». `page` — меню в шапке самого плейлиста */
  function menuFor(p: PlaylistSummary, opts: { page?: boolean } = {}): MenuEntry[] {
    const mine = editable(p)
    const entries: MenuEntry[] = []
    if (!opts.page) {
      entries.push(
        { kind: 'heading', label: p.title, hint: subOf(p), image: p.image ?? p.covers[0] ?? null },
        { label: t('common.play'), icon: 'ph:play', disabled: !p.tracksCount, onSelect: () => void playPlaylist(p.id) },
        { label: t('common.shuffle'), icon: 'ph:shuffle', disabled: !p.tracksCount, onSelect: () => void playPlaylist(p.id, { shuffle: true }) },
        { label: t('track.enqueue'), icon: 'ph:list-plus', disabled: !p.tracksCount, onSelect: () => void enqueue(p) },
        { kind: 'separator' },
      )
    }
    if (mine) entries.push({ label: t('pl.edit'), icon: 'ph:pencil-simple', onSelect: () => void edit(p) })
    else entries.push({ label: t('pl.saveAs'), icon: 'ph:plus-circle', onSelect: () => void duplicate(p) })
    entries.push({ label: p.pinned ? t('pl.unpin') : t('pl.pin'), icon: p.pinned ? 'ph:push-pin-slash' : 'ph:push-pin', onSelect: () => void library.setPinned(p, !p.pinned) })
    if (offline.available && p.tracksCount) {
      entries.push(offline.isPinned('playlist', p.id)
        ? { label: t('offline.remove'), icon: 'ph:x-circle', onSelect: () => void offline.unpin('playlist', p.id, p.title) }
        : { label: t('pl.download'), icon: 'ph:arrow-circle-down', onSelect: () => void offline.pin('playlist', p.id, p.title) })
    }
    if (mine) {
      entries.push(
        { label: t('pl.duplicate'), icon: 'ph:copy', onSelect: () => void duplicate(p) },
        { kind: 'separator' },
        { label: t('pl.delete'), icon: 'ph:trash', danger: true, onSelect: () => void remove(p) },
      )
    }
    return entries
  }

  // ---------- «Любимые треки» ----------

  async function likedTracks(): Promise<Track[]> {
    const out: Track[] = []
    let page = await bridge.library.liked(null, 200)
    out.push(...page.items.map(i => i.track))
    while (page.nextCursor && out.length < 2000) {
      page = await bridge.library.liked(page.nextCursor, 200)
      out.push(...page.items.map(i => i.track))
    }
    return out
  }

  async function playLiked(shuffle = false) {
    try {
      const tracks = await likedTracks()
      if (tracks.length) await player.playTracks(tracks, 0, { type: 'library', id: 'liked', title: t('nav.liked') }, { shuffle })
    }
    catch {
      toast.error(t('pl.failed'))
    }
  }

  function likedMenu(): MenuEntry[] {
    return [
      { kind: 'heading', label: t('nav.liked'), hint: formatCount(library.likedCount, 'track', lang.value) },
      { label: t('common.play'), icon: 'ph:play', disabled: !library.likedCount, onSelect: () => void playLiked() },
      { label: t('common.shuffle'), icon: 'ph:shuffle', disabled: !library.likedCount, onSelect: () => void playLiked(true) },
    ]
  }

  return { own, editable, subOf, addTo, createWith, duplicate, remove, enqueue, edit, menuFor, likedMenu, pick: pui.pick }
}
