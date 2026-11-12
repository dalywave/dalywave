//! Версии одной песни, выпущенные самими исполнителями: оригинал, slowed, super slowed, sped up
//! (`GET /api/tracks/{id}/versions`). Звук сервис сам не обрабатывает — только находит готовые релизы.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::catalog::Track;

/// Вид версии по названию релиза: «(Slowed + Reverb)» — slowed, «(Ultra Slowed)» — super slowed,
/// «(Nightcore)» — sped up. Ремиксы, 8D и концертные записи версиями не считаются.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Original,
    Slowed,
    /// Super, ultra, hyper, extremely slowed — сильнее замедленная.
    SuperSlowed,
    /// Sped up, speed up, nightcore.
    SpedUp,
}

names!(VersionKind {
    Original => "original",
    Slowed => "slowed",
    SuperSlowed => "super_slowed",
    SpedUp => "sped_up",
});

/// Одна версия песни: по одной на вид.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackVersion {
    pub kind: VersionKind,
    pub track: Track,
}

/// Версии песни трека. В `items` — и сам трек; версий нет — в `items` только он.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackVersions {
    pub track_id: Uuid,
    /// Вид этого трека.
    pub kind: VersionKind,
    /// По порядку: оригинал, slowed, super slowed, sped up.
    pub items: Vec<TrackVersion>,
    /// Какую версию человек выбирал для основного исполнителя трека в прошлый раз.
    #[schema(required = true)]
    pub preferred: Option<VersionKind>,
}

/// Выбор версии: в следующий раз у этого исполнителя играет та же.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionChoiceBody {
    pub kind: VersionKind,
}

/// Исполнитель, у которого человек выбрал не оригинал.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionChoice {
    pub artist_id: Uuid,
    pub kind: VersionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionChoices {
    pub items: Vec<VersionChoice>,
}
