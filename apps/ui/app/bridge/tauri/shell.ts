// КОПИЯ договора оболочки: apps/shell/bridge/shell.ts (источник — там, вместе с apps/shell/BRIDGE.md).
// Сборка сайта в Docker видит только apps/ui, поэтому файл лежит здесь как есть. Обновлять копированием.
/* eslint-disable */
// Мост интерфейса к оболочке DalyWave (Tauri). Договор — apps/shell/BRIDGE.md.
// Файл без зависимостей: invoke и listen передаёт интерфейс из @tauri-apps/api,
// поэтому его можно подключить псевдонимом или скопировать в apps/ui как есть.

export type Os = 'macos' | 'windows' | 'linux'

/**
 * Кто рисует кнопки окна:
 * - `mac-inset` — «светофор» рисует macOS поверх интерфейса, слева в строке заголовка;
 * - `windows` — интерфейс рисует свои «свернуть / развернуть / закрыть» справа;
 * - `none` — у окна обычная системная рамка.
 */
export type WindowControls = 'mac-inset' | 'windows' | 'none'

/** CSS-пиксели от левого верхнего угла окна */
export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

export interface PlatformInfo {
  os: Os
  arch: string
  appVersion: string
  windowControls: WindowControls
  titleBar: {
    /** Рекомендуемая высота своей строки заголовка (macOS: «светофор» по центру; Windows: 32) */
    height: number
    /** Где лежит «светофор» macOS; на Windows — null */
    controls: Rect | null
  }
  /** `mica` — под прозрачными местами интерфейса системный фон Windows 11 */
  backdrop: 'mica' | 'none'
  /** Запущено через `cargo tauri dev` с dev-сервера интерфейса */
  dev: boolean
  /** Адрес сервера при запуске; дальше — `server.get()` и событие `server:changed` */
  serverUrl: string
  locale: Locale
}

export type Locale = 'ru' | 'en'
export type Theme = 'dark' | 'light'

export type ShellErrorCode =
  | 'cancelled'
  | 'timeout'
  | 'network'
  | 'server'
  | 'keychain'
  | 'invalid'
  | 'update'
  | 'internal'
  // коды сервера при входе — без изменений
  | 'expired'
  | 'state'
  | 'disabled'
  | 'provider'
  | 'invalid_request'
  | 'invalid_grant'
  | 'forbidden'
  | 'too_many_requests'
  | (string & {})

/** Так отклоняется промис любой команды */
export interface ShellError {
  code: ShellErrorCode
  /** Для журнала; тексты для людей — в i18n интерфейса по `code` */
  message: string
}

export function isShellError(e: unknown): e is ShellError {
  return typeof e === 'object' && e !== null && 'code' in e && 'message' in e
}

/** `User` и `Device` — типы API сервера (`~/api/types`) */
export interface Session<User = unknown, Device = unknown> {
  serverUrl: string
  /** null — токен есть, но профиль не сохранился: запросить `/api/auth/me` */
  user: User | null
  device: Device | null
}

export interface NowPlaying {
  title: string
  artist: string
  album?: string | null
  /** http(s) или file:// */
  artworkUrl?: string | null
  durationMs?: number | null
  /** Позиция в момент события; дальше система досчитывает сама */
  positionMs?: number | null
  playing: boolean
}

/** Откуда команда: медиаклавиши, наушники, Пункт управления, плашка Windows — `system` */
export type MediaSource = 'system' | 'tray'

export type MediaCommand =
  | { action: 'play' | 'pause' | 'toggle' | 'next' | 'previous' | 'stop', source: MediaSource }
  | { action: 'seek', positionMs: number, source: MediaSource }
  | { action: 'seekBy', offsetMs: number, source: MediaSource }

export interface UpdateInfo {
  version: string
  currentVersion: string
  notes: string | null
  date: string | null
}

export interface ShellEvents {
  'window:maximized': { maximized: boolean }
  'window:fullscreen': { fullscreen: boolean }
  'window:focus': { focused: boolean }
  /** Windows: наведение и нажатие на кнопку «развернуть» (её закрывает слой привязки окон) */
  'window:caption': { button: 'maximize', state: 'enter' | 'leave' | 'down' | 'up' }
  'auth:changed': { session: Session | null }
  'media:command': MediaCommand
  'server:changed': { serverUrl: string }
  'update:available': UpdateInfo
  'update:progress': { downloadedBytes: number, totalBytes: number | null }
  /** Ссылки dalywave:// кроме возврата входа — на будущее (треки, альбомы) */
  'deep-link:open': { url: string }
}

export type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>
export type ListenFn = <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>

declare global {
  interface Window {
    __DALYWAVE_SHELL__?: PlatformInfo
  }
}

/** Сведения оболочки до загрузки страницы: синхронно, для первой отрисовки. На сайте — null. */
export function shellInfo(): PlatformInfo | null {
  return typeof window !== 'undefined' ? window.__DALYWAVE_SHELL__ ?? null : null
}

export function createShell(tauri: { invoke: InvokeFn, listen: ListenFn }) {
  const call = <T>(cmd: string, args?: Record<string, unknown>) => tauri.invoke<T>(cmd, args)
  return {
    info: shellInfo(),
    /** То же, что `info`, но с живым замером «светофора» */
    platformInfo: () => call<PlatformInfo>('platform_info'),
    window: {
      minimize: () => call<void>('window_minimize'),
      /** Новое состояние: true — развёрнуто */
      toggleMaximize: () => call<boolean>('window_toggle_maximize'),
      /** Прячет окно в трей, музыка продолжает играть */
      close: () => call<void>('window_close'),
      startDragging: () => call<void>('window_start_dragging'),
      isMaximized: () => call<boolean>('window_is_maximized'),
      /** Тема системных частей окна; null — как в системе */
      setTheme: (theme: Theme | null) => call<void>('window_set_theme', { theme }),
      /** Windows 11: где кнопка «развернуть» (для меню привязки окон); null — убрать */
      setSnapArea: (rect: Rect | null) => call<void>('window_set_snap_area', { rect }),
    },
    auth: {
      /** Открывает браузер и ждёт возврата (до 10 минут); токен остаётся в связке ключей */
      login: <User = unknown, Device = unknown>() => call<Session<User, Device>>('auth_login'),
      /** Прервать вход: промис login() отклонится с `cancelled` */
      cancel: () => call<boolean>('auth_cancel'),
      logout: () => call<void>('auth_logout'),
      /** `Authorization: Bearer <token>`; null — вход не выполнен */
      getToken: () => call<string | null>('auth_token'),
      /** Сохранённый вход без сети; null — токена нет */
      session: <User = unknown, Device = unknown>() => call<Session<User, Device> | null>('auth_session'),
    },
    nowPlaying: {
      /** При смене трека, паузе, запуске и перемотке; null — ничего не играет */
      update: (state: NowPlaying | null) => call<void>('now_playing_update', { state }),
    },
    /** http, https, mailto — в системном браузере */
    openExternal: (url: string) => call<void>('open_external', { url }),
    server: {
      get: () => call<string>('server_url_get'),
      /** null — адрес по умолчанию */
      set: (url: string | null) => call<string>('server_url_set', { url }),
    },
    /** Язык меню в трее и строке меню */
    setLocale: (locale: Locale) => call<void>('shell_set_locale', { locale }),
    updates: {
      check: () => call<UpdateInfo | null>('update_check'),
      /** Скачивает, ставит и перезапускает приложение */
      install: () => call<void>('update_install'),
    },
    on<E extends keyof ShellEvents>(event: E, handler: (payload: ShellEvents[E]) => void): Promise<() => void> {
      return tauri.listen<ShellEvents[E]>(event, e => handler(e.payload))
    },
  }
}

export type Shell = ReturnType<typeof createShell>

/**
 * Windows 11: держит слой меню привязки окон точно над своей кнопкой «развернуть».
 * Слой перехватывает мышь, поэтому :hover над кнопкой не сработает — вместо него
 * кнопка получает атрибуты `data-caption-hover` и `data-caption-pressed`.
 * На macOS ничего не делает. Возвращает функцию отписки.
 */
export function trackSnapArea(shell: Shell, button: HTMLElement): () => void {
  if (shell.info?.windowControls !== 'windows') return () => {}
  let last = ''
  const sync = () => {
    const r = button.getBoundingClientRect()
    const visible = r.width > 0 && r.height > 0
    const rect = visible ? { x: r.left, y: r.top, width: r.width, height: r.height } : null
    const key = JSON.stringify(rect)
    if (key === last) return
    last = key
    void shell.window.setSnapArea(rect)
  }
  const observer = new ResizeObserver(sync)
  observer.observe(button)
  observer.observe(document.documentElement)
  window.addEventListener('resize', sync)
  sync()
  const stop = shell.on('window:caption', ({ state }) => {
    if (state === 'enter') button.dataset.captionHover = ''
    if (state === 'leave') {
      delete button.dataset.captionHover
      delete button.dataset.captionPressed
    }
    if (state === 'down') button.dataset.captionPressed = ''
    if (state === 'up') delete button.dataset.captionPressed
  })
  return () => {
    observer.disconnect()
    window.removeEventListener('resize', sync)
    void stop.then(unlisten => unlisten())
    void shell.window.setSnapArea(null)
  }
}
