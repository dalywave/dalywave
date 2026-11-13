//! Чарты: Россия, мир и жанры — треки каталога по местам, альбомы и артисты по трекам чарта.
//!
//! Мир — редакционный плейлист Deezer «Top Worldwide», жанры — чарты жанров Deezer. Чарта России у
//! Deezer нет, поэтому Россия — открытый чарт Apple Music: в нём только названия и артисты, трек
//! каталога находится поиском Deezer и запоминается (`chart_matches`), так что при следующем
//! обновлении ищутся только новые строки. Поиск идёт с паузами, чтобы не съесть лимит Deezer
//! (50 запросов за 5 секунд на весь сервер).
//!
//! Чарт собирается при обращении, если его нет или он старше [`TTL`]: показывается прежний, а новый
//! собирается в фоне (один сбор на сервер: отметки «собирается» — в его состоянии, [`Refreshing`]).

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::Duration,
};

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    catalog::RecordType,
    info::{ChartDetail, ChartKind, ChartSummary},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    catalog::{self, views},
    deezer::{Deezer, DeezerError, types as dz},
    info::{
        InfoError,
        apple::{AppleCharts, Song, search_title, split_artists},
        loose,
    },
    lyrics::norm,
    state::AppState,
};

/// Чарт старше — собирается заново.
pub const TTL: chrono::Duration = chrono::Duration::hours(4);
/// Мест в чарте.
const SIZE: u32 = 100;
/// Альбомов и артистов на странице чарта.
const SIDE: usize = 20;
/// Пауза между поисками Deezer при сопоставлении: остальным запросам сервера хватает лимита.
const SEARCH_GAP: Duration = Duration::from_millis(300);
/// «Не нашли» — столько не ищем снова.
const MISS_RETRY: chrono::Duration = chrono::Duration::hours(24);
/// Первый сбор чарта сохраняется кусками: первые места видны, пока ищутся остальные.
const FIRST_FILL_STEP: usize = 20;
/// Редакционный плейлист Deezer «Top Worldwide».
const TOP_WORLDWIDE: i64 = 3155776842;
const APPLE: &str = "apple";

#[derive(Debug, Clone, Copy)]
enum Source {
    /// Открытый чарт Apple Music страны.
    Apple(&'static str),
    Playlist(i64),
    /// Чарт жанра Deezer по id жанра.
    Genre(i64),
}

#[derive(Debug, Clone, Copy)]
pub struct Def {
    pub id: &'static str,
    pub title: &'static str,
    pub kind: ChartKind,
    source: Source,
}

const fn genre(id: &'static str, title: &'static str, genre: i64) -> Def {
    Def {
        id,
        title,
        kind: ChartKind::Genre,
        source: Source::Genre(genre),
    }
}

/// Все чарты по порядку показа.
pub const CHARTS: &[Def] = &[
    Def {
        id: "ru",
        title: "Россия",
        kind: ChartKind::Country,
        source: Source::Apple("ru"),
    },
    Def {
        id: "world",
        title: "Мир",
        kind: ChartKind::World,
        source: Source::Playlist(TOP_WORLDWIDE),
    },
    genre("genre-132", "Поп", 132),
    genre("genre-116", "Хип-хоп", 116),
    genre("genre-152", "Рок", 152),
    genre("genre-113", "Танцевальная", 113),
    genre("genre-106", "Электроника", 106),
    genre("genre-165", "R&B", 165),
    genre("genre-85", "Альтернатива", 85),
    genre("genre-464", "Метал", 464),
    genre("genre-129", "Джаз", 129),
    genre("genre-98", "Классика", 98),
];

pub fn find(id: &str) -> Option<&'static Def> {
    CHARTS.iter().find(|d| d.id == id)
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error(transparent)]
    Deezer(#[from] DeezerError),
    #[error(transparent)]
    Source(#[from] InfoError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

// ───────────── Сопоставление строки чужого чарта ─────────────

fn similar(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || (a.chars().count().min(b.chars().count()) >= 3 && (a.contains(b) || b.contains(a)))
}

/// Имена артистов для сравнения: каждое по отдельности и целиком.
fn artist_keys(name: &str) -> Vec<String> {
    let mut keys: Vec<String> = split_artists(name).iter().map(|a| loose(a)).collect();
    keys.push(loose(name));
    keys.retain(|k| !k.is_empty());
    keys
}

/// Трек Deezer для строки чарта: название похоже, хотя бы один артист совпадает. Из подходящих —
/// с точным названием, затем самый популярный (оригинал, а не кавер или караоке).
pub fn best_match<'a>(song: &Song, candidates: &'a [dz::Track]) -> Option<&'a dz::Track> {
    let title = norm(&song.name);
    let wanted = artist_keys(&song.artist_name);
    candidates
        .iter()
        .filter(|t| similar(&norm(&t.title), &title))
        .filter(|t| {
            let names = t
                .artist
                .iter()
                .chain(t.contributors.iter())
                .flat_map(|a| artist_keys(&a.name));
            names
                .into_iter()
                .any(|n| wanted.iter().any(|w| similar(&n, w)))
        })
        .max_by_key(|t| (norm(&t.title) == title, t.rank.unwrap_or(0)))
}

/// Ищет строку чарта в Deezer: «первый артист + название», иначе только название.
async fn search_song(deezer: &Deezer, song: &Song) -> Result<Option<dz::Track>, DeezerError> {
    let title = search_title(&song.name);
    let first = split_artists(&song.artist_name)
        .into_iter()
        .next()
        .unwrap_or_default();
    for q in [format!("{first} {title}"), title.clone()] {
        let found = match deezer.search_tracks(q.trim(), 10).await {
            Ok(f) => f,
            Err(DeezerError::NotFound) => Vec::new(),
            Err(e) => return Err(e),
        };
        if let Some(t) = best_match(song, &found) {
            return Ok(Some(t.clone()));
        }
        tokio::time::sleep(SEARCH_GAP).await;
    }
    Ok(None)
}

// ───────────── Сбор ─────────────

async fn save(db: &PgPool, id: &str, tracks: &[Uuid]) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO charts (id, track_ids, fetched_at) VALUES ($1, $2, now())
         ON CONFLICT (id) DO UPDATE SET track_ids = $2, fetched_at = now()",
        id,
        tracks,
    )
    .execute(db)
    .await?;
    Ok(())
}

fn push_unique(list: &mut Vec<Uuid>, id: Uuid) {
    if !list.contains(&id) {
        list.push(id);
    }
}

/// Чарт страны из Apple Music: найденные раньше строки берутся из `chart_matches`, новые ищутся.
async fn from_apple(
    db: &PgPool,
    deezer: &Deezer,
    apple: &AppleCharts,
    def: &Def,
    country: &str,
) -> Result<Vec<Uuid>, RefreshError> {
    let songs = apple.most_played(country, SIZE).await?;
    let ids: Vec<String> = songs.iter().map(|s| s.id.clone()).collect();
    let known: HashMap<String, (Option<Uuid>, DateTime<Utc>)> = sqlx::query!(
        "SELECT external_id, track_id, checked_at FROM chart_matches WHERE source = $1 AND external_id = ANY($2)",
        APPLE,
        &ids,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.external_id, (r.track_id, r.checked_at)))
    .collect();
    let first_fill = sqlx::query_scalar!("SELECT fetched_at FROM charts WHERE id = $1", def.id)
        .fetch_optional(db)
        .await?
        .flatten()
        .is_none();
    let mut out = Vec::new();
    let mut searched = 0usize;
    for song in &songs {
        let track = match known.get(&song.id) {
            Some((Some(id), _)) => Some(*id),
            Some((None, at)) if *at > Utc::now() - MISS_RETRY => None,
            _ => {
                if searched > 0 {
                    tokio::time::sleep(SEARCH_GAP).await;
                }
                searched += 1;
                let found = match search_song(deezer, song).await {
                    Ok(f) => f,
                    // Одна строка не нашлась из-за сбоя — остальные всё равно
                    Err(e) if e.is_temporary() => {
                        tracing::warn!(song = %song.id, error = %e, "chart search failed");
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                };
                let track = match found {
                    Some(t) => catalog::save_listed(db, std::slice::from_ref(&t))
                        .await?
                        .first()
                        .copied(),
                    None => None,
                };
                sqlx::query!(
                    "INSERT INTO chart_matches (source, external_id, track_id, checked_at) VALUES ($1, $2, $3, now())
                     ON CONFLICT (source, external_id) DO UPDATE SET track_id = $3, checked_at = now()",
                    APPLE,
                    song.id,
                    track,
                )
                .execute(db)
                .await?;
                track
            }
        };
        if let Some(id) = track {
            push_unique(&mut out, id);
            if first_fill && out.len() % FIRST_FILL_STEP == 0 {
                save(db, def.id, &out).await?;
            }
        }
    }
    Ok(out)
}

/// Собирает чарт и сохраняет. Сколько мест.
pub async fn refresh(
    db: &PgPool,
    deezer: &Deezer,
    apple: &AppleCharts,
    def: &Def,
) -> Result<usize, RefreshError> {
    let tracks = match def.source {
        Source::Apple(country) => from_apple(db, deezer, apple, def, country).await?,
        Source::Playlist(id) => {
            let list = deezer.playlist_tracks(id, SIZE).await?;
            catalog::save_listed(db, &list).await?
        }
        Source::Genre(id) => {
            let list = deezer.genre_chart(id, SIZE).await?;
            catalog::save_listed(db, &list).await?
        }
    };
    // Пустой ответ источника — сбой, а не пустой чарт: прежний остаётся
    if !tracks.is_empty() {
        save(db, def.id, &tracks).await?;
    }
    Ok(tracks.len())
}

/// Чарты, которые собираются прямо сейчас. Живёт в состоянии сервера, а не в глобальной
/// переменной: у каждого сервера (и у каждого теста с своей базой) свои отметки.
#[derive(Default)]
pub struct Refreshing(Mutex<HashSet<&'static str>>);

impl Refreshing {
    fn set(&self) -> std::sync::MutexGuard<'_, HashSet<&'static str>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.set().contains(id)
    }
}

fn stale(fetched_at: Option<DateTime<Utc>>) -> bool {
    fetched_at.is_none_or(|at| at < Utc::now() - TTL)
}

/// Собирает чарт в фоне, если его нет или он устарел. `true` — сбор идёт.
pub fn refresh_if_stale(
    st: &AppState,
    def: &'static Def,
    fetched_at: Option<DateTime<Utc>>,
) -> bool {
    if !stale(fetched_at) {
        return st.info.charts.contains(def.id);
    }
    if !st.info.charts.set().insert(def.id) {
        return true;
    }
    let st = st.clone();
    tokio::spawn(async move {
        match refresh(&st.db, &st.deezer, &st.info.apple, def).await {
            Ok(n) => tracing::info!(chart = def.id, tracks = n, "chart refreshed"),
            Err(e) => tracing::warn!(chart = def.id, error = %e, "could not refresh a chart"),
        }
        st.info.charts.set().remove(def.id);
    });
    true
}

// ───────────── Чтение ─────────────

struct Row {
    track_ids: Vec<Uuid>,
    fetched_at: Option<DateTime<Utc>>,
}

async fn rows(db: &PgPool) -> Result<HashMap<String, Row>, sqlx::Error> {
    Ok(sqlx::query!("SELECT id, track_ids, fetched_at FROM charts")
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.id,
                Row {
                    track_ids: r.track_ids,
                    fetched_at: r.fetched_at,
                },
            )
        })
        .collect())
}

/// Все чарты с обложкой первого места. Россия и мир, если устарели, собираются в фоне.
pub async fn list(st: &AppState, user: Uuid) -> Result<Vec<ChartSummary>, sqlx::Error> {
    let rows = rows(&st.db).await?;
    let firsts: Vec<Uuid> = CHARTS
        .iter()
        .filter_map(|d| rows.get(d.id)?.track_ids.first().copied())
        .collect();
    let images: HashMap<Uuid, _> = views::tracks(&st.db, &firsts, user)
        .await?
        .into_iter()
        .map(|t| (t.id, t.album.and_then(|a| a.image)))
        .collect();
    Ok(CHARTS
        .iter()
        .map(|d| {
            let row = rows.get(d.id);
            if d.kind != ChartKind::Genre {
                refresh_if_stale(st, d, row.and_then(|r| r.fetched_at));
            }
            ChartSummary {
                id: d.id.into(),
                title: d.title.into(),
                kind: d.kind,
                image: row
                    .and_then(|r| r.track_ids.first())
                    .and_then(|id| images.get(id).cloned().flatten()),
                updated_at: row.and_then(|r| r.fetched_at),
            }
        })
        .collect())
}

/// Альбомы (и EP) и артисты по трекам чарта: чем выше трек, тем больше очков.
fn ranked<K: Copy + Eq + std::hash::Hash>(keys: impl Iterator<Item = (usize, K)>) -> Vec<K> {
    let mut score: HashMap<K, (i64, usize)> = HashMap::new();
    for (place, key) in keys {
        let e = score.entry(key).or_insert((0, place));
        e.0 += SIZE as i64 - place as i64;
    }
    let mut out: Vec<(K, (i64, usize))> = score.into_iter().collect();
    out.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.1.1.cmp(&b.1.1)));
    out.into_iter().map(|(k, _)| k).collect()
}

/// Чарт целиком. Устаревший или ещё не собранный — собирается в фоне, а жанр без единого сбора
/// собирается сразу: это один запрос к Deezer.
pub async fn detail(
    st: &AppState,
    def: &'static Def,
    user: Uuid,
) -> Result<ChartDetail, RefreshError> {
    let mut row = rows(&st.db).await?.remove(def.id);
    if row.is_none() && !matches!(def.source, Source::Apple(_)) && !st.info.charts.contains(def.id)
    {
        refresh(&st.db, &st.deezer, &st.info.apple, def).await?;
        row = rows(&st.db).await?.remove(def.id);
    }
    let fetched_at = row.as_ref().and_then(|r| r.fetched_at);
    let updating = refresh_if_stale(st, def, fetched_at);
    let ids = row.map(|r| r.track_ids).unwrap_or_default();
    let tracks = views::tracks(&st.db, &ids, user).await?;
    // Сингл называется как его трек: в «Альбомы» идут релизы с другим названием и больше трёх треков
    let album_ids = ranked(tracks.iter().enumerate().filter_map(|(i, t)| {
        let album = t.album.as_ref()?;
        (norm(&album.title) != norm(&t.title)).then_some((i, album.id))
    }));
    let albums: Vec<_> = views::albums(&st.db, &album_ids, user)
        .await?
        .into_iter()
        .filter(|a| matches!(a.record_type, RecordType::Album | RecordType::Ep))
        .filter(|a| a.tracks_count.is_none_or(|n| n > 3))
        .take(SIDE)
        .collect();
    let artist_ids: Vec<Uuid> = ranked(
        tracks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| Some((i, t.artists.first()?.id))),
    )
    .into_iter()
    .take(SIDE)
    .collect();
    let artists = views::artists(&st.db, &artist_ids, user).await?;
    Ok(ChartDetail {
        chart: ChartSummary {
            id: def.id.into(),
            title: def.title.into(),
            kind: def.kind,
            image: tracks.first().and_then(|t| t.album.as_ref()?.image.clone()),
            updated_at: fetched_at,
        },
        tracks,
        albums,
        artists,
        updating,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(name: &str, artist: &str) -> Song {
        Song {
            id: "1".into(),
            name: name.into(),
            artist_name: artist.into(),
        }
    }

    fn track(id: i64, title: &str, artist: &str, rank: i32) -> dz::Track {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "duration": 180, "rank": rank,
            "artist": { "id": id * 10, "name": artist }
        }))
        .unwrap()
    }

    #[test]
    fn the_original_wins_over_covers_and_namesakes() {
        let found = vec![
            track(1, "I Got Love (Karaoke)", "Karaoke Hits", 900_000),
            track(2, "I Got Love", "MiyaGi & Эндшпиль", 721_974),
            track(3, "I Got Love", "Miyagi", 154_298),
            track(4, "Minor", "Miyagi & Andy Panda", 522_231),
        ];
        let s = song("I Got Love (feat. Рем Дигга)", "Miyagi & Эндшпиль");
        assert_eq!(best_match(&s, &found).map(|t| t.id), Some(2));
        // Другой артист с тем же названием — не совпадение
        let s = song("I Got Love", "Кто-то другой");
        assert_eq!(best_match(&s, &found).map(|t| t.id), None);
    }

    #[test]
    fn several_artists_match_by_any_of_them() {
        let found = vec![track(5, "Шадэ", "Xcho", 400_000)];
        let s = song("Шадэ", "By Индия, Xcho & МОТ");
        assert_eq!(best_match(&s, &found).map(|t| t.id), Some(5));
    }

    #[test]
    fn higher_places_rank_first() {
        // Альбом A: места 0 и 5, альбом B: место 1 — A выше
        let order = ranked([(0usize, 'a'), (1, 'b'), (5, 'a'), (7, 'c')].into_iter());
        assert_eq!(order, vec!['a', 'b', 'c']);
    }

    #[test]
    fn every_chart_is_findable_and_unique() {
        let ids: HashSet<&str> = CHARTS.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), CHARTS.len());
        assert_eq!(find("ru").map(|d| d.kind), Some(ChartKind::Country));
        assert!(find("nope").is_none());
    }
}
