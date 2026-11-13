//! «Плейлист по описанию»: человек пишет словами — «40 минут фонка для зала, без медляков», «спокойный
//! инди под дождь, только русское», «как Скриптонит, но менее известное», — сервер разбирает запрос
//! правилами (своими средствами, без внешних нейросетей) и собирает плейлист: предпросмотр, «ещё в том же
//! духе», сохранение обычным плейлистом и, по желанию, обновление раз в неделю.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::wave::{WaveActivity, WaveCharacter, WaveItem, WaveLanguage, WaveMood};

/// Артист-ориентир («как Скриптонит») или исключение («без Моргенштерна»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BriefArtist {
    /// Как написал человек: «Скриптонита».
    pub said: String,
    /// Найденный артист каталога; `None` — не нашли.
    #[serde(default)]
    #[schema(required = true)]
    pub id: Option<Uuid>,
    /// Имя артиста в каталоге (или как написано, если не нашли).
    pub name: String,
}

/// Что понято из описания: по этому собирается плейлист. Отдаётся в предпросмотре и приходит обратно
/// в «Ещё в том же духе» и при сохранении — повторно запрос не разбирается.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DescribeBrief {
    /// Описание как есть.
    pub text: String,
    /// Длительность, минут («40 минут», «на час»).
    #[schema(required = true)]
    pub minutes: Option<u32>,
    /// Число треков («20 треков»).
    #[schema(required = true)]
    pub count: Option<u32>,
    pub activity: WaveActivity,
    pub mood: WaveMood,
    pub language: WaveLanguage,
    pub character: WaveCharacter,
    /// Жанры: `phonk`, `indie`, `rap` …
    pub genres: Vec<String>,
    /// Жанры, которых не надо («без попсы»).
    pub avoid_genres: Vec<String>,
    /// «Без медляков».
    pub no_slow: bool,
    /// «Без мата».
    pub no_explicit: bool,
    /// Ориентиры: «как Скриптонит».
    pub like: Vec<BriefArtist>,
    /// Исключённые артисты: «без Моргенштерна».
    pub without: Vec<BriefArtist>,
    /// Обстановка словами для названия: «для зала», «под дождь».
    #[schema(required = true)]
    pub scene: Option<String>,
    /// Текст для поиска по звучанию (CLAP): то, что описывает звук.
    #[schema(required = true)]
    pub sound: Option<String>,
    /// В описании есть конкретика — жанр, ориентир или своё слово о звуке: подбор держится описания,
    /// а вкус только подмешивается. Без неё («40 минут для зала») — подбор из своего вкуса под настройки.
    pub focused: bool,
}

/// Собрать плейлист по описанию. `exclude` — треки прошлой сборки («Пересобрать»): их сервер
/// постарается не повторять.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescribeRequest {
    /// 2–300 символов.
    pub text: String,
    #[serde(default)]
    pub exclude: Vec<Uuid>,
}

/// Предпросмотр: название, описание и треки по порядку (плавно, как сет).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DescribePreview {
    pub brief: DescribeBrief,
    /// «Фонк для зала · 40 мин».
    pub title: String,
    /// «Без медляков · только русское»; пустая строка — описывать нечего.
    pub description: String,
    pub items: Vec<WaveItem>,
    /// Длительность всех треков.
    pub duration_ms: i64,
}

/// «Ещё в том же духе» для предпросмотра: следующие треки после `afterTrackId` без уже показанных.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescribeMore {
    pub brief: DescribeBrief,
    /// Уже показанные треки.
    #[serde(default)]
    pub exclude: Vec<Uuid>,
    /// Последний трек списка — к нему подстраивается начало новых.
    #[serde(default)]
    pub after_track_id: Option<Uuid>,
    /// 1–30, по умолчанию 10.
    #[serde(default)]
    pub count: Option<u32>,
}

/// Сохранить предпросмотр обычным плейлистом.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveDescribed {
    pub brief: DescribeBrief,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub track_ids: Vec<Uuid>,
    /// Раз в неделю менять прослушанное на новое в том же духе.
    #[serde(default)]
    pub weekly: bool,
}

/// Плейлист, собранный по описанию.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DescribedPlaylist {
    pub playlist_id: Uuid,
    pub brief: DescribeBrief,
    pub weekly: bool,
    /// Когда неделя обновила его в последний раз.
    #[schema(required = true)]
    pub refreshed_at: Option<DateTime<Utc>>,
}

/// Настройки сохранённого плейлиста по описанию.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescribedPatch {
    pub weekly: bool,
}

/// «Ещё в том же духе» для сохранённого плейлиста: сколько добавить в конец.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescribedMore {
    /// 1–30, по умолчанию 10.
    #[serde(default)]
    pub count: Option<u32>,
}

/// Добавленное «Ещё в том же духе»: id треков по порядку.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DescribedAdded {
    pub track_ids: Vec<Uuid>,
}
