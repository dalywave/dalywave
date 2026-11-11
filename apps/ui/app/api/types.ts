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

// Главная
export type ShelfKind = Schemas['ShelfKind']
export type ShelfItem = Schemas['ShelfItem']
export type Shelf = Schemas['Shelf']
export type Home = Schemas['Home']

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

// Плейлисты
export type PlaylistKind = Schemas['PlaylistKind']
export type PlaylistSummary = Schemas['PlaylistSummary']
export type PlaylistItem = Schemas['PlaylistItem']
export type PlaylistDetail = Schemas['PlaylistDetail']
export type PlaylistList = Schemas['PlaylistList']
export type NewPlaylist = Schemas['NewPlaylist']
export type PlaylistPatch = Schemas['PlaylistPatch']
export type AddPlaylistItems = Schemas['AddPlaylistItems']

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
export type RecoOverview = Schemas['RecoOverview']
export type RecoSettings = Schemas['RecoSettings']
export type RecoSettingsPatch = Schemas['RecoSettingsPatch']
export type RecoTask = Schemas['RecoTask']
export type FacetView = Schemas['FacetView']
export type RecoRunResult = Schemas['RecoRunResult']

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
