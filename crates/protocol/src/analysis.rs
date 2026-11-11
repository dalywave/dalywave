//! Анализ трека для переходов «Диджей» (`GET /api/tracks/{id}/analysis`).
//!
//! Плеер берёт его вместе со следующим треком: сетка долей, фразы и предложенные точки сведения.
//! Имена полей совпадают с `dalywave_transitions::Analysis` — приложение разбирает ответ прямо в
//! него. Всё — по полному файлу записи (`fileId`): у анализа превью сетка не та.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Предложенные точки сведения: с какой сильной доли входить в трек и где из него выходить.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisCues {
    pub mix_in_ms: i32,
    pub mix_out_ms: i32,
}

/// Ответ `GET /api/tracks/{id}/analysis`. Списки — пустые, а не `null`, если данных нет.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackAnalysis {
    pub track_id: Uuid,
    /// Файл, к которому привязана сетка: не совпал с `fileId` потока — сетка от другого файла.
    #[schema(required = true)]
    pub file_id: Option<Uuid>,
    #[schema(required = true)]
    pub duration_ms: Option<i32>,
    #[schema(required = true)]
    pub bpm: Option<f32>,
    /// Уверенность модели в сетке долей, 0…1.
    #[schema(required = true)]
    pub bpm_confidence: Option<f32>,
    pub beats_ms: Vec<i32>,
    pub downbeats_ms: Vec<i32>,
    /// Начала фраз по 8 тактов.
    pub phrases_ms: Vec<i32>,
    /// Начала фраз по 16 тактов (подмножество `phrasesMs`).
    pub major_phrases_ms: Vec<i32>,
    /// 8A, 11B …
    #[schema(required = true)]
    pub key_camelot: Option<String>,
    #[schema(required = true)]
    pub key_confidence: Option<f32>,
    #[schema(required = true)]
    pub energy: Option<f32>,
    /// Энергия по секундам, 0…1000.
    pub energy_curve: Vec<i16>,
    /// Участки с вокалом: `[начало, конец]` в мс.
    #[schema(value_type = Vec<Vec<i32>>)]
    pub vocal_segments: Vec<[i32; 2]>,
    #[schema(required = true)]
    pub intro_end_ms: Option<i32>,
    #[schema(required = true)]
    pub outro_start_ms: Option<i32>,
    /// Нет — сетка слишком короткая, чтобы предлагать точки.
    #[schema(required = true)]
    pub cues: Option<AnalysisCues>,
}
