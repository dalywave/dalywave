// Вывод звука приложения (apps/shell/BRIDGE.md, «Куда идёт звук»). Звук играет движок на Rust,
// поэтому и выбирает вывод он: на компьютере — свой список устройств CoreAudio/WASAPI (колонки
// AirPlay там тоже есть), на iPhone и iPad — системное окно AirPlay поверх приложения
import type { AudioOutputs, AudioRoutes, Platform } from '../types'
import type { Shell } from './shell'

export function createTauriOutputs(platform: Platform, shell: () => Promise<Shell>): AudioRoutes | null {
  const mode = platform === 'ios' || platform === 'ipados' ? 'picker' : platform === 'macos' || platform === 'windows' ? 'list' : null
  if (!mode) return null
  const strip = (o: AudioOutputs): AudioOutputs => ({ outputs: o.outputs, followsSystem: o.followsSystem })
  return {
    mode,
    get: async () => strip(await (await shell()).audio.outputs()),
    async select(id) {
      await (await shell()).audio.setOutput(id)
    },
    async showPicker() {
      await (await shell()).audio.showPicker()
    },
    onChange(cb) {
      let off: (() => void) | null = null
      let stopped = false
      void shell().then(sh => sh.on('audio:outputs', o => cb(strip(o)))).then((unlisten) => {
        if (stopped) unlisten()
        else off = unlisten
      }).catch(() => {})
      return () => {
        stopped = true
        off?.()
      }
    },
  }
}
