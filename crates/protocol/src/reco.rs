//! Раздел «Рекомендации» админки: индекс каталога вокруг вкуса, анализ звука, грани вкуса, последние
//! показы с причинами и настройки разведчика.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{catalog::Track, wave::Reason};

/// Настройки разведчика. Остановить анализ или разведку — пауза полос `analysis` и `scout`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecoSettings {
    /// Сколько треков в сутки разведчик отдаёт на анализ превью.
    pub tracks_per_day: i32,
    /// Предел индекса: анализов по превью; дальние от вкуса вытесняются.
    pub max_index: i32,
    /// Сколько шагов по похожим артистам Deezer (0–3).
    pub max_hops: i16,
    /// Во сколько раз падает близость на каждом шаге (0..1).
    pub hop_decay: f32,
    /// Часовой пояс слушателя в минутах от UTC: время суток у граней вкуса.
    pub utc_offset_min: i16,
    pub updated_at: DateTime<Utc>,
}

/// Изменение настроек: только переданные поля.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoSettingsPatch {
    /// 0–50 000.
    #[serde(default)]
    pub tracks_per_day: Option<i32>,
    /// 100–2 000 000.
    #[serde(default)]
    pub max_index: Option<i32>,
    /// 0–3.
    #[serde(default)]
    pub max_hops: Option<i16>,
    /// 0.05–0.95.
    #[serde(default)]
    pub hop_decay: Option<f32>,
    /// −720…840.
    #[serde(default)]
    pub utc_offset_min: Option<i16>,
}

/// Сколько найдено и проанализировано по виду находки разведчика.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoutSourceStats {
    /// `top`, `release`, `radio`, `chart`, `editorial`.
    pub source: String,
    pub found: i64,
    pub analyzed: i64,
    pub pending: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    /// Анализов по превью — это и есть индекс каталога.
    pub size: i64,
    pub max_index: i64,
    /// Полных анализов скачанного.
    pub full: i64,
    /// Записей на сервере без полного анализа.
    pub ready_without_analysis: i64,
    pub by_source: Vec<ScoutSourceStats>,
    /// Артистов в окрестности вкуса; из них семян.
    pub artists: i64,
    pub seeds: i64,
    /// Задачи в очереди.
    pub queued_full: i64,
    pub queued_preview: i64,
    /// За сутки: проанализировано (полностью и по превью) и сбоев.
    pub analyzed_24h: i64,
    pub failed_24h: i64,
}

/// Грань вкуса с примерами.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FacetView {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub position: i16,
    pub label: String,
    /// Доля вкуса, 0..1.
    pub weight: f32,
    pub tracks: i32,
    /// Поправки по времени суток (`timeOfDay`) и устройству (`platform`): больше 1 — грань слушают чаще.
    #[schema(value_type = Object)]
    pub context: Value,
    pub examples: Vec<Track>,
    pub updated_at: DateTime<Utc>,
}

/// Показ рекомендации и чем он кончился.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImpressionView {
    pub id: Uuid,
    pub user_name: String,
    /// `wave`, `radio`, `similar`, `mix`, `discovery`.
    pub surface: String,
    pub track: Track,
    #[schema(required = true)]
    pub reason: Option<Reason>,
    #[schema(required = true)]
    pub score: Option<f32>,
    /// `like`, `dislike`, `skip`, `not_now`, `more_like_this`, `completed`; `null` — реакции не было.
    #[schema(required = true)]
    pub outcome: Option<String>,
    #[schema(required = true)]
    pub wave_session_id: Option<Uuid>,
    pub shown_at: DateTime<Utc>,
}

/// Сервис анализа звука.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MlStatus {
    pub url: String,
    pub reachable: bool,
    /// Ответ `/v1/healthz`: загружены ли модели, занят ли сервис, версии.
    #[schema(value_type = Option<Object>, required = true)]
    pub health: Option<Value>,
    #[schema(required = true)]
    pub error: Option<String>,
}

/// Раздел «Рекомендации».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecoOverview {
    pub index: IndexStats,
    pub settings: RecoSettings,
    /// Версия, которая собирает поток: `rules-1`, пока своей модели нет.
    #[schema(required = true)]
    pub model_version: Option<String>,
    pub ml: MlStatus,
    /// Грани вкуса, самые весомые первыми.
    pub facets: Vec<FacetView>,
    /// Последние показы, новые первыми.
    pub impressions: Vec<ImpressionView>,
}

/// Что запустить сейчас, не дожидаясь расписания.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoTask {
    /// Проход разведчика: окрестность вкуса, кандидаты, анализ превью в пределах бюджета.
    Scout,
    /// Грани вкуса, миксы дня и «Открытия недели» — для всех слушателей.
    Taste,
    /// Полный анализ скачанного, которое ещё не разобрано.
    Analysis,
}

names!(RecoTask { Scout => "scout", Taste => "taste", Analysis => "analysis" });

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoRun {
    pub task: RecoTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecoRunResult {
    /// Сколько задач поставлено (уже стоящие не считаются).
    pub queued: i64,
}
