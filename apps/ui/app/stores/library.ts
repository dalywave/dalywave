import { defineStore } from 'pinia'
import type { AlbumSummary, ArtistSummary, AudioStatus, LibraryOverview, PlaylistFolder, PlaylistList, PlaylistSummary, Track } from '~/api/types'

// Медиатека: действие применяется сразу, при ошибке откатывается; у удалений есть «Отменить».
// Живые события сервера поправляют состояние треков и подтягивают изменения с других устройств
export const useLibraryStore = defineStore('library', () => {
  const bridge = useBridge()
  const toast = useToast()
  const overview = ref<LibraryOverview | null>(null)
  const playlists = ref<PlaylistSummary[] | null>(null)
  /** Папки плейлистов по «Своему порядку» */
  const folders = ref<PlaylistFolder[]>([])
  const likedOverride = ref(new Map<string, boolean>())
  const savedOverride = ref(new Map<string, boolean>())
  const followOverride = ref(new Map<string, boolean>())
  const statusOverride = ref(new Map<string, AudioStatus>())
  const likedDelta = ref(0)
  /** Счётчик изменений медиатеки: страницы перезагружают свои списки, когда он растёт */
  const revision = ref(0)

  async function loadOverview(force = false) {
    if (overview.value && !force) return overview.value
    overview.value = await bridge.library.overview()
    likedDelta.value = 0
    return overview.value
  }

  /** Все плейлисты: закреплённые первыми; и папки */
  async function loadPlaylists(force = false) {
    if (playlists.value && !force) return playlists.value
    applyList(await bridge.playlists.list())
    return playlists.value!
  }

  /** Плейлисты и папки из ответа сервера (перекладка, папки отвечают списком целиком) */
  function applyList(list: PlaylistList) {
    playlists.value = [...list.items].sort((a, b) => Number(b.pinned) - Number(a.pinned))
    folders.value = list.folders ?? []
  }

  const likedCount = computed(() => (overview.value?.counts.likedTracks ?? 0) + likedDelta.value)

  function isLiked(track: Pick<Track, 'id' | 'liked'>): boolean {
    return likedOverride.value.get(track.id) ?? track.liked
  }

  function isSaved(album: Pick<AlbumSummary, 'id' | 'saved'>): boolean {
    return savedOverride.value.get(album.id) ?? album.saved
  }

  function isFollowed(artist: Pick<ArtistSummary, 'id' | 'followed'>): boolean {
    return followOverride.value.get(artist.id) ?? artist.followed
  }

  /** Состояние звука трека с учётом живых событий («скачивается» → «на сервере») */
  function statusOf(track: Pick<Track, 'id' | 'audioStatus'>): AudioStatus {
    return statusOverride.value.get(track.id) ?? track.audioStatus
  }

  function setAudioStatus(trackId: string, status: AudioStatus) {
    statusOverride.value.set(trackId, status)
  }

  async function setLiked(track: Track, liked: boolean, opts: { silent?: boolean } = {}) {
    const was = isLiked(track)
    if (was === liked) return
    likedOverride.value.set(track.id, liked)
    likedDelta.value += liked ? 1 : -1
    try {
      if (liked) await bridge.library.like(track.id)
      else await bridge.library.unlike(track.id)
      bridge.events.feedback({ kind: liked ? 'like' : 'unlike', trackId: track.id })
      if (opts.silent) return
      if (liked) toast.show({ text: `«${track.title}» — в любимых`, icon: 'ph:heart-fill' })
      else toast.show({ text: `«${track.title}» убран из любимых`, icon: 'ph:heart-break', undo: () => setLiked(track, true, { silent: true }) })
    }
    catch {
      likedOverride.value.set(track.id, was)
      likedDelta.value += liked ? -1 : 1
      toast.error('Не получилось сохранить — проверьте связь и попробуйте ещё раз')
    }
  }

  function toggleLike(track: Track) {
    return setLiked(track, !isLiked(track))
  }

  async function setSaved(album: AlbumSummary, saved: boolean, opts: { silent?: boolean } = {}) {
    const was = isSaved(album)
    if (was === saved) return
    savedOverride.value.set(album.id, saved)
    try {
      if (saved) await bridge.library.saveAlbum(album.id)
      else await bridge.library.unsaveAlbum(album.id)
      if (opts.silent) return
      if (saved) toast.show({ text: `«${album.title}» — в медиатеке`, icon: 'ph:check-circle-fill' })
      else toast.show({ text: `«${album.title}» убран из медиатеки`, icon: 'ph:trash', undo: () => setSaved(album, true, { silent: true }) })
    }
    catch {
      savedOverride.value.set(album.id, was)
      toast.error('Не получилось изменить медиатеку — попробуйте ещё раз')
    }
  }

  async function setFollowed(artist: ArtistSummary, followed: boolean, opts: { silent?: boolean } = {}) {
    const was = isFollowed(artist)
    if (was === followed) return
    followOverride.value.set(artist.id, followed)
    try {
      if (followed) await bridge.library.follow(artist.id)
      else await bridge.library.unfollow(artist.id)
      void loadOverview(true).catch(() => {})
      if (opts.silent) return
      if (followed) toast.show({ text: `Подписка на ${artist.name}: сервер скачает альбомы, EP и синглы`, icon: 'ph:bell-ringing' })
      else toast.show({ text: `Подписка на ${artist.name} отменена`, icon: 'ph:bell-slash', undo: () => setFollowed(artist, true, { silent: true }) })
    }
    catch {
      followOverride.value.set(artist.id, was)
      toast.error('Не получилось изменить подписку — попробуйте ещё раз')
    }
  }

  async function setPinned(playlist: PlaylistSummary, pinned: boolean) {
    try {
      await bridge.playlists.pin(playlist.id, pinned)
      await Promise.all([loadPlaylists(true), loadOverview(true)])
      toast.show({ text: pinned ? `«${playlist.title}» закреплён` : `«${playlist.title}» откреплён`, icon: 'ph:push-pin' })
    }
    catch {
      toast.error('Не получилось — попробуйте ещё раз')
    }
  }

  // Изменения с других устройств и вкладок
  let reloadTimer: ReturnType<typeof setTimeout> | undefined
  function reloadSoon() {
    clearTimeout(reloadTimer)
    reloadTimer = setTimeout(() => {
      revision.value++
      if (overview.value) void loadOverview(true).catch(() => {})
      if (playlists.value) void loadPlaylists(true).catch(() => {})
    }, 400)
  }

  if (import.meta.client) {
    bridge.live.on('library.changed', reloadSoon)
    bridge.live.onReconnect(reloadSoon)
    // Приложение: медиатека в локальной базе обновилась после синхронизации
    bridge.offline?.onSynced(reloadSoon)
  }

  return {
    overview, playlists, folders, likedCount, revision, loadOverview, loadPlaylists, applyList, isLiked, isSaved, isFollowed, statusOf,
    setAudioStatus, setLiked, toggleLike, setSaved, setFollowed, setPinned,
  }
})
