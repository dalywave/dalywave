//! Раздел «Рекомендации» админки: вкус выбранного слушателя (грани, показы, сессии волны, заготовки,
//! миксы) и общий индекс каталога вокруг вкуса — анализ звука и настройки разведчика.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    catalog::Track,
    wave::{Reason, WaveSettings},
};

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

/// Чей вкус показать.
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct RecoQuery {
    /// Слушатель; по умолчанию — тот, кто смотрит.
    pub user_id: Option<Uuid>,
}

/// Слушатель в выборе раздела: лайки, прослушивания и граней вкуса.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecoListener {
    pub id: Uuid,
    pub name: String,
    pub likes: i64,
    pub plays: i64,
    pub facets: i64,
}

/// Сессия волны или радио слушателя и чем кончались её показы.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WaveSessionView {
    pub id: Uuid,
    /// «Моя волна», «Радио: Lowborn».
    pub title: String,
    pub settings: WaveSettings,
    /// Сколько треков выдано.
    pub served: i64,
    /// Исходы показов: лайки, дослушано, пропуски (с «не нравится»).
    pub likes: i64,
    pub completed: i64,
    pub skips: i64,
    pub started_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

/// Заготовки волны под одно сочетание настроек.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WavePoolView {
    /// `занятие:характер:настроение:язык[:темп]`.
    pub combo: String,
    pub settings: WaveSettings,
    pub tracks: i64,
    /// Из них уже на сервере.
    pub ready: i64,
    /// Последний подбор.
    pub picked_at: DateTime<Utc>,
}

/// Собранный плейлист: «Микс дня» или «Открытия недели».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MixView {
    pub id: Uuid,
    pub title: String,
    /// `daily_mix` или `discover_weekly`.
    pub generator: String,
    pub tracks: i64,
    pub updated_at: DateTime<Utc>,
}

/// Раздел «Рекомендации»: вкус выбранного слушателя и общий индекс каталога.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecoOverview {
    /// Чей вкус показан.
    pub user: RecoListener,
    /// Кого можно выбрать: тот, кто смотрит, и все, у кого есть лайки или прослушивания.
    pub listeners: Vec<RecoListener>,
    /// Грани вкуса слушателя, самые весомые первыми.
    pub facets: Vec<FacetView>,
    /// Последние показы слушателю, новые первыми.
    pub impressions: Vec<ImpressionView>,
    /// Последние сессии волны и радио слушателя.
    pub sessions: Vec<WaveSessionView>,
    /// Заготовки волны слушателя по сочетаниям настроек.
    pub pools: Vec<WavePoolView>,
    /// «Миксы дня» и «Открытия недели» слушателя.
    pub mixes: Vec<MixView>,
    /// Общий индекс каталога: один на всех слушателей.
    pub index: IndexStats,
    pub settings: RecoSettings,
    /// Версия, которая собирает поток: `rules-1`, пока своей модели нет.
    #[schema(required = true)]
    pub model_version: Option<String>,
    pub ml: MlStatus,
    /// Версии сети вкуса с метриками на отложенных данных, новые первыми (и правила).
    pub models: Vec<ModelVersionView>,
    /// Последнее обучение сети вкуса.
    pub training: ModelTraining,
}

/// Голова сети вкуса на отложенных по времени данных: у сети и у правил.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HeadMetrics {
    /// `skip`, `complete`, `like`.
    pub head: String,
    /// Сколько положительных исходов в отложенной части.
    pub positives: i64,
    #[schema(required = true)]
    pub auc: Option<f64>,
    #[schema(required = true)]
    pub logloss: Option<f64>,
    #[schema(required = true)]
    pub baseline_auc: Option<f64>,
    #[schema(required = true)]
    pub baseline_logloss: Option<f64>,
}

/// Версия модели вкуса: правила (`rules-1`) или обученная сеть (`nn-…`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelVersionView {
    pub id: Uuid,
    pub version: String,
    /// Собирает поток сейчас.
    pub active: bool,
    /// Сеть (`true`) или правила.
    pub learned: bool,
    pub created_at: DateTime<Utc>,
    /// Средние по головам AUC и logloss на отложенной части: у сети и у правил. У правил — `null`.
    #[schema(required = true)]
    pub auc: Option<f64>,
    #[schema(required = true)]
    pub logloss: Option<f64>,
    #[schema(required = true)]
    pub baseline_auc: Option<f64>,
    #[schema(required = true)]
    pub baseline_logloss: Option<f64>,
    pub heads: Vec<HeadMetrics>,
    /// Строк на обучение и на проверку.
    #[schema(required = true)]
    pub train_rows: Option<i64>,
    #[schema(required = true)]
    pub holdout_rows: Option<i64>,
    /// Обучаемых параметров.
    #[schema(required = true)]
    pub params: Option<i64>,
    /// Точнее ли правил на отложенных данных — то есть включена ли при обучении.
    #[schema(required = true)]
    pub beats_rules: Option<bool>,
    /// Почему включена или не включена: «точнее правил и текущей», «правила не хуже».
    #[schema(required = true)]
    pub note: Option<String>,
    /// Порядок внутри сессии на отложенной части: доля верных пар «пропустил — дослушал» и NDCG@10,
    /// у сети и у правил.
    #[schema(required = true)]
    pub session_auc: Option<f64>,
    #[schema(required = true)]
    pub baseline_session_auc: Option<f64>,
    #[schema(required = true)]
    pub ndcg: Option<f64>,
    #[schema(required = true)]
    pub baseline_ndcg: Option<f64>,
    /// Доля бутстреп-выборок по сессиям, где сеть точнее правил.
    #[schema(required = true)]
    pub p_better: Option<f64>,
    /// Версия, которая была включена при проверке, и её AUC на той же отложенной части.
    #[schema(required = true)]
    pub current_version: Option<String>,
    #[schema(required = true)]
    pub current_auc: Option<f64>,
    #[schema(required = true)]
    pub beats_current: Option<bool>,
    /// Лучшая эпоха.
    #[schema(required = true)]
    pub epochs: Option<i64>,
    /// Где обучена: `server`, `device` (компьютер с приложением) или `rules`.
    pub origin: String,
    /// Компьютер, на котором обучена.
    #[schema(required = true)]
    pub device_name: Option<String>,
    /// Своя сеть слушателя; `null` — общая.
    #[schema(required = true)]
    pub user_name: Option<String>,
}

/// Задание «обучить сеть вкуса» компьютеру с приложением.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainTaskView {
    pub id: Uuid,
    pub device_name: String,
    /// Своя сеть слушателя; `null` — общая.
    #[schema(required = true)]
    pub user_name: Option<String>,
    /// `leased`, `done`, `failed`, `expired`.
    pub status: String,
    /// Строк обучающей части.
    pub rows: i64,
    /// Версия из присланных весов.
    #[schema(required = true)]
    pub version: Option<String>,
    /// Решение сервера или ошибка компьютера.
    #[schema(required = true)]
    pub message: Option<String>,
    pub leased_at: DateTime<Utc>,
    #[schema(required = true)]
    pub finished_at: Option<DateTime<Utc>>,
}

/// Последнее обучение сети вкуса (задача `taste_model`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelTraining {
    /// Обучение стоит в очереди или идёт.
    pub running: bool,
    #[schema(required = true)]
    pub last_run_at: Option<DateTime<Utc>>,
    /// `trained`, `too_little_data`, `failed`.
    #[schema(required = true)]
    pub outcome: Option<String>,
    /// Что вышло: «Мало данных: 120 прослушиваний из 300», текст ошибки.
    #[schema(required = true)]
    pub message: Option<String>,
    /// Размеченных прослушиваний для обучения сейчас и сколько нужно.
    pub rows: i64,
    pub min_rows: i64,
    /// Последние задания компьютерам, новые первыми.
    pub device_tasks: Vec<TrainTaskView>,
}

/// Что запустить сейчас, не дожидаясь расписания.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoTask {
    /// Проход разведчика: окрестность вкуса, кандидаты, анализ превью в пределах бюджета.
    Scout,
    /// Грани вкуса, миксы дня и «Открытия недели» — выбранного слушателя или всех.
    Taste,
    /// Полный анализ скачанного, которое ещё не разобрано.
    Analysis,
    /// Обучить сеть вкуса сейчас; включится, только если точнее правил.
    Model,
}

names!(RecoTask { Scout => "scout", Taste => "taste", Analysis => "analysis", Model => "model" });

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoRun {
    pub task: RecoTask,
    /// Для `taste`: только этот слушатель; без него — все.
    #[serde(default)]
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecoRunResult {
    /// Сколько задач поставлено (уже стоящие не считаются).
    pub queued: i64,
}
