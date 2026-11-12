import type { WaveActivity, WaveCharacter, WaveLanguage, WaveMood, WaveSettings } from '~/bridge/types'

/** Команда Siri, «Команд» и виджетов от оболочки (apps/shell/src/siri.rs, событие `siri:command`);
 *  характер и язык приходят с сохранённым сочетанием из виджета «Моя волна» */
type SiriCommand =
  | { action: 'wave', mood?: WaveMood, activity?: WaveActivity, character?: WaveCharacter, language?: WaveLanguage }
  | { action: 'play', kind: 'album' | 'artist' | 'playlist' | 'track' | 'liked', id: string, title?: string, shuffle?: boolean }
  | { action: 'liked', trackId: string }

// iPhone и iPad: «Включи мою волну», «Включи <плейлист> в DalyWave» голосом. Пауза и «дальше» оболочка
// выполняет сама; сюда — то, что включает интерфейс, как его кнопки (волна сервера, история, контекст).
// Не забрали за 3 с (WebView спит) — оболочка включает сама. Забираем после входа.
export default defineNuxtPlugin(() => {
  if (isMiniWindow()) return
  const bridge = useBridge()
  if (bridge.kind !== 'tauri') return
  const auth = useAuthStore()
  const player = usePlayerStore()
  const library = useLibraryStore()
  const actions = usePlayActions()

  async function run(cmd: SiriCommand) {
    switch (cmd.action) {
      case 'wave': {
        const settings: Partial<WaveSettings> = {}
        if (cmd.mood) settings.mood = cmd.mood
        if (cmd.activity) settings.activity = cmd.activity
        if (cmd.character) settings.character = cmd.character
        if (cmd.language) settings.language = cmd.language
        return player.startWave(settings)
      }
      case 'play': {
        const opts = { shuffle: !!cmd.shuffle }
        // Уже играет это же — не останавливать (кнопки страниц на повторное нажатие ставят паузу)
        if (cmd.kind !== 'track' && cmd.kind !== 'liked' && actions.isPlayingContext(cmd.kind, cmd.id) && !opts.shuffle) return
        if (cmd.kind === 'album') return actions.playAlbum(cmd.id, opts)
        if (cmd.kind === 'artist') return actions.playArtist(cmd.id, opts)
        if (cmd.kind === 'playlist') return actions.playPlaylist(cmd.id, opts)
        if (cmd.kind === 'track') {
          const tr = await bridge.catalog.track(cmd.id)
          return player.playTracks([tr], 0, { type: 'search', id: null, title: tr.title })
        }
        let page = await bridge.library.liked(null, 200)
        const tracks = [...page.items.map(i => i.track)]
        while (page.nextCursor && tracks.length < 1000) {
          page = await bridge.library.liked(page.nextCursor, 200)
          tracks.push(...page.items.map(i => i.track))
        }
        return player.playTracks(tracks, 0, { type: 'library', id: 'liked', title: 'Мне нравится' }, opts)
      }
      case 'liked': {
        // Оболочка уже отметила на сервере — сердечко и тост здесь
        const tr = player.current?.track
        if (tr && tr.id === cmd.trackId) await library.setLiked(tr, true)
      }
    }
  }

  void Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')]).then(async ([core, event]) => {
    const take = async () => {
      if (auth.status !== 'ready') return
      const cmds = await core.invoke<SiriCommand[]>('siri_take').catch(() => [] as SiriCommand[])
      for (const cmd of cmds) await run(cmd).catch(() => {})
    }
    await event.listen('siri:command', () => void take())
    watch(() => auth.status, () => void take(), { immediate: true })
  }).catch(() => {})
})
