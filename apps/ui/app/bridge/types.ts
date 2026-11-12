// «Мост» — единственный путь экранов к данным и звуку. На сайте за ним HTTP и HTML-аудио,
// в приложениях — оболочка Tauri (окно, вход через системный браузер) и аудиодвижок на Rust,
// который сам ведёт «Сейчас играет» и пульт системы. Экраны пишутся один раз.
import type { EqConfig } from '~/utils/eq'
import type {
  AddPlaylistItems, AddedPlaylistItems, AdminAcquisition, AdminAcquisitionPage, AdminImportList, AdminOverview, AdminRecordingPage,
  AcquisitionStatus, AudioStatus, DownloaderToken, RecoOverview, RecoRunResult, RecoSettings,
  RecoSettingsPatch, RecoTask, ResolveRequest, ServerSettings, ServerSettingsPatch, Sources, StorageOverview, TrackPreview, AlbumDetail, ArtistDetail, AuditPage, AuthProviders, BlockedKind, BlockedList,
  ClientMessage, DeviceInfo, FeedbackEvent, FollowRequest, FollowedArtistPage, Health, HistoryPage, Home, JobPage, JobStatus,
  LaneList, LanePatch, LibraryOverview, LikedTrackPage, Me, NewPlaylist, PlayEvent, PlaylistDetail, PlaylistList,
  PlaylistPatch, PlaylistSummary, SavedAlbumPage, SearchResults, SearchScope, ServerEvent, ServerEventType,
  SessionList, Track, UserList, UserPatch, WaveFeedbackKind,
  SoundSearchResults, TasteDamper, TasteOverview, Rewind, RewindEvent, RewindEventPatch, RewindMore, RewindSoundtrack, RewindYears, WaveItems, WaveSeed, NewWavePreset, WavePreset, WavePresetPatch,
  ImportItem, ImportItemFix, ImportItemPage, ImportItemStatus, ImportList, ImportSummary, ListImportRequest, YandexImportRequest,
  LyricsSearchResults, TrackLyrics, TrackVersions, VersionChoice, VersionKind,
  HistoryDays, WrongRecordingReport, WrongRecordingResult,
  Recognition, RecognitionPage, RecognizeRequest,
  ResolvedLink,
  MovePlaylist, MovePlaylistFolder, SetApplied, SetOptions, SetPreview, NetworkKind, NewPlaylistFolder, PlaylistFolder, PlaylistFolderPatch, UserSettings, UserSettingsPatch,
  Jam, JamGuestView, JamInvite, JamJoined, JamPatch, JamRequest, JamSkip, JamStream, JamTrack,
  NotificationPage, NotificationSettings, NotificationSettingsPatch, NotificationsRead, PushInfo, PushRegistered, PushRegistration,
  PushTest, PushUnregister,
  FriendList, FriendProfile, FriendRequestResult, InviteLink, PeopleList, PersonCard, Privacy, PrivacyPatch, PlaylistInvite,
  DescribeBrief, DescribePreview, DescribedPlaylist, SaveDescribed,
} from '~/api/types'
import type { DownloaderSnapshot } from './tauri/shell'

export type { DownloaderPhase, DownloaderSnapshot, DownloaderStage, DownloaderStatus } from './tauri/shell'

export type Platform = 'web' | 'macos' | 'windows' | 'linux' | 'ios' | 'ipados' | 'android'

/** Разделы, которые в разработке могут отдаваться моком, пока сервер их не умеет */
export type Section = 'catalog' | 'home' | 'library' | 'events' | 'stream' | 'wave' | 'devices'

export type WaveMood = 'any' | 'energetic' | 'happy' | 'calm' | 'sad'
export type WaveActivity = 'any' | 'wake' | 'road' | 'work' | 'sport' | 'rest' | 'sleep' | 'party'
export type WaveCharacter = 'fav' | 'mix' | 'new' | 'pop' | 'rare'
export type WaveLanguage = 'any' | 'ru' | 'foreign' | 'none'

export interface WaveSettings {
  mood: WaveMood
  activity: WaveActivity
  character: WaveCharacter
  language: WaveLanguage
}

/** Трек волны и почему он здесь («Похоже на Lowborn») */
export interface WaveTrack {
  track: Track
  reason: string
}

/**
 * Начало волны: сервер подбирает его под настройки до запуска. `preparing` — первые треки ещё
 * скачиваются (не дольше полутора минут), `ready` — очередь можно играть, готовое в ней первым.
 */
export interface WaveStart {
  id: string
  state: 'preparing' | 'ready'
  /** Сколько треков начала уже на сервере … */
  ready: number
  /** … и сколько нужно, чтобы начать */
  need: number
  queue: WaveTrack[]
}

export interface TrackSource {
  url: string
  /** Длительность файла, если известна заранее */
  durationMs?: number
  /** Выравнивание громкости: на сколько дБ сдвинуть, чтобы трек звучал как остальные */
  gainDb?: number
  /** Громкость из анализа сервера — движку приложения: он выравнивает сам */
  loudnessLufs?: number
  truePeakDb?: number
  /** Сетка долей из анализа трека — для пульса волны */
  bpm?: number
  beatPhase?: number
  /** 30-секундное превью вместо трека (только разработка) */
  preview?: boolean
  /** Что на самом деле играет — для значка качества: «Opus 131», «FLAC» */
  codec?: string
  bitrateKbps?: number
}

/**
 * Ответ `GET /api/tracks/{id}/stream` (этап 1b):
 * 200 — файл готов; 202 — сервер скачивает запись или ждёт загрузчик (прогресс — событием
 * `acquisition.updated` по WebSocket).
 */
export type StreamResponse =
  | {
    status: 'ready'
    url: string
    mimeType: string
    codec?: string
    bitrateKbps?: number | null
    loudnessLufs?: number | null
    truePeakDb?: number | null
    /** 512 значений 0…1000 — мост переводит в 0…1 */
    waveform?: number[] | null
    durationMs?: number | null
    bpm?: number | null
  }
  | {
    status: 'acquiring' | 'waiting_for_downloader'
    acquisitionId: string | null
    queuePosition: number | null
  }

/** Чем закончился запрос звука для трека */
export type SourceResult =
  /** source = null — звука нет, время идёт (только разработка, пока нет этапа 1b) */
  | { kind: 'ready', source: TrackSource | null, peaks: number[] | null }
  /** Сервер скачивает запись или ждёт загрузчик на Mac — трек начнётся сам, когда файл появится */
  | { kind: 'pending', status: 'acquiring' | 'waiting_for_downloader', acquisitionId: string | null, queuePosition: number | null }
  /** Звука нет: записи нет в источниках или файл недоступен */
  | { kind: 'unavailable' }

/** Устройство для «Где играть»: приложения и вкладки сайта (управление между устройствами) */
export interface PlaybackDevice {
  id: string
  name: string
  kind: 'laptop' | 'desktop' | 'phone' | 'tablet' | 'web' | 'watch'
  /** Это устройство */
  current: boolean
  online: boolean
  lastSeenAt: string | null
  /** Звук играет здесь (активное устройство) — даже если оно пропало из сети */
  active?: boolean
  /** Громкость меняется с пульта */
  volume?: boolean
  /** Только пульт (часы): звук там не играет, перенести туда нельзя */
  remote?: boolean
}

/** Куда идёт звук этого устройства: динамики, наушники, Bluetooth, AirPlay */
export type AudioOutputKind = 'builtin' | 'headphones' | 'bluetooth' | 'airplay' | 'usb' | 'hdmi' | 'other'

export interface AudioOutput {
  id: string
  name: string
  kind: AudioOutputKind
  /** Сейчас звук идёт сюда */
  current: boolean
}

export interface AudioOutputs {
  /** list — все устройства вывода; picker — текущий маршрут (обычно один) */
  outputs: AudioOutput[]
  /** list: выбрано «как в системе» — поток идёт за устройством по умолчанию */
  followsSystem: boolean
}

/** Вывод звука: приложение для компьютера показывает свой список, iPhone и Safari — системное окно */
export interface AudioRoutes {
  /** `list` — свой список, выбор — `select`; `picker` — системное окно AirPlay и Bluetooth */
  readonly mode: 'list' | 'picker'
  get(): Promise<AudioOutputs>
  /** list: устройство вывода; null — как в системе */
  select(id: string | null): Promise<void>
  /** picker: системное окно выбора */
  showPicker(): Promise<void>
  /** Маршрут сменился: подключили наушники, выбрали колонку */
  onChange(cb: (outputs: AudioOutputs) => void): () => void
}

export interface AudioEngineEvents {
  time: (positionMs: number) => void
  /** Сайт — трек доиграл; приложение — очередь движка доиграла, следующий решает плеер */
  ended: () => void
  state: (s: { playing: boolean, buffering: boolean }) => void
  /** uid — элемент очереди, если сбой у конкретного трека (приложение) */
  error: (error: unknown, uid?: string) => void
  /** Приложение: движок сам перешёл к другому элементу очереди — без пауз, кроссфейдом,
   *  с пульта системы (`user`), пропустив сбойный трек */
  advance: (uid: string, reason: 'auto' | 'user') => void
  /** Приложение: таймер сна в движке поставили, сняли, началось затухание, сработал */
  sleep: (s: SleepState) => void
}

/** Таймер сна: через `ms` или в конце текущего трека */
export type SleepTimer = { mode: 'minutes', ms: number } | { mode: 'track' }

export interface SleepState {
  /** null — таймера нет */
  timer: 'minutes' | 'track' | null
  /** Когда пауза, мс от эпохи Unix */
  endsAtMs: number | null
  fading: boolean
  /** Сработал: музыка на паузе */
  finished: boolean
}

/** Трек очереди для движка приложения: метаданные для «Сейчас играет» и ссылка, если есть */
export interface EngineTrack {
  /** uid элемента очереди плеера */
  uid: string
  trackId: string
  title: string
  artist: string
  album: string | null
  artworkUrl: string | null
  durationMs: number
  /** null — ссылку движок спросит у сервера сам, когда трек понадобится */
  source: TrackSource | null
}

/** Уровень звука ~20 раз в секунду (приложение): волна движется от него вместо анализатора */
export interface AudioLevels {
  rms: number
  peak: number
  /** До 150 Гц, 150–2000 Гц, выше 2 кГц */
  bands: [number, number, number]
}

/** Сайт: два «дека» с наложением, громкость, анализатор для волны. Приложение: движок на Rust */
export interface AudioEngine {
  /** Звук в движке приложения: очередь, переходы и пульт системы — на его стороне */
  readonly native: boolean
  /** Анализатор звука; появляется после первого запуска (нужен жест пользователя). Сайт */
  readonly analyser: AnalyserNode | null
  readonly positionMs: number
  readonly durationMs: number
  readonly playing: boolean
  /** Позиция текущего звука в секундах — для сетки долей волны */
  audioTime(): number
  /** Последний уровень звука (приложение); null — нет */
  levels(): AudioLevels | null
  /** source = null — трек без звука: время идёт, звука нет. `track` — для движка приложения */
  load(source: TrackSource | null, opts: { durationMs: number, startMs?: number, track?: EngineTrack }): void
  play(): Promise<void>
  pause(): void
  seek(ms: number): void
  setVolume(volume: number): void
  setMuted(muted: boolean): void
  /** Следующий трек входит с `startMs` (по умолчанию с начала), текущий уходит за fadeMs (сайт) */
  crossfade(next: TrackSource | null, opts: { durationMs: number, fadeMs: number, startMs?: number }): void
  /** Что играть после текущего (приложение): движок готовит заранее и переходит сам */
  setUpcoming(tracks: EngineTrack[]): void
  /** Переход и выравнивание громкости (приложение) */
  /** `dj` — сведение по долям в движке приложения; на сайте его нет (там «Умный»). `expressiveness` 0…1 */
  configure(opts: { transition: 'gapless' | 'crossfade' | 'dj', fadeMs: number, normalize: boolean, expressiveness?: number }): void
  /** Эквалайзер есть (сайт — WebAudio; без него — нет) */
  readonly eqSupported: boolean
  /** Эквалайзер (utils/eq.ts): приложение запоминает его в оболочке для этого устройства; null — выключить */
  setEq(eq: EqConfig | null): void
  /** Что запомнила оболочка (приложение); сайт — null, настройка в localStorage */
  savedEq(): Promise<EqConfig | null>
  /** Таймер сна в движке приложения: затухание и пауза там же, работает и в фоне. Сайт — ничего:
   *  таймер ведёт интерфейс (stores/sound.ts) через fadeOut */
  setSleep(timer: SleepTimer | null): void
  /** Таймер сна в движке сейчас (приложение); сайт — null */
  sleepState(): Promise<SleepState | null>
  /** Сайт: звук затухает до тишины за `ms` (таймер сна); null — громкость обратно */
  fadeOut(ms: number | null): void
  /** Приложение: перемешивание и повтор — повтор одного трека движок ведёт сам (без паузы, и в
   *  фоне), оба режима видны в Пункте управления. Сайт — нет: повтор ведёт плеер */
  setModes?(shuffle: boolean, repeat: RepeatMode): void
  on<E extends keyof AudioEngineEvents>(event: E, cb: AudioEngineEvents[E]): () => void
}

/** Мини-плеер приложения для компьютера: маленькое окно поверх всех, главное прячется */
export interface MiniPlayerControl {
  open(): Promise<void>
  close(): Promise<void>
  toggle(): Promise<void>
}

/** Что показать системе: экран блокировки, Пункт управления, плашка Windows */
export interface NowPlayingInfo {
  title: string
  artist: string
  album: string | null
  artworkUrl: string | null
  durationMs: number | null
  positionMs: number | null
  playing: boolean
}

/** Команды от медиаклавиш, наушников и системного плеера */
export type MediaCommand =
  | { action: 'play' | 'pause' | 'toggle' | 'next' | 'previous' | 'stop' }
  | { action: 'seek', positionMs: number }
  | { action: 'seekBy', offsetMs: number }
  | { action: 'setShuffle', shuffle: boolean }
  | { action: 'setRepeat', mode: RepeatMode }

/** Повтор: выключен, вся очередь (альбом, плейлист) по кругу, один трек */
export type RepeatMode = 'off' | 'all' | 'one'

export type LiveState = 'idle' | 'connecting' | 'open' | 'closed'
export type ServerEventOf<T extends ServerEventType> = Extract<ServerEvent, { type: T }>

/** Как устроено окно приложения: кто рисует кнопки и где «светофор» macOS */
export interface WindowChrome {
  controls: 'mac-inset' | 'windows' | 'none'
  /** Высота своей строки заголовка, px */
  height: number
  /** Где «светофор» macOS */
  inset: { x: number, y: number, width: number, height: number } | null
  /** Windows 11: под прозрачными местами — системный фон Mica */
  backdrop: 'mica' | 'none'
}

/** Состояние окна приложения: развёрнуто, в фокусе, во весь экран */
export interface WindowState {
  maximized?: boolean
  focused?: boolean
  fullscreen?: boolean
}

export interface AppUpdate {
  version: string
  currentVersion: string
  notes: string | null
}

/** Обновление приложения для компьютера: найденное качается в фоне само, ставится по
 *  «Перезапустить» (на macOS — ещё и при выходе). Смысл состояний — apps/shell/BRIDGE.md, «Обновления» */
export interface AppUpdateState {
  status: 'idle' | 'checking' | 'downloading' | 'ready' | 'installing' | 'failed'
  currentVersion: string
  update: AppUpdate | null
  /** Доля скачанного 0…1, пока качается; null — размер неизвестен */
  progress: number | null
  error: string | null
  installOnQuit: boolean
}

/** Загрузчик музыки в приложении для компьютера (macOS, Windows): пока приложение открыто,
 *  компьютер качает музыку для сервера со своего адреса. Ведёт оболочка — apps/shell/BRIDGE.md */
export interface Downloader {
  state(): Promise<DownloaderSnapshot>
  /** Каждая перемена; ход задания — до 4 раз в секунду */
  onState(cb: (state: DownloaderSnapshot) => void): () => void
  /** «Скачивать на этом Mac»; первое включение заводит токен загрузчика (только администратору) */
  setEnabled(enabled: boolean): Promise<DownloaderSnapshot>
  /** Пауза: текущий трек доделывает, новых заданий не берёт */
  setPaused(paused: boolean): Promise<DownloaderSnapshot>
  /** Сервер не принял токен (отозван в админке) — завести новый */
  recreateToken(): Promise<DownloaderSnapshot>
  /** Журнал загрузчика в системном просмотрщике */
  openLog(): Promise<void>
}

/** Что можно закрепить на устройстве */
export type DownloadKind = 'track' | 'album' | 'playlist' | 'artist' | 'liked'

/** Тактильный отклик (iPhone): удар, выбор, уведомление; на остальных устройствах — ничего */
export type HapticKind = 'light' | 'medium' | 'heavy' | 'soft' | 'rigid' | 'selection' | 'success' | 'warning' | 'error'

/** Настройки загрузок этого устройства */
export interface OfflineSettings {
  limitBytes: number
  /** Качать только по Wi-Fi (на компьютере не влияет) */
  wifiOnly: boolean
  /** Умный запас: набор волны с сервера докачивается сам */
  reserveEnabled: boolean
  /** «Без сети»: только скачанное, на сервер приложение не ходит */
  forceOffline: boolean
}

/** Закреплённое и сколько из него уже на устройстве */
export interface OfflinePin {
  kind: DownloadKind
  refId: string | null
  tracksTotal: number
  tracksReady: number
  bytes: number
}

/** Загрузки и офлайн приложения — apps/shell/BRIDGE.md, «Умный офлайн» */
export interface OfflineStatus {
  online: boolean
  network: 'wifi' | 'cellular' | 'none' | 'unknown'
  /** Медиатека в локальной базе: экраны читают её первой */
  synced: boolean
  syncedAt: string | null
  syncing: boolean
  settings: OfflineSettings
  usage: { pinnedBytes: number, reserveBytes: number, cacheBytes: number, totalBytes: number, files: number }
  downloads: { active: number, queued: number, failed: number, overLimit: number, waitingForWifi: boolean }
  pins: OfflinePin[]
  reserve: { tracks: number, ready: number, builtAt: string | null }
}

/** Загрузка трека на устройство */
export interface TrackDownload {
  trackId: string
  state: 'queued' | 'downloading' | 'ready' | 'failed'
  received: number
  total: number
}

/** Умный офлайн приложения: закреплённое, умный запас, место. На сайте — null */
export interface OfflineBridge {
  status(): Promise<OfflineStatus>
  onStatus(cb: (status: OfflineStatus) => void): () => void
  setSettings(patch: Partial<OfflineSettings>): Promise<OfflineStatus>
  /** Синхронизировать локальную базу сейчас */
  sync(): Promise<void>
  /** Медиатека в локальной базе изменилась — перечитать списки */
  onSynced(cb: () => void): () => void
  pin(kind: DownloadKind, refId: string | null): Promise<OfflineStatus>
  unpin(kind: DownloadKind, refId: string | null): Promise<OfflineStatus>
  /** «Удалить всё»: файлы, закреплённое; умный запас выключается */
  clear(): Promise<OfflineStatus>
  /** Все треки на устройстве и ход их загрузки */
  downloads(): Promise<TrackDownload[]>
  onProgress(cb: (d: TrackDownload) => void): () => void
  /** Состав умного запаса: все треки в порядке загрузки и уже скачанные из них */
  reserve(): Promise<OfflineReserve>
}

/** Умный запас волны на устройстве */
export interface OfflineReserve {
  trackIds: string[]
  readyIds: string[]
  builtAt: string | null
}

/** Что узнал ShazamKit: название, исполнитель и ISRC — трек каталога по ним находит сервер */
export interface RecognizeMatch {
  title: string
  artist: string
  isrc: string | null
  appleMusicId: string | null
  shazamId: string | null
  artworkUrl: string | null
}

/** Почему не узнали: тишина или шум, нет доступа к микрофону, микрофона нет, отменили, сбой Shazam */
export type RecognizeErrorCode = 'no_match' | 'permission' | 'no_microphone' | 'cancelled' | 'shazam' | 'unsupported'

/** «Распознать»: ShazamKit в приложениях для iPhone, iPad и macOS. Своя музыка на паузе, пока слушаем */
export interface Recognizer {
  /** Слушать до совпадения, не дольше 15 с; ошибка — `{ code: RecognizeErrorCode }` */
  listen(): Promise<RecognizeMatch>
  cancel(): Promise<void>
  /** Открыть экран распознавания просят трей macOS или быстрое действие на значке iPhone
   *  (и при запуске, если приложение открыли этим действием) */
  onOpen(cb: () => void): () => void
}

/** «Слушать вместе» глазами гостя: свой токен `dwj_…` — только эта сессия */
export interface JamGuestClient {
  view(): Promise<JamGuestView>
  /** Треки каталога (до 20) */
  search(q: string): Promise<JamTrack[]>
  /** В очередь хозяина (или на одобрение) */
  add(trackId: string): Promise<JamRequest>
  react(): Promise<void>
  skip(): Promise<JamSkip>
  /** «Слушать у себя»: ссылка на аудио текущего или следующего трека */
  stream(trackId: string): Promise<JamStream>
  leave(): Promise<void>
  /** `jam.now`, `jam.changed`, `jam.reaction` */
  live: Pick<Bridge['live'], 'connect' | 'disconnect' | 'on' | 'onReconnect'>
}

/** «Слушать вместе», как Jam у Spotify: сессия хозяина, гости по ссылке без аккаунта */
export interface JamBridge {
  get(): Promise<Jam | null>
  start(): Promise<Jam>
  update(patch: JamPatch): Promise<Jam>
  end(): Promise<void>
  removeGuest(id: string): Promise<void>
  approve(requestId: string): Promise<Jam>
  decline(requestId: string): Promise<Jam>
  /** Приглашение по коду из ссылки (у вошедшего — его имя) */
  invite(code: string): Promise<JamInvite>
  /** Войти гостем: имя (у вошедшего можно не указывать) → токен */
  join(code: string, name: string | null): Promise<JamJoined>
  guest(token: string): JamGuestClient
}

export interface Bridge {
  readonly kind: 'web' | 'tauri'
  readonly platform: Platform
  /** Приложение с собственным окном: нужен свой заголовок окна */
  readonly isDesktopApp: boolean
  readonly chrome: WindowChrome | null
  /** Определить, что умеет сервер (в разработке — какие разделы ещё мок) */
  ready(): Promise<void>
  /** Раздел сейчас отдаётся моком (только в разработке) */
  mocked(section: Section): boolean
  /** Адрес картинки или файла с сервера: на сайте — как есть, в приложении — с адресом сервера */
  resolveUrl(url: string): string

  window: {
    minimize(): Promise<void>
    /** true — окно развёрнуто */
    toggleMaximize(): Promise<boolean>
    /** Приложение: окно прячется в трей, музыка играет дальше */
    close(): Promise<void>
    /** Развёрнуто ли, в фокусе ли, во весь экран ли */
    onState(cb: (state: WindowState) => void): () => void
    /** Windows 11: меню привязки окон над своей кнопкой «развернуть» */
    trackSnap(button: HTMLElement): () => void
    /** Тема окна и системных частей вокруг страницы («светофор», Mica, строка состояния, фон
     * под WebView); null — как в системе. На сайте ничего не делает */
    setTheme(theme: 'dark' | 'light' | null): void
  }

  /** Обновления приложения (на сайте и телефонах — ничего: `state()` отдаёт null) */
  updates: {
    onAvailable(cb: (update: AppUpdate) => void): () => void
    state(): Promise<AppUpdateState | null>
    /** Каждая перемена: проверка, ход скачивания, «готово» */
    onState(cb: (state: AppUpdateState) => void): () => void
    /** Проверить сейчас; найденное скачается в фоне */
    check(): Promise<void>
    /** «Перезапустить»: поставить скачанное и перезапустить приложение */
    install(): Promise<void>
  }

  /** Язык меню в трее и строке меню приложения */
  setLocale(locale: 'ru' | 'en'): void

  /** Загрузчик на этом компьютере — приложение macOS и Windows; на сайте и телефонах — null */
  readonly downloader: Downloader | null

  /** Куда идёт звук (AirPlay, Bluetooth, динамики); null — платформа не умеет, строки нет */
  readonly outputs: AudioRoutes | null

  /** Загрузки на устройство и медиатека без сети (приложение); на сайте — null */
  readonly offline: OfflineBridge | null

  /** Тактильный отклик (iPhone); на остальных — ничего */
  haptic(kind: HapticKind): void

  /** Push приложения на iPhone и iPad (APNs); на сайте и компьютерах — null (сайт — Web Push сам) */
  readonly appPush: AppPush | null

  /** Системное уведомление, пока приложение открыто (macOS, Windows); на остальных — null */
  readonly systemNotify: SystemNotify | null

  auth: {
    providers(): Promise<AuthProviders>
    /** null — сессии нет */
    me(): Promise<Me | null>
    /** Вход через daly.rest: на сайте — переход на страницу провайдера, в приложении — системный браузер */
    login(next: string): Promise<Me | null>
    /** Прервать незаконченный вход в браузере (приложение) */
    cancelLogin(): Promise<void>
    /** Аварийный вход паролем — только на сайте: он открывает cookie-сессию */
    readonly passwordAllowed: boolean
    passwordLogin(password: string): Promise<Me>
    logout(): Promise<void>
    /** Адрес начала входа через daly.rest (сайт) */
    loginUrl(next: string): string
    /** Сессия кончилась не по нашей кнопке: сервер ответил 401 или вышли в другом окне */
    onSignedOut(cb: () => void): () => void
  }

  catalog: {
    search(q: string, scope: SearchScope, limit?: number): Promise<SearchResults>
    artist(id: string): Promise<ArtistDetail>
    album(id: string): Promise<AlbumDetail>
    track(id: string): Promise<Track>
    /** Треки по id, до 500 за раз; неизвестные пропускаются */
    lookup(ids: string[]): Promise<Track[]>
    /** Поиск словами по звучанию: «спокойный инструментал для работы» — из медиатеки и индекса каталога */
    soundSearch(text: string, limit?: number): Promise<SoundSearchResults>
    /** Текст песни: строки с временем, простой текст, «без слов» или «не нашли» */
    lyrics(trackId: string): Promise<TrackLyrics>
    /** Песни, в тексте которых есть строчка (среди уже взятых текстов) */
    searchLyrics(q: string, limit?: number): Promise<LyricsSearchResults>
    /** Версии той же песни, выпущенные исполнителем: оригинал, slowed, super slowed, sped up */
    versions(trackId: string): Promise<TrackVersions>
    /** Запомнить выбранную версию для основного исполнителя трека; `original` — забыть */
    chooseVersion(trackId: string, kind: VersionKind): Promise<void>
    /** Исполнители, у которых выбрана не оригинальная версия */
    versionChoices(): Promise<VersionChoice[]>
    /** Похожие по звучанию на трек (не больше двух у артиста) — «Подходит к плейлисту» */
    similar(trackId: string, limit?: number): Promise<WaveItems>
    /** «Умное перемешивание»: похожее на весь список (не из него и не из `exclude`) — подпись «Подобрано» */
    suggest(trackIds: string[], exclude?: string[], limit?: number): Promise<WaveItems>
    /** «Поделиться → DalyWave»: ссылка Deezer, Spotify, Apple Music, YouTube, SoundCloud, Bandcamp → трек,
     *  альбом или артист каталога. Ошибки: `unsupported_link` (422), `not_found` (404) */
    resolveLink(url: string): Promise<ResolvedLink>
  }

  home(): Promise<Home>

  /** «Мой вкус»: грани, статистика и ручки управления рекомендациями */
  taste: {
    overview(): Promise<TasteOverview>
    /** «Меньше такого» для грани; повтор — сильнее */
    less(facetId: string): Promise<TasteDamper>
    /** Отменить «меньше такого» */
    undoLess(damperId: string): Promise<void>
    /** Сбросить вкус: грани и миксы собираются заново по новым прослушиваниям */
    reset(): Promise<void>
  }

  /** «Плейлист по описанию»: словами — предпросмотр, «ещё в том же духе», сохранение обычным плейлистом */
  describe: {
    /** Собрать по описанию; `exclude` — треки прошлой сборки («Пересобрать») */
    preview(text: string, exclude?: string[]): Promise<DescribePreview>
    /** Ещё треки в том же духе: без показанных, начало — к последнему */
    more(brief: DescribeBrief, exclude: string[], afterTrackId: string | null, count?: number): Promise<WaveItems>
    save(body: SaveDescribed): Promise<PlaylistSummary>
    /** Понятое у сохранённого плейлиста; собран не по описанию — `null` */
    get(playlistId: string): Promise<DescribedPlaylist | null>
    setWeekly(playlistId: string, weekly: boolean): Promise<DescribedPlaylist>
    /** «Ещё в том же духе» в конец сохранённого; id добавленных треков */
    extend(playlistId: string, count?: number): Promise<string[]>
  }

  /** «Итоги года»: годы с прослушиваниями, итоги года («по сей день» — у текущего), плейлист лучших треков */
  rewind: {
    years(): Promise<RewindYears>
    get(year: number): Promise<Rewind>
    /** «Твои лучшие треки {год}»: создать или обновить; id плейлиста */
    savePlaylist(year: number): Promise<string>
    /** Музыка историй: трек и место начала у слайдов; чего нет на сервере, сервер ставит на скачивание */
    soundtrack(year: number): Promise<RewindSoundtrack>
    /** «Ещё послушать»: новое, похожее на любимое за год */
    more(year: number): Promise<RewindMore>
    /** Сохранить показанную подборку плейлистом «Ещё послушать {год}»; id плейлиста */
    saveMore(year: number, trackIds: string[]): Promise<string>
    /** Истории открывали: карточка «Твои итоги готовы» больше не зовёт */
    seen(year: number): Promise<void>
  }

  library: {
    overview(): Promise<LibraryOverview>
    liked(cursor?: string | null, limit?: number): Promise<LikedTrackPage>
    savedAlbums(cursor?: string | null): Promise<SavedAlbumPage>
    followed(cursor?: string | null): Promise<FollowedArtistPage>
    like(trackId: string): Promise<void>
    unlike(trackId: string): Promise<void>
    saveAlbum(albumId: string): Promise<void>
    unsaveAlbum(albumId: string): Promise<void>
    follow(artistId: string, request?: FollowRequest): Promise<void>
    unfollow(artistId: string): Promise<void>
    blocked(): Promise<BlockedList>
    block(kind: BlockedKind, id: string): Promise<void>
    unblock(kind: BlockedKind, id: string): Promise<void>
    /** «Недавно прослушанное», новые первыми: `q` — поиск по треку, артисту и альбому, `before` — переход к дате */
    history(cursor?: string | null, opts?: { q?: string, before?: string, limit?: number }): Promise<HistoryPage>
    /** Дни с прослушиваниями по часовому поясу устройства — для календаря */
    historyDays(): Promise<HistoryDays>
    /** Убрать прослушивание из истории */
    hidePlay(id: string): Promise<void>
    /** Очистить историю: вкус и миксы её по-прежнему учитывают */
    clearHistory(): Promise<void>
    /** «Неверная запись»: спорная запись в админке и новое скачивание без этого видео */
    reportRecording(trackId: string, report: WrongRecordingReport): Promise<WrongRecordingResult>
  }

  /** Перенос музыки: архив Spotify, списки и свои файлы. Сопоставление идёт на сервере в фоне */
  imports: {
    list(): Promise<ImportList>
    get(id: string): Promise<ImportSummary>
    items(id: string, status?: ImportItemStatus | null, cursor?: string | null, limit?: number): Promise<ImportItemPage>
    /** Выбрать трек для позиции (наш id или кандидат Deezer) или пропустить её */
    fix(id: string, itemId: string, fix: ImportItemFix): Promise<ImportItem>
    /** Идущий импорт — остановить, законченный — убрать из истории (медиатека не меняется) */
    remove(id: string): Promise<void>
    /** Архив `my_spotify_data.zip` целиком; `onProgress` — доля отправленного 0..1 */
    spotify(file: File, onProgress?: (share: number) => void, signal?: AbortSignal): Promise<ImportSummary>
    /** Список «Артист — Трек», CSV или M3U — в новый плейлист или в «Мне нравится» */
    fromList(body: ListImportRequest): Promise<ImportSummary>
    /** «Яндекс Музыка»: плейлист по ссылке; с ключом входа без ссылки — лайки и плейлисты аккаунта.
     *  Ключ сервер использует один раз и не хранит */
    yandex(body: YandexImportRequest): Promise<ImportSummary>
    /** Плейлист YouTube / YouTube Music по ссылке */
    youtube(url: string): Promise<ImportSummary>
    /** Архив Google Takeout «YouTube и YouTube Music» */
    takeout(file: File, onProgress?: (share: number) => void, signal?: AbortSignal): Promise<ImportSummary>
    /** Новый импорт файлов: дальше `file` по одному, затем `finish` */
    startFiles(title?: string): Promise<ImportSummary>
    file(id: string, file: File, onProgress?: (share: number) => void, signal?: AbortSignal): Promise<ImportItem>
    finish(id: string): Promise<ImportSummary>
  }

  playlists: {
    list(): Promise<PlaylistList>
    get(id: string): Promise<PlaylistDetail>
    create(body: NewPlaylist): Promise<PlaylistSummary>
    update(id: string, patch: PlaylistPatch): Promise<PlaylistSummary>
    remove(id: string): Promise<void>
    /** Новые позиции — их id нужны для «Отменить» */
    addItems(id: string, body: AddPlaylistItems): Promise<AddedPlaylistItems>
    removeItem(id: string, itemId: string): Promise<void>
    /** Несколько позиций разом (выбранные в списке) */
    removeItems(id: string, itemIds: string[]): Promise<void>
    moveItem(id: string, itemId: string, afterItemId: string | null): Promise<void>
    /** «Собрать как сет»: новый порядок, плейлист не меняется (у микса — тоже) */
    setPreview(id: string, opts: SetOptions): Promise<SetPreview>
    /** Применить сет одной операцией; в ответе — прежний порядок для «Отменить» */
    applySet(id: string, opts: SetOptions): Promise<SetApplied>
    /** Порядок целиком: названные позиции — так, остальные — за ними */
    setOrder(id: string, itemIds: string[]): Promise<void>
    pin(id: string, pinned: boolean): Promise<void>
    /** Своя обложка: картинка обрезается по центру до квадрата */
    setCover(id: string, file: Blob): Promise<PlaylistSummary>
    /** Убрать свою обложку — снова мозаика */
    removeCover(id: string): Promise<PlaylistSummary>
    /** Копия плейлиста или микса — обычный плейлист человека */
    duplicate(id: string, title?: string): Promise<PlaylistSummary>
    /** Переложить плейлист в папку (или из неё, `folderId: null`) и на место среди соседей */
    move(id: string, body: MovePlaylist): Promise<PlaylistList>
    /** Папки: новая встаёт первой, названные плейлисты сразу в ней */
    createFolder(body: NewPlaylistFolder): Promise<PlaylistList>
    updateFolder(id: string, patch: PlaylistFolderPatch): Promise<PlaylistFolder>
    /** Плейлисты удалённой папки встают на её место */
    removeFolder(id: string): Promise<PlaylistList>
    moveFolder(id: string, body: MovePlaylistFolder): Promise<PlaylistList>
    /** Совместный: участники добавляют, убирают и переставляют треки; выключить — участники уходят */
    setCollaborative(id: string, on: boolean): Promise<PlaylistSummary>
    /** Видят ли друзья плейлист на странице владельца */
    setVisibility(id: string, friendsCanSee: boolean): Promise<PlaylistSummary>
    /** Ссылка-приглашение в совместный; `reset` — новая, прежняя перестаёт работать */
    invite(id: string, reset?: boolean): Promise<InviteLink>
    /** Позвать друзей (плейлист становится совместным) */
    addMembers(id: string, userIds: string[]): Promise<PlaylistSummary>
    /** Убрать участника; свой id — выйти из плейлиста */
    removeMember(id: string, userId: string): Promise<void>
    /** Куда зовёт ссылка: плейлист, владелец, вошёл ли уже */
    joinPreview(code: string): Promise<PlaylistInvite>
    join(code: string): Promise<PlaylistSummary>
  }

  /** Друзья: запросы, ссылка, что слушают, страница друга, приватность */
  friends: {
    list(): Promise<FriendList>
    /** Люди сервиса по имени (пусто — все) */
    people(q?: string): Promise<PeopleList>
    request(userId: string): Promise<FriendRequestResult>
    cancel(userId: string): Promise<void>
    accept(userId: string): Promise<FriendRequestResult>
    decline(userId: string): Promise<void>
    remove(userId: string): Promise<void>
    profile(userId: string): Promise<FriendProfile>
    /** Своя ссылка «Добавить в друзья»; `reset` — новая */
    invite(reset?: boolean): Promise<InviteLink>
    invitePreview(code: string): Promise<PersonCard>
    useInvite(code: string): Promise<PersonCard>
    privacy(): Promise<Privacy>
    updatePrivacy(patch: PrivacyPatch): Promise<Privacy>
  }

  /** Уведомления: центр, настройки по видам и тихие часы, подписки устройств на push */
  notifications: {
    /** Новые первыми; `before` — курсор следующей страницы */
    list(before?: string | null, limit?: number): Promise<NotificationPage>
    /** Отметить прочитанными; пустой список — все */
    read(ids: string[]): Promise<NotificationsRead>
    settings(): Promise<NotificationSettings>
    updateSettings(patch: NotificationSettingsPatch): Promise<NotificationSettings>
    /** Какие каналы push есть на сервере и какие устройства подписаны */
    push(): Promise<PushInfo>
    subscribe(reg: PushRegistration): Promise<PushRegistered>
    unsubscribe(body: PushUnregister): Promise<void>
    /** Пробное уведомление на все подписанные устройства */
    test(): Promise<PushTest>
  }

  /** Настройки слушателя — общие для всех его устройств */
  settings: {
    get(): Promise<UserSettings>
    update(patch: UserSettingsPatch): Promise<UserSettings>
  }

  /** «Моя волна»: сессия на сервере помнит выданное и реакции, её id — и в событиях прослушивания */
  wave: {
    /** Новая сессия под настройки (они у сессии не меняются: другие настройки — новая сессия)
     *  с подобранным началом; `seed` — радио от трека, артиста, альбома, плейлиста или описания словами */
    start(settings: WaveSettings, seed?: WaveSeed): Promise<WaveStart>
    /** Ход подготовки начала: готово ли и сколько из нужного уже на сервере */
    session(id: string): Promise<WaveStart>
    /** Следующие треки с подписью «почему этот трек»; `exclude` — сыгранное до смены настроек */
    next(session: string, settings: WaveSettings, count: number, exclude: string[]): Promise<WaveTrack[]>
    /** Реакция на трек волны: лайк, «не нравится», пропуск, «не сейчас», «больше такого», дослушан */
    feedback(session: string, trackId: string, kind: WaveFeedbackKind, positionMs: number): Promise<void>
    /** Сохранённые сочетания настроек: карточки под волной, по порядку */
    presets: {
      list(): Promise<WavePreset[]>
      /** Новое — в конец ряда */
      create(body: NewWavePreset): Promise<WavePreset>
      update(id: string, patch: WavePresetPatch): Promise<WavePreset>
      remove(id: string): Promise<void>
      /** Названные — первыми в этом порядке, остальные — после */
      reorder(ids: string[]): Promise<WavePreset[]>
    }
  }

  media: {
    /** `network` — сеть устройства: качество по настройкам «По Wi-Fi» / «По сотовой» (сайт — Wi-Fi) */
    source(track: Track, network?: NetworkKind): Promise<SourceResult>
    waveform(track: Track, bars: number): Promise<number[]>
    /** Сказать системе, что играет (экран блокировки, медиаклавиши) */
    nowPlaying(info: NowPlayingInfo | null): void
    /** Команды от медиаклавиш и системного плеера */
    onCommand(cb: (cmd: MediaCommand) => void): () => void
  }

  events: {
    play(event: Omit<PlayEvent, 'clientEventId'>): void
    feedback(event: Omit<FeedbackEvent, 'clientEventId' | 'at'>): void
    flush(): Promise<void>
  }

  /** Живые события сервера: загрузки, изменения медиатеки, задачи */
  live: {
    readonly state: LiveState
    connect(): void
    disconnect(): void
    on<T extends ServerEventType>(type: T, cb: (event: ServerEventOf<T>) => void): () => void
    /** Связь восстановилась после обрыва — данные могли устареть */
    onReconnect(cb: () => void): () => void
    /** Сообщение серверу (управление между устройствами); false — соединения сейчас нет */
    send(msg: ClientMessage): boolean
  }

  devices(): Promise<PlaybackDevice[]>

  /** «Слушать вместе» */
  jam: JamBridge

  admin: {
    health(): Promise<Health>
    overview(): Promise<AdminOverview>
    users(): Promise<UserList>
    updateUser(id: string, patch: UserPatch): Promise<void>
    sessions(userId?: string): Promise<SessionList>
    revokeSession(id: string): Promise<void>
    audit(cursor?: string | null, limit?: number, filter?: { kind?: string, actor?: string }): Promise<AuditPage>
    jobs(query?: { lane?: string, status?: JobStatus, kind?: string, cursor?: string | null, limit?: number }): Promise<JobPage>
    retryJob(id: string): Promise<void>
    cancelJob(id: string): Promise<void>
    lanes(): Promise<LaneList>
    updateLane(lane: string, patch: LanePatch): Promise<void>
    acquisitions(query?: { status?: AcquisitionStatus, active?: boolean, cursor?: string | null, limit?: number }): Promise<AdminAcquisitionPage>
    acquisition(id: string): Promise<AdminAcquisition>
    resolve(id: string, req: ResolveRequest): Promise<AdminAcquisition>
    preview(trackId: string): Promise<TrackPreview>
    sources(): Promise<Sources>
    checkDownload(trackId?: string): Promise<AdminAcquisition>
    createDownloader(name: string): Promise<DownloaderToken>
    revokeDownloader(id: string): Promise<void>
    recordings(query?: { status?: AudioStatus, source?: string, verified?: boolean, q?: string, cursor?: string | null, limit?: number }): Promise<AdminRecordingPage>
    reacquire(recordingId: string, url?: string): Promise<AdminAcquisition>
    storage(): Promise<StorageOverview>
    imports(): Promise<AdminImportList>
    settings(): Promise<ServerSettings>
    updateSettings(patch: ServerSettingsPatch): Promise<ServerSettings>
    /** «Итоги года» как событие: стиль, режим, что открыто сейчас */
    rewindEvent(): Promise<RewindEvent>
    updateRewindEvent(patch: RewindEventPatch): Promise<RewindEvent>
    /** Вкус слушателя (по умолчанию — свой) и общий индекс каталога */
    reco(userId?: string | null): Promise<RecoOverview>
    updateReco(patch: RecoSettingsPatch): Promise<RecoSettings>
    /** `taste` с `userId` — пересчёт только этого слушателя */
    runReco(task: RecoTask, userId?: string | null): Promise<RecoRunResult>
  }

  audio: AudioEngine
  /** Мини-плеер (приложение на macOS и Windows); иначе нет */
  miniPlayer?: MiniPlayerControl | null

  /** Распознавание песни рядом (приложение на iPhone, iPad, macOS); на сайте и Windows — нет */
  recognizer?: Recognizer | null
  /** Распознанное: трек каталога по совпадению и история — на всех устройствах */
  recognize: {
    resolve(match: RecognizeRequest): Promise<Recognition>
    history(cursor?: string | null, limit?: number): Promise<RecognitionPage>
    remove(id: string): Promise<void>
  }
}

export type { DeviceInfo, Track }

/** Разрешение на уведомления в системе */
export type PushPermission = 'default' | 'granted' | 'denied'

/** Push приложения на iPhone и iPad (apps/shell, APNs). Разрешение спрашивается только по `register(true)` */
export interface AppPush {
  state(): Promise<{ supported: boolean, permission: PushPermission }>
  /** Токен устройства для сервера; `prompt` — спросить разрешение, если система ещё не спрашивала.
   *  null — запрещено, не вышло или не дождались */
  register(prompt: boolean): Promise<{ token: string, environment: 'sandbox' | 'production' } | null>
  /** Число на значке приложения */
  setBadge(count: number): void
}

/** Системное уведомление приложения для компьютера, пока оно открыто; щелчок открывает `link` */
export interface SystemNotify {
  show(n: { id: string, title: string, body: string, link: string | null }): Promise<boolean>
}
