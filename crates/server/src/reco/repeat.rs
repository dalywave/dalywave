//! Плейлисты по истории прослушиваний, как On Repeat и Repeat Rewind в Spotify: «На повторе» — что
//! чаще всего играло за 30 дней, «Старые хиты на повторе» — что крутилось 6–24 месяца назад, а в последние
//! три месяца не звучало. Пересчёт — вместе с миксами дня (задача `taste`); история из архива Spotify
//! тоже считается.

use sqlx::PgPool;
use uuid::Uuid;

use super::taste::{MIX_SIZE, changed, save_mix};

pub const ON_REPEAT: &str = "on_repeat";
pub const REPEAT_REWIND: &str = "repeat_rewind";
/// Меньше — плейлиста нет (или он удаляется).
const MIN_TRACKS: usize = 5;
/// Прослушивание считается, если трек играл хотя бы столько или дослушан.
const COUNTED_MS: i32 = 30_000;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RepeatReport {
    pub on_repeat: usize,
    pub repeat_rewind: usize,
}

/// «На повторе»: больше всего прослушиваний за 30 дней, при равенстве — недавнее первым.
pub async fn on_repeat(db: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"WITH counted AS (
               SELECT p.track_id, count(*) AS plays, max(p.started_at) AS last
               FROM play_events p
               WHERE p.user_id = $1 AND p.started_at > now() - interval '30 days'
                 AND (p.end_reason = 'completed' OR p.played_ms >= $3)
               GROUP BY p.track_id
           ),
           one_per_recording AS (
               SELECT DISTINCT ON (COALESCE(t.recording_id, t.id)) c.track_id, c.plays, c.last
               FROM counted c JOIN tracks t ON t.id = c.track_id
               WHERE NOT EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $1 AND b.kind = 'track' AND b.ref_id = c.track_id)
                 AND NOT EXISTS (SELECT 1 FROM blocked_items b JOIN track_artists ta ON ta.artist_id = b.ref_id
                                 WHERE b.user_id = $1 AND b.kind = 'artist' AND ta.track_id = c.track_id)
                 AND NOT COALESCE((SELECT f.kind = 'dislike' FROM feedback_events f
                                   WHERE f.user_id = $1 AND f.track_id = c.track_id AND f.kind IN ('dislike', 'undislike')
                                   ORDER BY f.at DESC LIMIT 1), false)
               ORDER BY COALESCE(t.recording_id, t.id), c.plays DESC, c.last DESC
           )
           SELECT track_id AS "track_id!" FROM one_per_recording
           WHERE plays >= 2
           ORDER BY plays DESC, last DESC, track_id
           LIMIT $2"#,
        user_id,
        MIX_SIZE as i64,
        COUNTED_MS,
    )
    .fetch_all(db)
    .await
}

/// «Старые хиты на повторе»: не меньше трёх прослушиваний 6–24 месяца назад и ни одного за 90 дней —
/// самые заслушанные тогда первыми.
pub async fn repeat_rewind(db: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"WITH counted AS (
               SELECT p.track_id, count(*) AS plays, max(p.started_at) AS last
               FROM play_events p
               WHERE p.user_id = $1
                 AND p.started_at BETWEEN now() - interval '24 months' AND now() - interval '6 months'
                 AND (p.end_reason = 'completed' OR p.played_ms >= $3)
               GROUP BY p.track_id
               HAVING count(*) >= 3
           ),
           one_per_recording AS (
               SELECT DISTINCT ON (COALESCE(t.recording_id, t.id)) c.track_id, c.plays, c.last
               FROM counted c JOIN tracks t ON t.id = c.track_id
               WHERE NOT EXISTS (SELECT 1 FROM play_events r JOIN tracks rt ON rt.id = r.track_id
                                 WHERE r.user_id = $1 AND r.started_at > now() - interval '90 days'
                                   AND COALESCE(rt.recording_id, rt.id) = COALESCE(t.recording_id, t.id))
                 AND NOT EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $1 AND b.kind = 'track' AND b.ref_id = c.track_id)
                 AND NOT EXISTS (SELECT 1 FROM blocked_items b JOIN track_artists ta ON ta.artist_id = b.ref_id
                                 WHERE b.user_id = $1 AND b.kind = 'artist' AND ta.track_id = c.track_id)
                 AND NOT COALESCE((SELECT f.kind = 'dislike' FROM feedback_events f
                                   WHERE f.user_id = $1 AND f.track_id = c.track_id AND f.kind IN ('dislike', 'undislike')
                                   ORDER BY f.at DESC LIMIT 1), false)
               ORDER BY COALESCE(t.recording_id, t.id), c.plays DESC, c.last DESC
           )
           SELECT track_id AS "track_id!" FROM one_per_recording
           ORDER BY plays DESC, last DESC, track_id
           LIMIT $2"#,
        user_id,
        MIX_SIZE as i64,
        COUNTED_MS,
    )
    .fetch_all(db)
    .await
}

/// Пересчитывает оба плейлиста: хватает треков — тот же плейлист с новыми позициями, нет — удаляется.
pub async fn refresh(db: &PgPool, user_id: Uuid) -> Result<RepeatReport, sqlx::Error> {
    let mut report = RepeatReport::default();
    let plans = [
        (
            ON_REPEAT,
            "На повторе",
            "Что вы слушаете чаще всего прямо сейчас",
            on_repeat(db, user_id).await?,
        ),
        (
            REPEAT_REWIND,
            "Старые хиты на повторе",
            "Когда-то крутились без остановки",
            repeat_rewind(db, user_id).await?,
        ),
    ];
    for (generator, title, description, tracks) in plans {
        let made = tracks.len() >= MIN_TRACKS;
        if made {
            save_mix(db, user_id, generator, 1, title, description, &tracks).await?;
        } else {
            let mut tx = db.begin().await?;
            let removed = sqlx::query!(
                "DELETE FROM playlists WHERE user_id = $1 AND kind = 'mix' AND rules ->> 'generator' = $2",
                user_id,
                generator,
            )
            .execute(&mut *tx)
            .await?;
            if removed.rows_affected() > 0 {
                changed(&mut tx, user_id).await;
            }
            tx.commit().await?;
        }
        let n = if made { tracks.len() } else { 0 };
        if generator == ON_REPEAT {
            report.on_repeat = n;
        } else {
            report.repeat_rewind = n;
        }
    }
    Ok(report)
}
