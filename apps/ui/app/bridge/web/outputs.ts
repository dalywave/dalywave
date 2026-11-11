// Вывод звука на сайте: в Safari у аудиоэлемента есть системное окно AirPlay. Другие браузеры
// выбирать вывод для страницы не дают — тогда строки «Другие устройства» нет вовсе
import type { AudioOutputs, AudioRoutes } from '../types'
import type { WebAudioEngine } from './audio'

interface WebKitAudio extends HTMLAudioElement {
  webkitShowPlaybackTargetPicker?: () => void
  webkitCurrentPlaybackTargetIsWireless?: boolean
}

const CHANGED = 'webkitcurrentplaybacktargetiswirelesschanged'

export function safariOutputs(engine: WebAudioEngine): AudioRoutes | null {
  if (typeof window === 'undefined' || !('WebKitPlaybackTargetAvailabilityEvent' in window)) return null
  const el = engine.element as WebKitAudio
  if (typeof el.webkitShowPlaybackTargetPicker !== 'function') return null

  // Safari не называет колонку — только «идёт ли звук по воздуху»
  const snapshot = (): AudioOutputs => {
    const wireless = (engine.element as WebKitAudio).webkitCurrentPlaybackTargetIsWireless === true
    return { outputs: wireless ? [{ id: 'airplay', name: 'AirPlay', kind: 'airplay', current: true }] : [], followsSystem: true }
  }

  return {
    mode: 'picker',
    get: async () => snapshot(),
    select: async () => {},
    showPicker: async () => (engine.element as WebKitAudio).webkitShowPlaybackTargetPicker?.(),
    onChange(cb) {
      const handler = () => cb(snapshot())
      for (const e of engine.elements) e.addEventListener(CHANGED, handler)
      return () => {
        for (const e of engine.elements) e.removeEventListener(CHANGED, handler)
      }
    },
  }
}

/** Только разработка: `?outputs=picker|list` — строка вывода звука в обычном браузере (снимки) */
export function devOutputs(scenario: string): AudioRoutes {
  const list = scenario === 'list'
  let state: AudioOutputs = list
    ? {
        followsSystem: true,
        outputs: [
          { id: 'builtin', name: 'Динамики MacBook Pro', kind: 'builtin', current: true },
          { id: 'airpods', name: 'AirPods Захара', kind: 'bluetooth', current: false },
          { id: 'homepod', name: 'HomePod в гостиной', kind: 'airplay', current: false },
        ],
      }
    : { followsSystem: true, outputs: [{ id: 'Speaker', name: 'Динамик iPhone', kind: 'builtin', current: true }] }
  const subs = new Set<(o: AudioOutputs) => void>()
  return {
    mode: list ? 'list' : 'picker',
    get: async () => state,
    async select(id) {
      state = { followsSystem: id == null, outputs: state.outputs.map(o => ({ ...o, current: id == null ? o.id === 'builtin' : o.id === id })) }
      subs.forEach(cb => cb(state))
    },
    showPicker: async () => console.info('[dev] системное окно AirPlay'),
    onChange(cb) {
      subs.add(cb)
      return () => subs.delete(cb)
    },
  }
}
