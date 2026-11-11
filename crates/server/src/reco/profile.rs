//! «Мой вкус» слушателя: грани глазами человека, статистика прослушиваний за месяц, из чего волна знает
//! вкус, и ручки — «меньше такого» (приглушение направления звучания, `taste_dampers`) и сброс вкуса.
//! Приглушение читают грани ([`super::taste::facets`]: вес), кандидаты волны и миксов
//! ([`super::candidates`]: `damped`) — через SQL-функцию `taste_damp`.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use dalywave_protocol::{
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
    taste::{
        DayPartShare, ListeningStats, TasteDamper, TasteFacet, TasteKnowledge, TasteOverview,
        TopArtist, TopGenre,
    },
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{DayPart, Settings, taste};
use crate::{
    audit,
    catalog::views,
    realtime::{self, Audience},
};

/// Примеров у грани и у приглушения, артистов у грани.
const FACET_EXAMPLES: usize = 6;
const DAMPER_EXAMPLES: usize = 3;
const FACET_ARTISTS: usize = 3;
/// Первое «меньше такого» и шаг следующих.
pub const DAMPER_START: f32 = 0.5;
pub const DAMPER_STEP: f32 = 0.25;
/// Повторное «меньше такого» той же грани усиливает приглушение, а не заводит новое.
const SAME_DAMPER: f32 = 0.95;
/// Грань «слушают чаще» в части суток, если её поправка там не ниже этой.
const PEAK_LIFT: f32 = 1.15;
const TOP: i64 = 5;

/// Полночь первого числа месяца по времени слушателя (в UTC); `back` — сколько месяцев назад.
pub fn month_start(now: DateTime<Utc>, utc_offset_min: i16, back: u32) -> DateTime<Utc> {
    let offset = Duration::minutes(i64::from(utc_offset_min));
    let local = (now + offset).date_naive();
    let mut year = local.year();
    let mut month = local.month() as i32 - back as i32;
    while month < 1 {
        month += 12;
        year -= 1;
    }
    let first = NaiveDate::from_ymd_opt(year, month as u32, 1).unwrap_or(local);
    Utc.from_utc_datetime(&first.and_hms_opt(0, 0, 0).unwrap_or_default()) - offset
}

/// Часть суток, когда грань слушают заметно чаще остальных.
pub fn peak(context: &Value) -> Option<String> {
    let parts = context.get("timeOfDay")?.as_object()?;
    parts
        .iter()
        .filter_map(|(k, v)| Some((k, v.as_f64()? as f32)))
        .filter(|(_, lift)| *lift >= PEAK_LIFT)
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(a.0)))
        .map(|(k, _)| k.clone())
}

/// Сила приглушения после ещё одного нажатия.
pub fn stronger(strength: f32) -> f32 {
    (strength + DAMPER_STEP).min(1.0)
}

/// Жанр Deezer по-русски; незнакомый — как есть.
pub fn genre_ru(raw: &str) -> String {
    match raw {
        "Pop" => "Поп",
        "Rap/Hip Hop" => "Рэп и хип-хоп",
        "Rock" => "Рок",
        "Alternative" => "Альтернатива",
        "Indie Rock" => "Инди-рок",
        "Indie Pop" => "Инди-поп",
        "Dance" => "Танцевальная",
        "Electro" => "Электроника",
        "Techno/House" => "Техно и хаус",
        "R&B" => "R&B",
        "Soul & Funk" => "Соул и фанк",
        "Metal" => "Метал",
        "Jazz" => "Джаз",
        "Classical" => "Классика",
        "Films/Games" => "Кино и игры",
        "Reggae" => "Регги",
        "Reggaeton" => "Реггетон",
        "Folk" => "Фолк",
        "Blues" => "Блюз",
        "Country" => "Кантри",
        "Latin Music" => "Латино",
        "Asian Music" => "Азиатская",
        "African Music" => "Африканская",
        "Kids" => "Детская",
        "Chanson" => "Шансон",
        "Variété Internationale" => "Эстрада",
        _ => raw,
    }
    .to_string()
}

async fn tracks_by_id(
    db: &PgPool,
    ids: &[Uuid],
    user_id: Uuid,
) -> Result<HashMap<Uuid, dalywave_protocol::catalog::Track>, sqlx::Error> {
    Ok(views::tracks(db, ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect())
}

async fn facets(db: &PgPool, user_id: Uuid) -> Result<Vec<TasteFacet>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT f.id, f.label, f.weight, f.tracks, f.context, f.examples, f.artists, f.updated_at,
                  taste_damp(f.user_id, f.centroid, 0.85, 0.97) AS "damped!",
                  (SELECT d.id FROM taste_dampers d WHERE d.user_id = f.user_id
                   ORDER BY d.centroid <=> f.centroid LIMIT 1) AS damper
           FROM taste_facets f WHERE f.user_id = $1 ORDER BY f.position"#,
        user_id,
    )
    .fetch_all(db)
    .await?;
    let track_ids: Vec<Uuid> = rows
        .iter()
        .flat_map(|f| f.examples.iter().take(FACET_EXAMPLES).copied())
        .collect();
    let artist_ids: Vec<Uuid> = rows
        .iter()
        .flat_map(|f| f.artists.iter().take(FACET_ARTISTS).copied())
        .collect();
    let tracks = tracks_by_id(db, &track_ids, user_id).await?;
    let artists: HashMap<Uuid, _> = views::artists(db, &artist_ids, user_id)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let weights: Vec<f32> = rows
        .iter()
        .map(|f| f.weight * (1.0 - 0.8 * f.damped))
        .collect();
    let total = weights.iter().sum::<f32>().max(1e-6);
    Ok(rows
        .into_iter()
        .zip(weights)
        .map(|(f, w)| TasteFacet {
            id: f.id,
            share: w / total,
            tracks: f.tracks,
            examples: f
                .examples
                .iter()
                .take(FACET_EXAMPLES)
                .filter_map(|id| tracks.get(id).cloned())
                .collect(),
            artists: f
                .artists
                .iter()
                .take(FACET_ARTISTS)
                .filter_map(|id| artists.get(id).cloned())
                .collect(),
            peak: peak(&f.context),
            damper_id: (f.damped > 0.0).then_some(f.damper).flatten(),
            damped: f.damped,
            label: f.label,
            updated_at: f.updated_at,
        })
        .collect())
}

async fn dampers(db: &PgPool, user_id: Uuid) -> Result<Vec<TasteDamper>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT id, label, strength, examples, created_at, updated_at FROM taste_dampers
         WHERE user_id = $1 ORDER BY updated_at DESC, id",
        user_id,
    )
    .fetch_all(db)
    .await?;
    let ids: Vec<Uuid> = rows
        .iter()
        .flat_map(|d| d.examples.iter().take(DAMPER_EXAMPLES).copied())
        .collect();
    let tracks = tracks_by_id(db, &ids, user_id).await?;
    Ok(rows
        .into_iter()
        .map(|d| TasteDamper {
            examples: d
                .examples
                .iter()
                .take(DAMPER_EXAMPLES)
                .filter_map(|id| tracks.get(id).cloned())
                .collect(),
            id: d.id,
            label: d.label,
            strength: d.strength,
            created_at: d.created_at,
            updated_at: d.updated_at,
        })
        .collect())
}

/// Статистика за текущий месяц по времени слушателя: часы, топ артистов и жанров, части суток.
pub async fn stats(
    db: &PgPool,
    user_id: Uuid,
    settings: &Settings,
    now: DateTime<Utc>,
) -> Result<ListeningStats, sqlx::Error> {
    let since = month_start(now, settings.utc_offset_min, 0);
    let before = month_start(now, settings.utc_offset_min, 1);
    let totals = sqlx::query!(
        r#"SELECT count(*) FILTER (WHERE p.started_at >= $2) AS "plays!",
                  (COALESCE(sum(p.played_ms) FILTER (WHERE p.started_at >= $2), 0) / 60000)::bigint AS "minutes!",
                  (COALESCE(sum(p.played_ms) FILTER (WHERE p.started_at < $2), 0) / 60000)::bigint AS "previous!",
                  count(DISTINCT p.track_id) FILTER (WHERE p.started_at >= $2) AS "tracks!",
                  (SELECT count(DISTINCT ta.artist_id) FROM play_events q
                   JOIN track_artists ta ON ta.track_id = q.track_id AND ta.role = 'main'
                   WHERE q.user_id = $1 AND q.started_at >= $2) AS "artists!"
           FROM play_events p WHERE p.user_id = $1 AND p.started_at >= $3"#,
        user_id,
        since,
        before,
    )
    .fetch_one(db)
    .await?;
    let top = sqlx::query!(
        r#"SELECT ta.artist_id AS "artist_id!", count(*) AS "plays!",
                  (sum(p.played_ms) / 60000)::bigint AS "minutes!"
           FROM play_events p
           JOIN track_artists ta ON ta.track_id = p.track_id AND ta.role = 'main' AND ta.position = 0
           WHERE p.user_id = $1 AND p.started_at >= $2
           GROUP BY ta.artist_id
           ORDER BY sum(p.played_ms) DESC, count(*) DESC, ta.artist_id
           LIMIT $3"#,
        user_id,
        since,
        TOP,
    )
    .fetch_all(db)
    .await?;
    let ids: Vec<Uuid> = top.iter().map(|r| r.artist_id).collect();
    let artists: HashMap<Uuid, _> = views::artists(db, &ids, user_id)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let genres = sqlx::query!(
        r#"WITH g AS (
               SELECT unnest(CASE WHEN cardinality(al.genres) > 0 THEN al.genres ELSE NULL END) AS genre,
                      p.played_ms
               FROM play_events p JOIN tracks t ON t.id = p.track_id JOIN albums al ON al.id = t.album_id
               WHERE p.user_id = $1 AND p.started_at >= $2
           )
           SELECT genre AS "genre!", (sum(played_ms)::real / NULLIF((SELECT sum(played_ms) FROM g), 0))::real AS "share!"
           FROM g WHERE genre IS NOT NULL
           GROUP BY genre ORDER BY sum(played_ms) DESC, genre LIMIT $3"#,
        user_id,
        since,
        TOP,
    )
    .fetch_all(db)
    .await?;
    let parts = sqlx::query!(
        r#"SELECT extract(hour FROM p.started_at + make_interval(mins => $3))::int AS "hour!",
                  sum(p.played_ms)::bigint AS "ms!"
           FROM play_events p WHERE p.user_id = $1 AND p.started_at >= $2
           GROUP BY 1"#,
        user_id,
        since,
        i32::from(settings.utc_offset_min),
    )
    .fetch_all(db)
    .await?;
    let mut by_part: HashMap<&'static str, i64> = HashMap::new();
    for r in &parts {
        *by_part
            .entry(DayPart::of_hour(r.hour.rem_euclid(24) as u32).as_str())
            .or_default() += r.ms;
    }
    let all = by_part.values().sum::<i64>().max(1) as f32;
    let day_parts = [
        DayPart::Morning,
        DayPart::Day,
        DayPart::Evening,
        DayPart::Night,
    ]
    .iter()
    .map(|p| DayPartShare {
        part: p.as_str().into(),
        share: by_part.get(p.as_str()).copied().unwrap_or(0) as f32 / all,
    })
    .collect();
    Ok(ListeningStats {
        since,
        minutes: totals.minutes,
        previous_minutes: totals.previous,
        plays: totals.plays,
        tracks: totals.tracks,
        artists: totals.artists,
        top_artists: top
            .into_iter()
            .filter_map(|r| {
                Some(TopArtist {
                    artist: artists.get(&r.artist_id)?.clone(),
                    plays: r.plays,
                    minutes: r.minutes,
                })
            })
            .collect(),
        top_genres: genres
            .into_iter()
            .map(|g| TopGenre {
                name: genre_ru(&g.genre),
                id: g.genre,
                share: g.share,
            })
            .collect(),
        day_parts,
    })
}

async fn knowledge(db: &PgPool, user_id: Uuid) -> Result<TasteKnowledge, sqlx::Error> {
    let r = sqlx::query!(
        r#"WITH known AS (
               SELECT track_id FROM liked_tracks WHERE user_id = $1
               UNION
               SELECT track_id FROM play_events
               WHERE user_id = $1 AND (end_reason = 'completed' OR played_ms >= 0.8 * duration_ms)
           )
           SELECT (SELECT count(*) FROM liked_tracks WHERE user_id = $1) AS "likes!",
                  (SELECT count(DISTINCT track_id) FROM play_events
                   WHERE user_id = $1 AND (end_reason = 'completed' OR played_ms >= 0.8 * duration_ms)) AS "listened!",
                  (SELECT count(*) FROM known k JOIN tracks t ON t.id = k.track_id
                   WHERE EXISTS (SELECT 1 FROM track_analysis a WHERE a.recording_id = t.recording_id
                                   AND a.sound IS NOT NULL)) AS "analyzed!",
                  (SELECT count(*) FROM (
                       SELECT DISTINCT ON (track_id) kind FROM feedback_events
                       WHERE user_id = $1 AND track_id IS NOT NULL AND kind IN ('dislike', 'undislike')
                       ORDER BY track_id, at DESC) d WHERE d.kind = 'dislike') AS "dislikes!",
                  (SELECT count(*) FROM blocked_items WHERE user_id = $1) AS "blocked!",
                  (SELECT count(*) FROM recommendation_impressions
                   WHERE user_id = $1 AND outcome IS NOT NULL AND shown_at > now() - interval '30 days') AS "reactions!",
                  (SELECT max(updated_at) FROM taste_facets WHERE user_id = $1) AS updated_at,
                  (SELECT reset_at FROM taste_profiles WHERE user_id = $1) AS reset_at"#,
        user_id,
    )
    .fetch_one(db)
    .await?;
    Ok(TasteKnowledge {
        likes: r.likes,
        listened: r.listened,
        analyzed: r.analyzed,
        dislikes: r.dislikes,
        blocked: r.blocked,
        reactions: r.reactions,
        updated_at: r.updated_at,
        reset_at: r.reset_at,
    })
}

pub async fn overview(
    db: &PgPool,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<TasteOverview, sqlx::Error> {
    let settings = Settings::load(db).await?;
    Ok(TasteOverview {
        facets: facets(db, user_id).await?,
        dampers: dampers(db, user_id).await?,
        stats: stats(db, user_id, &settings, now).await?,
        knows: knowledge(db, user_id).await?,
    })
}

/// «Меньше такого» для грани: новое приглушение по её центру или, если такое уже есть, — сильнее.
/// `None` — нет такой грани у слушателя.
pub async fn dampen(
    db: &PgPool,
    user_id: Uuid,
    facet_id: Uuid,
) -> Result<Option<TasteDamper>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let Some(facet) = sqlx::query!(
        "SELECT label, examples FROM taste_facets WHERE id = $1 AND user_id = $2",
        facet_id,
        user_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(None);
    };
    let same = sqlx::query!(
        "SELECT d.id, d.strength FROM taste_dampers d, taste_facets f
         WHERE f.id = $1 AND d.user_id = $2 AND 1 - (d.centroid <=> f.centroid) >= $3
         ORDER BY d.centroid <=> f.centroid LIMIT 1 FOR UPDATE OF d",
        facet_id,
        user_id,
        f64::from(SAME_DAMPER),
    )
    .fetch_optional(&mut *tx)
    .await?;
    let examples: Vec<Uuid> = facet.examples.into_iter().take(DAMPER_EXAMPLES).collect();
    let id = match same {
        Some(d) => {
            sqlx::query!(
                "UPDATE taste_dampers SET strength = $2, label = $3, updated_at = now() WHERE id = $1",
                d.id,
                stronger(d.strength),
                facet.label,
            )
            .execute(&mut *tx)
            .await?;
            d.id
        }
        None => {
            sqlx::query_scalar!(
                "INSERT INTO taste_dampers (user_id, label, centroid, strength, examples)
                 SELECT $1, $2, f.centroid, $3, $4 FROM taste_facets f WHERE f.id = $5
                 RETURNING id",
                user_id,
                facet.label,
                DAMPER_START,
                &examples,
                facet_id,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    tx.commit().await?;
    Ok(dampers(db, user_id).await?.into_iter().find(|d| d.id == id))
}

/// Убирает «меньше такого». `false` — такого не было.
pub async fn undampen(db: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query!(
        "DELETE FROM taste_dampers WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(db)
    .await?
    .rows_affected()
        > 0)
}

/// Сброс вкуса: грани, приглушения, заготовки волны, векторы сессий и миксы — заново; прослушивания и лайки
/// до этой минуты вкус больше не строят (медиатека, «не рекомендовать» и «не нравится» остаются).
pub async fn reset(db: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query!(
        "INSERT INTO taste_profiles (user_id, reset_at) VALUES ($1, now())
         ON CONFLICT (user_id) DO UPDATE SET reset_at = now(), updated_at = now()",
        user_id,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM taste_facets WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM taste_dampers WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM wave_pools WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!(
        "UPDATE wave_sessions SET hits = NULL, misses = NULL WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await?;
    let mixes = sqlx::query!(
        "DELETE FROM playlists WHERE user_id = $1 AND kind = 'mix' AND rules ->> 'generator' IN ($2, $3)",
        user_id,
        taste::DAILY_MIX,
        taste::DISCOVER_WEEKLY,
    )
    .execute(&mut *tx)
    .await?;
    if mixes.rows_affected() > 0 {
        realtime::publish(
            &mut *tx,
            Audience::User(user_id),
            ServerEvent::LibraryChanged(LibraryChange {
                entities: vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
            }),
        )
        .await;
    }
    audit::record(
        &mut *tx,
        Some(user_id),
        audit::TASTE_RESET,
        None,
        json!({ "mixes": mixes.rows_affected() }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_starts_in_listener_time() {
        // 30 ноября 22:30 UTC — в Москве (+3) уже 1 декабря
        let now = Utc.with_ymd_and_hms(2026, 11, 30, 22, 30, 0).unwrap();
        assert_eq!(
            month_start(now, 180, 0),
            Utc.with_ymd_and_hms(2026, 11, 30, 21, 0, 0).unwrap()
        );
        assert_eq!(
            month_start(now, 180, 1),
            Utc.with_ymd_and_hms(2026, 10, 31, 21, 0, 0).unwrap()
        );
        // Январь: прошлый месяц — декабрь прошлого года
        let jan = Utc.with_ymd_and_hms(2027, 1, 15, 12, 0, 0).unwrap();
        assert_eq!(
            month_start(jan, 0, 1),
            Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap()
        );
    }

    #[test]
    fn peak_needs_a_clear_lift() {
        let ctx =
            json!({ "timeOfDay": { "morning": 0.8, "day": 1.1, "evening": 1.4, "night": 1.2 } });
        assert_eq!(peak(&ctx).as_deref(), Some("evening"));
        let flat = json!({ "timeOfDay": { "morning": 1.0, "day": 1.1 } });
        assert_eq!(peak(&flat), None);
        assert_eq!(peak(&json!({})), None);
    }

    #[test]
    fn dampers_grow_to_full() {
        assert_eq!(stronger(DAMPER_START), 0.75);
        assert_eq!(stronger(0.9), 1.0);
        assert_eq!(genre_ru("Rap/Hip Hop"), "Рэп и хип-хоп");
        assert_eq!(genre_ru("Hyperpop"), "Hyperpop");
    }
}
