//! Анализ звука: полный — для скачанного (полоса «Анализ звука», задача `analyze_full`), по превью Deezer —
//! для индекса каталога (полоса «Разведка каталога», `analyze_preview`). Полный анализ заменяет анализ
//! превью, превью полный не перезаписывает.
//!
//! Файл скачанного сервис анализа берёт по подписанной ссылке хранилища, превью — по свежей ссылке Deezer
//! (она подписана и живёт недолго, поэтому берётся прямо перед анализом). Сервис недоступен или занят —
//! задача откладывается, не тратя попытку, а воркер на минуту перестаёт брать анализы.

use std::{sync::Arc, time::Duration};

use dalywave_protocol::jobs::{AnalyzeFullPayload, AnalyzePreviewPayload, kind};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{retry_deadlocks, scout, vectors::camelot};
use crate::{
    acquire, catalog,
    deezer::DeezerError,
    jobs::{
        self, Enqueued, JobRow, NewJob, lanes,
        worker::{Context, Done, JobError, JobResult, Outcome},
    },
    ml::{Analysis, MlError, Mode},
    storage::ML_GET_TTL,
};

/// Пока сервис анализа недоступен, анализы ждут столько.
pub const SERVICE_PAUSE: Duration = Duration::from_secs(60);
/// Превью, которого ждёт человек (радио от трека, похожие): раньше разведки.
pub const ON_DEMAND_PRIORITY: i16 = 50;
/// Полный анализ скачанного — раньше задач, поставленных догоняющим проходом.
const FULL_PRIORITY: i16 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Preview,
    Full,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Full => "full",
        }
    }
}

/// Строка `track_analysis` из ответа сервиса: числа проверены и приведены к диапазонам.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnalysisRow {
    pub models: Value,
    pub duration_ms: Option<i32>,
    pub sound: Option<Vec<f32>>,
    pub semantic: Option<Vec<f32>>,
    pub bpm: Option<f32>,
    pub bpm_confidence: Option<f32>,
    pub key_camelot: Option<String>,
    pub key_confidence: Option<f32>,
    pub energy: Option<f32>,
    pub danceability: Option<f32>,
    pub valence: Option<f32>,
    pub moods: Option<Value>,
    pub voice: Option<f32>,
    pub language: Option<String>,
    pub styles: Option<Value>,
    pub approachability: Option<f32>,
    pub engagement: Option<f32>,
    pub intro_end_ms: Option<i32>,
    pub outro_start_ms: Option<i32>,
    pub beats_ms: Option<Vec<i32>>,
    pub downbeats_ms: Option<Vec<i32>>,
    pub vocal_segments: Option<Value>,
    pub energy_curve: Option<Vec<i16>>,
    pub extra: Value,
}

fn unit(x: Option<f64>) -> Option<f32> {
    x.filter(|v| v.is_finite())
        .map(|v| v.clamp(0.0, 1.0) as f32)
}

fn ms(x: Option<f64>) -> Option<i32> {
    x.filter(|v| v.is_finite() && *v >= 0.0 && *v < f64::from(i32::MAX))
        .map(|v| v.round() as i32)
}

fn ms_list(x: Option<&[f64]>) -> Option<Vec<i32>> {
    x.map(|v| v.iter().filter_map(|t| ms(Some(*t))).collect())
}

impl AnalysisRow {
    /// Проверяет ответ сервиса. Ошибка — векторы не той длины или не числа: такой ответ не сохраняется.
    pub fn from_ml(a: &Analysis) -> Result<Self, MlError> {
        let extra = a
            .extra
            .clone()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        let head = |key: &str| unit(extra.get(key).and_then(Value::as_f64));
        Ok(Self {
            models: if a.models.is_object() {
                a.models.clone()
            } else {
                json!({})
            },
            duration_ms: ms(a.duration_ms),
            sound: a.sound_vector()?,
            semantic: a.semantic_vector()?,
            bpm: a
                .bpm
                .filter(|b| b.is_finite() && (30.0..=300.0).contains(b))
                .map(|b| b as f32),
            bpm_confidence: unit(a.bpm_confidence),
            key_camelot: a
                .key_camelot
                .as_deref()
                .and_then(camelot)
                .map(|(n, l)| format!("{n}{l}")),
            key_confidence: unit(a.key_confidence),
            energy: unit(a.energy),
            danceability: unit(a.danceability),
            valence: unit(a.valence),
            moods: a.moods.clone().filter(Value::is_object),
            voice: unit(a.voice),
            language: a
                .language
                .as_deref()
                .map(|l| l.trim().to_ascii_lowercase())
                .filter(|l| !l.is_empty() && l.len() <= 8),
            styles: a.styles.clone().filter(Value::is_array).map(|s| match s {
                Value::Array(mut items) => {
                    items.truncate(10);
                    Value::Array(items)
                }
                other => other,
            }),
            approachability: head("approachability"),
            engagement: head("engagement"),
            intro_end_ms: ms(a.intro_end_ms),
            outro_start_ms: ms(a.outro_start_ms),
            beats_ms: ms_list(a.beats_ms.as_deref()),
            downbeats_ms: ms_list(a.downbeats_ms.as_deref()),
            vocal_segments: a.vocal_segments.clone().filter(Value::is_array),
            energy_curve: a.energy_curve.as_ref().map(|c| {
                c.iter()
                    .map(|v| {
                        if v.is_finite() {
                            v.round().clamp(0.0, 1000.0) as i16
                        } else {
                            0
                        }
                    })
                    .collect()
            }),
            extra,
        })
    }
}

/// Записывает анализ. Превью не перезаписывает полный анализ: тогда `false`.
pub async fn store(
    conn: &mut PgConnection,
    recording_id: Uuid,
    source: Source,
    row: &AnalysisRow,
    file_id: Option<Uuid>,
) -> Result<bool, sqlx::Error> {
    let stored = sqlx::query_scalar!(
        r#"INSERT INTO track_analysis (recording_id, source, file_id, models, duration_ms, sound, semantic, bpm,
                                       bpm_confidence, key_camelot, key_confidence, energy, danceability, valence,
                                       moods, voice, language, styles, approachability, engagement, intro_end_ms,
                                       outro_start_ms, beats_ms, downbeats_ms, vocal_segments, energy_curve, extra)
           VALUES ($1, $2, $3, $4, $5, $6::real[]::halfvec(1280), $7::real[]::halfvec(512), $8, $9, $10, $11, $12,
                   $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27)
           ON CONFLICT (recording_id) DO UPDATE SET
               source = EXCLUDED.source, file_id = EXCLUDED.file_id, models = EXCLUDED.models,
               duration_ms = EXCLUDED.duration_ms, sound = EXCLUDED.sound, semantic = EXCLUDED.semantic,
               bpm = EXCLUDED.bpm, bpm_confidence = EXCLUDED.bpm_confidence, key_camelot = EXCLUDED.key_camelot,
               key_confidence = EXCLUDED.key_confidence, energy = EXCLUDED.energy,
               danceability = EXCLUDED.danceability, valence = EXCLUDED.valence, moods = EXCLUDED.moods,
               voice = EXCLUDED.voice, language = EXCLUDED.language, styles = EXCLUDED.styles,
               approachability = EXCLUDED.approachability, engagement = EXCLUDED.engagement,
               intro_end_ms = EXCLUDED.intro_end_ms, outro_start_ms = EXCLUDED.outro_start_ms,
               beats_ms = EXCLUDED.beats_ms, downbeats_ms = EXCLUDED.downbeats_ms,
               vocal_segments = EXCLUDED.vocal_segments, energy_curve = EXCLUDED.energy_curve,
               extra = EXCLUDED.extra, analyzed_at = now()
           WHERE track_analysis.source = 'preview' OR EXCLUDED.source = 'full'
           RETURNING recording_id"#,
        recording_id,
        source.as_str(),
        file_id,
        row.models,
        row.duration_ms,
        row.sound.as_deref(),
        row.semantic.as_deref(),
        row.bpm,
        row.bpm_confidence,
        row.key_camelot,
        row.key_confidence,
        row.energy,
        row.danceability,
        row.valence,
        row.moods,
        row.voice,
        row.language,
        row.styles,
        row.approachability,
        row.engagement,
        row.intro_end_ms,
        row.outro_start_ms,
        row.beats_ms.as_deref(),
        row.downbeats_ms.as_deref(),
        row.vocal_segments,
        row.energy_curve.as_deref(),
        row.extra,
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(stored.is_some())
}

/// Полный анализ записи, у которой появился файл. Одна задача на запись: повтор безопасен.
pub async fn request_full(
    conn: &mut PgConnection,
    recording_id: Uuid,
) -> Result<Enqueued, sqlx::Error> {
    jobs::enqueue(
        conn,
        NewJob::new(
            kind::ANALYZE_FULL,
            lanes::ANALYSIS,
            AnalyzeFullPayload { recording_id },
        )
        .dedupe(format!("analyze_full:{recording_id}"))
        .priority(FULL_PRIORITY),
    )
    .await
}

/// Анализ превью трека для индекса. `priority` — [`ON_DEMAND_PRIORITY`], если его ждёт человек.
pub async fn request_preview(
    conn: &mut PgConnection,
    track_id: Uuid,
    priority: i16,
) -> Result<Enqueued, sqlx::Error> {
    jobs::enqueue(
        conn,
        NewJob::new(
            kind::ANALYZE_PREVIEW,
            lanes::SCOUT,
            AnalyzePreviewPayload { track_id },
        )
        .dedupe(format!("analyze_preview:{track_id}"))
        .priority(priority),
    )
    .await
}

/// Скачанное без полного анализа (или проанализированное по другому файлу): догоняющие задачи.
pub async fn schedule_backfill(db: &PgPool, limit: i64) -> Result<usize, sqlx::Error> {
    let recordings = sqlx::query_scalar!(
        r#"SELECT r.id FROM recordings r
           LEFT JOIN track_analysis a ON a.recording_id = r.id
           WHERE r.audio_status = 'ready' AND r.primary_file_id IS NOT NULL
             AND (a.recording_id IS NULL OR a.source = 'preview' OR a.file_id IS DISTINCT FROM r.primary_file_id)
             AND NOT EXISTS (SELECT 1 FROM jobs j WHERE j.dedupe_key = 'analyze_full:' || r.id
                               AND j.status = 'failed' AND j.finished_at > now() - interval '7 days')
           ORDER BY r.status_changed_at DESC
           LIMIT $1"#,
        limit,
    )
    .fetch_all(db)
    .await?;
    let mut created = 0;
    let mut conn = db.acquire().await?;
    for id in recordings {
        created += usize::from(request_full(&mut conn, id).await?.created);
    }
    Ok(created)
}

/// Сбой сервиса анализа. Сервис лежит или занят — ждём его, не тратя попытку, и минуту не берём
/// другие анализы; ссылка не скачалась — повтор; файл не разобрать — окончательно.
async fn ml_failed(ctx: &Context, job: &JobRow, e: MlError) -> JobResult {
    if e.is_service() {
        ctx.cooldowns.pause(kind::ANALYZE_FULL, SERVICE_PAUSE);
        ctx.cooldowns.pause(kind::ANALYZE_PREVIEW, SERVICE_PAUSE);
        let lock = job.locked_by.clone().unwrap_or_default();
        let message = e.to_string();
        tracing::warn!(error = %message, "ml service is not available, analysis postponed");
        let postponed = jobs::postpone(&ctx.db, job.id, &lock, &message, SERVICE_PAUSE).await?;
        return Ok(Done::Settled(match postponed {
            Some(_) => Outcome::Retry(message),
            None => Outcome::Lost,
        }));
    }
    if e.is_temporary() {
        Err(JobError::retry(e))
    } else {
        Err(JobError::permanent(e))
    }
}

fn summary(source: Source, row: &AnalysisRow, a: &Analysis) -> Value {
    json!({
        "source": source.as_str(),
        "bpm": row.bpm,
        "key": row.key_camelot,
        "energy": row.energy,
        "valence": row.valence,
        "voice": row.voice,
        "totalMs": a.timings_ms.as_ref().and_then(|t| t.get("total")).cloned(),
    })
}

/// Задача `analyze_full`: файл записи по подписанной ссылке → полный анализ.
pub async fn analyze_full(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: AnalyzeFullPayload = job.payload()?;
    let Some(file) = acquire::primary_file(&ctx.db, p.recording_id).await? else {
        return Err(JobError::permanent("у записи нет готового файла"));
    };
    let done = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM track_analysis
                          WHERE recording_id = $1 AND source = 'full' AND file_id = $2) AS "done!""#,
        p.recording_id,
        file.id,
    )
    .fetch_one(&ctx.db)
    .await?;
    if done {
        return Ok(json!({ "skipped": "analyzed" }).into());
    }
    let url = ctx
        .storage
        .presign_get_for_ml(&file.storage_key, ML_GET_TTL)
        .await?;
    let analysis = match ctx.ml.analyze(url.as_str(), Mode::Full).await {
        Ok(a) => a,
        Err(e) => return ml_failed(&ctx, &job, e).await,
    };
    let row = AnalysisRow::from_ml(&analysis).map_err(JobError::retry)?;
    let mut tx = ctx.db.begin().await?;
    store(&mut tx, p.recording_id, Source::Full, &row, Some(file.id)).await?;
    scout::mark_analyzed(&mut tx, p.recording_id).await?;
    tx.commit().await?;
    Ok(summary(Source::Full, &row, &analysis).into())
}

/// Задача `analyze_preview`: свежая ссылка на превью у Deezer → анализ превью → индекс каталога.
pub async fn analyze_preview(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: AnalyzePreviewPayload = job.payload()?;
    let row = sqlx::query!(
        r#"SELECT t.deezer_id, a.source AS "source?" FROM tracks t
           LEFT JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = $1"#,
        p.track_id,
    )
    .fetch_optional(&ctx.db)
    .await?;
    let Some(row) = row else {
        return Err(JobError::permanent("нет такого трека"));
    };
    if row.source.is_some() {
        scout::mark_track(&ctx.db, p.track_id, None).await?;
        return Ok(json!({ "skipped": "analyzed" }).into());
    }
    let Some(deezer_id) = row.deezer_id else {
        scout::mark_track(&ctx.db, p.track_id, Some("no_deezer")).await?;
        return Ok(json!({ "skipped": "no_deezer" }).into());
    };
    // Ссылка на превью подписана и живёт недолго: берём свежую прямо перед анализом
    let track = match ctx.deezer.track(deezer_id).await {
        Ok(t) => t,
        Err(DeezerError::NotFound) => {
            scout::mark_track(&ctx.db, p.track_id, Some("gone")).await?;
            return Ok(json!({ "skipped": "gone" }).into());
        }
        Err(e) => return Err(e.into()),
    };
    let track_id = retry_deadlocks(|| catalog::save_track(&ctx.db, &track)).await?;
    let Some(preview) = track.preview.clone() else {
        scout::mark_track(&ctx.db, track_id, Some("no_preview")).await?;
        return Ok(json!({ "skipped": "no_preview" }).into());
    };
    let recording = sqlx::query!(
        r#"SELECT t.recording_id, a.source AS "source?" FROM tracks t
           LEFT JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = $1"#,
        track_id,
    )
    .fetch_one(&ctx.db)
    .await?;
    let Some(recording_id) = recording.recording_id else {
        scout::mark_track(&ctx.db, track_id, Some("no_recording")).await?;
        return Ok(json!({ "skipped": "no_recording" }).into());
    };
    // Тот же ISRC у другого трека — запись уже разобрана
    if recording.source.is_some() {
        scout::mark_track(&ctx.db, track_id, None).await?;
        return Ok(json!({ "skipped": "analyzed" }).into());
    }
    let analysis = match ctx.ml.analyze(&preview, Mode::Preview).await {
        Ok(a) => a,
        Err(e) => return ml_failed(&ctx, &job, e).await,
    };
    let row = AnalysisRow::from_ml(&analysis).map_err(JobError::retry)?;
    let mut tx = ctx.db.begin().await?;
    store(&mut tx, recording_id, Source::Preview, &row, None).await?;
    scout::mark_analyzed(&mut tx, recording_id).await?;
    tx.commit().await?;
    Ok(summary(Source::Preview, &row, &analysis).into())
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::STANDARD};

    use super::*;

    #[test]
    fn analysis_is_checked_and_clamped() {
        let ones = STANDARD.encode(
            std::iter::repeat_n(0x3c00u16.to_le_bytes(), 1280)
                .flatten()
                .collect::<Vec<u8>>(),
        );
        let a: Analysis = serde_json::from_value(json!({
            "source": "full", "models": {"sound": "effnet"}, "durationMs": 238514.4,
            "sound": ones, "bpm": 106.64, "bpmConfidence": 1.7, "keyCamelot": "10a",
            "energy": -0.2, "valence": 0.6, "voice": 0.88, "language": "RU",
            "styles": [{"name": "Hip Hop---Trap", "p": 0.3}], "beatsMs": [20.4, 1380, -5],
            "energyCurve": [278.6, 1500, -3], "extra": {"approachability": 0.54, "engagement": 0.81}
        }))
        .unwrap();
        let row = AnalysisRow::from_ml(&a).unwrap();
        assert_eq!(row.sound.as_ref().unwrap().len(), 1280);
        assert!(row.semantic.is_none());
        assert_eq!(row.duration_ms, Some(238514));
        assert_eq!(row.bpm_confidence, Some(1.0));
        assert_eq!(row.key_camelot.as_deref(), Some("10A"));
        assert_eq!(row.energy, Some(0.0));
        assert_eq!(row.language.as_deref(), Some("ru"));
        assert_eq!(row.beats_ms, Some(vec![20, 1380]));
        assert_eq!(row.energy_curve, Some(vec![279, 1000, 0]));
        assert_eq!(row.approachability, Some(0.54));
        assert_eq!(row.models, json!({"sound": "effnet"}));
        let bad: Analysis = serde_json::from_value(json!({"sound": "AAAA"})).unwrap();
        assert!(AnalysisRow::from_ml(&bad).is_err());
    }
}
