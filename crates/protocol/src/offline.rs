//! Умный офлайн (этап 6): что держать скачанным на устройстве, набор волны для устройства и ссылки
//! на файлы для загрузки в приложение.
//!
//! Что качать, решает само приложение: закреплённое человеком (`/api/devices/current/downloads`) и
//! набор волны (`device_sets`, приходит синхронизацией). Ссылки на файлы — пачкой
//! (`/api/devices/current/files`), с размером и хешем для проверки скачанного.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::stream::{AudioCodec, AudioContainer};

/// Что закрепить на устройстве.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
    Track,
    Album,
    Playlist,
    /// Артист из подписок: вся его дискография на сервере.
    Artist,
    /// Все любимые треки.
    Liked,
}

names!(DownloadKind {
    Track => "track",
    Album => "album",
    Playlist => "playlist",
    Artist => "artist",
    Liked => "liked",
});

/// Закрепить на этом устройстве. `refId` — у всех, кроме `liked`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewDeviceDownload {
    pub kind: DownloadKind,
    #[serde(default)]
    pub ref_id: Option<Uuid>,
}

/// Закреплённое на устройстве.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDownload {
    pub id: Uuid,
    pub kind: DownloadKind,
    #[schema(required = true)]
    pub ref_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// Треков в нём на сервере сейчас и всего.
    pub tracks_ready: i64,
    pub tracks_total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDownloadList {
    pub items: Vec<DeviceDownload>,
}

/// Собрать набор волны для устройства: сколько треков любимого и нового поместится в запас.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceSetRequest {
    /// 0–2000.
    pub favorites: u32,
    /// 0–2000.
    pub fresh: u32,
}

/// Набор волны устройства после пересборки. Сами строки приходят синхронизацией (`deviceSets`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSet {
    pub favorites: i64,
    pub fresh: i64,
    /// Ушло из набора: новое, которое уже сыграно, и то, что больше не подходит.
    pub removed: i64,
    pub built_at: DateTime<Utc>,
}

/// Ссылки на файлы треков: до 200 за раз.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileLinksRequest {
    pub track_ids: Vec<Uuid>,
}

/// Файл трека для загрузки на устройство.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileLink {
    pub track_id: Uuid,
    pub recording_id: Uuid,
    pub file_id: Uuid,
    /// Подписанная ссылка на файл; поддерживает `Range` — недокачанное продолжается.
    pub url: String,
    pub expires_at: DateTime<Utc>,
    /// Размер файла: по нему приложение проверяет скачанное.
    pub size_bytes: i64,
    /// SHA-256 в hex, если известен.
    #[schema(required = true)]
    pub sha256: Option<String>,
    pub codec: AudioCodec,
    pub container: AudioContainer,
    pub mime_type: String,
    pub duration_ms: i32,
    #[schema(required = true)]
    pub loudness_lufs: Option<f32>,
    #[schema(required = true)]
    pub true_peak_db: Option<f32>,
    /// Огибающая для полосы прокрутки: 512 точек, 0..1000.
    #[schema(required = true)]
    pub waveform: Option<Vec<i16>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileLinks {
    pub items: Vec<FileLink>,
    /// Файла на сервере ещё нет: скачивание поставлено в очередь, спросить позже.
    pub pending: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_match_database() {
        for k in DownloadKind::ALL {
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
            assert_eq!(k.as_str().parse::<DownloadKind>().unwrap(), *k);
        }
    }
}
