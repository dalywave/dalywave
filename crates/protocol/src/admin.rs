use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    auth::{AuthVia, Device, Role, SessionKind},
    catalog::{ArtistRef, AudioStatus, Track},
    library::DownloadPolicy,
};

/// Пользователь в разделе «Доступ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    pub id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub email: Option<String>,
    pub email_verified: bool,
    #[schema(required = true)]
    pub avatar_url: Option<String>,
    /// Провайдер, через который человек вошёл впервые; `null` у своих аккаунтов и владельца из аварийного входа.
    #[schema(required = true)]
    pub issuer: Option<String>,
    /// Логин своего аккаунта.
    #[schema(required = true)]
    pub username: Option<String>,
    /// Может входить логином и паролем.
    pub has_password: bool,
    /// Через каких провайдеров входит: названия из настроек (или адрес отключённого провайдера).
    pub providers: Vec<String>,
    pub roles: Vec<Role>,
    /// Роль admin выдана провайдером и снимется, когда её снимут там.
    pub idp_admin: bool,
    pub disabled: bool,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserList {
    pub items: Vec<AdminUser>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

/// Изменение пользователя: только переданные поля.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPatch {
    /// Полный новый список ролей.
    #[serde(default)]
    pub roles: Option<Vec<Role>>,
    #[serde(default)]
    pub disabled: Option<bool>,
}

/// Способы входа на сервере — только чтение: настраиваются переменными окружения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthOverview {
    pub providers: Vec<ProviderInfo>,
    /// Свои аккаунты (`DALYWAVE_LOCAL_AUTH`).
    pub local: bool,
    /// Аварийный пароль из `.env`.
    pub emergency_password: bool,
}

/// Подключённый провайдер OIDC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub title: String,
    #[schema(required = true)]
    pub icon: Option<String>,
    pub issuer: String,
    /// Адрес возврата, который надо зарегистрировать у провайдера.
    pub callback_url: String,
    /// Claim ролей или групп; `null` — провайдер не выдаёт админов.
    #[schema(required = true)]
    pub admin_claim: Option<String>,
    pub admin_values: Vec<String>,
    pub auto_register: bool,
    pub trust_email: bool,
    /// Сколько людей входит через него.
    pub users: i64,
}

/// Новый свой аккаунт от админа.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewUser {
    #[serde(default)]
    pub name: Option<String>,
    pub username: String,
    /// Без пароля ответ содержит одноразовую ссылку, по которой человек задаст его сам.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub roles: Vec<Role>,
}

/// Одноразовая ссылка: показывается один раз.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OneTimeLink {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatedUser {
    pub user: AdminUser,
    /// Ссылка, чтобы задать пароль; `null`, если пароль задан сразу.
    #[schema(required = true)]
    pub link: Option<OneTimeLink>,
}

/// Новое приглашение.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewInvite {
    #[serde(default)]
    pub roles: Vec<Role>,
    /// Сколько дней действует: 1–30, по умолчанию 7.
    #[serde(default)]
    pub days: Option<u32>,
    /// Подпись для себя: кому отправили.
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    Active,
    Used,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub id: Uuid,
    pub roles: Vec<Role>,
    #[schema(required = true)]
    pub note: Option<String>,
    pub status: InviteStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    /// Кто принял приглашение.
    #[schema(required = true)]
    pub used_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InviteList {
    pub items: Vec<Invite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatedInvite {
    pub invite: Invite,
    /// Ссылка-приглашение; показывается один раз.
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct SessionsQuery {
    /// Только сессии этого пользователя.
    pub user_id: Option<Uuid>,
}

/// Открытая сессия: вкладка сайта или приложение.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub kind: SessionKind,
    pub via: AuthVia,
    #[schema(required = true)]
    pub device: Option<Device>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    #[schema(required = true)]
    pub user_agent: Option<String>,
    #[schema(required = true)]
    pub ip: Option<String>,
    /// Это сессия, из которой сделан запрос.
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionList {
    pub items: Vec<SessionInfo>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct AuditQuery {
    /// `nextCursor` из предыдущей страницы.
    pub cursor: Option<String>,
    /// Сколько событий вернуть: 1–200, по умолчанию 50.
    pub limit: Option<u32>,
    /// Вид события или его группа: `auth.sign_in` — ровно он, `auth.` — все входы и выходы.
    pub kind: Option<String>,
    /// Только события этого человека.
    pub actor: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditActor {
    pub id: Uuid,
    pub name: String,
}

/// Событие журнала: входы, смены ролей, отзывы сессий.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: Uuid,
    pub at: DateTime<Utc>,
    /// Кто сделал; `null` — сам сервер или вход без сессии.
    #[schema(required = true)]
    pub actor: Option<AuditActor>,
    /// Вид события, например `auth.sign_in` или `user.roles_changed`.
    pub kind: String,
    /// Над чем: id пользователя, сессии или устройства.
    #[schema(required = true)]
    pub subject: Option<String>,
    #[schema(value_type = Object)]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub items: Vec<AuditEvent>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

// ───────────── Каталог: записи на сервере ─────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct RecordingsQuery {
    /// Состояние аудио; по умолчанию — все, у которых есть файл или запрос на скачивание.
    pub status: Option<AudioStatus>,
    /// Источник основного файла: `youtube`, `youtube_music`, `soundcloud`, `bandcamp`, `upload`, `other`.
    pub source: Option<String>,
    /// Только непроверенные отпечатком (`false`) или только проверенные (`true`).
    pub verified: Option<bool>,
    /// Название трека или имя артиста.
    pub q: Option<String>,
    /// `nextCursor` из предыдущей страницы.
    pub cursor: Option<String>,
    /// 1–200, по умолчанию 50.
    pub limit: Option<u32>,
}

/// Аудиофайл записи с техническими данными.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioFileInfo {
    pub id: Uuid,
    pub codec: String,
    pub container: String,
    pub lossless: bool,
    #[schema(required = true)]
    pub bitrate_kbps: Option<i32>,
    #[schema(required = true)]
    pub sample_rate: Option<i32>,
    pub duration_ms: i32,
    pub size_bytes: i64,
    pub source: String,
    /// id видео или ссылка.
    #[schema(required = true)]
    pub source_ref: Option<String>,
    #[schema(required = true)]
    pub source_title: Option<String>,
    #[schema(required = true)]
    pub loudness_lufs: Option<f32>,
    #[schema(required = true)]
    pub true_peak_db: Option<f32>,
    #[schema(required = true)]
    pub match_score: Option<f32>,
    pub verified: bool,
    #[schema(required = true)]
    pub verify_similarity: Option<f32>,
    /// `server` или `device:<id>`.
    #[schema(required = true)]
    pub acquired_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Запись в разделе «Каталог».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminRecording {
    pub id: Uuid,
    #[schema(required = true)]
    pub isrc: Option<String>,
    pub duration_ms: i32,
    pub audio_status: AudioStatus,
    pub status_changed_at: DateTime<Utc>,
    /// Один из треков записи (сначала с альбомом); `null`, если треков не осталось.
    #[schema(required = true)]
    pub track: Option<Track>,
    /// Сколько треков каталога делят запись.
    pub tracks: i64,
    /// Основной файл.
    #[schema(required = true)]
    pub file: Option<AudioFileInfo>,
    /// Всего файлов у записи, с вытесненными.
    pub files: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminRecordingPage {
    pub items: Vec<AdminRecording>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

/// Скачать запись заново: по ссылке (YouTube, SoundCloud, Bandcamp…) или поиском, если ссылки нет.
/// Файл, выбранный человеком, заменяет основной.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceAudio {
    #[serde(default)]
    pub url: Option<String>,
}

/// Свежая ссылка на 30-секундное превью Deezer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackPreview {
    /// `null` — у трека нет превью или его нет у Deezer.
    #[schema(required = true)]
    pub url: Option<String>,
}

// ───────────── Хранилище ─────────────

/// Сколько места занимает группа файлов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageGroup {
    /// Для видов: `audio`, `covers`, `photos`; для звука — кодек или источник.
    pub key: String,
    pub files: i64,
    pub bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageArtist {
    pub artist: ArtistRef,
    pub files: i64,
    pub bytes: i64,
}

/// Прирост звука за день (UTC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageDay {
    /// `2026-11-11`.
    pub day: String,
    pub files: i64,
    pub bytes: i64,
}

/// Раздел «Хранилище».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StorageOverview {
    pub total_bytes: i64,
    /// Звук, обложки, фото артистов.
    pub kinds: Vec<StorageGroup>,
    /// Звук по кодекам.
    pub codecs: Vec<StorageGroup>,
    /// Звук по источникам.
    pub sources: Vec<StorageGroup>,
    /// Самые «тяжёлые» артисты по основным файлам их записей, до 12.
    pub artists: Vec<StorageArtist>,
    /// Прирост звука за последние 30 дней, по дням.
    pub growth: Vec<StorageDay>,
    /// Файлы, вытесненные лучшими копиями: занимают место, но не играются.
    pub replaced_files: i64,
    pub replaced_bytes: i64,
}

// ───────────── Импорт ─────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminImport {
    pub id: Uuid,
    pub user_name: String,
    /// `text`, `csv`, `m3u`, `files`, `spotify_archive`.
    pub kind: String,
    pub title: String,
    /// `running`, `done`, `failed`, `cancelled`.
    pub status: String,
    pub total: i32,
    pub matched: i32,
    /// Несопоставленных позиций.
    pub unmatched: i64,
    /// `uploading`, `matching`, `applying`, `done`.
    pub phase: String,
    /// Разобрано позиций из `total`.
    pub processed: i32,
    #[schema(required = true)]
    pub error: Option<String>,
    /// Что добавилось в медиатеку.
    pub stats: crate::imports::ImportStats,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminImportList {
    pub items: Vec<AdminImport>,
}

// ───────────── Настройки сервера ─────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    /// Что докачивать по новой подписке, если человек не выбрал сам.
    pub follow_policy: DownloadPolicy,
    /// Часовой пояс владельца в минутах от UTC: окна работы полос, время суток у граней вкуса.
    pub utc_offset_min: i16,
    pub updated_at: DateTime<Utc>,
}

/// Изменение настроек: только переданные поля.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerSettingsPatch {
    #[serde(default)]
    pub follow_policy: Option<DownloadPolicy>,
    /// −720…840.
    #[serde(default)]
    pub utc_offset_min: Option<i16>,
}

// ───────────── Своя музыка ─────────────

/// Папка своей музыки на сервере.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDir {
    pub path: String,
    /// Папка есть и читается.
    pub available: bool,
    /// Файлов из неё в медиатеке.
    pub files: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LibraryScanKind {
    /// Все папки целиком.
    Full,
    /// Только изменившееся (наблюдение за папками).
    Changes,
}

names!(LibraryScanKind { Full => "full", Changes => "changes" });

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LibraryScanStatus {
    Running,
    Done,
    Failed,
}

names!(LibraryScanStatus { Running => "running", Done => "done", Failed => "failed" });

/// Проход сканера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScan {
    pub id: Uuid,
    pub kind: LibraryScanKind,
    pub status: LibraryScanStatus,
    pub files_total: i32,
    pub files_done: i32,
    pub added: i32,
    pub updated: i32,
    pub moved: i32,
    pub removed: i32,
    pub failed: i32,
    #[schema(required = true)]
    pub message: Option<String>,
    pub started_at: DateTime<Utc>,
    #[schema(required = true)]
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LibraryFileProblem {
    /// Файл не прочитался (повреждён, нет прав).
    Error,
    /// Формат не поддерживается.
    Unsupported,
}

names!(LibraryFileProblem { Error => "error", Unsupported => "unsupported" });

/// Файл, который не попал в медиатеку.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProblem {
    pub path: String,
    pub problem: LibraryFileProblem,
    #[schema(required = true)]
    pub message: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Раздел админки «Библиотека»: папки, сканирование, файлы с ошибками.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub dirs: Vec<LibraryDir>,
    /// Идущий или последний проход.
    #[schema(required = true)]
    pub scan: Option<LibraryScan>,
    /// Треков из своих папок в медиатеке.
    pub tracks: i64,
    /// Файлов, которые ещё ждут громкости и анализа.
    pub analyzing: i64,
    /// Файлов, которые играются через копию: их формат клиенты не играют.
    pub transcoded: i64,
    pub errors: i64,
    pub unsupported: i64,
    /// Первые файлы с ошибками и неподдерживаемые — для списка.
    pub problems: Vec<LibraryProblem>,
}
