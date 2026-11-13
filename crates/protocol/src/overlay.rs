//! Оверлей «Сейчас играет» для стрима (OBS Browser Source): страница `/overlay?k=dwo_…` с прозрачным
//! фоном. Секретная ссылка даёт только одно право — читать, что сейчас играет у её владельца.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Начало токена оверлея.
pub const TOKEN_PREFIX: &str = "dwo_";

/// Ссылка на оверлей: `GET /api/overlay`, `POST /api/overlay/link`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverlayLink {
    /// Адрес страницы для OBS без параметров вида: `https://…/overlay?k=dwo_…`.
    pub url: String,
    pub created_at: DateTime<Utc>,
    /// Плашка прячется, пока идёт приватная сессия.
    pub hide_private: bool,
}

/// `GET /api/overlay`: ссылки ещё нет — `link: null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverlayInfo {
    #[schema(required = true)]
    pub link: Option<OverlayLink>,
}

/// `PATCH /api/overlay`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlayPatch {
    #[serde(default)]
    pub hide_private: Option<bool>,
}

/// Трек на плашке.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverlayTrack {
    pub id: Uuid,
    pub title: String,
    /// Артисты через запятую.
    pub artist: String,
    /// Обложка 250×250 (путь `/api/images/…` или адрес Deezer).
    #[schema(required = true)]
    pub cover: Option<String>,
}

/// Что показать на плашке: `GET /api/overlay/now`, события `now` потока `/api/overlay/stream`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverlayNow {
    /// Показывать плашку: играет и не приватная сессия. Иначе — тихо спрятать.
    pub visible: bool,
    #[schema(required = true)]
    pub track: Option<OverlayTrack>,
    pub playing: bool,
    /// Позиция на момент `at`; пока играет, время идёт.
    pub position_ms: u32,
    pub duration_ms: u32,
    /// Время сервера.
    pub at: DateTime<Utc>,
}
