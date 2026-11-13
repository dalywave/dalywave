//! Настройки слушателя, общие для всех его устройств (`GET/PATCH /api/settings`): фильтр нецензурного,
//! качество звука по типу сети и порядок плейлистов. Приложения получают их и синхронизацией.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Качество по Wi-Fi: «Высокое» — lossless, если он есть у записи, иначе лучший файл; «Обычное» — Opus
/// 130–160 кбит/с.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WifiQuality {
    #[default]
    High,
    Normal,
}

names!(WifiQuality { High => "high", Normal => "normal" });

/// Качество по сотовой сети: «Обычное» или «Экономия» — Opus 64 кбит/с.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CellularQuality {
    #[default]
    Normal,
    Saver,
}

names!(CellularQuality { Normal => "normal", Saver => "saver" });

/// Качество загрузок на устройство.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DownloadQuality {
    #[default]
    High,
    Normal,
}

names!(DownloadQuality { High => "high", Normal => "normal" });

/// Порядок плейлистов в медиатеке и боковой панели. Закреплённые — всегда первыми.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistSort {
    /// Недавно игравшие или изменённые — первыми.
    #[default]
    Recent,
    /// По названию.
    Title,
    /// Свой порядок: перетаскиванием.
    Custom,
}

names!(PlaylistSort { Recent => "recent", Title => "title", Custom => "custom" });

/// «Подхват на другом устройстве»: наушники перешли с компьютера на телефон (или телефон открыли, пока
/// компьютер заблокирован) — музыка за ними. По умолчанию — предложение: ложный перенос (наушники просто
/// сели) останавливает музыку там, где её слушали, а предложение стоит одного нажатия.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandoffMode {
    Off,
    /// «Продолжить здесь?» на устройстве, куда ушли наушники.
    #[default]
    Suggest,
    /// Сразу: музыка переходит сама, если наушники совпали по имени.
    Auto,
}

names!(HandoffMode { Off => "off", Suggest => "suggest", Auto => "auto" });

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    /// «Без нецензурного»: explicit-треки не играют в волне, миксах и автопродолжении, в списках — серым;
    /// если в каталоге есть чистая версия, играет она.
    pub hide_explicit: bool,
    pub quality_wifi: WifiQuality,
    pub quality_cellular: CellularQuality,
    pub quality_download: DownloadQuality,
    pub playlist_sort: PlaylistSort,
    /// Подхват музыки на другом устройстве (`connect.signal` → `connect.handoff`).
    #[serde(default)]
    pub handoff: HandoffMode,
    /// `null` — ни разу не меняли (всё по умолчанию).
    #[schema(required = true)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// Изменить часть настроек; не названные поля остаются как были.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserSettingsPatch {
    #[serde(default)]
    pub hide_explicit: Option<bool>,
    #[serde(default)]
    pub quality_wifi: Option<WifiQuality>,
    #[serde(default)]
    pub quality_cellular: Option<CellularQuality>,
    #[serde(default)]
    pub quality_download: Option<DownloadQuality>,
    #[serde(default)]
    pub playlist_sort: Option<PlaylistSort>,
    #[serde(default)]
    pub handoff: Option<HandoffMode>,
}

/// Настройки в синхронизации приложений: одна строка, `rev` — как у строк медиатеки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncSettings {
    pub rev: i64,
    #[serde(flatten)]
    pub settings: UserSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_database() {
        let s = UserSettings::default();
        assert!(!s.hide_explicit);
        assert_eq!(s.quality_wifi.as_str(), "high");
        assert_eq!(s.quality_cellular.as_str(), "normal");
        assert_eq!(s.quality_download.as_str(), "high");
        assert_eq!(s.playlist_sort.as_str(), "recent");
        assert_eq!(s.handoff.as_str(), "suggest");
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["qualityCellular"], "normal");
        assert_eq!(json["playlistSort"], "recent");
    }

    #[test]
    fn patch_rejects_unknown_fields() {
        assert!(serde_json::from_str::<UserSettingsPatch>(r#"{"hideExplicit": true}"#).is_ok());
        assert!(serde_json::from_str::<UserSettingsPatch>(r#"{"bass": 9}"#).is_err());
        assert!(
            serde_json::from_str::<UserSettingsPatch>(r#"{"qualityCellular": "lossless"}"#)
                .is_err()
        );
    }
}
