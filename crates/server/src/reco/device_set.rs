//! Набор волны для устройства (умный офлайн): любимое вперемешку с новым, всё — уже на сервере.
//!
//! Любимое — лайки с файлом на сервере: свежие и дослушанные чаще, с долей случайности; то, что уже
//! в наборе, держится (приложение не перекачивает запас каждый раз). Новое — тот же подбор, что у волны
//! (`wave::select`, характер «Незнакомое», без прослушанного за месяц), только из готового. Сыгранное новое
//! уходит из набора при пересборке: его место занимает свежее. Строки набора приходят в приложение
//! синхронизацией (`deviceSets`), удалённые — tombstone'ами.

use std::collections::HashSet;

use chrono::Utc;
use dalywave_protocol::{
    offline::{DeviceSet, DeviceSetRequest},
    wave::{WaveCharacter, WaveSettings},
};
use uuid::Uuid;

use super::wave::{self, Env, Options, Session};
use crate::{error::AppResult, state::AppState};

/// Больше этого в наборе не держим ни любимого, ни нового: запас — не вся медиатека.
pub const MAX_PART: u32 = 2000;
/// Новое подбирается правилами волны: дороже любимого, поэтому предел ниже.
pub const MAX_FRESH: u32 = 400;

/// Пересобрать набор устройства под запас приложения.
pub async fn build(
    st: &AppState,
    user: Uuid,
    device: Uuid,
    req: &DeviceSetRequest,
) -> AppResult<DeviceSet> {
    let favorites_want = i64::from(req.favorites.min(MAX_PART));
    let fresh_want = req.fresh.min(MAX_FRESH) as usize;
    let db = &st.db;

    // Любимое: держим прежнее, добираем по свежести лайка и дослушиваниям
    let favorites: Vec<Uuid> = sqlx::query_scalar!(
        r#"SELECT l.track_id AS "track_id!"
           FROM liked_tracks l
           JOIN tracks t ON t.id = l.track_id
           JOIN recordings r ON r.id = t.recording_id
           WHERE l.user_id = $1 AND r.audio_status = 'ready'
             AND NOT EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $1 AND b.kind = 'track' AND b.ref_id = l.track_id)
           ORDER BY
             (CASE WHEN EXISTS (SELECT 1 FROM device_sets s WHERE s.device_id = $2 AND s.track_id = l.track_id
                                  AND s.kind = 'favorite') THEN 0.4 ELSE 0 END)
             + 0.5 * exp(-extract(epoch FROM now() - l.created_at) / 86400.0 / 180.0)
             + 0.1 * ln(1 + (SELECT count(*) FROM play_events p WHERE p.user_id = $1 AND p.track_id = l.track_id
                              AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)))
             + 0.3 * random() DESC
           LIMIT $3"#,
        user,
        device,
        favorites_want,
    )
    .fetch_all(db)
    .await?;

    // Новое, что уже в наборе и ещё не сыграно, остаётся: его файл уже на устройстве
    let mut fresh: Vec<(Uuid, f32)> = sqlx::query!(
        r#"SELECT s.track_id, s.score
           FROM device_sets s
           JOIN tracks t ON t.id = s.track_id
           JOIN recordings r ON r.id = t.recording_id
           WHERE s.device_id = $1 AND s.kind = 'fresh' AND r.audio_status = 'ready'
             AND NOT EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = $2 AND p.track_id = s.track_id
                               AND p.started_at > s.created_at - interval '30 days')
             AND NOT EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $2 AND l.track_id = s.track_id)
             AND NOT EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $2 AND b.kind = 'track' AND b.ref_id = s.track_id)
           ORDER BY s.score DESC
           LIMIT $3"#,
        device,
        user,
        fresh_want as i64,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.track_id, r.score))
    .collect();

    if fresh.len() < fresh_want {
        let mut served: Vec<Uuid> = favorites.clone();
        served.extend(fresh.iter().map(|(id, _)| *id));
        let settings = WaveSettings {
            character: WaveCharacter::New,
            ..Default::default()
        };
        let session = Session::transient(user, settings, served);
        let env = Env::of(st).await?;
        let platform = sqlx::query_scalar!("SELECT platform FROM devices WHERE id = $1", device)
            .fetch_optional(db)
            .await?
            .unwrap_or_else(|| "web".into());
        let picked = wave::select(
            &env,
            &session,
            Some(&platform),
            fresh_want - fresh.len(),
            Options {
                only_ready: true,
                skip_heard: true,
                ..Default::default()
            },
        )
        .await?;
        let have: HashSet<Uuid> = fresh.iter().map(|(id, _)| *id).collect();
        for (s, _) in picked {
            if fresh.len() >= fresh_want {
                break;
            }
            let c = &s.entry.cand;
            // Новое — значит неслышанное: лайки и прослушанное держит «любимое»
            if c.ready() && !c.heard() && !have.contains(&c.track_id) {
                fresh.push((c.track_id, s.score));
            }
        }
    }

    let fav_scores: Vec<f32> = (0..favorites.len())
        .map(|i| 1.0 - i as f32 / favorites.len().max(1) as f32)
        .collect();
    let fresh_ids: Vec<Uuid> = fresh.iter().map(|(id, _)| *id).collect();
    let fresh_scores: Vec<f32> = fresh.iter().map(|(_, s)| *s).collect();
    let mut keep: Vec<Uuid> = favorites.clone();
    keep.extend(&fresh_ids);

    let mut tx = db.begin().await?;
    // Лишнее — tombstone'ами (триггер device_sets_gone): приложение удалит файл, если он не закреплён
    let removed = sqlx::query!(
        "DELETE FROM device_sets WHERE device_id = $1 AND NOT (track_id = ANY($2))",
        device,
        &keep,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    sqlx::query!(
        "INSERT INTO device_sets (device_id, track_id, kind, score)
         SELECT $1, t, 'favorite', s FROM unnest($2::uuid[], $3::real[]) AS x(t, s)
         ON CONFLICT (device_id, track_id) DO UPDATE SET kind = EXCLUDED.kind, score = EXCLUDED.score
         WHERE (device_sets.kind, device_sets.score) IS DISTINCT FROM (EXCLUDED.kind, EXCLUDED.score)",
        device,
        &favorites,
        &fav_scores,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO device_sets (device_id, track_id, kind, score)
         SELECT $1, t, 'fresh', s FROM unnest($2::uuid[], $3::real[]) AS x(t, s)
         ON CONFLICT (device_id, track_id) DO UPDATE SET kind = EXCLUDED.kind, score = EXCLUDED.score
         WHERE (device_sets.kind, device_sets.score) IS DISTINCT FROM (EXCLUDED.kind, EXCLUDED.score)",
        device,
        &fresh_ids,
        &fresh_scores,
    )
    .execute(&mut *tx)
    .await?;
    let built_at = Utc::now();
    sqlx::query!(
        "INSERT INTO device_set_builds (device_id, favorites, fresh, built_at) VALUES ($1, $2, $3, $4)
         ON CONFLICT (device_id) DO UPDATE SET favorites = $2, fresh = $3, built_at = $4",
        device,
        favorites.len() as i32,
        fresh_ids.len() as i32,
        built_at,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(
        %device,
        favorites = favorites.len(),
        fresh = fresh_ids.len(),
        removed,
        "device set rebuilt"
    );
    Ok(DeviceSet {
        favorites: favorites.len() as i64,
        fresh: fresh_ids.len() as i64,
        removed: removed as i64,
        built_at,
    })
}
