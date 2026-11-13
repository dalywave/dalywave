//! Настройки админки: клипы и переменные окружения (только чтение).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Высоты клипа, которые можно выбрать.
pub const CLIP_HEIGHTS: [i32; 4] = [360, 480, 720, 1080];

/// Клипы вместо обложки: качество, что качать, сроки хранения и потолок места.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipSettings {
    /// Высота кадра: 360, 480, 720 или 1080.
    pub height: i32,
    /// Клип целиком, а не кусок с припевом.
    pub whole: bool,
    /// Клип целиком идёт в такт с треком: сервер сопоставляет звук клипа с треком. Не вышло —
    /// кусок с припевом по кругу.
    pub sync: bool,
    /// Длина куска с припевом, с (6–30).
    pub length_s: i32,
    /// Сколько дней клип живёт с последнего показа.
    pub keep_days: i32,
    /// То же для треков в любимых.
    pub keep_liked_days: i32,
    /// Потолок места под клипы, ГБ.
    pub max_gb: i32,
    /// Потолок задан переменной `DALYWAVE_CLIPS_MAX_GB`: здесь не меняется.
    pub max_gb_from_env: bool,
    /// Искать клипы сами, когда трек показывается. Выкл. — только выбранные администратором.
    pub auto_search: bool,
    /// Сколько клипов в хранилище сейчас и сколько они занимают.
    pub stored: i64,
    pub stored_bytes: i64,
    /// Средний размер готового клипа при нынешних настройках, байт (по уже нарезанным; нет —
    /// оценка по битрейту).
    pub avg_bytes: i64,
}

/// Изменение настроек клипов: только переданные поля.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipSettingsPatch {
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub whole: Option<bool>,
    #[serde(default)]
    pub sync: Option<bool>,
    #[serde(default)]
    pub length_s: Option<i32>,
    #[serde(default)]
    pub keep_days: Option<i32>,
    #[serde(default)]
    pub keep_liked_days: Option<i32>,
    #[serde(default)]
    pub max_gb: Option<i32>,
    #[serde(default)]
    pub auto_search: Option<bool>,
}

/// Раздел настроек, к которому относится переменная окружения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSection {
    General,
    SignIn,
    Music,
    Storage,
    Clips,
    Reco,
    Notifications,
    Integrations,
}

/// Настройка, заданная переменной окружения: видна, но меняется только на сервере.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvSetting {
    /// Имя переменной: `DALYWAVE_PUBLIC_URL`.
    pub name: String,
    pub section: SettingsSection,
    /// Переменная задана.
    pub set: bool,
    /// Значение; у секретов — `null` (видно только, задан ли).
    #[schema(required = true)]
    pub value: Option<String>,
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvSettings {
    pub items: Vec<EnvSetting>,
}
