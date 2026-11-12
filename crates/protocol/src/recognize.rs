//! «Распознать»: приложение узнаёт песню через ShazamKit, сервер находит её в каталоге
//! и хранит историю распознанного.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::catalog::Track;

/// Что узнал Shazam.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizeRequest {
    /// ISRC записи: по нему трек находится точнее всего.
    #[serde(default)]
    pub isrc: Option<String>,
    /// Название (до 300 символов).
    pub title: String,
    /// Артист (до 300 символов).
    pub artist: String,
    #[serde(default)]
    pub apple_music_id: Option<String>,
    #[serde(default)]
    pub shazam_id: Option<String>,
    /// Обложка от Shazam, только `https://`.
    #[serde(default)]
    pub artwork_url: Option<String>,
}

/// Распознанная песня из истории.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recognition {
    pub id: Uuid,
    pub recognized_at: DateTime<Utc>,
    /// Как прислал Shazam.
    pub title: String,
    pub artist: String,
    #[schema(required = true)]
    pub isrc: Option<String>,
    #[schema(required = true)]
    pub artwork_url: Option<String>,
    /// Трек каталога; `null` — в каталоге не нашёлся (или его оттуда убрали).
    #[schema(required = true)]
    pub track: Option<Track>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionPage {
    pub items: Vec<Recognition>,
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}
