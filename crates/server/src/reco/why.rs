//! «Почему этот трек»: объяснение рекомендации понятными словами — по тому, что на самом деле повлияло
//! на выбор. При выдаче волна записывает в показ, откуда пришёл трек и что о нём знала ([`record`]);
//! объяснение ([`explain`]) собирается из этого, граней вкуса, истории сессии и недавних прослушиваний.
//! Слушателю — без чисел и технических слов; администратору — ещё и подробности, как в «Рекомендациях».

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use dalywave_protocol::wave::{WhyKind, WhyLine, WhyTrack};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    DayPart, Settings,
    rank::{Entry, Origin, popularity},
    taste, track_sound,
    vectors::cosine,
};
use crate::{
    catalog::views,
    error::{AppError, AppResult},
};

/// Сколько строк показывать: главное и пара уточнений.
const MAX_LINES: usize = 4;
/// Столько дослушиваний артиста — «ты часто его слушаешь».
const OFTEN_ARTIST: i64 = 5;
/// Недавнее похожее: настолько близко по звучанию.
const RECENT_SIM: f32 = 0.85;
/// Ближе этого к вкусу — не «разведка».
const EXPLORE_SIM: f32 = 0.5;
/// Вклад сети, который стоит упомянуть.
const MODEL_NOTE: f32 = 0.03;

fn round(x: f32) -> f64 {
    (f64::from(x) * 1000.0).round() / 1000.0
}

/// Что записать в показ: откуда трек, сходства, знакомство, вклад сети и итоговая оценка.
pub fn record(e: &Entry, score: f32) -> Value {
    let c = &e.cand;
    let (origin, artists, label) = match &e.origin {
        Origin::Facet { artists } => (
            "facet",
            artists.iter().map(|(_, n)| n.clone()).collect(),
            None,
        ),
        Origin::Session { artist } => (
            "session",
            artist.iter().map(|(_, n)| n.clone()).collect(),
            None,
        ),
        Origin::Seed { label, .. } => ("seed", Vec::new(), Some(label.clone())),
        Origin::Query { text } => ("query", Vec::new(), Some(text.clone())),
        Origin::Favorite => ("favorite", Vec::new(), None),
        Origin::Forgotten => ("forgotten", Vec::new(), None),
        Origin::NewRelease => ("new_release", Vec::new(), None),
    };
    // Свой артист трека — не «похоже на»: из подписи грани он убирается
    let artists: Vec<String> = artists
        .into_iter()
        .filter(|a: &String| !c.artist_names.contains(a))
        .take(3)
        .collect();
    json!({
        "origin": origin,
        "artists": artists,
        "label": label,
        "sim": round(e.sim),
        "tasteSim": c.taste_sim.map(round),
        "hitSim": c.hit_sim.map(round),
        "missSim": c.miss_sim.map(round),
        "model": c.model.map(round),
        "modelVersion": c.model_version,
        "score": round(score),
        "liked": c.liked,
        "completed": c.completed,
        "plays30d": c.plays_30d,
        "artistFamiliar": c.artist_familiar,
        "popularity": round(popularity(c)),
    })
}

fn line(kind: WhyKind, text: impl Into<String>) -> WhyLine {
    WhyLine {
        kind,
        text: text.into(),
    }
}

/// «вчера вечером», «сегодня утром», «в среду днём» — по местному времени слушателя.
pub fn when(at: DateTime<Utc>, now: DateTime<Utc>, offset_min: i16) -> String {
    let shift = Duration::minutes(i64::from(offset_min));
    let (at, now) = (at + shift, now + shift);
    let part = match DayPart::of_hour(at.hour()) {
        DayPart::Morning => "утром",
        DayPart::Day => "днём",
        DayPart::Evening => "вечером",
        DayPart::Night => "ночью",
    };
    let days = (now.date_naive() - at.date_naive()).num_days();
    match days {
        0 => format!("сегодня {part}"),
        1 => format!("вчера {part}"),
        _ => {
            let day = match at.weekday().num_days_from_monday() {
                0 => "в понедельник",
                1 => "во вторник",
                2 => "в среду",
                3 => "в четверг",
                4 => "в пятницу",
                5 => "в субботу",
                _ => "в воскресенье",
            };
            format!("{day} {part}")
        }
    }
}

fn day_part_ru(part: DayPart) -> &'static str {
    match part {
        DayPart::Morning => "утром",
        DayPart::Day => "днём",
        DayPart::Evening => "вечером",
        DayPart::Night => "ночью",
    }
}

/// Объяснение трека из последнего показа слушателю (в этой сессии волны, если она указана).
pub async fn explain(
    db: &PgPool,
    user: Uuid,
    track_id: Uuid,
    session: Option<Uuid>,
    admin: bool,
) -> AppResult<WhyTrack> {
    let imp = sqlx::query!(
        r#"SELECT i.reason, i.score, i.model_version, i.shown_at, i.wave_session_id, i.surface
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
    let track = views::tracks(db, &[track_id], user)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)?;
    let reason = imp.reason.unwrap_or(Value::Null);
    let why = reason.get("why").cloned().unwrap_or(Value::Null);
    let origin = why
        .get("origin")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let kind = reason.get("kind").and_then(Value::as_str).unwrap_or("");
    let num = |k: &str| why.get(k).and_then(Value::as_f64).map(|x| x as f32);
    let flag = |k: &str| why.get(k).and_then(Value::as_bool).unwrap_or(false);
    let count = |k: &str| why.get(k).and_then(Value::as_i64).unwrap_or(0);
    let artists: Vec<String> = why
        .get("artists")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let label = why.get("label").and_then(Value::as_str).map(str::to_string);
    let settings = Settings::load(db).await?;
    let now = Utc::now();

    let history = sqlx::query!(
        r#"SELECT count(*) AS "plays!",
                  count(*) FILTER (WHERE p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms) AS "completed!",
                  max(p.started_at) AS last_played
           FROM play_events p WHERE p.user_id = $1 AND p.track_id = $2 AND p.started_at < $3"#,
        user,
        track_id,
        imp.shown_at,
    )
    .fetch_one(db)
    .await?;
    let sound = track_sound(db, track_id).await?;
    let facets = taste::facets(db, user).await?;
    let nearest = sound.as_ref().and_then(|v| {
        facets
            .iter()
            .map(|f| (f, cosine(v, &f.centroid)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    });

    let mut lines: Vec<WhyLine> = Vec::new();
    let liked = flag("liked") || track.liked;
    let familiar = liked || history.completed > 0;
    let artist_familiar = flag("artistFamiliar");
    match origin.as_str() {
        "seed" => lines.push(line(
            WhyKind::Seed,
            match label.as_deref() {
                Some(l) if kind == "seed" => format!(
                    "{} — радио от «{l}»",
                    reason
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("Похоже")
                ),
                _ => "Похоже на то, с чего началось радио".into(),
            },
        )),
        "query" => lines.push(line(
            WhyKind::Seed,
            format!("Подходит под «{}»", label.unwrap_or_default()),
        )),
        "new_release" => lines.push(line(
            WhyKind::Release,
            match track.artists.first() {
                Some(a) => format!("Новинка {} — ты подписан на этого артиста", a.name),
                None => "Новинка артиста из твоих подписок".into(),
            },
        )),
        "forgotten" => lines.push(line(
            WhyKind::Familiar,
            match history.last_played {
                Some(t) if now - t > Duration::days(60) => format!(
                    "Ты любил этот трек, но давно не включал — с {}",
                    month_ru(t)
                ),
                _ => "Ты любил этот трек, но давно не включал".into(),
            },
        )),
        _ => {}
    }
    if lines.is_empty() && familiar {
        lines.push(line(
            WhyKind::Familiar,
            if history.completed >= 5 || count("completed") >= 5 {
                "Ты часто его слушаешь"
            } else if liked {
                "Из твоих лайков"
            } else {
                "Ты уже слушал его до конца"
            },
        ));
    }

    // Похоже на любимого артиста — грань вкуса, из которой пришёл трек
    if matches!(origin.as_str(), "facet" | "session")
        && let Some(name) = artists.first()
    {
        let plays = sqlx::query_scalar!(
            r#"SELECT count(*) AS "n!" FROM play_events p
               JOIN track_artists ta ON ta.track_id = p.track_id
               JOIN artists a ON a.id = ta.artist_id
               WHERE p.user_id = $1 AND a.name = $2 AND p.started_at < $3
                 AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)"#,
            user,
            name,
            imp.shown_at,
        )
        .fetch_one(db)
        .await?;
        if origin == "session" {
            // Подробнее ниже: что именно зашло в этой сессии
        } else if plays >= OFTEN_ARTIST {
            lines.push(line(
                WhyKind::Artist,
                format!("Похоже на {name} — ты часто слушаешь этого артиста"),
            ));
        } else {
            lines.push(line(WhyKind::Artist, format!("Похоже на {name}")));
        }
    }

    // Что зашло в этой сессии волны, похожее по звучанию
    if let (Some(v), Some(sid)) = (&sound, imp.wave_session_id) {
        let hit = sqlx::query!(
            r#"SELECT t.title, (SELECT ar.name FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id
                                WHERE ta.track_id = t.id ORDER BY ta.position LIMIT 1) AS artist,
                      a.sound::real[] AS "sound!"
               FROM recommendation_impressions i
               JOIN tracks t ON t.id = i.track_id
               JOIN track_analysis a ON a.recording_id = t.recording_id AND a.sound IS NOT NULL
               WHERE i.wave_session_id = $1 AND i.outcome IN ('completed', 'like', 'more_like_this')
                 AND i.outcome_at < $2 AND i.track_id <> $3
               ORDER BY i.outcome_at DESC LIMIT 20"#,
            sid,
            imp.shown_at,
            track_id,
        )
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| (cosine(v, &r.sound), r.title, r.artist))
        .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, title, artist)) = hit.filter(|h| h.0 >= RECENT_SIM || origin == "session") {
            let by = artist.map(|a| format!(" — {a}")).unwrap_or_default();
            lines.push(line(
                WhyKind::Recent,
                format!("В этой волне тебе зашло похожее: «{title}»{by}"),
            ));
        }
    }
    // Недавно дослушанное похожее — вне этой сессии
    if !lines.iter().any(|l| l.kind == WhyKind::Recent)
        && let Some(v) = &sound
    {
        let recent = sqlx::query!(
            r#"SELECT t.title, p.started_at, (1 - (a.sound <=> $2::real[]::halfvec(1280)))::real AS "sim!"
               FROM play_events p
               JOIN tracks t ON t.id = p.track_id
               JOIN track_analysis a ON a.recording_id = t.recording_id AND a.sound IS NOT NULL
               WHERE p.user_id = $1 AND p.track_id <> $3
                 AND p.started_at > $4::timestamptz - interval '4 days' AND p.started_at < $4::timestamptz - interval '1 hour'
                 AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)
               ORDER BY a.sound <=> $2::real[]::halfvec(1280) LIMIT 1"#,
            user,
            v,
            track_id,
            imp.shown_at,
        )
        .fetch_optional(db)
        .await?;
        if let Some(r) = recent.filter(|r| r.sim >= RECENT_SIM) {
            lines.push(line(
                WhyKind::Recent,
                format!(
                    "Тебе зашло похожее {}: «{}»",
                    when(r.started_at, imp.shown_at, settings.utc_offset_min),
                    r.title
                ),
            ));
        }
    }

    // Новое или знакомое
    let taste_sim = num("tasteSim").or(nearest.map(|n| n.1));
    if !familiar && history.plays == 0 {
        let facet = nearest
            .filter(|(_, s)| *s >= EXPLORE_SIM)
            .map(|(f, _)| f.label.clone());
        lines.push(line(
            WhyKind::New,
            match (artist_familiar, facet) {
                (false, Some(l)) => format!("Новое для тебя — в духе твоей грани «{l}»"),
                (false, None) => {
                    "Новое для тебя: ни этот трек, ни артиста ты раньше не слушал".into()
                }
                (true, _) => "Новый для тебя трек знакомого артиста".into(),
            },
        ));
    } else if let Some((f, _)) = nearest.filter(|(_, s)| *s >= EXPLORE_SIM)
        && !lines.iter().any(|l| l.kind == WhyKind::Artist)
    {
        lines.push(line(
            WhyKind::Facet,
            format!("В духе твоей грани «{}»", f.label),
        ));
    }
    // Разведка: далеко от вкуса и не знакомо
    if !familiar
        && !matches!(origin.as_str(), "seed" | "query" | "new_release")
        && taste_sim.is_some_and(|s| s < EXPLORE_SIM)
    {
        lines.push(line(
            WhyKind::Explore,
            "Немного разведки: волна иногда пробует непривычное, чтобы не ходить по кругу",
        ));
    }
    // Время суток: грань, на которую похож трек, звучит у тебя в это время чаще обычного
    if let Some((f, _)) = nearest.filter(|(_, s)| *s >= EXPLORE_SIM) {
        let part = DayPart::of_hour(settings.local_hour(imp.shown_at));
        let lift = f
            .context
            .get("timeOfDay")
            .and_then(|t| t.get(part.as_str()))
            .and_then(Value::as_f64)
            .unwrap_or(1.0);
        if lift >= 1.25 {
            lines.push(line(
                WhyKind::Moment,
                format!("Такое ты чаще слушаешь {}", day_part_ru(part)),
            ));
        }
    }
    // Сеть вкуса подняла трек: похожее обычно дослушивают
    if num("model").is_some_and(|m| m >= MODEL_NOTE) {
        lines.push(line(
            WhyKind::Learned,
            "Похожее ты обычно дослушиваешь до конца",
        ));
    }
    if lines.is_empty() {
        lines.push(line(
            WhyKind::Facet,
            reason
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("Подобрано под твой вкус")
                .to_string(),
        ));
    }
    lines.truncate(MAX_LINES);

    let details = admin.then(|| {
        json!({
            "surface": imp.surface,
            "shownAt": imp.shown_at,
            "score": imp.score,
            "modelVersion": imp.model_version,
            "reason": { "kind": kind, "text": reason.get("text") },
            "why": why,
            "facet": nearest.map(|(f, s)| json!({ "label": f.label, "weight": round(f.weight), "sim": round(s) })),
            "history": { "plays": history.plays, "completed": history.completed, "lastPlayed": history.last_played },
        })
    });
    Ok(WhyTrack {
        track,
        lines,
        details,
    })
}

fn month_ru(t: DateTime<Utc>) -> String {
    const MONTHS: [&str; 12] = [
        "января",
        "февраля",
        "марта",
        "апреля",
        "мая",
        "июня",
        "июля",
        "августа",
        "сентября",
        "октября",
        "ноября",
        "декабря",
    ];
    let m = MONTHS[t.month0() as usize];
    if t.year() == Utc::now().year() {
        m.to_string()
    } else {
        format!("{m} {}", t.year())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_time_phrases() {
        let at = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc);
        // Москва (+180): 19:30 вчера — «вчера вечером»
        let now = at("2026-11-12T12:00:00Z");
        assert_eq!(when(at("2026-11-11T16:30:00Z"), now, 180), "вчера вечером");
        assert_eq!(when(at("2026-11-12T06:00:00Z"), now, 180), "сегодня утром");
        // 9 ноября 2026 — понедельник
        assert_eq!(
            when(at("2026-11-09T11:00:00Z"), now, 180),
            "в понедельник днём"
        );
        assert_eq!(when(at("2026-11-11T22:30:00Z"), now, 180), "сегодня ночью");
    }
}
