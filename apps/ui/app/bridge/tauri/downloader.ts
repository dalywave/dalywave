// Загрузчик музыки на этом компьютере (приложение macOS и Windows): команды и событие
// `downloader:state` оболочки (apps/shell/BRIDGE.md, «Загрузчик») в форме моста. Процессом
// загрузчика управляет Rust (apps/shell/src/downloader.rs); интерфейс только показывает и просит.
import type { Downloader, DownloaderSnapshot } from '../types'
import type { Shell } from './shell'

/** Загрузчик, каким его видит мост: подписка может вернуть отписку и промисом */
export interface DownloaderSource {
  state(): Promise<DownloaderSnapshot>
  listen(cb: (state: DownloaderSnapshot) => void): Promise<() => void> | (() => void)
  setEnabled(enabled: boolean): Promise<DownloaderSnapshot>
  setPaused(paused: boolean): Promise<DownloaderSnapshot>
  recreateToken(): Promise<DownloaderSnapshot>
  openLog(): Promise<void>
}

/**
 * Загрузчик, источник которого грузится лениво (`@tauri-apps/api` или мок разработки):
 * команды ждут загрузки, отписка работает синхронно.
 */
export function lazyDownloader(load: () => Promise<DownloaderSource>): Downloader {
  let job: Promise<DownloaderSource> | null = null
  const source = () => (job ??= load())
  return {
    state: async () => (await source()).state(),
    onState(cb) {
      let off: (() => void) | null = null
      let stopped = false
      void source().then(s => s.listen(cb)).then((unlisten) => {
        if (stopped) unlisten()
        else off = unlisten
      }).catch(() => {})
      return () => {
        stopped = true
        off?.()
      }
    },
    setEnabled: async enabled => (await source()).setEnabled(enabled),
    setPaused: async paused => (await source()).setPaused(paused),
    recreateToken: async () => (await source()).recreateToken(),
    openLog: async () => (await source()).openLog(),
  }
}

/** Загрузчик приложения: команды `downloader_*` и событие `downloader:state` */
export function createTauriDownloader(loadShell: () => Promise<Shell>): Downloader {
  return lazyDownloader(async () => {
    const shell = await loadShell()
    return {
      state: () => shell.downloader.state(),
      listen: cb => shell.on('downloader:state', cb),
      setEnabled: enabled => shell.downloader.setEnabled(enabled),
      setPaused: paused => shell.downloader.setPaused(paused),
      recreateToken: () => shell.downloader.recreateToken(),
      openLog: () => shell.downloader.openLog(),
    }
  })
}
