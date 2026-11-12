//! «Итоги года»: год слушателя для историй. Считается по `play_events` (с импортированной историей Spotify
//! — она входит в год, когда трек играл), лайкам и анализу звука; год — по часовому поясу слушателя
//! (`reco_settings.utc_offset_min`).
//!
//! Посчитанное кешируется в `rewinds` (только id и числа) с отпечатком данных года: изменились
//! прослушивания или лайки — пересчёт при следующем запросе. В декабре ночью итоги текущего года
//! пересчитываются заранее ([`warm`]), чтобы утром истории открывались сразу.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
use dalywave_protocol::{
    library::PlaylistSummary,
    realtime::{LibraryChange, ServerEvent},
    rewind::{
        Rewind, RewindAlbum, RewindArtist, RewindDay, RewindDiscovery, RewindFirst, RewindGenre,
        RewindMonth, RewindMood, RewindPersonality, RewindStreak, RewindTrack, RewindTrackOfYear,
        RewindYear, RewindYears,
    },
    sync::SyncEntity,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    catalog::views,
    realtime::{self, Audience},
    reco::{DayPart, Settings, profile::genre_ru, taste::style_ru},
    routes::playlists,
};

/// Сколько позиций в топах историй.
pub const TOP: usize = 5;
/// Треков в плейлисте «Твои лучшие треки».
pub const PLAYLIST_SIZE: i64 = 50;
/// Прослушивание считается, если сыграно от 30 секунд (или трек дослушан — короткий).
pub const LISTEN_MS: i32 = 30_000;
/// Самый ранний год итогов.
pub const FIRST_YEAR: i32 = 2000;
/// Меняется вместе с правилами подсчёта: старые строки кеша пересчитываются.
const VERSION: u32 = 1;
/// Стилей трека в жанрах года: верхние по вероятности.
const STYLES_PER_TRACK: usize = 3;
/// Открытие года: столько прослушиваний — уже любимый артист (или лайк, или подписка).
const DISCOVERY_PLAYS: i64 = 3;
pub const MOODS: [&str; 5] = ["happy", "sad", "aggressive", "relaxed", "party"];

fn offset(utc_offset_min: i16) -> Duration {
    Duration::minutes(i64::from(utc_offset_min))
}

/// Дата по времени слушателя.
pub fn local_date(at: DateTime<Utc>, utc_offset_min: i16) -> NaiveDate {
    (at + offset(utc_offset_min)).date_naive()
}

/// Год по времени слушателя.
pub fn local_year(at: DateTime<Utc>, utc_offset_min: i16) -> i32 {
    local_date(at, utc_offset_min).year()
}

/// Начало и конец года по времени слушателя (в UTC), конец не входит.
pub fn year_bounds(year: i32, utc_offset_min: i16) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = |y: i32| {
        let d = NaiveDate::from_ymd_opt(y, 1, 1).unwrap_or_default();
        Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default()) - offset(utc_offset_min)
    };
    (start(year), start(year + 1))
}

/// Что входит в итоги: год, а у идущего — по эту минуту (из будущего ничего не попадает).
pub fn range(year: i32, now: DateTime<Utc>, utc_offset_min: i16) -> (DateTime<Utc>, DateTime<Utc>) {
    let (since, until) = year_bounds(year, utc_offset_min);
    (since, until.min(now).max(since))
}

/// Сезон итогов: с 1 декабря этого года (и всегда — за прошедшие годы).
pub fn in_season(year: i32, now: DateTime<Utc>, utc_offset_min: i16) -> bool {
    let today = local_date(now, utc_offset_min);
    today.year() > year || (today.year() == year && today.month() == 12)
}

/// Самая длинная серия дней подряд: (дней, с, по). Дни — по возрастанию, без повторов.
pub fn longest_streak(days: &[NaiveDate]) -> Option<(i64, NaiveDate, NaiveDate)> {
    let mut best: Option<(i64, NaiveDate, NaiveDate)> = None;
    let mut run: Option<(i64, NaiveDate, NaiveDate)> = None;
    for &d in days {
        run = match run {
            Some((n, from, to)) if to.succ_opt() == Some(d) => Some((n + 1, from, d)),
            _ => Some((1, d, d)),
        };
        if let Some(r) = run
            && best.is_none_or(|b| r.0 > b.0)
        {
            best = Some(r);
        }
    }
    best
}

/// Черты слушателя за год, 0..1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Traits {
    pub night: f32,
    pub morning: f32,
    /// `None` — истории до этого года нет: всё «новое», и это ничего не говорит.
    pub novelty: Option<f32>,
    pub repeat: f32,
    pub wave: f32,
}

/// Характер слушателя: черта, которая сильнее всего выше обычного; ни одна не выше — `balanced`.
pub fn personality(t: &Traits) -> &'static str {
    // Обычные доли: ночью (6 часов из 24) слушают меньше, чем днём; повторы у всех велики
    let candidates = [
        ("night_owl", t.night / 0.22),
        ("early_bird", t.morning / 0.4),
        ("explorer", t.novelty.map_or(0.0, |n| n / 0.55)),
        ("loyal", t.repeat / 0.8),
        ("wave_rider", t.wave / 0.35),
    ];
    candidates
        .iter()
        .filter(|(_, ratio)| *ratio >= 1.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or("balanced", |(id, _)| id)
}

fn listened(played_ms: i32, end_reason: &str) -> bool {
    played_ms >= LISTEN_MS || end_reason == "completed"
}

// ───────────── Кеш: id и числа ─────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Counted {
    id: Uuid,
    plays: i64,
    ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DayData {
    date: NaiveDate,
    ms: i64,
    plays: i64,
    track: Option<Uuid>,
    track_plays: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveryData {
    artist: Uuid,
    first_heard: NaiveDate,
    plays: i64,
    likes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackOfYearData {
    track: Uuid,
    plays: i64,
    ms: i64,
    first_played: NaiveDate,
    peak_month: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Data {
    ms: i64,
    plays: i64,
    tracks: i64,
    artists: i64,
    new_artists: i64,
    days: i64,
    likes: i64,
    wave_sessions: i64,
    top_tracks: Vec<Counted>,
    top_artists: Vec<Counted>,
    top_albums: Vec<Counted>,
    genres: Vec<RewindGenre>,
    moods: Vec<RewindMood>,
    months: Vec<RewindMonth>,
    personality: RewindPersonality,
    first_track: Option<(Uuid, DateTime<Utc>)>,
    top_day: Option<DayData>,
    streak: Option<RewindStreak>,
    discoveries: Vec<DiscoveryData>,
    track_of_year: Option<TrackOfYearData>,
}

struct Play {
    track: Uuid,
    at: DateTime<Utc>,
    ms: i64,
    listened: bool,
    wave: bool,
    wave_session: Option<Uuid>,
    album: Option<Uuid>,
    artist: Option<Uuid>,
}

/// Топ по прослушиваниям, при равенстве — по времени; `limit` позиций.
fn top(counts: HashMap<Uuid, (i64, i64)>, limit: usize) -> Vec<Counted> {
    let mut v: Vec<Counted> = counts
        .into_iter()
        .filter(|(_, (plays, _))| *plays > 0)
        .map(|(id, (plays, ms))| Counted { id, plays, ms })
        .collect();
    v.sort_by(|a, b| {
        b.plays
            .cmp(&a.plays)
            .then(b.ms.cmp(&a.ms))
            .then(a.id.cmp(&b.id))
    });
    v.truncate(limit);
    v
}

/// Отпечаток данных года: меняется с каждым новым прослушиванием (и импортом) и лайком.
async fn fingerprint(
    db: &PgPool,
    user_id: Uuid,
    since: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Result<String, sqlx::Error> {
    let r = sqlx::query!(
        r#"SELECT count(*) AS "n!", COALESCE(sum(played_ms), 0)::bigint AS "ms!", max(created_at) AS last,
                  (SELECT count(*) FROM liked_tracks WHERE user_id = $1) AS "likes!",
                  (SELECT max(created_at) FROM liked_tracks WHERE user_id = $1) AS liked_at,
                  (SELECT count(*) FROM followed_artists WHERE user_id = $1) AS "follows!"
           FROM play_events WHERE user_id = $1 AND started_at >= $2 AND started_at < $3"#,
        user_id,
        since,
        until,
    )
    .fetch_one(db)
    .await?;
    let stamp = |t: Option<DateTime<Utc>>| t.map_or(0, |t| t.timestamp_micros());
    Ok(format!(
        "v{VERSION}:{}:{}:{}:{}:{}:{}",
        r.n,
        r.ms,
        stamp(r.last),
        r.likes,
        stamp(r.liked_at),
        r.follows
    ))
}

async fn compute(
    db: &PgPool,
    user_id: Uuid,
    (since, until): (DateTime<Utc>, DateTime<Utc>),
    utc_offset_min: i16,
) -> Result<Data, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT p.track_id, p.started_at, p.played_ms, p.end_reason, p.context_type, p.wave_session_id,
                  t.album_id,
                  (SELECT ta.artist_id FROM track_artists ta
                   WHERE ta.track_id = p.track_id AND ta.role = 'main' ORDER BY ta.position LIMIT 1) AS "artist_id?"
           FROM play_events p JOIN tracks t ON t.id = p.track_id
           WHERE p.user_id = $1 AND p.started_at >= $2 AND p.started_at < $3
           ORDER BY p.started_at, p.id"#,
        user_id,
        since,
        until,
    )
    .fetch_all(db)
    .await?;
    let plays: Vec<Play> = rows
        .into_iter()
        .map(|r| Play {
            track: r.track_id,
            at: r.started_at,
            ms: i64::from(r.played_ms.max(0)),
            listened: listened(r.played_ms, &r.end_reason),
            wave: r.wave_session_id.is_some()
                || matches!(r.context_type.as_deref(), Some("wave" | "radio")),
            wave_session: r.wave_session_id,
            album: r.album_id,
            artist: r.artist_id,
        })
        .collect();

    let earlier_tracks: HashSet<Uuid> = sqlx::query_scalar!(
        "SELECT DISTINCT track_id FROM play_events WHERE user_id = $1 AND started_at < $2",
        user_id,
        since,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .collect();
    let earlier_artists: HashSet<Uuid> = sqlx::query_scalar!(
        "SELECT DISTINCT ta.artist_id FROM play_events p
         JOIN track_artists ta ON ta.track_id = p.track_id AND ta.role = 'main'
         WHERE p.user_id = $1 AND p.started_at < $2",
        user_id,
        since,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .collect();
    let likes = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM liked_tracks WHERE user_id = $1 AND created_at >= $2 AND created_at < $3"#,
        user_id,
        since,
        until,
    )
    .fetch_one(db)
    .await?;

    let total_ms: i64 = plays.iter().map(|p| p.ms).sum();
    let mut by_track: HashMap<Uuid, (i64, i64)> = HashMap::new();
    let mut by_artist: HashMap<Uuid, (i64, i64)> = HashMap::new();
    let mut by_album: HashMap<Uuid, (i64, i64)> = HashMap::new();
    let mut by_day: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    let mut month_ms = [0i64; 12];
    let mut part_ms: HashMap<DayPart, i64> = HashMap::new();
    let mut artist_first: HashMap<Uuid, NaiveDate> = HashMap::new();
    let mut wave_ms = 0i64;
    let mut sessions: HashSet<Uuid> = HashSet::new();
    let mut first_track = None;
    for p in &plays {
        let local = p.at + offset(utc_offset_min);
        let date = local.date_naive();
        month_ms[local.month0() as usize] += p.ms;
        *part_ms.entry(DayPart::of_hour(local.hour())).or_default() += p.ms;
        if p.wave {
            wave_ms += p.ms;
        }
        if let Some(s) = p.wave_session {
            sessions.insert(s);
        }
        let l = i64::from(p.listened);
        let add = |m: &mut HashMap<Uuid, (i64, i64)>, id: Uuid| {
            let e = m.entry(id).or_default();
            e.0 += l;
            e.1 += p.ms;
        };
        add(&mut by_track, p.track);
        if let Some(a) = p.artist {
            add(&mut by_artist, a);
            if p.listened {
                artist_first.entry(a).or_insert(date);
            }
        }
        if let Some(a) = p.album {
            add(&mut by_album, a);
        }
        if p.listened {
            let d = by_day.entry(date).or_default();
            d.0 += p.ms;
            d.1 += 1;
            if first_track.is_none() {
                first_track = Some((p.track, p.at));
            }
        }
    }
    let listened_plays: i64 = by_track.values().map(|v| v.0).sum();
    let listened_tracks: Vec<Uuid> = by_track
        .iter()
        .filter(|(_, v)| v.0 > 0)
        .map(|(id, _)| *id)
        .collect();
    let artists_heard: Vec<Uuid> = by_artist
        .iter()
        .filter(|(_, v)| v.0 > 0)
        .map(|(id, _)| *id)
        .collect();

    // Самый громкий день и что в нём играло чаще всего
    let top_day = by_day
        .iter()
        .max_by(|a, b| a.1.0.cmp(&b.1.0).then(b.0.cmp(a.0)))
        .map(|(date, (ms, n))| {
            let mut that_day: HashMap<Uuid, i64> = HashMap::new();
            for p in plays
                .iter()
                .filter(|p| p.listened && local_date(p.at, utc_offset_min) == *date)
            {
                *that_day.entry(p.track).or_default() += 1;
            }
            let best = that_day
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
            DayData {
                date: *date,
                ms: *ms,
                plays: *n,
                track: best.map(|b| b.0),
                track_plays: best.map_or(0, |b| b.1),
            }
        });
    let days: Vec<NaiveDate> = by_day.keys().copied().collect();
    let streak = longest_streak(&days).map(|(days, from, to)| RewindStreak { days, from, to });

    // Открытия: впервые услышаны в этом году и стали любимыми
    let new_artists: Vec<Uuid> = artists_heard
        .iter()
        .filter(|a| !earlier_artists.contains(a))
        .copied()
        .collect();
    let liked_by_artist: HashMap<Uuid, i64> = sqlx::query!(
        r#"SELECT ta.artist_id, count(DISTINCT l.track_id) AS "n!" FROM liked_tracks l
           JOIN track_artists ta ON ta.track_id = l.track_id AND ta.role = 'main'
           WHERE l.user_id = $1 AND ta.artist_id = ANY($2) GROUP BY ta.artist_id"#,
        user_id,
        &new_artists,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.artist_id, r.n))
    .collect();
    let followed: HashSet<Uuid> = sqlx::query_scalar!(
        "SELECT artist_id FROM followed_artists WHERE user_id = $1 AND artist_id = ANY($2)",
        user_id,
        &new_artists,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .collect();
    let mut discoveries: Vec<DiscoveryData> = new_artists
        .iter()
        .filter_map(|a| {
            let plays = by_artist.get(a).map_or(0, |v| v.0);
            let likes = liked_by_artist.get(a).copied().unwrap_or(0);
            (plays >= DISCOVERY_PLAYS || likes > 0 || followed.contains(a)).then(|| DiscoveryData {
                artist: *a,
                first_heard: artist_first.get(a).copied().unwrap_or_default(),
                plays,
                likes,
            })
        })
        .collect();
    discoveries.sort_by(|a, b| {
        b.plays
            .cmp(&a.plays)
            .then(b.likes.cmp(&a.likes))
            .then(a.artist.cmp(&b.artist))
    });
    discoveries.truncate(TOP);

    // Жанры и настроение — по анализу звука, без него — жанры Deezer у альбома
    let analysis = sqlx::query!(
        r#"SELECT t.id, a.styles, a.moods FROM unnest($1::uuid[]) AS ids(id)
           JOIN tracks t ON t.id = ids.id JOIN track_analysis a ON a.recording_id = t.recording_id"#,
        &by_track.keys().copied().collect::<Vec<_>>(),
    )
    .fetch_all(db)
    .await?;
    let analysis: HashMap<Uuid, (Option<Value>, Option<Value>)> = analysis
        .into_iter()
        .map(|r| (r.id, (r.styles, r.moods)))
        .collect();
    let album_genres: HashMap<Uuid, Vec<String>> = sqlx::query!(
        "SELECT id, genres FROM albums WHERE id = ANY($1)",
        &by_album.keys().copied().collect::<Vec<_>>(),
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.genres))
    .collect();
    let track_album: HashMap<Uuid, Uuid> = plays
        .iter()
        .filter_map(|p| Some((p.track, p.album?)))
        .collect();
    let mut genre_w: HashMap<String, (String, f64)> = HashMap::new();
    let mut mood_w: HashMap<&str, f64> = HashMap::new();
    let mut mood_total = 0.0f64;
    for (track, (_, ms)) in &by_track {
        let ms = *ms as f64;
        let (styles, moods) = analysis.get(track).cloned().unwrap_or((None, None));
        let mut styled = false;
        if let Some(Value::Array(list)) = styles {
            let mut list: Vec<(String, f64)> = list
                .iter()
                .filter_map(|s| Some((s.get("name")?.as_str()?.to_string(), s.get("p")?.as_f64()?)))
                .collect();
            list.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (raw, p) in list.into_iter().take(STYLES_PER_TRACK) {
                let e = genre_w.entry(style_ru(&raw)).or_insert((raw, 0.0));
                e.1 += ms * p;
                styled = true;
            }
        }
        if !styled
            && let Some(genres) = track_album.get(track).and_then(|a| album_genres.get(a))
            && !genres.is_empty()
        {
            // Без анализа жанр Deezer весит как уверенный стиль
            let share = 0.3 / genres.len() as f64;
            for g in genres {
                let e = genre_w.entry(genre_ru(g)).or_insert((g.clone(), 0.0));
                e.1 += ms * share;
            }
        }
        if let Some(Value::Object(m)) = moods {
            mood_total += ms;
            for id in MOODS {
                *mood_w.entry(id).or_default() +=
                    ms * m.get(id).and_then(Value::as_f64).unwrap_or(0.0);
            }
        }
    }
    let genre_total: f64 = genre_w.values().map(|v| v.1).sum();
    let mut genres: Vec<RewindGenre> = genre_w
        .into_iter()
        .filter(|(_, (_, w))| *w > 0.0)
        .map(|(name, (id, w))| RewindGenre {
            id,
            name,
            share: (w / genre_total.max(1e-9)) as f32,
        })
        .collect();
    genres.sort_by(|a, b| b.share.total_cmp(&a.share).then(a.name.cmp(&b.name)));
    genres.truncate(TOP);
    let mut moods: Vec<RewindMood> = if mood_total > 0.0 {
        MOODS
            .iter()
            .map(|id| RewindMood {
                id: (*id).into(),
                score: (mood_w.get(id).copied().unwrap_or(0.0) / mood_total) as f32,
            })
            .collect()
    } else {
        Vec::new()
    };
    moods.sort_by(|a, b| b.score.total_cmp(&a.score));

    // Характер
    let share = |ms: i64| {
        if total_ms > 0 {
            ms as f32 / total_ms as f32
        } else {
            0.0
        }
    };
    let part_share = |p: DayPart| share(part_ms.get(&p).copied().unwrap_or(0));
    let new_tracks = listened_tracks
        .iter()
        .filter(|t| !earlier_tracks.contains(t))
        .count();
    let traits = Traits {
        night: part_share(DayPart::Night),
        morning: part_share(DayPart::Morning),
        novelty: (!earlier_tracks.is_empty() && !listened_tracks.is_empty())
            .then(|| new_tracks as f32 / listened_tracks.len() as f32),
        repeat: if listened_plays > 0 {
            1.0 - listened_tracks.len() as f32 / listened_plays as f32
        } else {
            0.0
        },
        wave: share(wave_ms),
    };
    let peak_part = DayPart::ALL
        .iter()
        .copied()
        .max_by(|a, b| part_share(*a).total_cmp(&part_share(*b)))
        .filter(|_| total_ms > 0)
        .unwrap_or(DayPart::Evening);
    let personality = RewindPersonality {
        id: personality(&traits).into(),
        night_share: traits.night,
        morning_share: traits.morning,
        novelty_share: if listened_tracks.is_empty() {
            0.0
        } else {
            new_tracks as f32 / listened_tracks.len() as f32
        },
        repeat_share: traits.repeat,
        wave_share: traits.wave,
        peak_part: peak_part.as_str().into(),
    };

    let top_tracks = top(by_track.clone(), TOP);
    let track_of_year = top_tracks.first().map(|t| {
        let mut months = [0i64; 12];
        let mut first = None;
        for p in plays.iter().filter(|p| p.track == t.id && p.listened) {
            let local = p.at + offset(utc_offset_min);
            months[local.month0() as usize] += 1;
            first.get_or_insert(local.date_naive());
        }
        let peak = (0..12)
            .max_by(|a, b| months[*a].cmp(&months[*b]).then(b.cmp(a)))
            .unwrap_or(0);
        TrackOfYearData {
            track: t.id,
            plays: t.plays,
            ms: t.ms,
            first_played: first.unwrap_or_default(),
            peak_month: peak as u32 + 1,
        }
    });

    Ok(Data {
        ms: total_ms,
        plays: listened_plays,
        tracks: listened_tracks.len() as i64,
        artists: artists_heard.len() as i64,
        new_artists: new_artists.len() as i64,
        days: by_day.len() as i64,
        likes,
        wave_sessions: sessions.len() as i64,
        top_tracks,
        top_artists: top(by_artist, TOP),
        top_albums: top(by_album, TOP),
        genres,
        moods,
        months: month_ms
            .iter()
            .enumerate()
            .map(|(i, ms)| RewindMonth {
                month: i as u32 + 1,
                minutes: ms / 60_000,
            })
            .collect(),
        personality,
        first_track,
        top_day,
        streak,
        discoveries,
        track_of_year,
    })
}

async fn store(
    db: &PgPool,
    user_id: Uuid,
    year: i32,
    fingerprint: &str,
    data: &Data,
) -> Result<DateTime<Utc>, sqlx::Error> {
    let value = serde_json::to_value(data).unwrap_or(Value::Null);
    sqlx::query_scalar!(
        "INSERT INTO rewinds (user_id, year, fingerprint, data, computed_at) VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (user_id, year) DO UPDATE
         SET fingerprint = EXCLUDED.fingerprint, data = EXCLUDED.data, computed_at = now()
         RETURNING computed_at",
        user_id,
        year,
        fingerprint,
        value,
    )
    .fetch_one(db)
    .await
}

/// Посчитанный год из кеша, если данные не менялись, иначе — заново.
async fn cached(
    db: &PgPool,
    user_id: Uuid,
    year: i32,
    settings: &Settings,
    now: DateTime<Utc>,
) -> Result<(Data, DateTime<Utc>), sqlx::Error> {
    let (since, until) = range(year, now, settings.utc_offset_min);
    let fp = fingerprint(db, user_id, since, until).await?;
    let row = sqlx::query!(
        "SELECT fingerprint, data, computed_at FROM rewinds WHERE user_id = $1 AND year = $2",
        user_id,
        year,
    )
    .fetch_optional(db)
    .await?;
    if let Some(r) = row
        && r.fingerprint == fp
        && let Ok(data) = serde_json::from_value::<Data>(r.data)
    {
        return Ok((data, r.computed_at));
    }
    let data = compute(db, user_id, (since, until), settings.utc_offset_min).await?;
    let at = store(db, user_id, year, &fp, &data).await?;
    Ok((data, at))
}

async fn playlist_of(db: &PgPool, user_id: Uuid, year: i32) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT id FROM playlists WHERE user_id = $1 AND rules ->> 'generator' = 'rewind'
           AND rules ->> 'year' = $2 ORDER BY created_at LIMIT 1",
        user_id,
        year.to_string(),
    )
    .fetch_optional(db)
    .await
}

/// Итоги года слушателя. `None` — год ещё не наступил или раньше [`FIRST_YEAR`].
pub async fn get(
    db: &PgPool,
    user_id: Uuid,
    year: i32,
    now: DateTime<Utc>,
) -> Result<Option<Rewind>, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let off = settings.utc_offset_min;
    if year < FIRST_YEAR || year > local_year(now, off) {
        return Ok(None);
    }
    let (data, computed_at) = cached(db, user_id, year, &settings, now).await?;
    let (since, until) = year_bounds(year, off);
    let complete = now >= until;
    let days_in_range = if complete {
        (until - since).num_days()
    } else {
        i64::from(local_date(now, off).ordinal())
    };

    let mut track_ids: Vec<Uuid> = data.top_tracks.iter().map(|t| t.id).collect();
    track_ids.extend(data.first_track.map(|f| f.0));
    track_ids.extend(data.top_day.as_ref().and_then(|d| d.track));
    let tracks: HashMap<Uuid, _> = views::tracks(db, &track_ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let mut artist_ids: Vec<Uuid> = data.top_artists.iter().map(|a| a.id).collect();
    artist_ids.extend(data.discoveries.iter().map(|d| d.artist));
    let artists: HashMap<Uuid, _> = views::artists(db, &artist_ids, user_id)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let album_ids: Vec<Uuid> = data.top_albums.iter().map(|a| a.id).collect();
    let albums: HashMap<Uuid, _> = views::albums(db, &album_ids, user_id)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();

    Ok(Some(Rewind {
        year,
        complete,
        season: in_season(year, now, off),
        computed_at,
        minutes: data.ms / 60_000,
        plays: data.plays,
        tracks: data.tracks,
        artists: data.artists,
        new_artists: data.new_artists,
        days: data.days,
        days_in_range,
        likes: data.likes,
        wave_sessions: data.wave_sessions,
        top_tracks: data
            .top_tracks
            .iter()
            .filter_map(|c| {
                Some(RewindTrack {
                    track: tracks.get(&c.id)?.clone(),
                    plays: c.plays,
                    minutes: c.ms / 60_000,
                })
            })
            .collect(),
        top_artists: data
            .top_artists
            .iter()
            .filter_map(|c| {
                Some(RewindArtist {
                    artist: artists.get(&c.id)?.clone(),
                    plays: c.plays,
                    minutes: c.ms / 60_000,
                })
            })
            .collect(),
        top_albums: data
            .top_albums
            .iter()
            .filter_map(|c| {
                Some(RewindAlbum {
                    album: albums.get(&c.id)?.clone(),
                    plays: c.plays,
                    minutes: c.ms / 60_000,
                })
            })
            .collect(),
        genres: data.genres,
        moods: data.moods,
        months: data.months,
        personality: data.personality,
        first_track: data.first_track.and_then(|(id, at)| {
            Some(RewindFirst {
                track: tracks.get(&id)?.clone(),
                at,
            })
        }),
        top_day: data.top_day.map(|d| RewindDay {
            date: d.date,
            minutes: d.ms / 60_000,
            plays: d.plays,
            track: d.track.and_then(|id| tracks.get(&id).cloned()),
            track_plays: d.track_plays,
        }),
        streak: data.streak,
        discoveries: data
            .discoveries
            .iter()
            .filter_map(|d| {
                Some(RewindDiscovery {
                    artist: artists.get(&d.artist)?.clone(),
                    first_heard: d.first_heard,
                    plays: d.plays,
                    likes: d.likes,
                })
            })
            .collect(),
        track_of_year: data.track_of_year.and_then(|t| {
            Some(RewindTrackOfYear {
                track: tracks.get(&t.track)?.clone(),
                plays: t.plays,
                minutes: t.ms / 60_000,
                first_played: t.first_played,
                peak_month: t.peak_month,
            })
        }),
        playlist_id: playlist_of(db, user_id, year).await?,
    }))
}

/// Годы с прослушиваниями (новые первыми) и текущий год слушателя — он есть всегда, итоги «по сей день».
pub async fn years(
    db: &PgPool,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<RewindYears, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let off = settings.utc_offset_min;
    let current = local_year(now, off);
    let rows = sqlx::query!(
        r#"SELECT extract(year FROM started_at + make_interval(mins => $2))::int AS "year!",
                  (sum(played_ms) / 60000)::bigint AS "minutes!"
           FROM play_events WHERE user_id = $1 AND started_at < $3
           GROUP BY 1 ORDER BY 1 DESC"#,
        user_id,
        i32::from(off),
        now,
    )
    .fetch_all(db)
    .await?;
    let mut years: Vec<RewindYear> = rows
        .into_iter()
        .filter(|r| (FIRST_YEAR..=current).contains(&r.year))
        .map(|r| RewindYear {
            year: r.year,
            minutes: r.minutes,
        })
        .collect();
    if years.first().is_none_or(|y| y.year != current) {
        years.insert(
            0,
            RewindYear {
                year: current,
                minutes: 0,
            },
        );
    }
    Ok(RewindYears {
        current,
        season: in_season(current, now, off),
        years,
    })
}

/// «Твои лучшие треки {год}»: самое слушаемое за год, до [`PLAYLIST_SIZE`] треков. Повтор обновляет
/// тот же плейлист. `None` — за год нечего собрать.
pub async fn save_playlist(
    db: &PgPool,
    user_id: Uuid,
    year: i32,
) -> Result<Option<(PlaylistSummary, bool)>, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let (since, until) = range(year, Utc::now(), settings.utc_offset_min);
    let tracks = sqlx::query_scalar!(
        "SELECT track_id FROM play_events
         WHERE user_id = $1 AND started_at >= $2 AND started_at < $3 AND (played_ms >= $4 OR end_reason = 'completed')
         GROUP BY track_id ORDER BY count(*) DESC, sum(played_ms) DESC, track_id LIMIT $5",
        user_id,
        since,
        until,
        LISTEN_MS,
        PLAYLIST_SIZE,
    )
    .fetch_all(db)
    .await?;
    if tracks.is_empty() {
        return Ok(None);
    }
    let title = format!("Твои лучшие треки {year}");
    let description = format!("Итоги {year}: что ты слушал больше всего");
    let rules = json!({ "generator": "rewind", "year": year, "builtAt": Utc::now() });
    let mut tx = db.begin().await?;
    let existing = sqlx::query_scalar!(
        "SELECT id FROM playlists WHERE user_id = $1 AND rules ->> 'generator' = 'rewind'
           AND rules ->> 'year' = $2 ORDER BY created_at LIMIT 1 FOR UPDATE",
        user_id,
        year.to_string(),
    )
    .fetch_optional(&mut *tx)
    .await?;
    let created = existing.is_none();
    let id = match existing {
        Some(id) => {
            sqlx::query!(
                "UPDATE playlists SET rules = $2, updated_at = now() WHERE id = $1",
                id,
                rules
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!("DELETE FROM playlist_items WHERE playlist_id = $1", id)
                .execute(&mut *tx)
                .await?;
            id
        }
        None => {
            sqlx::query_scalar!(
                "INSERT INTO playlists (user_id, title, description, kind, rules) VALUES ($1, $2, $3, 'user', $4)
                 RETURNING id",
                user_id,
                title,
                description,
                rules,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    sqlx::query!(
        "INSERT INTO playlist_items (playlist_id, track_id, position)
         SELECT $1, t, p::double precision * $3::double precision FROM unnest($2::uuid[]) WITH ORDINALITY AS x(t, p)",
        id,
        &tracks,
        playlists::STEP,
    )
    .execute(&mut *tx)
    .await?;
    realtime::publish(
        &mut *tx,
        Audience::User(user_id),
        ServerEvent::LibraryChanged(LibraryChange {
            entities: vec![SyncEntity::Playlists, SyncEntity::PlaylistItems],
        }),
    )
    .await;
    tx.commit().await?;
    Ok(playlists::summaries(db, user_id, Some(&[id]), false)
        .await?
        .into_iter()
        .next()
        .map(|s| (s, created)))
}

/// Декабрь, ночь (3–6 по времени слушателя): итоги текущего года пересчитываются заранее — тем, у кого
/// они старше 20 часов. Раз в час из обслуживания воркера; вне окна — ничего.
pub async fn warm(db: &PgPool, now: DateTime<Utc>) -> Result<usize, sqlx::Error> {
    let settings = Settings::load(db).await?;
    let local = now + offset(settings.utc_offset_min);
    if local.month() != 12 || !(3..6).contains(&local.hour()) {
        return Ok(0);
    }
    let year = local.year();
    let (since, until) = range(year, now, settings.utc_offset_min);
    let users = sqlx::query_scalar!(
        "SELECT u.id FROM users u
         WHERE NOT u.disabled
           AND EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = u.id AND p.started_at >= $1 AND p.started_at < $2)
           AND NOT EXISTS (SELECT 1 FROM rewinds r WHERE r.user_id = u.id AND r.year = $3
                             AND r.computed_at > now() - interval '20 hours')",
        since,
        until,
        year,
    )
    .fetch_all(db)
    .await?;
    for user in &users {
        let fp = fingerprint(db, *user, since, until).await?;
        let data = compute(db, *user, (since, until), settings.utc_offset_min).await?;
        store(db, *user, year, &fp, &data).await?;
    }
    Ok(users.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    #[test]
    fn year_is_in_listener_time() {
        let (since, until) = year_bounds(2026, 180);
        assert_eq!(since, Utc.with_ymd_and_hms(2025, 12, 31, 21, 0, 0).unwrap());
        assert_eq!(until, Utc.with_ymd_and_hms(2026, 12, 31, 21, 0, 0).unwrap());
        // 31 декабря 22:00 UTC — в Москве уже новый год
        let at = Utc.with_ymd_and_hms(2026, 12, 31, 22, 0, 0).unwrap();
        assert_eq!(local_year(at, 180), 2027);
        assert_eq!(local_year(at, 0), 2026);
    }

    #[test]
    fn season_starts_in_december() {
        let nov = Utc.with_ymd_and_hms(2026, 11, 30, 12, 0, 0).unwrap();
        let dec = Utc.with_ymd_and_hms(2026, 11, 30, 22, 0, 0).unwrap();
        assert!(!in_season(2026, nov, 180));
        assert!(in_season(2026, dec, 180), "в Москве уже 1 декабря");
        assert!(in_season(2025, nov, 180), "прошлый год — всегда");
    }

    #[test]
    fn streaks() {
        assert_eq!(longest_streak(&[]), None);
        assert_eq!(longest_streak(&[d(3, 1)]), Some((1, d(3, 1), d(3, 1))));
        let days = [
            d(1, 1),
            d(1, 2),
            d(1, 4),
            d(1, 5),
            d(1, 6),
            d(2, 28),
            d(3, 1),
        ];
        assert_eq!(longest_streak(&days), Some((3, d(1, 4), d(1, 6))));
        // Через границу месяца, первая из равных
        let days = [d(2, 27), d(2, 28), d(3, 1), d(5, 1), d(5, 2), d(5, 3)];
        assert_eq!(longest_streak(&days), Some((3, d(2, 27), d(3, 1))));
    }

    #[test]
    fn personalities() {
        let base = Traits {
            night: 0.1,
            morning: 0.2,
            novelty: Some(0.3),
            repeat: 0.5,
            wave: 0.05,
        };
        assert_eq!(personality(&base), "balanced");
        assert_eq!(personality(&Traits { night: 0.4, ..base }), "night_owl");
        assert_eq!(
            personality(&Traits {
                novelty: Some(0.9),
                ..base
            }),
            "explorer"
        );
        // Без истории до года всё «новое» — это не исследователь
        assert_eq!(
            personality(&Traits {
                novelty: None,
                ..base
            }),
            "balanced"
        );
        assert_eq!(
            personality(&Traits {
                repeat: 0.92,
                ..base
            }),
            "loyal"
        );
        assert_eq!(personality(&Traits { wave: 0.6, ..base }), "wave_rider");
        // Сильнейшая черта побеждает: ночь 0.33/0.22 = 1.5 против волны 0.42/0.35 = 1.2
        assert_eq!(
            personality(&Traits {
                night: 0.33,
                wave: 0.42,
                ..base
            }),
            "night_owl"
        );
    }

    #[test]
    fn tops_order_by_plays_then_time() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let c = Uuid::from_u128(3);
        let counts = HashMap::from([(a, (3, 100)), (b, (3, 500)), (c, (0, 900))]);
        let t = top(counts, 5);
        assert_eq!(t.iter().map(|x| x.id).collect::<Vec<_>>(), vec![b, a]);
    }

    #[test]
    fn listens_need_thirty_seconds() {
        assert!(listened(30_000, "skipped"));
        assert!(!listened(29_999, "skipped"));
        assert!(listened(12_000, "completed"));
    }
}
