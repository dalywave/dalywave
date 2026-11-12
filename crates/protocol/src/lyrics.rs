//! Тексты песен (`GET /api/tracks/{id}/lyrics`) и поиск песни по строчке (`GET /api/lyrics/search`).
//!
//! Источник — LRCLIB: строки с временем (подсветка текущей строки) или простой текст.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::catalog::Track;

/// Что известно о тексте трека.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LyricsKind {
    /// Строки с временем: `lines`.
    Synced,
    /// Текст без времени: `plain`.
    Plain,
    /// Песня без слов.
    Instrumental,
    /// Текста не нашли.
    None,
}

names!(LyricsKind {
    Synced => "synced",
    Plain => "plain",
    Instrumental => "instrumental",
    None => "none",
});

/// Строка синхронного текста. Пустой `text` — проигрыш без слов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    /// Начало строки от начала трека, мс.
    pub t_ms: u32,
    pub text: String,
}

/// Ответ `GET /api/tracks/{id}/lyrics`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackLyrics {
    pub track_id: Uuid,
    pub kind: LyricsKind,
    /// Строки по возрастанию времени — только у `synced`, иначе пусто.
    pub lines: Vec<LyricsLine>,
    /// Текст целиком — у `plain` (и у `synced`, если источник его дал).
    #[schema(required = true)]
    pub plain: Option<String>,
    /// Откуда текст: `lrclib`.
    #[schema(required = true)]
    pub source: Option<String>,
}

/// Параметры `GET /api/lyrics/search`.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LyricsSearchQuery {
    /// Строчка песни, от 3 символов.
    pub q: String,
    /// До 50, по умолчанию 20.
    pub limit: Option<u32>,
}

/// Трек, в тексте которого нашлась строчка.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LyricsHit {
    pub track: Track,
    /// Строка текста, где нашлось.
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchResults {
    pub items: Vec<LyricsHit>,
}
