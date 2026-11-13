//! «Мой вкус»: что волна знает о слушателе — грани вкуса с примерами, статистика прослушиваний — и ручки
//! управления рекомендациями: «меньше такого», «не рекомендовать» (см. `library::BlockedList`), сброс.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::catalog::{ArtistSummary, Track};

/// Грань вкуса глазами слушателя.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TasteFacet {
    pub id: Uuid,
    /// «Трэп · Lowborn, Mzlff»
    pub label: String,
    /// Доля во вкусе, 0..1, с учётом «меньше такого»; сумма по граням — 1.
    pub share: f32,
    pub tracks: i32,
    /// Треки-примеры (до шести), самые характерные первыми.
    pub examples: Vec<Track>,
    /// Главные артисты грани (до трёх).
    pub artists: Vec<ArtistSummary>,
    /// Когда эту грань слушают заметно чаще остальных: `morning`, `day`, `evening`, `night`.
    #[schema(required = true)]
    pub peak: Option<String>,
    /// Насколько грань приглушена «меньше такого», 0..1.
    pub damped: f32,
    /// «Меньше такого», которое её приглушает: отмена — `DELETE /api/taste/dampers/{id}`.
    #[schema(required = true)]
    pub damper_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

/// «Меньше такого»: приглушённое направление звучания. Переживает ночной пересчёт граней.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TasteDamper {
    pub id: Uuid,
    pub label: String,
    /// Сила, 0..1: первое нажатие — 0,5, каждое следующее — сильнее, до 1.
    pub strength: f32,
    /// До трёх треков, по которым понятно, о чём речь.
    pub examples: Vec<Track>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopArtist {
    pub artist: ArtistSummary,
    pub plays: i64,
    pub minutes: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopGenre {
    /// Жанр Deezer как есть («Rap/Hip Hop») — английский интерфейс показывает его.
    pub id: String,
    /// По-русски («Рэп и хип-хоп»).
    pub name: String,
    /// Доля времени прослушивания, 0..1.
    pub share: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DayPartShare {
    /// `morning` (5–12), `day` (12–17), `evening` (17–23), `night`.
    pub part: String,
    /// Доля времени прослушивания, 0..1.
    pub share: f32,
}

/// Прослушивания за текущий месяц по времени слушателя.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListeningStats {
    /// Начало месяца.
    pub since: DateTime<Utc>,
    pub minutes: i64,
    /// Столько же за прошлый месяц — для сравнения.
    pub previous_minutes: i64,
    pub plays: i64,
    pub tracks: i64,
    pub artists: i64,
    pub top_artists: Vec<TopArtist>,
    pub top_genres: Vec<TopGenre>,
    pub day_parts: Vec<DayPartShare>,
}

/// Из чего волна знает вкус.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TasteKnowledge {
    pub likes: i64,
    /// Треков, которые слушатель дослушивал.
    pub listened: i64,
    /// Из лайкнутого и дослушанного разобрано по звучанию.
    pub analyzed: i64,
    pub dislikes: i64,
    pub blocked: i64,
    /// Реакций в волне (лайк, пропуск, «не сейчас»…) за 30 дней.
    pub reactions: i64,
    /// Когда грани собирались в последний раз.
    #[schema(required = true)]
    pub updated_at: Option<DateTime<Utc>>,
    /// Последний сброс вкуса: прослушивания до него грани не строят.
    #[schema(required = true)]
    pub reset_at: Option<DateTime<Utc>>,
}

/// `GET /api/taste`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TasteOverview {
    pub facets: Vec<TasteFacet>,
    pub dampers: Vec<TasteDamper>,
    pub stats: ListeningStats,
    pub knows: TasteKnowledge,
}

/// `POST /api/taste/reset`: подтверждение обязательно — сброс не отменить.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TasteReset {
    pub confirm: bool,
}

/// Чью сеть учить на этом компьютере.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainScope {
    /// Решит сервер: администратору — общую, остальным — ничего (они помогают, только включив это в
    /// настройках, и учат свою).
    #[default]
    Auto,
    /// Общая сеть всех слушателей — только администратору.
    Global,
    /// Своя сеть того, кто вошёл.
    Personal,
}

names!(TrainScope { Auto => "auto", Global => "global", Personal => "personal" });

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainClaimBody {
    #[serde(default)]
    pub scope: TrainScope,
}

/// Задание компьютеру: набор и описание обучения (`dalywave-taste`, `wire::Package`). Отложенной части
/// в наборе нет — её проверяет сервер.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainClaim {
    pub task_id: Uuid,
    /// `global` или `personal`.
    pub scope: TrainScope,
    pub rows: i64,
    #[schema(value_type = Object)]
    pub package: serde_json::Value,
}

/// Веса и как шло обучение (`dalywave-taste`: `wire::WireParams`, `TrainReport`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainSubmit {
    #[schema(value_type = Object)]
    pub params: serde_json::Value,
    #[schema(value_type = Object)]
    pub report: serde_json::Value,
}

/// Что сервер решил о присланных весах.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainOutcome {
    pub version: String,
    /// Собирает ли поток теперь эта версия.
    pub active: bool,
    pub note: String,
}

/// Обучение на компьютере не удалось.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainFailure {
    pub message: String,
}
