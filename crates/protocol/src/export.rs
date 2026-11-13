//! Выгрузка медиатеки в файл и встраиваемый плеер для чужих сайтов.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Формат выгрузки.
///
/// `csv` — таблица «Артист, Трек, Альбом, ISRC, Длительность, Добавлено» (UTF-8 с BOM: её понимают
/// Excel, TuneMyMusic и Soundiiz); `m3u8` — список для плееров со ссылками на страницы треков;
/// `json` — всё с порядком, папками и датами; `zip` — вся медиатека отдельными файлами.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    M3u8,
    Json,
    Zip,
}

names!(ExportFormat { Csv => "csv", M3u8 => "m3u8", Json => "json", Zip => "zip" });

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        self.as_str()
    }

    pub fn mime(self) -> &'static str {
        match self {
            ExportFormat::Csv => "text/csv; charset=utf-8",
            ExportFormat::M3u8 => "audio/x-mpegurl; charset=utf-8",
            ExportFormat::Json => "application/json; charset=utf-8",
            ExportFormat::Zip => "application/zip",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct ExportQuery {
    /// Формат файла. Вся медиатека — `zip` (по умолчанию) или `json`; список — `csv`, `m3u8`, `json`.
    pub format: Option<ExportFormat>,
    /// Вся медиатека: добавить историю прослушиваний.
    #[serde(default)]
    pub history: bool,
}

/// Можно ли встраивать плейлист на чужие сайты (`/embed/playlist/{id}`). По умолчанию — нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmbedSettings {
    pub embeddable: bool,
}

/// Откуда отрывок встраиваемого плеера.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClipSource {
    /// 30 секунд с припева из звука на сервере.
    Chorus,
    /// Превью Deezer: пока отрывок с припева не нарезан или звука на сервере нет.
    Preview,
}

/// Отрывок для встраиваемого плеера: ссылка живёт недолго, полный трек без входа не отдаётся.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmbedClip {
    pub url: String,
    pub source: ClipSource,
    pub length_ms: i64,
}
