// «Распознать» в приложении: ShazamKit в оболочке (apps/shell/src/recognize.rs) — macOS, iPhone, iPad.
// Просьбу открыть экран (трей, быстрое действие на значке) оболочка шлёт событием `recognize:open`,
// а пришедшую до загрузки страницы — отдаёт `takePending()` один раз
import type { Platform, Recognizer } from '../types'
import type { Shell } from './shell'

const SUPPORTED: Platform[] = ['macos', 'ios', 'ipados']

export function createTauriRecognizer(platform: Platform, loadShell: () => Promise<Shell>): Recognizer | null {
  if (!SUPPORTED.includes(platform)) return null
  return {
    listen: async () => (await loadShell()).recognize.listen(),
    cancel: async () => {
      await (await loadShell()).recognize.cancel()
    },
    onOpen(cb) {
      let off: (() => void) | null = null
      let stopped = false
      void loadShell().then(async (shell) => {
        const unlisten = await shell.on('recognize:open', () => {
          void shell.recognize.takePending().catch(() => {})
          cb()
        })
        if (stopped) unlisten()
        else off = unlisten
        if (!stopped && await shell.recognize.takePending()) cb()
      }).catch(() => {})
      return () => {
        stopped = true
        off?.()
      }
    },
  }
}
