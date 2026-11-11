//! Анализ трека для переходов «Диджей»: сетка долей, фразы и точки сведения. Плеер берёт его
//! заранее, вместе со следующим треком. Фразы и точки считаются тем же кодом, что и в аудиодвижке
//! приложений (`dalywave-transitions`), поэтому сервер и плеер не расходятся.

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use dalywave_protocol::{
    ErrorBody,
    analysis::{AnalysisCues, TrackAnalysis},
};
use dalywave_transitions::Analysis;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    extract::{Json, Path},
    state::AppState,
};

/// Версия расчёта фраз и точек в ETag: меняется вместе с алгоритмом, иначе кеш приложений отвечал бы
/// 304 со старыми фразами при том же `rev` анализа.
const ETAG_VERSION: u32 = 1;

const CACHE: &str = "private, max-age=3600";

pub fn routes() -> Router<AppState> {
    Router::new().route("/tracks/{id}/analysis", get(track_analysis))
}

/// Округление до целых мс; `as` насыщается, так что мусорные значения не роняют обработчик.
fn ms(x: f64) -> i32 {
    x.round() as i32
}

fn ms_list(v: &[f64]) -> Vec<i32> {
    v.iter().copied().map(ms).collect()
}

/// `vocal_segments` — jsonb от сервиса анализа: берутся только пары чисел с концом после начала.
fn vocal_segments(v: Option<&Value>) -> Vec<[f64; 2]> {
    let Some(items) = v.and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|seg| {
            let pair = seg.as_array()?;
            let (a, b) = (pair.first()?.as_f64()?, pair.get(1)?.as_f64()?);
            (a.is_finite() && b.is_finite() && b > a).then_some([a, b])
        })
        .collect()
}

/// Слабый ETag: ответ проходит через сжатие, байты зависят от кодировки.
fn etag(rev: i64) -> String {
    format!("W/\"{ETAG_VERSION}.{rev}\"")
}

fn not_modified(headers: &HeaderMap, etag: &str) -> bool {
    let bare = |t: &str| t.trim().trim_start_matches("W/").to_owned();
    let ours = bare(etag);
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|t| t.trim() == "*" || bare(t) == ours)
}

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/analysis",
    tag = "playback",
    security(("session" = []), ("device" = [])),
    params(
        ("id" = Uuid, Path, description = "id трека"),
        ("If-None-Match" = Option<String>, Header, description = "ETag прошлого ответа: не изменился — 304"),
    ),
    responses(
        (status = 200, description = "Анализ полного файла записи: доли, фразы по 8 и 16 тактов, точки сведения. \
                                      Кешируется на час (`private`), ETag меняется с анализом",
         body = TrackAnalysis,
         headers(("ETag" = String, description = "Версия анализа"), ("Cache-Control" = String))),
        (status = 304, description = "Анализ не изменился с переданного `If-None-Match`"),
        (status = 404, description = "Нет такого трека (`not_found`) или полного анализа его записи ещё нет \
                                      (`analysis_missing`: файл не скачан или не разобран)", body = ErrorBody),
    ),
)]
pub async fn track_analysis(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> AppResult<Response> {
    // Анализ превью не годится: его сетка — от 30-секундного отрывка, а не от файла, который играет плеер
    let row = sqlx::query!(
        "SELECT a.rev, a.file_id, a.duration_ms, a.bpm, a.bpm_confidence, a.key_camelot, a.key_confidence,
                a.energy, a.intro_end_ms, a.outro_start_ms, a.beats_ms, a.downbeats_ms, a.vocal_segments,
                a.energy_curve
           FROM tracks t
           JOIN track_analysis a ON a.recording_id = t.recording_id
          WHERE t.id = $1 AND a.source = 'full'",
        id
    )
    .fetch_optional(&st.db)
    .await?;
    let Some(row) = row else {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM tracks WHERE id = $1) AS "exists!""#,
            id
        )
        .fetch_one(&st.db)
        .await?;
        return Err(if exists {
            AppError::Missing {
                code: "analysis_missing",
                message: "трек ещё не разобран по полному файлу".into(),
            }
        } else {
            AppError::NotFound
        });
    };

    let tag = etag(row.rev);
    let cache = [
        (header::CACHE_CONTROL, CACHE.to_owned()),
        (header::ETAG, tag.clone()),
    ];
    if not_modified(&headers, &tag) {
        return Ok((StatusCode::NOT_MODIFIED, cache).into_response());
    }

    let beats_ms = row.beats_ms.unwrap_or_default();
    let downbeats_ms = row.downbeats_ms.unwrap_or_default();
    let energy_curve = row.energy_curve.unwrap_or_default();
    let segments = vocal_segments(row.vocal_segments.as_ref());
    let analysis = Analysis {
        duration_ms: row.duration_ms.map(f64::from),
        bpm: row.bpm.map(f64::from),
        bpm_confidence: row.bpm_confidence.map(f64::from),
        beats_ms: beats_ms.iter().copied().map(f64::from).collect(),
        downbeats_ms: downbeats_ms.iter().copied().map(f64::from).collect(),
        energy_curve: energy_curve.iter().copied().map(f64::from).collect(),
        vocal_segments: segments.clone(),
        intro_end_ms: row.intro_end_ms.map(f64::from),
        outro_start_ms: row.outro_start_ms.map(f64::from),
        ..Default::default()
    };
    let phrases = analysis.phrases();
    let cues = analysis.cues().map(|c| AnalysisCues {
        mix_in_ms: ms(c.mix_in_ms),
        mix_out_ms: ms(c.mix_out_ms),
    });
    let body = TrackAnalysis {
        track_id: id,
        file_id: row.file_id,
        duration_ms: row.duration_ms,
        bpm: row.bpm,
        bpm_confidence: row.bpm_confidence,
        beats_ms,
        downbeats_ms,
        phrases_ms: ms_list(&phrases.starts),
        major_phrases_ms: ms_list(&phrases.major),
        key_camelot: row.key_camelot,
        key_confidence: row.key_confidence,
        energy: row.energy,
        energy_curve,
        vocal_segments: segments.iter().map(|[a, b]| [ms(*a), ms(*b)]).collect(),
        intro_end_ms: row.intro_end_ms,
        outro_start_ms: row.outro_start_ms,
        cues,
    };
    Ok((StatusCode::OK, cache, Json(body)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use serde_json::json;

    #[test]
    fn vocal_segments_skip_junk() {
        let v = json!([
            [1000, 5000],
            [7000.5, 6000],
            "x",
            [1],
            [null, 3],
            [8000, 9000, 1],
            {}
        ]);
        assert_eq!(
            vocal_segments(Some(&v)),
            vec![[1000.0, 5000.0], [8000.0, 9000.0]]
        );
        assert!(vocal_segments(Some(&json!({"a": 1}))).is_empty());
        assert!(vocal_segments(None).is_empty());
    }

    #[test]
    fn if_none_match_lists_and_weak_tags() {
        let tag = etag(42);
        let mut h = HeaderMap::new();
        assert!(!not_modified(&h, &tag));
        h.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_static("\"other\", \"1.42\""),
        );
        assert!(not_modified(&h, &tag));
        h.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_static("W/\"1.41\""),
        );
        assert!(!not_modified(&h, &tag));
        h.insert(header::IF_NONE_MATCH, HeaderValue::from_static("*"));
        assert!(not_modified(&h, &tag));
    }
}
