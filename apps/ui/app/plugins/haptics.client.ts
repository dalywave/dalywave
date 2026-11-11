// Тактильный отклик на iPhone (bridge.haptic; на остальных устройствах — ничего): действия плеера и
// медиатеки, вызванные интерфейсом. Внутренние вызовы сторов (автопереход к следующему треку) отклика
// не дают — $onAction видит только вызовы снаружи
export default defineNuxtPlugin(() => {
  const bridge = useBridge()
  if (bridge.kind !== 'tauri') return
  const player = usePlayerStore()
  const library = useLibraryStore()
  // «Играть», «пауза», «следующий», смена настроения волны — лёгкий удар
  const LIGHT = new Set(['toggle', 'play', 'pause', 'next', 'setWave'])
  player.$onAction(({ name }) => {
    if (LIGHT.has(name)) bridge.haptic('light')
  })
  // Лайк — «успех»; снять лайк — без отклика
  library.$onAction(({ name, args, after }) => {
    if (name === 'setLiked' && args[1] === true) bridge.haptic('success')
    if (name === 'toggleLike') {
      const track = args[0]
      after(() => {
        if (library.isLiked(track)) bridge.haptic('success')
      })
    }
  })
})
