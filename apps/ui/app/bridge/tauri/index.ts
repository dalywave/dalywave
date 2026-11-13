// Мост приложений (Tauri) по договору apps/shell/BRIDGE.md, типы — ./shell.ts: macOS, Windows,
// iPhone и iPad. Компьютеры: окно без рамки, свои кнопки на Windows, отступ под «светофор» на
// macOS, перетаскивание. Вход — системный браузер (на iPhone — окно входа поверх приложения),
// токен устройства в связке ключей; запросы — на адрес сервера с `Authorization: Bearer`.
// Звук — движок на Rust (./audio.ts); «Сейчас играет» и пульт системы ведёт он же.
import type { Me } from '~/api/types'
import type { AppUpdateState, Bridge, Platform, WindowChrome, WindowState } from '../types'
import { createWebBridge, shareOrSaveBlob } from '../web'
import { toApiError } from '../web/http'
import { NativeAudioEngine } from './audio'
import { createTauriDownloader } from './downloader'
import { createTauriTrainer } from './trainer'
import { appPush, createOfflineBridge, haptic, systemNotify, withOffline } from './offline'
import { createTauriOutputs } from './outputs'
import { createTauriRecognizer } from './recognize'
import { createShell, isShellError, shellInfo, trackSnapArea, type InvokeFn, type ListenFn, type Shell, type UpdateState } from './shell'

export function isTauri(): boolean {
  return typeof window !== 'undefined' && ('__DALYWAVE_SHELL__' in window || '__TAURI_INTERNALS__' in window)
}

function detectPlatform(): Platform {
  const ua = navigator.userAgent
  if (/iPad/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) return 'ipados'
  if (/iPhone|iPod/.test(ua)) return 'ios'
  if (/Mac OS X|Macintosh/.test(ua)) return 'macos'
  if (/Windows/.test(ua)) return 'windows'
  return 'linux'
}

const DESKTOP: Platform[] = ['macos', 'windows', 'linux']

// @tauri-apps/api подгружается только внутри приложения
let shellJob: Promise<Shell> | null = null
function loadShell(): Promise<Shell> {
  shellJob ??= Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')]).then(([core, event]) =>
    createShell({ invoke: core.invoke as InvokeFn, listen: event.listen as unknown as ListenFn }))
  return shellJob
}

/** Подписка на событие оболочки с отпиской синхронно */
function on(subscribe: (shell: Shell) => Promise<() => void>): () => void {
  let off: (() => void) | null = null
  let stopped = false
  void loadShell().then(subscribe).then((unlisten) => {
    if (stopped) unlisten()
    else off = unlisten
  }).catch(() => {})
  return () => {
    stopped = true
    off?.()
  }
}

function toUpdateState(s: UpdateState): AppUpdateState {
  const u = s.update
  return {
    status: s.status,
    currentVersion: s.currentVersion,
    update: u ? { version: u.version, currentVersion: u.currentVersion, notes: u.notes } : null,
    progress: s.status === 'downloading' && s.totalBytes ? Math.min(1, (s.downloadedBytes ?? 0) / s.totalBytes) : null,
    error: s.error,
    installOnQuit: s.installOnQuit,
  }
}

/** Ошибки входа оболочки — коды для текстов на экране входа */
export class ShellLoginError extends Error {
  constructor(readonly code: string, message: string) {
    super(message)
    this.name = 'ShellLoginError'
  }
}

export function createTauriBridge(): Bridge {
  const info = shellInfo()
  const platform: Platform = info?.os ?? detectPlatform()
  // Телефон и планшет: окна с заголовком нет, отступы — env(safe-area-inset-*)
  const desktop = DESKTOP.includes(platform)
  const chrome: WindowChrome = info
    ? { controls: info.windowControls, height: info.titleBar.height, inset: info.titleBar.controls, backdrop: info.backdrop }
    : desktop
      ? { controls: platform === 'macos' ? 'mac-inset' : 'windows', height: platform === 'macos' ? 48 : 32, inset: null, backdrop: 'none' }
      : { controls: 'none', height: 0, inset: null, backdrop: 'none' }

  // В `cargo tauri dev` страница открыта с dev-сервера, и /api проксируется — адрес не нужен
  const server = (info?.serverUrl ?? '').replace(/\/$/, '')
  const baseUrl = server && server !== location.origin ? server : ''

  // Токен держим в памяти и обновляем, когда оболочка сообщает о входе или выходе
  let token: string | null | undefined
  const getToken = async () => {
    if (token === undefined) token = await (await loadShell()).auth.getToken().catch(() => null)
    return token
  }

  async function me(): Promise<Me | null> {
    const t = await getToken()
    if (!t) return null
    let res: Response
    try {
      res = await fetch(`${baseUrl}/api/auth/me`, { headers: { Authorization: `Bearer ${t}`, Accept: 'application/json' } })
    }
    catch {
      throw new Error('network')
    }
    if (res.status === 401) {
      await (await loadShell()).auth.logout().catch(() => {})
      token = null
      return null
    }
    if (!res.ok) throw new Error(`me ${res.status}`)
    return await res.json() as Me
  }

  if (typeof document !== 'undefined') {
    document.documentElement.dataset.backdrop = chrome.backdrop
    void loadShell().then((shell) => {
      // Новый адрес сервера — перезагрузка: все запросы и сокет пойдут туда
      void shell.on('server:changed', () => location.reload())
    }).catch(() => {})
  }

  // Умный офлайн: медиатека из локальной базы, скачанное, волна без сети (./offline.ts)
  const offline = createOfflineBridge()
  let forceOffline = false
  offline.onStatus(s => (forceOffline = s.settings.forceOffline))
  void offline.status().then(s => (forceOffline = s.settings.forceOffline)).catch(() => {})

  // Мини-плеер — окно оболочки на компьютере (apps/shell/src/mini.rs)
  const miniPlayer = platform === 'macos' || platform === 'windows'
    ? {
        open: async () => (await loadShell()).mini.open(),
        close: async () => (await loadShell()).mini.close(),
        toggle: async () => (await loadShell()).mini.toggle(),
      }
    : null

  const bridge = withOffline(createWebBridge({
    kind: 'tauri',
    offline,
    haptic,
    // Push (APNs) — на iPhone и iPad; на компьютере, пока приложение открыто, — системные уведомления
    appPush: platform === 'ios' || platform === 'ipados' ? appPush : null,
    systemNotify: platform === 'macos' || platform === 'windows' ? systemNotify : null,
    // «Без сети»: запросы на сервер не уходят — экраны берут локальную базу и скачанное
    blocked: () => forceOffline,
    platform,
    isDesktopApp: desktop,
    chrome,
    audio: () => new NativeAudioEngine(loadShell),
    // Загрузчик музыки — только в приложении для компьютера (apps/shell/src/downloader.rs)
    downloader: platform === 'macos' || platform === 'windows' ? createTauriDownloader(loadShell) : null,
    // Обучение рекомендаций на этом компьютере (apps/shell/src/trainer.rs)
    trainer: platform === 'macos' || platform === 'windows' ? createTauriTrainer(loadShell) : null,
    outputs: createTauriOutputs(platform, loadShell),
    baseUrl,
    token: getToken,
    // «Экспорт»: на компьютере — окно «Сохранить как», файл качает оболочка потоком (apps/shell/src/export.rs);
    // на iPhone и iPad — файл целиком и системное «Поделиться»
    async saveFile(path, name, http) {
      if (desktop) {
        const { invoke } = await import('@tauri-apps/api/core')
        const saved = await invoke<string | null>('export_save', { path, name })
        return saved ? 'saved' : 'cancelled'
      }
      const res = await http.raw('GET', path)
      if (!res.ok) throw toApiError(res.status, await res.json().catch(() => null))
      return shareOrSaveBlob(await res.blob(), name)
    },
    // 401 на любом запросе: токен отозван — стираем его, экран входа покажет хранилище входа
    onUnauthorized: () => {
      token = null
      void loadShell().then(shell => shell.auth.logout()).catch(() => {})
    },
    onSessionEnded: (end) => {
      void loadShell().then(shell => shell.on('auth:changed', ({ session }) => {
        token = undefined
        if (!session) end()
      })).catch(() => {})
    },
    window: {
      minimize: async () => (await loadShell()).window.minimize(),
      toggleMaximize: async () => (await loadShell()).window.toggleMaximize(),
      close: async () => (await loadShell()).window.close(),
      onState(cb: (state: WindowState) => void) {
        const offs = [
          on(shell => shell.on('window:maximized', ({ maximized }) => cb({ maximized }))),
          on(shell => shell.on('window:focus', ({ focused }) => cb({ focused }))),
          on(shell => shell.on('window:fullscreen', ({ fullscreen }) => cb({ fullscreen }))),
        ]
        void loadShell().then(shell => shell.window.isMaximized()).then(maximized => cb({ maximized })).catch(() => {})
        return () => offs.forEach(off => off())
      },
      // Тему выбирает интерфейс (plugins/theme.client.ts): окно, «светофор», Mica, строка состояния
      setTheme(theme) {
        void loadShell().then(shell => shell.window.setTheme(theme)).catch(() => {})
      },
      // «Назад» смахиванием — только iPhone и iPad (plugins/back-swipe.client.ts)
      setBackSwipe: platform === 'ios' || platform === 'ipados'
        ? (enabled) => {
            void loadShell().then(shell => shell.window.setBackSwipe(enabled)).catch(() => {})
          }
        : () => {},
      trackSnap(button) {
        let stop: (() => void) | null = null
        let stopped = false
        void loadShell().then((shell) => {
          if (!stopped) stop = trackSnapArea(shell, button)
        }).catch(() => {})
        return () => {
          stopped = true
          stop?.()
        }
      },
    },
    auth: {
      me,
      async login() {
        const shell = await loadShell()
        try {
          await shell.auth.login()
        }
        catch (e) {
          if (isShellError(e)) {
            if (e.code === 'cancelled') return null
            throw new ShellLoginError(e.code, e.message)
          }
          throw e
        }
        token = undefined
        return me()
      },
      cancelLogin: async () => {
        await (await loadShell()).auth.cancel()
      },
      logout: async () => {
        await (await loadShell()).auth.logout()
        token = null
      },
    },
    media: {
      // «Сейчас играет» ведёт движок: он знает трек, позицию и паузу раньше интерфейса
      nowPlaying() {},
      // Сюда приходит только то, что движок не выполнил сам (например, «играть» без очереди)
      onCommand(cb) {
        return on(shell => shell.on('media:command', (c) => {
          if (c.action === 'seek') cb({ action: 'seek', positionMs: c.positionMs })
          else if (c.action === 'seekBy') cb({ action: 'seekBy', offsetMs: c.offsetMs })
          else if (c.action === 'setShuffle') cb({ action: 'setShuffle', shuffle: c.shuffle })
          else if (c.action === 'setRepeat') cb({ action: 'setRepeat', mode: c.mode })
          else cb({ action: c.action })
        }))
      },
    },
    // Обновляется только приложение для компьютера: на iPhone и iPad команд обновления нет
    updates: {
      onAvailable: cb => on(shell => shell.on('update:available', u => cb({ version: u.version, currentVersion: u.currentVersion, notes: u.notes }))),
      state: async () => (desktop ? toUpdateState(await (await loadShell()).updates.state()) : null),
      onState: cb => (desktop ? on(shell => shell.on('update:state', s => cb(toUpdateState(s)))) : () => {}),
      check: async () => {
        if (desktop) await (await loadShell()).updates.check()
      },
      install: async () => (await loadShell()).updates.install(),
    },
    setLocale(locale) {
      void loadShell().then(shell => shell.setLocale(locale)).catch(() => {})
    },
  }), offline)
  bridge.miniPlayer = miniPlayer
  bridge.recognizer = createTauriRecognizer(platform, loadShell)
  return bridge
}
