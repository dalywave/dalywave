import type { ContextType } from '~/api/types'

/** Запуск альбома, артиста или плейлиста целиком — с карточек, полок и шапок страниц */
export function usePlayActions() {
  const bridge = useBridge()
  const player = usePlayerStore()
  const toast = useToast()
  const loading = ref<string | null>(null)

  const isContext = (type: ContextType, id: string) => player.context?.type === type && player.context.id === id
  const isPlayingContext = (type: ContextType, id: string) => isContext(type, id) && player.playing

  async function run(key: string, job: () => Promise<void>) {
    if (loading.value) return
    loading.value = key
    try {
      await job()
    }
    catch {
      toast.error('Не получилось включить — сервер не ответил. Попробуйте ещё раз')
    }
    finally {
      loading.value = null
    }
  }

  function playAlbum(id: string, opts: { shuffle?: boolean } = {}) {
    if (isContext('album', id) && !opts.shuffle) return player.toggle()
    return run(`album:${id}`, async () => {
      const d = await bridge.catalog.album(id)
      await player.playTracks(d.tracks, 0, { type: 'album', id, title: d.album.title }, opts)
    })
  }

  function playArtist(id: string, opts: { shuffle?: boolean } = {}) {
    if (isContext('artist', id) && !opts.shuffle) return player.toggle()
    return run(`artist:${id}`, async () => {
      const d = await bridge.catalog.artist(id)
      await player.playTracks(d.topTracks, 0, { type: 'artist', id, title: d.artist.name }, opts)
    })
  }

  function playPlaylist(id: string, opts: { shuffle?: boolean } = {}) {
    if (isContext('playlist', id) && !opts.shuffle) return player.toggle()
    return run(`playlist:${id}`, async () => {
      const d = await bridge.playlists.get(id)
      await player.playTracks(d.items.map(i => i.track), 0, { type: 'playlist', id, title: d.playlist.title }, opts)
    })
  }

  return { loading, isContext, isPlayingContext, playAlbum, playArtist, playPlaylist }
}
