//! «Умное перемешивание», как Smart Shuffle в Spotify: треки, похожие по звучанию на весь список
//! (альбом, плейлист), — плеер подмешивает их в очередь примерно через три трека списка.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use dalywave_protocol::wave::{Reason, ReasonKind, WaveCharacter, WaveItem};
use serde_json::json;
use uuid::Uuid;

use super::{
    Cand, MODEL_VERSION, candidates, nearest_sound, rank,
    vectors::{normalized, weighted_mean},
    wave, widen_search,
};
use crate::{
    error::{AppResult, bad},
    state::AppState,
};

/// Треков списка, по которым ищется похожее, и исключений — не больше.
pub const MAX_IDS: usize = 500;
/// Не больше стольких предложений у одного артиста.
const PER_ARTIST: usize = 2;

pub const REASON: &str = "Подобрано";

/// Похожее на центр звучания списка: не из списка и не из `exclude`, можно сыграть, не заслушано,
/// не больше двух у артиста. Ни один трек списка не разобран — похожее на первый ([`wave::similar`]).
pub async fn for_list(
    st: &AppState,
    user: Uuid,
    track_ids: &[Uuid],
    exclude: &[Uuid],
    limit: usize,
) -> AppResult<Vec<WaveItem>> {
    if track_ids.is_empty() {
        return Err(bad("нужен хотя бы один трек списка"));
    }
    if track_ids.len() > MAX_IDS || exclude.len() > MAX_IDS {
        return Err(bad(format!(
            "не больше {MAX_IDS} треков списка и исключений"
        )));
    }
    let rows = sqlx::query!(
        r#"SELECT t.recording_id, a.sound::real[] AS "sound!" FROM tracks t
           JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = ANY($1) AND a.sound IS NOT NULL"#,
        track_ids,
    )
    .fetch_all(&st.db)
    .await?;
    let Some(center) = weighted_mean(rows.iter().map(|r| (r.sound.as_slice(), 1.0))) else {
        return wave::similar(st, user, track_ids[0], limit).await;
    };
    let center = normalized(center);

    let own: HashSet<Uuid> = track_ids.iter().chain(exclude).copied().collect();
    let own_recordings: HashSet<Uuid> = sqlx::query_scalar!(
        r#"SELECT recording_id AS "recording_id!" FROM tracks WHERE id = ANY($1) AND recording_id IS NOT NULL"#,
        &own.iter().copied().collect::<Vec<_>>(),
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .collect();

    let want = (limit * 4 + 40) as i64;
    let mut tx = st.db.begin().await?;
    widen_search(&mut tx, want).await?;
    let near = nearest_sound(&mut tx, &center, want, user).await?;
    tx.commit().await?;
    let ids: Vec<Uuid> = near.iter().map(|n| n.track_id).collect();

    let now = Utc::now();
    let mut picked: Vec<(Cand, Reason)> = Vec::new();
    let mut per_artist: HashMap<Uuid, usize> = HashMap::new();
    let mut seen_recordings: HashSet<Uuid> = HashSet::new();
    for c in candidates(&st.db, &ids, user, None, None).await? {
        if picked.len() >= limit {
            break;
        }
        let recording = c.recording_id.unwrap_or(c.track_id);
        if c.excluded
            || own.contains(&c.track_id)
            || own_recordings.contains(&recording)
            || !seen_recordings.insert(recording)
            || !rank::acquirable(&c)
            || rank::resting(&c, WaveCharacter::Mix, now)
        {
            continue;
        }
        let artist = c.artist_ids.first().copied().unwrap_or(c.track_id);
        let n = per_artist.entry(artist).or_default();
        if *n >= PER_ARTIST {
            continue;
        }
        *n += 1;
        picked.push((
            c,
            Reason {
                kind: ReasonKind::Similar,
                text: REASON.into(),
            },
        ));
    }

    let ids: Vec<Uuid> = picked.iter().map(|(c, _)| c.track_id).collect();
    let reasons: Vec<String> = picked
        .iter()
        .map(|(_, r)| json!({ "kind": r.kind, "text": r.text, "smartShuffle": true }).to_string())
        .collect();
    let positions: Vec<i32> = (0..picked.len() as i32).collect();
    sqlx::query!(
        "INSERT INTO recommendation_impressions (user_id, track_id, surface, model_version, reason, position)
         SELECT $1, t, 'similar', $2, r::jsonb, p FROM unnest($3::uuid[], $4::text[], $5::int[]) AS x(t, r, p)",
        user,
        MODEL_VERSION,
        &ids,
        &reasons,
        &positions,
    )
    .execute(&st.db)
    .await?;
    wave::items(&st.db, user, picked).await
}
