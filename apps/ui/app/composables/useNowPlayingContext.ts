// Откуда играет музыка — для шапки плеера: «Играет из» + альбом, плейлист, артист, волна с настроением.
// Один источник для полного плеера и плеера сбоку на планшете
const CONTEXT_KEY: Record<string, string> = { album: 'record.album', playlist: 'record.playlist', artist: 'record.artist', radio: 'record.radio' }

export function useNowPlayingContext() {
  const player = usePlayerStore()
  const { t } = useI18n()

  /** «Альбом · Название», «Моя волна · Спокойное» — полная подпись */
  const label = computed(() => {
    const c = player.context
    if (!c) return ''
    if (c.type === 'wave') return player.wave.mood === 'any' ? t('wave.title') : `${t('wave.title')} · ${t(`wave.mood.${player.wave.mood}`)}`
    const kind = CONTEXT_KEY[c.type]
    return kind ? `${t(kind)} · ${c.title}` : c.title
  })
  /** Мелкая строка над названием: «Играет из» или «Играет волна» */
  const kicker = computed(() => (player.waveActive ? t('player.playingWave') : t('player.playingFrom')))
  /** Короткое название под ней: у волны — с настроением, у остального — просто имя */
  const title = computed(() => (player.waveActive ? label.value : player.context?.title ?? ''))

  return { label, kicker, title }
}
