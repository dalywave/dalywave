// «Мост» — единственный путь экранов к данным и звуку. На сайте за ним HTTP и HTML-аудио,
// в приложениях — оболочка Tauri (окно, вход через системный браузер) и аудиодвижок на Rust,
// который сам ведёт «Сейчас играет» и пульт системы. Экраны пишутся один раз.
import type {
  AddPlaylistItems, AdminOverview, AlbumDetail, ArtistDetail, AuditPage, AuthProviders, BlockedKind, BlockedList,
  DeviceInfo, FeedbackEvent, FollowRequest, FollowedArtistPage, Health, HistoryPage, Home, JobPage, JobStatus,
  LaneList, LanePatch, LibraryOverview, LikedTrackPage, Me, NewPlaylist, PlayEvent, PlaylistDetail, PlaylistList,
  PlaylistPatch, PlaylistSummary, SavedAlbumPage, SearchResults, SearchScope, ServerEvent, ServerEventType,
  SessionList, Track, UserList, UserPatch,
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

/** Устройство для «Где играть». Список с сервера появится на этапе управления между устройствами */
export interface PlaybackDevice {
  id: string
  name: string
  kind: 'laptop' | 'desktop' | 'phone' | 'tablet' | 'web'
  current: boolean
  online: boolean
  lastSeenAt: string | null
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
  /** Следующий трек входит, текущий уходит за fadeMs (сайт) */
  crossfade(next: TrackSource | null, opts: { durationMs: number, fadeMs: number }): void
  /** Что играть после текущего (приложение): движок готовит заранее и переходит сам */
  setUpcoming(tracks: EngineTrack[]): void
  /** Переход и выравнивание громкости (приложение) */
  configure(opts: { transition: 'gapless' | 'crossfade', fadeMs: number, normalize: boolean }): void
  on<E extends keyof AudioEngineEvents>(event: E, cb: AudioEngineEvents[E]): () => void
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
  }

  /** Обновления приложения (на сайте — ничего) */
  updates: {
    onAvailable(cb: (update: AppUpdate) => void): () => void
    install(): Promise<void>
  }

  /** Язык меню в трее и строке меню приложения */
  setLocale(locale: 'ru' | 'en'): void

  /** Загрузчик на этом компьютере — приложение macOS и Windows; на сайте и телефонах — null */
  readonly downloader: Downloader | null

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
  }

  home(): Promise<Home>

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
    history(cursor?: string | null): Promise<HistoryPage>
  }

  playlists: {
    list(): Promise<PlaylistList>
    get(id: string): Promise<PlaylistDetail>
    create(body: NewPlaylist): Promise<PlaylistSummary>
    update(id: string, patch: PlaylistPatch): Promise<PlaylistSummary>
    remove(id: string): Promise<void>
    addItems(id: string, body: AddPlaylistItems): Promise<void>
    removeItem(id: string, itemId: string): Promise<void>
    moveItem(id: string, itemId: string, afterItemId: string | null): Promise<void>
    pin(id: string, pinned: boolean): Promise<void>
  }

  wave: {
    next(settings: WaveSettings, count: number, exclude: string[]): Promise<WaveTrack[]>
  }

  media: {
    source(track: Track): Promise<SourceResult>
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
  }

  devices(): Promise<PlaybackDevice[]>

  admin: {
    health(): Promise<Health>
    overview(): Promise<AdminOverview>
    users(): Promise<UserList>
    updateUser(id: string, patch: UserPatch): Promise<void>
    sessions(userId?: string): Promise<SessionList>
    revokeSession(id: string): Promise<void>
    audit(cursor?: string | null, limit?: number): Promise<AuditPage>
    jobs(query?: { lane?: string, status?: JobStatus, kind?: string, cursor?: string | null, limit?: number }): Promise<JobPage>
    retryJob(id: string): Promise<void>
    cancelJob(id: string): Promise<void>
    lanes(): Promise<LaneList>
    updateLane(lane: string, patch: LanePatch): Promise<void>
  }

  audio: AudioEngine
}

export type { DeviceInfo, Track }
