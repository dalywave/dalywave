// Имена типов API для экранов. Источник правды — OpenAPI сервера: `bun run api-types`
// пересобирает schema.d.ts, отсюда экраны берут понятные имена.
import type { components, paths } from './schema'

type Schemas = components['schemas']

export type ApiPaths = paths

// Вход и пользователи
export type Me = Schemas['Me']
export type User = Schemas['User']
export type Role = Schemas['Role']
export type Permission = Schemas['Permission']
export type AuthProviders = Schemas['AuthProviders']
export type DeviceInfo = Schemas['Device']

// Каталог
export type AudioStatus = Schemas['AudioStatus']
export type RecordType = Schemas['RecordType']
export type ImageSet = Schemas['ImageSet']
export type ArtistRef = Schemas['ArtistRef']
export type TrackArtist = Schemas['TrackArtist']
export type AlbumRef = Schemas['AlbumRef']
export type Track = Schemas['Track']
export type ArtistSummary = Schemas['ArtistSummary']
export type AlbumSummary = Schemas['AlbumSummary']
export type AlbumDetail = Schemas['AlbumDetail']
export type Releases = Schemas['Releases']
export type ArtistDetail = Schemas['ArtistDetail']
export type SearchScope = Schemas['SearchScope']
export type SearchResults = Schemas['SearchResults']
export type TrackList = Schemas['TrackList']

// Тексты песен
export type LyricsKind = Schemas['LyricsKind']
export type LyricsLine = Schemas['LyricsLine']
export type TrackLyrics = Schemas['TrackLyrics']
export type LyricsHit = Schemas['LyricsHit']
export type LyricsSearchResults = Schemas['LyricsSearchResults']

// Главная
export type ShelfKind = Schemas['ShelfKind']
export type ShelfItem = Schemas['ShelfItem']
export type Shelf = Schemas['Shelf']
export type Home = Schemas['Home']

// «Мой вкус»
export type TasteOverview = Schemas['TasteOverview']
export type TasteFacet = Schemas['TasteFacet']
export type TasteDamper = Schemas['TasteDamper']
export type ListeningStats = Schemas['ListeningStats']
export type TasteKnowledge = Schemas['TasteKnowledge']
export type TopArtist = Schemas['TopArtist']
export type TopGenre = Schemas['TopGenre']

// Итоги года
export type Rewind = Schemas['Rewind']
export type RewindYears = Schemas['RewindYears']
export type RewindTrack = Schemas['RewindTrack']
export type RewindArtist = Schemas['RewindArtist']
export type RewindAlbum = Schemas['RewindAlbum']
export type RewindCue = Schemas['RewindCue']
export type RewindSoundtrack = Schemas['RewindSoundtrack']
export type RewindMore = Schemas['RewindMore']
export type RewindStyle = Schemas['RewindStyle']
export type RewindStyleSetting = Schemas['RewindStyleSetting']
export type RewindMode = Schemas['RewindMode']
export type RewindEvent = Schemas['RewindEvent']
export type RewindEventPatch = Schemas['RewindEventPatch']

// Медиатека
export type DownloadPolicy = Schemas['DownloadPolicy']
export type FollowState = Schemas['FollowState']
export type FollowRequest = Schemas['FollowRequest']
export type FollowedArtist = Schemas['FollowedArtist']
export type FollowedArtistPage = Schemas['FollowedArtistPage']
export type LikedTrack = Schemas['LikedTrack']
export type LikedTrackPage = Schemas['LikedTrackPage']
export type SavedAlbum = Schemas['SavedAlbum']
export type SavedAlbumPage = Schemas['SavedAlbumPage']
export type LibraryCounts = Schemas['LibraryCounts']
export type LibraryOverview = Schemas['LibraryOverview']
export type BlockedKind = Schemas['BlockedKind']
export type BlockedItem = Schemas['BlockedItem']
export type BlockedList = Schemas['BlockedList']
export type HistoryItem = Schemas['HistoryItem']
export type HistoryPage = Schemas['HistoryPage']
export type HistoryDays = Schemas['HistoryDays']
export type HistoryDay = Schemas['HistoryDay']
export type WrongRecordingReason = Schemas['WrongRecordingReason']
export type WrongRecordingReport = Schemas['WrongRecordingReport']
export type WrongRecordingResult = Schemas['WrongRecordingResult']

// Плейлисты
export type PlaylistKind = Schemas['PlaylistKind']
export type PlaylistSummary = Schemas['PlaylistSummary']
export type PlaylistItem = Schemas['PlaylistItem']
export type PlaylistDetail = Schemas['PlaylistDetail']
export type PlaylistList = Schemas['PlaylistList']
export type NewPlaylist = Schemas['NewPlaylist']
export type PlaylistPatch = Schemas['PlaylistPatch']
export type AddPlaylistItems = Schemas['AddPlaylistItems']
export type AddedPlaylistItems = Schemas['AddedPlaylistItems']
export type PlaylistFolder = Schemas['PlaylistFolder']
export type NewPlaylistFolder = Schemas['NewPlaylistFolder']
export type PlaylistFolderPatch = Schemas['PlaylistFolderPatch']
export type MovePlaylist = Schemas['MovePlaylist']
export type MovePlaylistFolder = Schemas['MovePlaylistFolder']
// «Собрать как сет»
export type SetCurve = Schemas['SetCurve']
export type SetFit = Schemas['SetFit']
export type SetOptions = Schemas['SetOptions']
export type SetEntry = Schemas['SetEntry']
export type SetPreview = Schemas['SetPreview']
export type SetApplied = Schemas['SetApplied']

// Настройки слушателя (общие для устройств): фильтр нецензурного, качество, порядок плейлистов
export type UserSettings = Schemas['UserSettings']
export type UserSettingsPatch = Schemas['UserSettingsPatch']
export type WifiQuality = Schemas['WifiQuality']
export type CellularQuality = Schemas['CellularQuality']
export type DownloadQuality = Schemas['DownloadQuality']
export type PlaylistSort = Schemas['PlaylistSort']
export type StreamQuality = Schemas['StreamQuality']
export type NetworkKind = Schemas['NetworkKind']

// Импорт: архив Spotify, списки, свои файлы
export type ImportKind = Schemas['ImportKind']
export type ImportStatus = Schemas['ImportStatus']
export type ImportPhase = Schemas['ImportPhase']
export type ImportTarget = Schemas['ImportTarget']
export type ImportStats = Schemas['ImportStats']
export type ImportSummary = Schemas['ImportSummary']
export type ImportList = Schemas['ImportList']
export type ImportItem = Schemas['ImportItem']
export type ImportItemKind = Schemas['ImportItemKind']
export type ImportItemStatus = Schemas['ImportItemStatus']
export type ImportItemPage = Schemas['ImportItemPage']
export type ImportItemFix = Schemas['ImportItemFix']
export type ImportCandidate = Schemas['ImportCandidate']
export type ImportFileInfo = Schemas['ImportFileInfo']
export type ListFormat = Schemas['ListFormat']
export type ListImportRequest = Schemas['ListImportRequest']
export type YandexImportRequest = Schemas['YandexImportRequest']

// События прослушивания
export type EndReason = Schemas['EndReason']
export type ContextType = Schemas['ContextType']
export type FeedbackKind = Schemas['FeedbackKind']
export type PlayEvent = Schemas['PlayEvent']
export type FeedbackEvent = Schemas['FeedbackEvent']
export type EventBatch = Schemas['EventBatch']
export type EventResult = Schemas['EventResult']

// Живые события (WebSocket /api/ws)
export type ServerEvent = Schemas['ServerEvent']
export type ServerEventType = ServerEvent['type']
export type AcquisitionStatus = Schemas['AcquisitionStatus']
export type AcquisitionUpdate = Schemas['AcquisitionUpdate']
export type SyncEntity = Schemas['SyncEntity']
export type ClientMessage = Schemas['ClientMessage']

// Управление между устройствами (connect.* по WebSocket)
export type ConnectDevice = Schemas['ConnectDevice']
export type ConnectKind = Schemas['ConnectKind']
export type ConnectOverview = Schemas['ConnectOverview']
export type PlaybackState = Schemas['PlaybackState']
export type RemoteCommand = Schemas['RemoteCommand']
export type ConnectQueueItem = Schemas['QueueItem']

// «Слушать вместе» (Jam): сессия хозяина и страница гостя
export type Jam = Schemas['Jam']
export type JamGuest = Schemas['JamGuest']
export type JamRequest = Schemas['JamRequest']
export type JamRequestStatus = Schemas['JamRequestStatus']
export type JamSettings = Schemas['JamSettings']
export type JamPatch = Schemas['JamPatch']
export type JamTrack = Schemas['JamTrack']
export type JamInvite = Schemas['JamInvite']
export type JamJoined = Schemas['JamJoined']
export type JamGuestView = Schemas['JamGuestView']
export type JamNow = Schemas['JamNow']
export type JamQueueItem = Schemas['JamQueueItem']
export type JamSkip = Schemas['JamSkip']
export type JamStream = Schemas['JamStream']
export type JamReaction = Schemas['JamReaction']

// Админка
export type Health = Schemas['Health']
export type AdminUser = Schemas['AdminUser']
export type UserList = Schemas['UserList']
export type UserPatch = Schemas['UserPatch']
export type SessionInfo = Schemas['SessionInfo']
export type SessionList = Schemas['SessionList']
export type AuditEvent = Schemas['AuditEvent']
export type AuditPage = Schemas['AuditPage']
export type AdminOverview = Schemas['AdminOverview']
export type Job = Schemas['Job']
export type JobPage = Schemas['JobPage']
export type JobStatus = Schemas['JobStatus']
export type JobUpdate = Schemas['JobUpdate']
export type Lane = Schemas['Lane']
export type LaneList = Schemas['LaneList']
export type LanePatch = Schemas['LanePatch']
export type LaneSchedule = Schemas['LaneSchedule']
export type Acquisition = Schemas['Acquisition']
export type AcquireReason = Schemas['AcquireReason']
export type AdminAcquisition = Schemas['AdminAcquisition']
export type AdminAcquisitionPage = Schemas['AdminAcquisitionPage']
export type AcquisitionCandidate = Schemas['AcquisitionCandidate']
export type AcquisitionAttempt = Schemas['AcquisitionAttempt']
export type AttemptOutcome = Schemas['AttemptOutcome']
export type ResolveAction = Schemas['ResolveAction']
export type ResolveRequest = Schemas['ResolveRequest']
export type Downloader = Schemas['Downloader']
export type DownloaderToken = Schemas['DownloaderToken']
export type Sources = Schemas['Sources']
export type SourceDay = Schemas['SourceDay']
export type AdminRecording = Schemas['AdminRecording']
export type AdminRecordingPage = Schemas['AdminRecordingPage']
export type AudioFileInfo = Schemas['AudioFileInfo']
export type StorageOverview = Schemas['StorageOverview']
export type AdminImport = Schemas['AdminImport']
export type AdminImportList = Schemas['AdminImportList']
export type ServerSettings = Schemas['ServerSettings']
export type ServerSettingsPatch = Schemas['ServerSettingsPatch']
export type TrackPreview = Schemas['TrackPreview']

// Волна и рекомендации
export type WaveSession = Schemas['WaveSession']
export type WaveItem = Schemas['WaveItem']
export type WaveItems = Schemas['WaveItems']
export type WaveFeedbackKind = Schemas['WaveFeedbackKind']
export type WaveReason = Schemas['Reason']
export type SoundSearchResults = Schemas['SoundSearchResults']
export type WaveSeed = Schemas['WaveSeed']
export type WavePreset = Schemas['WavePreset']
export type WavePresetList = Schemas['WavePresetList']
export type NewWavePreset = Schemas['NewWavePreset']
export type WavePresetPatch = Schemas['WavePresetPatch']

// Плейлист по описанию
export type DescribeBrief = Schemas['DescribeBrief']
export type DescribePreview = Schemas['DescribePreview']
export type DescribedPlaylist = Schemas['DescribedPlaylist']
export type SaveDescribed = Schemas['SaveDescribed']
export type RecoOverview = Schemas['RecoOverview']
export type RecoSettings = Schemas['RecoSettings']
export type RecoSettingsPatch = Schemas['RecoSettingsPatch']
export type RecoTask = Schemas['RecoTask']
export type FacetView = Schemas['FacetView']
export type RecoListener = Schemas['RecoListener']
export type WaveSessionView = Schemas['WaveSessionView']
export type WavePoolView = Schemas['WavePoolView']
export type MixView = Schemas['MixView']
export type RecoRunResult = Schemas['RecoRunResult']

// «Поделиться»: карточка ссылки и разбор чужих ссылок
export type ShareKind = Schemas['ShareKind']
export type ShareCard = Schemas['ShareCard']
export type LinkSource = Schemas['LinkSource']
export type ResolvedLink = Schemas['ResolvedLink']

export type ErrorBody = Schemas['ErrorBody']

/** Ошибка API в едином виде: код для логики, текст по-русски для человека. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

// «Распознать» (Shazam): что узнали и какой это трек каталога
export type Recognition = Schemas['Recognition']
export type RecognitionPage = Schemas['RecognitionPage']
export type RecognizeRequest = Schemas['RecognizeRequest']

// Уведомления: центр, настройки по видам и тихие часы, подписки устройств на push
export type Notification = Schemas['Notification']
export type NotificationItem = Schemas['NotificationItem']
export type NotificationKind = Schemas['NotificationKind']
export type NotificationPage = Schemas['NotificationPage']
export type NotificationsRead = Schemas['NotificationsRead']
export type NotificationKindSetting = Schemas['NotificationKindSetting']
export type QuietHours = Schemas['QuietHours']
export type NotificationSettings = Schemas['NotificationSettings']
export type NotificationSettingsPatch = Schemas['NotificationSettingsPatch']
export type PushChannel = Schemas['PushChannel']
export type ApnsEnvironment = Schemas['ApnsEnvironment']
export type PushRegistration = Schemas['PushRegistration']
export type PushRegistered = Schemas['PushRegistered']
export type PushUnregister = Schemas['PushUnregister']
export type PushDevice = Schemas['PushDevice']
export type PushInfo = Schemas['PushInfo']
export type PushTest = Schemas['PushTest']
