//! «Как подобран»: карта вкуса для трека, который подобрала волна. Показ слушателя ([`super::why`]) →
//! след подбора ([`super::trace`]) → координаты на карте звучания ([`super::map`]): грани вкуса облаками,
//! что играло в сессии, варианты этого подбора и отсеянное, выбранный трек, соперники и причины.
//! Только свои показы: чужой показ — как не найденный.

use std::collections::{HashMap, HashSet};

use dalywave_protocol::wave::{
    ExplainCandidate, ExplainFacet, ExplainRival, ExplainStage, ExplainStep, WaveExplain,
    WaveFeedbackKind, WhyKind, WhyLine,
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::{map, why};
use crate::{
    catalog::views,
    error::{AppError, AppResult},
};

/// Треков сессии на линии: дальше она путает, а не объясняет.
const STEPS: i64 = 8;
/// Соперников выбранного.
const RIVALS: usize = 3;
/// Точек «пыли» — остального каталога.
const DUST: i64 = 1200;
/// Чипов причин.
const REASONS: usize = 4;
/// Вклад «что сейчас заходит» в оценку, который стоит назвать.
const HIT_NOTE: f64 = 0.05;

/// Строка «Почему этот трек» коротко: до тире («Похоже на Rushex — ты часто…» → «Похоже на Rushex»).
fn short(text: &str) -> String {
    text.split(" — ")
        .next()
        .unwrap_or(text)
        .split(": ")
        .next()
        .unwrap_or(text)
        .trim()
        .to_string()
}

/// Координаты треков на карте; у кого их ещё нет — сначала по соседям ([`map::fill`]).
async fn points(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, (f32, f32)>, sqlx::Error> {
    let missing: Vec<Uuid> = sqlx::query_scalar!(
        r#"SELECT a.recording_id FROM tracks t JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = ANY($1) AND a.sound IS NOT NULL AND a.map_x IS NULL"#,
        ids,
    )
    .fetch_all(db)
    .await?;
    if !missing.is_empty() {
        map::fill(db, Some(&missing), missing.len() as i64).await?;
    }
    Ok(sqlx::query!(
        r#"SELECT t.id, a.map_x AS "x!", a.map_y AS "y!" FROM tracks t
           JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = ANY($1) AND a.map_x IS NOT NULL AND a.map_y IS NOT NULL"#,
        ids,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, (r.x, r.y)))
    .collect())
}

/// Название и исполнители строкой.
async fn titles(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, (String, String)>, sqlx::Error> {
    Ok(sqlx::query!(
        r#"SELECT t.id, t.title,
                  array_to_string(ARRAY(SELECT ar.name FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id
                                        WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id), ', ') AS "artists!"
           FROM tracks t WHERE t.id = ANY($1)"#,
        ids,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, (r.title, r.artists)))
    .collect())
}

/// Строка варианта из следа: id, откуда, оценка, место в выдаче.
struct Variant {
    id: Uuid,
    origin: String,
    score: f32,
    pos: Option<u64>,
}

fn variants(trace: &Value) -> Vec<Variant> {
    trace
        .get("variants")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some(Variant {
                id: v.get(0)?.as_str()?.parse().ok()?,
                origin: v.get(1)?.as_str()?.to_string(),
                score: v.get(2)?.as_f64()? as f32,
                pos: v.get(5).and_then(Value::as_u64),
            })
        })
        .collect()
}

fn dropped(trace: &Value) -> Vec<(Uuid, String)> {
    trace
        .get("dropped")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some((
                v.get(0)?.as_str()?.parse().ok()?,
                v.get(1)?.as_str()?.to_string(),
            ))
        })
        .collect()
}

/// Карта вкуса для трека из последнего показа слушателю (в этой сессии волны, если она указана).
pub async fn explain(
    db: &PgPool,
    user: Uuid,
    track_id: Uuid,
    session: Option<Uuid>,
) -> AppResult<WaveExplain> {
    let imp = sqlx::query!(
        r#"SELECT i.wave_session_id, i.position, i.trace_id
           FROM recommendation_impressions i
           WHERE i.user_id = $1 AND i.track_id = $2 AND ($3::uuid IS NULL OR i.wave_session_id = $3)
           ORDER BY i.shown_at DESC, i.id DESC LIMIT 1"#,
        user,
        track_id,
        session,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    let trace: Option<Value> = match imp.trace_id {
        Some(id) => {
            sqlx::query_scalar!(
                "SELECT data FROM wave_traces WHERE id = $1 AND user_id = $2",
                id,
                user
            )
            .fetch_optional(db)
            .await?
        }
        None => None,
    };

    // Грани вкуса на карте; новые (после ночного пересчёта вкуса) — по соседям
    map::facets(db, Some(user), false).await?;
    let facet_rows = sqlx::query!(
        r#"SELECT id, label, weight, map_x AS "x!", map_y AS "y!", COALESCE(map_r, 0.05)::real AS "r!"
           FROM taste_facets WHERE user_id = $1 AND map_x IS NOT NULL AND map_y IS NOT NULL
           ORDER BY position"#,
        user,
    )
    .fetch_all(db)
    .await?;
    let facets: Vec<ExplainFacet> = facet_rows
        .into_iter()
        .map(|f| ExplainFacet {
            id: f.id,
            label: f.label,
            x: f.x,
            y: f.y,
            r: f.r,
            weight: f.weight,
        })
        .collect();
    // «f2» из следа → номер грани сейчас: по id, а после пересчёта вкуса — по подписи
    let trace_facets: Vec<(Option<Uuid>, String)> = trace
        .as_ref()
        .and_then(|t| t.get("facets"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|f| {
            (
                f.get(0)
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse().ok()),
                f.get(1).and_then(Value::as_str).unwrap_or("").to_string(),
            )
        })
        .collect();
    let facet_of = |origin: &str| -> Option<u32> {
        let i: usize = origin.strip_prefix('f')?.parse().ok()?;
        let (id, label) = trace_facets.get(i)?;
        facets
            .iter()
            .position(|f| Some(f.id) == *id)
            .or_else(|| facets.iter().position(|f| &f.label == label))
            .map(|p| p as u32)
    };

    // Что играло в сессии до этого трека
    let steps = match (imp.wave_session_id, imp.position) {
        (Some(sid), Some(pos)) => sqlx::query!(
            r#"SELECT track_id, outcome FROM recommendation_impressions
               WHERE wave_session_id = $1 AND user_id = $2 AND position < $3
               ORDER BY position DESC, shown_at DESC LIMIT $4"#,
            sid,
            user,
            pos,
            STEPS,
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .rev()
        .map(|r| (r.track_id, r.outcome))
        .collect(),
        _ => Vec::new(),
    };

    let vs = trace.as_ref().map(variants).unwrap_or_default();
    let ds = trace.as_ref().map(dropped).unwrap_or_default();
    let mut ids: Vec<Uuid> = vec![track_id];
    ids.extend(steps.iter().map(|s| s.0));
    ids.extend(vs.iter().map(|v| v.id));
    ids.extend(ds.iter().map(|d| d.0));
    let unique: Vec<Uuid> = {
        let mut seen = HashSet::new();
        ids.into_iter().filter(|i| seen.insert(*i)).collect()
    };
    let at = points(db, &unique).await?;
    let Some(&(x, y)) = at.get(&track_id) else {
        // Трек без анализа звучания: на карте его нет
        return Err(AppError::NotFound);
    };

    let mut rivals: Vec<&Variant> = vs
        .iter()
        .filter(|v| v.id != track_id && at.contains_key(&v.id))
        .collect();
    rivals.sort_by(|a, b| b.score.total_cmp(&a.score));
    rivals.truncate(RIVALS);
    let named: Vec<Uuid> = steps
        .iter()
        .map(|s| s.0)
        .chain(rivals.iter().map(|r| r.id))
        .collect();
    let names = titles(db, &named).await?;
    let name = |id: &Uuid| names.get(id).cloned().unwrap_or_default();

    let session_line: Vec<ExplainStep> = steps
        .iter()
        .filter_map(|(id, outcome)| {
            let (x, y) = *at.get(id)?;
            let (title, artists) = name(id);
            Some(ExplainStep {
                track_id: *id,
                title,
                artists,
                x,
                y,
                outcome: outcome
                    .as_deref()
                    .and_then(|o| o.parse::<WaveFeedbackKind>().ok()),
            })
        })
        .collect();
    let mut candidates: Vec<ExplainCandidate> = vs
        .iter()
        .filter(|v| v.id != track_id)
        .filter_map(|v| {
            let (x, y) = *at.get(&v.id)?;
            Some(ExplainCandidate {
                track_id: v.id,
                x,
                y,
                stage: if v.pos.is_some() {
                    ExplainStage::Picked
                } else {
                    ExplainStage::Ranked
                },
                facet: facet_of(&v.origin),
                score: Some(v.score),
            })
        })
        .collect();
    candidates.extend(ds.iter().filter_map(|(id, origin)| {
        let (x, y) = *at.get(id)?;
        Some(ExplainCandidate {
            track_id: *id,
            x,
            y,
            stage: ExplainStage::Dropped,
            facet: facet_of(origin),
            score: None,
        })
    }));
    let rivals: Vec<ExplainRival> = rivals
        .into_iter()
        .filter_map(|r| {
            let (x, y) = *at.get(&r.id)?;
            let (title, artists) = name(&r.id);
            Some(ExplainRival {
                track_id: r.id,
                title,
                artists,
                x,
                y,
            })
        })
        .collect();
    let facet = vs
        .iter()
        .find(|v| v.id == track_id)
        .and_then(|v| facet_of(&v.origin));

    // Причины: строки «Почему этот трек» коротко + «похоже на то, что зашло сейчас» из слагаемых оценки
    let why = why::explain(db, user, track_id, imp.wave_session_id, false).await?;
    let mut reasons: Vec<WhyLine> = why
        .lines
        .iter()
        .map(|l| WhyLine {
            kind: l.kind,
            text: short(&l.text),
        })
        .collect();
    let hit = trace
        .as_ref()
        .and_then(|t| t.pointer(&format!("/picks/{track_id}/parts/hit")))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if hit >= HIT_NOTE && !reasons.iter().any(|l| l.kind == WhyKind::Recent) {
        reasons.push(WhyLine {
            kind: WhyKind::Recent,
            text: "Похоже на то, что зашло сейчас".into(),
        });
    }
    let mut seen = HashSet::new();
    reasons.retain(|l| seen.insert(l.text.clone()));
    reasons.truncate(REASONS);

    let dust: Vec<f32> = sqlx::query!(
        r#"SELECT map_x AS "x!", map_y AS "y!" FROM track_analysis
           WHERE map_x IS NOT NULL AND map_y IS NOT NULL ORDER BY md5(recording_id::text) LIMIT $1"#,
        DUST,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .flat_map(|r| [r.x, r.y])
    .collect();
    let considered = trace
        .as_ref()
        .and_then(|t| t.get("considered"))
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let track = views::tracks(db, &[track_id], user)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)?;
    Ok(WaveExplain {
        track,
        x,
        y,
        facet,
        facets,
        session: session_line,
        candidates,
        rivals,
        reasons,
        considered,
        dust,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_reason() {
        assert_eq!(
            short("Похоже на Pyrokinesis — ты часто слушаешь этого артиста"),
            "Похоже на Pyrokinesis"
        );
        assert_eq!(short("Из твоих лайков"), "Из твоих лайков");
        assert_eq!(
            short("Новое для тебя: ни этот трек, ни артиста ты раньше не слушал"),
            "Новое для тебя"
        );
    }
}
