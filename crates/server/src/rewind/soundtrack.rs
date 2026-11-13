//! Музыка историй «Итогов года»: у каждого слайда свой трек года — трек года на своём слайде,
//! самое слушаемое у главного артиста, открытие на «Открытиях», любимое под характер слушателя — и место,
//! с которого он звучит: припев или самая энергичная часть по кривой энергии полного анализа, без
//! него — 30 % трека.
//!
//! Треков, которых нет на сервере, историям ждать нельзя: при открытии итогов (и ночью в декабре)
//! они ставятся на скачивание в полосу «сейчас» — первые слайды раньше.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    catalog::AudioStatus,
    jobs::AcquireReason,
    rewind::{RewindCue, RewindSoundtrack},
};
use sqlx::PgPool;
use uuid::Uuid;

use super::{Data, FIRST_YEAR, LISTEN_MS, MOODS, cached, local_year, range};
pub use crate::chorus::Shape;
use crate::{
    acquire::{self, Enqueue, Request},
    catalog::views,
    chorus,
    deezer::Deezer,
    jobs::lanes,
    reco::Settings,
};

/// После начала слайду нужно столько звука.
const TAIL_MS: i64 = 25_000;
/// Приоритет скачивания в полосе «сейчас»: ниже запуска трека (10) и срочной волны (6).
const PREFETCH_PRIORITY: i16 = 5;

/// Слайды по порядку историй.
pub const SLIDES: [&str; 12] = [
    "intro",
    "minutes",
    "days",
    "first",
    "tracks",
    "toy",
    "artists",
    "albums",
    "sound",
    "discoveries",
    "personality",
    "summary",
];

// ───────────── Место в треке ─────────────

/// Без анализа: 30 % трека, но так, чтобы слайду хватило звука.
pub fn fallback_start(duration_ms: i64) -> i64 {
    chorus::fallback_start(duration_ms, TAIL_MS)
}

/// Начало припева так, чтобы слайду хватило звука (см. [`chorus::chorus_start`]).
pub fn chorus_start(duration_ms: i64, shape: &Shape) -> Option<i64> {
    chorus::chorus_start(duration_ms, shape, TAIL_MS)
}

// ───────────── Какой трек на каком слайде ─────────────

/// Трек за год: прослушивания всего, ночью, утром и в волне.
#[derive(Debug, Clone, Default)]
pub struct Stat {
    pub id: Uuid,
    pub plays: i64,
    pub ms: i64,
    pub artist: Option<Uuid>,
    pub album: Option<Uuid>,
    pub night: i64,
    pub morning: i64,
    pub wave: i64,
    /// До этого года не играл.
    pub new: bool,
    /// Главное настроение по анализу.
    pub mood: Option<&'static str>,
    /// Звук уже на сервере — заиграет сразу, без ожидания скачивания.
    pub ready: bool,
}

/// Что историям известно о годе.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub top_tracks: Vec<Uuid>,
    pub track_of_year: Option<Uuid>,
    pub top_day: Option<Uuid>,
    pub first: Option<Uuid>,
    pub top_artist: Option<Uuid>,
    pub top_album: Option<Uuid>,
    pub discovery: Option<Uuid>,
    pub top_mood: Option<String>,
    pub personality: String,
}

impl Facts {
    fn of(data: &Data) -> Self {
        Self {
            top_tracks: data.top_tracks.iter().map(|t| t.id).collect(),
            track_of_year: data.track_of_year.as_ref().map(|t| t.track),
            top_day: data.top_day.as_ref().and_then(|d| d.track),
            first: data.first_track.map(|f| f.0),
            top_artist: data.top_artists.first().map(|a| a.id),
            top_album: data.top_albums.first().map(|a| a.id),
            discovery: data.discoveries.first().map(|d| d.artist),
            top_mood: data.moods.first().map(|m| m.id.clone()),
            personality: data.personality.id.clone(),
        }
    }
}

/// Трек каждого слайда. Трек года держит свой слайд и финал; остальные не повторяются — слайд без
/// своего трека продолжает прежний. Из нескольких подходящих сначала тот, чей звук уже на сервере:
/// иначе история ждёт скачивания и до тех пор тянет прежний трек. `stats` — по убыванию прослушиваний.
pub fn pick(facts: &Facts, stats: &[Stat]) -> Vec<(&'static str, Uuid)> {
    let toy = facts
        .track_of_year
        .or_else(|| facts.top_tracks.first().copied());
    let by = |f: &dyn Fn(&Stat) -> bool| -> Vec<Uuid> {
        stats.iter().filter(|s| f(s)).map(|s| s.id).collect()
    };
    let ranked = |key: &dyn Fn(&Stat) -> i64| -> Vec<Uuid> {
        let mut v: Vec<&Stat> = stats.iter().filter(|s| key(s) > 0).collect();
        v.sort_by(|a, b| key(b).cmp(&key(a)).then(b.plays.cmp(&a.plays)));
        v.into_iter().map(|s| s.id).collect()
    };
    let all = by(&|_| true);
    let ready: HashSet<Uuid> = by(&|s| s.ready).into_iter().collect();
    let mut used: HashSet<Uuid> = toy.into_iter().collect();
    let mut out = Vec::new();
    for slide in SLIDES {
        let (candidates, reserved): (Vec<Uuid>, bool) = match slide {
            "intro" => (facts.top_tracks.iter().skip(1).copied().collect(), false),
            "minutes" => (
                facts
                    .top_tracks
                    .iter()
                    .skip(2)
                    .copied()
                    .chain(all.iter().copied())
                    .collect(),
                false,
            ),
            "days" => (facts.top_day.into_iter().collect(), false),
            "first" => (facts.first.into_iter().collect(), false),
            "tracks" => (facts.top_tracks.clone(), false),
            "toy" | "summary" => (toy.into_iter().collect(), true),
            "artists" => (
                by(&|s| s.artist.is_some() && s.artist == facts.top_artist),
                false,
            ),
            "albums" => (
                by(&|s| s.album.is_some() && s.album == facts.top_album),
                false,
            ),
            "sound" => (
                by(&|s| s.mood.is_some() && s.mood == facts.top_mood.as_deref()),
                false,
            ),
            "discoveries" => (
                by(&|s| s.artist.is_some() && s.artist == facts.discovery),
                false,
            ),
            "personality" => (
                match facts.personality.as_str() {
                    "night_owl" => ranked(&|s| s.night),
                    "early_bird" => ranked(&|s| s.morning),
                    "wave_rider" => ranked(&|s| s.wave),
                    "explorer" => by(&|s| s.new),
                    _ => all.clone(),
                },
                false,
            ),
            _ => (Vec::new(), false),
        };
        let free = |id: &&Uuid| reserved || !used.contains(*id);
        let chosen = candidates
            .iter()
            .filter(free)
            .find(|id| ready.contains(*id))
            .or_else(|| candidates.iter().find(free))
            .copied();
        if let Some(id) = chosen {
            used.insert(id);
            out.push((slide, id));
        }
    }
    out
}

// ───────────── Из базы ─────────────

async fn stats(
    db: &PgPool,
    user_id: Uuid,
    (since, until): (DateTime<Utc>, DateTime<Utc>),
    utc_offset_min: i16,
) -> Result<Vec<Stat>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH p AS (
               SELECT p.track_id, p.played_ms, p.wave_session_id, p.context_type,
                      (p.played_ms >= $4 OR p.end_reason = 'completed') AS listened,
                      extract(hour FROM p.started_at + make_interval(mins => $5))::int AS hour
               FROM play_events p
               WHERE p.user_id = $1 AND p.started_at >= $2 AND p.started_at < $3
           )
           SELECT p.track_id AS "track_id!",
                  count(*) FILTER (WHERE p.listened) AS "plays!",
                  COALESCE(sum(p.played_ms), 0)::bigint AS "ms!",
                  count(*) FILTER (WHERE p.listened AND (p.hour >= 23 OR p.hour < 5)) AS "night!",
                  count(*) FILTER (WHERE p.listened AND p.hour BETWEEN 5 AND 11) AS "morning!",
                  count(*) FILTER (WHERE p.listened AND (p.wave_session_id IS NOT NULL
                                                         OR p.context_type IN ('wave', 'radio'))) AS "wave!",
                  t.album_id,
                  (SELECT ta.artist_id FROM track_artists ta
                   WHERE ta.track_id = p.track_id AND ta.role = 'main' ORDER BY ta.position LIMIT 1) AS "artist_id?",
                  NOT EXISTS (SELECT 1 FROM play_events e
                              WHERE e.user_id = $1 AND e.track_id = p.track_id AND e.started_at < $2) AS "new!",
                  r.audio_status AS "audio_status?"
           FROM p JOIN tracks t ON t.id = p.track_id
           LEFT JOIN recordings r ON r.id = t.recording_id
           GROUP BY p.track_id, t.album_id, r.audio_status
           HAVING count(*) FILTER (WHERE p.listened) > 0
           ORDER BY 2 DESC, 3 DESC, 1"#,
        user_id,
        since,
        until,
        LISTEN_MS,
        i32::from(utc_offset_min),
    )
    .fetch_all(db)
    .await?;
    // Записи нет в источниках или она ждёт решения в админке: играть историям нечего
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|r| !matches!(r.audio_status.as_deref(), Some("unavailable" | "disputed")))
        .collect();
    let ids: Vec<Uuid> = rows.iter().take(300).map(|r| r.track_id).collect();
    let moods: HashMap<Uuid, &'static str> = sqlx::query!(
        r#"SELECT t.id, a.moods AS "moods!" FROM tracks t JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = ANY($1) AND a.moods IS NOT NULL"#,
        &ids,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .filter_map(|r| {
        let top = MOODS
            .iter()
            .map(|m| (*m, r.moods.get(m).and_then(|v| v.as_f64()).unwrap_or(0.0)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, p)| *p > 0.0)?;
        Some((r.id, top.0))
    })
    .collect();
    Ok(rows
        .into_iter()
        .map(|r| Stat {
            id: r.track_id,
            plays: r.plays,
            ms: r.ms,
            artist: r.artist_id,
            album: r.album_id,
            night: r.night,
            morning: r.morning,
            wave: r.wave,
            new: r.new,
            mood: moods.get(&r.track_id).copied(),
            ready: r.audio_status.as_deref() == Some("ready"),
        })
        .collect())
}

/// Музыка историй года. `None` — год ещё не наступил или раньше [`FIRST_YEAR`].
pub async fn get(
    db: &PgPool,
    user_id: Uuid,
    year: i32,
    now: DateTime<Utc>,
) -> Result<Option<RewindSoundtrack>, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let off = settings.utc_offset_min;
    if year < FIRST_YEAR || year > local_year(now, off) {
        return Ok(None);
    }
    let (data, _) = cached(db, user_id, year, &settings, now).await?;
    let stats = stats(db, user_id, range(year, now, off), off).await?;
    let picks = pick(&Facts::of(&data), &stats);
    let mut ids: Vec<Uuid> = picks.iter().map(|p| p.1).collect();
    ids.dedup();
    let tracks: HashMap<Uuid, _> = views::tracks(db, &ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let starts = chorus::starts(db, &ids, TAIL_MS).await?;
    let cues = picks
        .into_iter()
        .filter_map(|(slide, id)| {
            let track = tracks.get(&id)?.clone();
            let (start_ms, analysed) = starts
                .get(&id)
                .copied()
                .unwrap_or_else(|| (fallback_start(i64::from(track.duration_ms)), false));
            Some(RewindCue {
                slide: slide.into(),
                track,
                start_ms,
                start_from: if analysed { "analysis" } else { "estimate" }.into(),
            })
        })
        .collect();
    Ok(Some(RewindSoundtrack { year, cues }))
}

/// Музыка историй года — на сервер для всех, кто слушал в этом году: зовётся, когда итоги открывают
/// из админки, чтобы к первому просмотру звук был, а не догонял. Сколько треков поставлено.
pub async fn prefetch_year(
    db: &PgPool,
    deezer: &Deezer,
    year: i32,
    now: DateTime<Utc>,
) -> Result<usize, sqlx::Error> {
    let off = Settings::load(db).await?.utc_offset_min;
    let (since, until) = range(year, now, off);
    let users = sqlx::query_scalar!(
        "SELECT u.id FROM users u
         WHERE NOT u.disabled
           AND EXISTS (SELECT 1 FROM play_events p
                       WHERE p.user_id = u.id AND p.started_at >= $1 AND p.started_at < $2)",
        since,
        until,
    )
    .fetch_all(db)
    .await?;
    let mut queued = 0;
    for user in users {
        if let Some(st) = get(db, user, year, now).await? {
            queued += prefetch(db, Some(deezer), user, &st.cues).await;
        }
    }
    Ok(queued)
}

/// Треки музыки историй, которых нет на сервере, — на скачивание в полосу «сейчас», первые слайды
/// раньше. Без Deezer (ночью из воркера) — только треки с известной записью. Сколько поставлено.
pub async fn prefetch(
    db: &PgPool,
    deezer: Option<&Deezer>,
    user_id: Uuid,
    cues: &[RewindCue],
) -> usize {
    let mut seen = HashSet::new();
    let mut queued = 0;
    for (i, cue) in cues.iter().enumerate() {
        let track = &cue.track;
        if track.audio_status == AudioStatus::Ready || !seen.insert(track.id) {
            continue;
        }
        let priority = (PREFETCH_PRIORITY - i16::try_from(i).unwrap_or(i16::MAX)).max(1);
        let req = Request::new(track.id, AcquireReason::Play, lanes::NOW)
            .by(user_id)
            .priority(priority);
        let res = match deezer {
            Some(d) => acquire::request(db, d, req)
                .await
                .map_err(|e| e.to_string()),
            None => acquire::enqueue(db, req).await.map_err(|e| e.to_string()),
        };
        match res {
            Ok(Enqueue::Queued { .. } | Enqueue::Active { .. }) => queued += 1,
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(track = %track.id, error = %e, "could not prefetch a year-in-review track")
            }
        }
    }
    queued
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slides_get_25_seconds_of_sound() {
        let mut curve = vec![200i16; 100];
        curve[70..90].fill(900);
        let shape = Shape {
            curve,
            ..Default::default()
        };
        assert_eq!(chorus_start(100_000, &shape), Some(70_000));
        assert_eq!(fallback_start(200_000), 60_000);
        assert_eq!(fallback_start(30_000), 5_000);
    }

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn stat(n: u128, plays: i64) -> Stat {
        Stat {
            id: id(n),
            plays,
            ms: plays * 180_000,
            ..Default::default()
        }
    }

    #[test]
    fn every_slide_gets_its_own_track() {
        let mut stats: Vec<Stat> = (1..=10).map(|n| stat(n, 100 - n as i64)).collect();
        stats[5].artist = Some(id(100));
        stats[6].album = Some(id(200));
        stats[7].mood = Some("party");
        stats[8].artist = Some(id(300));
        stats[9].night = 5;
        stats[3].night = 2;
        let facts = Facts {
            top_tracks: (1..=5).map(id).collect(),
            track_of_year: Some(id(1)),
            top_day: Some(id(3)),
            first: Some(id(9)),
            top_artist: Some(id(100)),
            top_album: Some(id(200)),
            discovery: Some(id(300)),
            top_mood: Some("party".into()),
            personality: "night_owl".into(),
        };
        let picks = pick(&facts, &stats);
        let get = |slide: &str| picks.iter().find(|p| p.0 == slide).map(|p| p.1);
        assert_eq!(get("toy"), Some(id(1)));
        assert_eq!(get("summary"), Some(id(1)), "финал — снова трек года");
        assert_eq!(get("intro"), Some(id(2)));
        assert_eq!(get("minutes"), Some(id(3)));
        // Самый громкий день — тот же трек, что на «Минутах»: слайд продолжает прежний
        assert_eq!(get("days"), None);
        assert_eq!(get("first"), Some(id(9)));
        assert_eq!(get("tracks"), Some(id(4)));
        assert_eq!(get("artists"), Some(id(6)));
        assert_eq!(get("albums"), Some(id(7)));
        assert_eq!(get("sound"), Some(id(8)));
        // Открытие — трек 9 уже на «Первом треке»
        assert_eq!(get("discoveries"), None);
        // Ночной слушатель: больше всего ночью — трек 10
        assert_eq!(get("personality"), Some(id(10)));
        // Порядок — как у слайдов
        let order: Vec<&str> = picks.iter().map(|p| p.0).collect();
        let mut sorted = order.clone();
        sorted.sort_by_key(|s| SLIDES.iter().position(|x| x == s));
        assert_eq!(order, sorted);
    }

    #[test]
    fn a_slide_prefers_a_track_already_on_the_server() {
        let mut stats: Vec<Stat> = (1..=6).map(|n| stat(n, 100 - n as i64)).collect();
        // Альбом года: самый слушаемый трек ещё не скачан, второй — на сервере
        stats[3].album = Some(id(200));
        stats[4].album = Some(id(200));
        // На сервере — треки 2, 3 и 5: «Вступлению» и «Минутам» хватает своих
        for i in [1, 2, 4] {
            stats[i].ready = true;
        }
        let facts = Facts {
            top_tracks: (1..=3).map(id).collect(),
            track_of_year: Some(id(1)),
            first: Some(id(6)),
            top_album: Some(id(200)),
            personality: "balanced".into(),
            ..Default::default()
        };
        let picks = pick(&facts, &stats);
        let get = |slide: &str| picks.iter().find(|p| p.0 == slide).map(|p| p.1);
        assert_eq!(get("albums"), Some(id(5)));
        assert_eq!(get("intro"), Some(id(2)));
        assert_eq!(get("minutes"), Some(id(3)));
        // Единственный подходящий трек остаётся, даже не скачанный: слайд о нём
        assert_eq!(get("first"), Some(id(6)));
        assert_eq!(get("toy"), Some(id(1)));
    }

    #[test]
    fn a_single_track_year_still_has_music() {
        let stats = vec![stat(1, 3)];
        let facts = Facts {
            top_tracks: vec![id(1)],
            track_of_year: Some(id(1)),
            personality: "balanced".into(),
            ..Default::default()
        };
        let picks = pick(&facts, &stats);
        assert_eq!(picks, vec![("toy", id(1)), ("summary", id(1))]);
        assert!(pick(&Facts::default(), &[]).is_empty());
    }
}
