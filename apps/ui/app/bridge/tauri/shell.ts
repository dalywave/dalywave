// КОПИЯ договора оболочки: apps/shell/bridge/shell.ts (источник — там, вместе с apps/shell/BRIDGE.md).
// Сборка сайта в Docker видит только apps/ui, поэтому файл лежит здесь как есть. Обновлять копированием.
/* eslint-disable */
// Мост интерфейса к оболочке DalyWave (Tauri). Договор — apps/shell/BRIDGE.md.
// Файл без зависимостей: invoke и listen передаёт интерфейс из @tauri-apps/api,
// поэтому его можно подключить псевдонимом или скопировать в apps/ui как есть.

export type Os = 'macos' | 'windows' | 'linux' | 'ios' | 'ipados'

/**
 * Кто рисует кнопки окна:
 * - `mac-inset` — «светофор» рисует macOS поверх интерфейса, слева в строке заголовка;
 * - `windows` — интерфейс рисует свои «свернуть / развернуть / закрыть» справа;
 * - `none` — у окна обычная системная рамка или окна нет вовсе (iPhone, iPad).
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
  | 'audio'
  /** Команды нет на этой платформе: `audio.setOutput` на iPhone, `audio.showPicker` на компьютере */
  | 'unsupported'
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

/** Трек очереди для движка приложения */
export interface PlayerItem {
  /** Ключ элемента очереди интерфейса: возвращается в событиях `player:*` */
  id: string
  /** Трек на сервере: по нему движок сам просит ссылку, если её нет или она истекла */
  trackId: string
  /** Подписанная ссылка на файл; null — движок спросит сервер, когда трек понадобится */
  url?: string | null
  /** Для выравнивания громкости (ответ `/api/tracks/{id}/stream`) */
  loudnessLufs?: number | null
  truePeakDb?: number | null
  durationMs?: number | null
  /** Для «Сейчас играет», экрана блокировки и трея */
  title: string
  artist: string
  album?: string | null
  /** http(s) */
  artworkUrl?: string | null
}

export type PlayerTransition = { mode: 'gapless' } | { mode: 'crossfade', ms: number } | { mode: 'dj', expressiveness: number }
export type PlayerStateName = 'idle' | 'loading' | 'playing' | 'paused' | 'ended'

export interface PlayerSnapshot {
  state: PlayerStateName
  /** Ключ текущего элемента очереди */
  id: string | null
  positionMs: number
  durationMs: number | null
  buffering: boolean
  queueLen: number
  deviceRate: number
}

/** Пресеты эквалайзера; `custom` — «Свой»: полосы `gains` как есть */
export type PlayerEqPreset = 'off' | 'bass-boost' | 'bass-cut' | 'vocal' | 'acoustic' | 'electronic' | 'loudness' | 'custom'

/** Эквалайзер этого устройства: 10 полос 31 Гц … 16 кГц, усиления ±12 дБ (`gains` — «Свой») */
export interface PlayerEq {
  preset: PlayerEqPreset
  gains: number[]
}

/** Таймер сна: через `ms` или в конце текущего трека */
export type PlayerSleepTimer = { mode: 'minutes', ms: number } | { mode: 'track' }

export interface PlayerSleep {
  /** null — таймера нет */
  timer: 'minutes' | 'track' | null
  /** Когда пауза, мс от эпохи Unix (у конца трека — по позиции) */
  endsAtMs: number | null
  remainingMs: number | null
  /** Звук уже затухает */
  fading: boolean
  /** Таймер сработал: музыка на паузе */
  finished: boolean
}

/** Что играет — для мини-плеера */
export interface PlayerNow {
  title: string
  artist: string
  album: string | null
  artworkUrl: string | null
  durationMs: number | null
  positionMs: number
  playing: boolean
  /** Ключ элемента очереди — для сверки с `player:position` */
  id: string | null
}

/** Уровень звука за ~50 мс до общей громкости: RMS, пик, полосы до 150 Гц, 150–2000 Гц, выше 2 кГц */
export interface PlayerLevel {
  rms: number
  peak: number
  bands: [number, number, number]
}

/** Откуда команда: медиаклавиши, наушники, Пункт управления, экран блокировки, плашка Windows — `system` */
export type MediaSource = 'system' | 'tray'

export type MediaCommand =
  | { action: 'play' | 'pause' | 'toggle' | 'next' | 'previous' | 'stop', source: MediaSource }
  | { action: 'seek', positionMs: number, source: MediaSource }
  | { action: 'seekBy', offsetMs: number, source: MediaSource }
  /** Пункт управления, CarPlay, Siri (macOS, iOS): перемешать или сменить повтор */
  | { action: 'setShuffle', shuffle: boolean, source: MediaSource }
  | { action: 'setRepeat', mode: PlayerRepeat, source: MediaSource }

/** Повтор: выключен, вся очередь по кругу, один трек */
export type PlayerRepeat = 'off' | 'all' | 'one'

export type AudioOutputKind = 'builtin' | 'headphones' | 'bluetooth' | 'airplay' | 'usb' | 'hdmi' | 'other'

export interface AudioOutput {
  /** Стабильный id устройства (macOS) или тип порта (iOS) */
  id: string
  /** Как назвать в интерфейсе: «Динамики MacBook Pro», «AirPods Захара» */
  name: string
  kind: AudioOutputKind
  /** Сейчас звук идёт сюда */
  current: boolean
}

export interface AudioOutputs {
  /** `list` — свой список устройств, выбор — `audio.setOutput` (macOS, Windows);
   *  `picker` — системное окно выбора, `audio.showPicker` (iOS, iPadOS) */
  mode: 'list' | 'picker'
  /** list: все устройства вывода; picker: текущий маршрут (обычно один элемент) */
  outputs: AudioOutput[]
  /** list: выбран «как в системе» (поток следует за устройством по умолчанию) */
  followsSystem: boolean
}

export interface UpdateInfo {
  version: string
  currentVersion: string
  notes: string | null
  date: string | null
}

/**
 * Обновление приложения (macOS, Windows): найденное скачивается в фоне само, приложение никогда не
 * перезапускается без человека.
 * - `idle` — обновлений нет; `checking` — спрашиваем сервер; `downloading` — качаем `update`;
 * - `ready` — скачано и проверено подписью: «Перезапустить» (`updates.install()`);
 * - `installing` — ставим; `failed` — проверка или скачивание не удались (`error`).
 */
export type UpdateStatus = 'idle' | 'checking' | 'downloading' | 'ready' | 'installing' | 'failed'

export interface UpdateState {
  status: UpdateStatus
  currentVersion: string
  /** Найденная версия: при downloading, ready, installing и сбое скачивания */
  update: UpdateInfo | null
  downloadedBytes: number | null
  totalBytes: number | null
  /** Последняя удачная проверка, мс от эпохи Unix */
  checkedAtMs: number | null
  error: string | null
  /** macOS: готовое обновление ставится и при обычном выходе из приложения */
  installOnQuit: boolean
}

/**
 * Загрузчик на этом Mac (только macOS) с точки зрения приложения:
 * - `disabled` — выключен («Скачивать на этом Mac»); `signed-out` — нет входа;
 *   `forbidden` — токен загрузчика заводит только администратор;
 * - `provisioning` — заводим токен на сервере; `starting` — процесс запущен, состояния ещё нет;
 * - `running` — работает, подробности — в `status`; `stopping` — останавливается;
 * - `restarting` — упал или сервер не выдал токен, новая попытка в `retryAtMs`;
 * - `token-rejected` — сервер не принял токен (отозван): `downloader.recreateToken()`;
 * - `broken` — программы загрузчика не нашлись или не запускаются.
 */
export type DownloaderPhase =
  | 'disabled' | 'signed-out' | 'forbidden' | 'provisioning' | 'starting' | 'running'
  | 'stopping' | 'restarting' | 'token-rejected' | 'broken'

/** Этап задания: Deezer и поиск на YouTube Music → скачивание → отпечаток и громкость → в хранилище */
export type DownloaderStage = 'matching' | 'downloading' | 'verifying' | 'uploading'

/** Событие `status` самого загрузчика (services/fetcher/SIDECAR.md, «Протокол») */
export interface DownloaderStatus {
  state: 'starting' | 'idle' | 'working' | 'paused' | 'offline' | 'error' | 'stopping'
  /** Желание пользователя: при `working` — «доделает трек, потом пауза» */
  paused: boolean
  server: string
  workerName: string
  /** Только при `working`; `track` и `artist` — null первые ~0,3 с */
  job: {
    id: string
    deezerTrackId: number | null
    track: string | null
    artist: string | null
    stage: DownloaderStage
    /** 0…1 внутри этапа */
    progress: number
    /** Номер кандидата */
    attempt: number
  } | null
  /** `offline`: `server_unreachable`, `server_error`; `error`: `bot_check`, `rate_limited`, … */
  error: { code: string, message: string } | null
  /** RFC 3339: следующая попытка (`offline`) или конец отдыха (`error`) */
  retryAt: string | null
  /** За сегодня по часам Mac */
  counters: { date: string, doneToday: number, failedToday: number }
  pot: { state: 'starting' | 'ready' | 'failed' | 'off', version?: string, message?: string }
}

export interface DownloaderSnapshot {
  /** «Скачивать на этом Mac» */
  enabled: boolean
  /** Пауза, которую оставил пользователь (запоминается) */
  paused: boolean
  phase: DownloaderPhase
  /** Последний `status` загрузчика — пока он работает */
  status: DownloaderStatus | null
  server: string
  /** Имя в админке («Источники»): «Mac <имя компьютера>» */
  workerName: string
  logFile: string
  /** Версии из `hello` */
  version: string | null
  ytDlp: string | null
  /** Почему не работает (выдача токена, запуск, код выхода); `message` — для журнала */
  error: { code: string, message: string } | null
  /** `restarting`: когда следующая попытка, мс от 1970 года */
  retryAtMs: number | null
}

/** «Распознать» (ShazamKit): совпадение как есть — трек в каталоге находит сервер (`POST /api/recognize/resolve`) */
export interface RecognizeMatch {
  title: string
  artist: string
  isrc: string | null
  appleMusicId: string | null
  shazamId: string | null
  artworkUrl: string | null
}

export interface ShellEvents {
  'window:maximized': { maximized: boolean }
  'window:fullscreen': { fullscreen: boolean }
  'window:focus': { focused: boolean }
  /** Windows: наведение и нажатие на кнопку «развернуть» (её закрывает слой привязки окон) */
  'window:caption': { button: 'maximize', state: 'enter' | 'leave' | 'down' | 'up' }
  'auth:changed': { session: Session | null }
  /** Команда пульта, которую движок не выполнил сам: «играть» без очереди, «следующий» в конце
   *  очереди движка, «предыдущий» в начале — их выполняет плеер интерфейса */
  'media:command': MediaCommand
  'player:state': { state: PlayerStateName, buffering: boolean }
  /** Каждые 250 мс во время игры, сразу после перемотки и паузы */
  'player:position': { id: string, positionMs: number, durationMs: number | null }
  /** Играет другой элемент очереди: `auto` — движок перешёл сам, `user` — команда (и с пульта) */
  'player:track': { id: string, index: number, reason: 'auto' | 'user', durationMs: number | null }
  'player:buffering': { buffering: boolean }
  'player:error': { id: string | null, message: string }
  /** Очередь движка доиграла: следующий трек решает интерфейс */
  'player:ended': null
  /** ~20 раз в секунду, пока включено `player.setLevels(true)` */
  'player:level': PlayerLevel
  /** Таймер сна: поставили, сняли, началось затухание, сработал (`finished`) */
  'player:sleep': PlayerSleep
  /** Только окну мини-плеера: другой трек, пауза, конец очереди */
  'player:now': PlayerNow | null
  /** Список устройств вывода или маршрут звука поменялся (AirPlay, наушники, Bluetooth) */
  'audio:outputs': AudioOutputs
  'server:changed': { serverUrl: string }
  'update:available': UpdateInfo
  'update:progress': { downloadedBytes: number, totalBytes: number | null }
  /** Обновление скачано и проверено — можно предложить «Перезапустить» */
  'update:ready': UpdateInfo
  /** Каждая перемена у обновления; ход скачивания — до 4 раз в секунду */
  'update:state': UpdateState
  /** Ссылки dalywave:// кроме возврата входа: `dalywave://share?url=…` и ссылки на экраны. Ссылку
   *  забирать через `links.take()` — так она не обработается дважды и не потеряется при запуске */
  'deep-link:open': { url: string }
  /** macOS: загрузчик на этом Mac — при каждой перемене, ход задания — до 4 раз в секунду */
  'downloader:state': DownloaderSnapshot
  /** Открыть экран «Распознать»: пункт трея macOS, быстрое действие на значке iPhone. Просьбу забрать
   *  `recognize.takePending()` — и при запуске (приложение открыли этим действием) */
  'recognize:open': null
}

/** Статус в Discord: `connected` — Discord запущен и статус до него доходит */
export interface DiscordState {
  available: boolean
  enabled: boolean
  connected: boolean
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
    /** Звук приложения: движок на Rust, «Сейчас играет» и пульт системы он ведёт сам */
    player: {
      /** Новая очередь: `items[start]` начинает играть с `startMs` */
      load: (items: PlayerItem[], opts: { start?: number, startMs?: number, autoplay?: boolean } = {}) =>
        call<void>('player_load', { items, start: opts.start ?? 0, startMs: opts.startMs ?? 0, autoplay: opts.autoplay ?? true }),
      /** Что играть после текущего: следующий движок готовит заранее и сводит сам */
      setUpcoming: (items: PlayerItem[]) => call<void>('player_set_upcoming', { items }),
      play: () => call<void>('player_play'),
      pause: () => call<void>('player_pause'),
      toggle: () => call<void>('player_toggle'),
      seek: (positionMs: number) => call<void>('player_seek', { positionMs: Math.max(0, Math.round(positionMs)) }),
      next: () => call<void>('player_next'),
      previous: () => call<void>('player_previous'),
      stop: () => call<void>('player_stop'),
      /** 0…1 */
      setVolume: (volume: number) => call<void>('player_set_volume', { volume }),
      setTransition: (transition: PlayerTransition) => call<void>('player_set_transition', { transition }),
      /** Выравнивание громкости по LUFS */
      setNormalize: (enabled: boolean) => call<void>('player_set_normalize', { enabled }),
      /** События `player:level` — только пока их кто-то рисует */
      setLevels: (enabled: boolean) => call<void>('player_set_levels', { enabled }),
      state: () => call<PlayerSnapshot>('player_state'),
      /** Эквалайзер этого устройства: запоминается оболочкой и действует сразу; null — выключить */
      setEq: (eq: PlayerEq | null) => call<void>('player_set_eq', { eq }),
      /** Сохранённый эквалайзер; null — не настраивали */
      eq: () => call<PlayerEq | null>('player_eq'),
      /** Таймер сна; null — снять. Ход — событие `player:sleep` */
      setSleep: (timer: PlayerSleepTimer | null) => call<void>('player_set_sleep', { timer }),
      sleep: () => call<PlayerSleep>('player_sleep'),
      /** Перемешивание и повтор плеера: повтор одного трека ведёт движок (и в фоне), оба режима
       *  показываются в Пункте управления */
      setModes: (shuffle: boolean, repeat: PlayerRepeat) => call<void>('player_set_modes', { shuffle, repeat }),
      /** Мини-плеер: что играет; null — ничего */
      now: () => call<PlayerNow | null>('player_now'),
      /** Мини-плеер: кнопки — как пульт (что движок не может сам, уходит главному окну) */
      remote: (action: 'toggle' | 'play' | 'pause' | 'next' | 'previous') => call<void>('player_remote', { action }),
    },
    /** Мини-плеер (macOS, Windows): окно поверх всех; главное окно на это время прячется */
    mini: {
      open: () => call<void>('mini_open'),
      /** Закрыть и вернуть главное окно */
      close: () => call<void>('mini_close'),
      toggle: () => call<void>('mini_toggle'),
    },
    /** Куда идёт звук: шторка «Устройства» */
    audio: {
      outputs: () => call<AudioOutputs>('audio_outputs'),
      /** macOS, Windows: вывести на устройство; null — как в системе. Выбор запоминается */
      setOutput: (id: string | null) => call<AudioOutputs>('audio_set_output', { id }),
      /** iPhone, iPad: системное окно выбора AirPlay и Bluetooth */
      showPicker: () => call<void>('audio_show_picker'),
    },
    /** http, https, mailto — в системном браузере */
    openExternal: (url: string) => call<void>('open_external', { url }),
    /** Ссылки dalywave:// для интерфейса (`dalywave://share?url=…` — «Поделиться → DalyWave»): последняя
     *  необработанная, и забыть её. Звать при запуске и на каждое `deep-link:open` */
    links: {
      take: () => call<string | null>('links_take'),
    },
    /** Статус «Слушает DalyWave» в Discord — только macOS и Windows (на iPhone команд нет).
     *  `available: false` — в сборке нет id приложения Discord, переключатель не показывать */
    discord: {
      state: () => call<DiscordState>('discord_state'),
      setEnabled: (enabled: boolean) => call<DiscordState>('discord_set_enabled', { enabled }),
    },
    /** «Распознать» — macOS, iPhone, iPad (ShazamKit); на Windows — `unsupported`. Своя музыка на паузе,
     *  пока слушаем; не узнали или отменили — продолжается */
    recognize: {
      /** До совпадения, не дольше 15 с. Ошибки: `no_match`, `permission`, `no_microphone`, `cancelled`, `shazam` */
      listen: () => call<RecognizeMatch>('recognize_listen'),
      cancel: () => call<boolean>('recognize_cancel'),
      /** Просьба открыть экран, пришедшая до загрузки интерфейса; забирается один раз */
      takePending: () => call<boolean>('recognize_take_pending'),
    },
    server: {
      get: () => call<string>('server_url_get'),
      /** null — адрес по умолчанию */
      set: (url: string | null) => call<string>('server_url_set', { url }),
    },
    /** Язык меню в трее и строке меню */
    setLocale: (locale: Locale) => call<void>('shell_set_locale', { locale }),
    updates: {
      /** Проверить сейчас; найденное скачивается в фоне (итог — `update:state`) */
      check: () => call<UpdateInfo | null>('update_check'),
      state: () => call<UpdateState>('update_state'),
      /** «Перезапустить»: ставит скачанное обновление и перезапускает приложение */
      install: () => call<void>('update_install'),
    },
    /** Загрузчик музыки на этом Mac — только macOS (на других системах команд нет) */
    downloader: {
      state: () => call<DownloaderSnapshot>('downloader_state'),
      /** «Скачивать на этом Mac» — запоминается; включение заводит токен, если его ещё нет */
      setEnabled: (enabled: boolean) => call<DownloaderSnapshot>('downloader_set_enabled', { enabled }),
      /** Пауза: текущий трек загрузчик доделывает, новых заданий не берёт; запоминается */
      setPaused: (paused: boolean) => call<DownloaderSnapshot>('downloader_set_paused', { paused }),
      /** `token-rejected`: забыть отозванный токен и завести новый */
      recreateToken: () => call<DownloaderSnapshot>('downloader_recreate_token'),
      /** Журнал загрузчика — в «Консоли» */
      openLog: () => call<void>('downloader_open_log'),
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
