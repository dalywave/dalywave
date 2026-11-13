//! Поиск для людей: находит независимо от записи. «капсайз» — Capsize, «сфзышяу» (не та раскладка) —
//! тоже Capsize, «lowbron» — Lowborn. Сравнение названий — [`dalywave_textmatch`], общий с приложениями.
//!
//! Каталог: сначала запрос как есть в Deezer и артисты, уже известные серверу, по звучанию. Если
//! уверенного совпадения нет — ещё не больше двух вариантов запроса (имя известного артиста, исправленная
//! раскладка, «английское» написание русской записи): у Deezer лимит 50 запросов за 5 секунд, а ответы
//! поиска помнятся 10 минут. Всё найденное ранжируется по сходству с запросом.

use std::collections::HashMap;

use dalywave_protocol::catalog::{DeezerKind, SearchTop};
use dalywave_textmatch::{self as tm, Query, STRONG};
use futures_util::future::join_all;
use sqlx::PgPool;
use uuid::Uuid;

use super::{CatalogError, Hits, Isrc, Types, save_album_detail, save_listed_track, upsert_artist};
use crate::deezer::{Deezer, DeezerError, types as dz};

/// Сколько вариантов запроса можно отправить в Deezer сверх самого запроса.
const MAX_VARIANTS: usize = 2;
const VARIANT_LIMIT: u32 = 25;

/// Найденное и что из него показать первым.
#[derive(Debug, Default)]
pub struct Found {
    pub hits: Hits,
    pub top: Option<SearchTop>,
    pub suggestion: Option<String>,
}

/// Что нашлось в одном запросе к Deezer.
#[derive(Default)]
struct Pool {
    tracks: Vec<dz::Track>,
    albums: Vec<dz::Album>,
    artists: Vec<dz::Artist>,
}

async fn fetch(deezer: &Deezer, q: &str, types: Types, limit: u32) -> (Pool, Vec<DeezerError>) {
    async fn one<T: serde::de::DeserializeOwned>(
        deezer: &Deezer,
        on: bool,
        kind: &str,
        q: &str,
        limit: u32,
    ) -> Option<Result<Vec<T>, DeezerError>> {
        match on {
            true => Some(deezer.search(kind, q, limit).await),
            false => None,
        }
    }
    let (tracks, albums, artists) = tokio::join!(
        one::<dz::Track>(deezer, types.track, "track", q, limit),
        one::<dz::Album>(deezer, types.album, "album", q, limit),
        one::<dz::Artist>(deezer, types.artist, "artist", q, limit),
    );
    let mut errors = Vec::new();
    let pool = Pool {
        tracks: super::take(tracks, &mut errors),
        albums: super::take(albums, &mut errors),
        artists: super::take(artists, &mut errors),
    };
    (pool, errors)
}

/// Артист, которого сервер уже знает.
struct Known {
    id: Uuid,
    name: String,
    fans: i64,
    score: f32,
}

/// Артисты из базы, звучащие как запрос или отличающиеся опечаткой. Только уверенные совпадения.
async fn known_artists(db: &PgPool, query: &Query) -> Result<Vec<Known>, sqlx::Error> {
    let sounds = query.sounds();
    let latin = query.latins().into_iter().next().unwrap_or_default();
    if sounds.is_empty() && latin.chars().count() < 4 {
        return Ok(Vec::new());
    }
    // «викенд» — это и The Weeknd: артикль в начале имени не слышен
    let mut keys = sounds.clone();
    keys.extend(sounds.iter().map(|k| format!("t {k}")));
    let rows = sqlx::query!(
        r#"SELECT id, name, COALESCE(fans, 0)::bigint AS "fans!" FROM artists
           WHERE deezer_id IS NOT NULL AND (sound = ANY($1) OR latin % $2)
           ORDER BY fans DESC NULLS LAST
           LIMIT 50"#,
        &keys,
        latin,
    )
    .fetch_all(db)
    .await?;
    let mut known: Vec<Known> = rows
        .into_iter()
        .map(|r| Known {
            score: query.score(&r.name),
            id: r.id,
            name: r.name,
            fans: r.fans,
        })
        .filter(|k| k.score >= STRONG)
        .collect();
    known.sort_by(|a, b| b.score.total_cmp(&a.score).then(b.fans.cmp(&a.fans)));
    known.truncate(3);
    Ok(known)
}

/// Варианты запроса для Deezer, по порядку пользы.
fn variants(query: &Query, best: f32, known: &[Known]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(k) = known.first() {
        out.push(k.name.clone());
    }
    // Набрано не в той раскладке — другие написания этой бессмыслицы ничего не дадут
    if let Some(l) = query.layout() {
        out.push(l.to_string());
    } else if query.is_cyrillic() {
        out.push(tm::english(query.raw()));
    } else if best < 0.6 {
        out.push(tm::cyrillic(query.raw()));
    }
    let mut seen = vec![tm::normalize(query.raw())];
    out.retain(|v| {
        let n = tm::normalize(v);
        let fresh = !n.is_empty() && !seen.contains(&n);
        seen.push(n);
        fresh
    });
    out.truncate(MAX_VARIANTS);
    out
}

fn main_artist(t: &dz::Track) -> Vec<&str> {
    t.artist.iter().map(|a| a.name.as_str()).collect()
}

/// Насколько хорош лучший из найденного: всё и только артисты.
fn best_of(query: &Query, pool: &Pool) -> (f32, f32) {
    let tracks = pool
        .tracks
        .iter()
        .map(|t| query.score_track(&t.title, &main_artist(t)));
    let albums = pool.albums.iter().map(|a| {
        let artist: Vec<&str> = a.artist.iter().map(|x| x.name.as_str()).collect();
        query.score_track(&a.title, &artist)
    });
    let artist = pool
        .artists
        .iter()
        .map(|a| query.score(&a.name))
        .fold(0.0, f32::max);
    (tracks.chain(albums).fold(artist, f32::max), artist)
}

/// Кандидат в выдачу: наш id, сходство, популярность и место в ответе Deezer.
struct Ranked {
    id: Uuid,
    score: f32,
    fans: i64,
    order: usize,
}

#[derive(Default)]
struct Ranking {
    items: Vec<Ranked>,
    index: HashMap<Uuid, usize>,
}

impl Ranking {
    fn add(&mut self, id: Uuid, score: f32, fans: i64) {
        let order = self.items.len();
        match self.index.get(&id) {
            Some(&i) => {
                let r = &mut self.items[i];
                r.score = r.score.max(score);
                r.fans = r.fans.max(fans);
            }
            None => {
                self.index.insert(id, order);
                self.items.push(Ranked {
                    id,
                    score,
                    fans,
                    order,
                });
            }
        }
    }

    /// По сходству ступенями по 0,02, внутри ступени — популярнее (если `by_fans`), затем как у Deezer.
    /// Без `rank` — как у Deezer.
    fn sorted(mut self, rank: bool, by_fans: bool, limit: usize) -> Vec<Ranked> {
        let step = |s: f32| (s * 50.0).round() as i32;
        if !rank {
            self.items.truncate(limit);
            return self.items;
        }
        self.items.sort_by(|a, b| {
            step(b.score)
                .cmp(&step(a.score))
                .then(if by_fans {
                    b.fans.cmp(&a.fans)
                } else {
                    std::cmp::Ordering::Equal
                })
                .then(a.order.cmp(&b.order))
        });
        self.items.truncate(limit);
        self.items
    }
}

fn ids(list: Vec<Ranked>) -> Vec<Uuid> {
    list.into_iter().map(|r| r.id).collect()
}

/// Из одинаково уверенных лучший результат — популярнее: The Weeknd, а не Weeknd на «викенд». Прибавка
/// меньше разницы в написании (Capsize и CUPSIZE на «капсайз») и не делает неуверенное уверенным.
fn with_fans(score: f32, fans: i64) -> f32 {
    match score >= STRONG {
        true => score + 0.0015 * (fans.max(0) as f32 + 1.0).log10(),
        false => score,
    }
}

/// Оценённые названия — только попавшие в выдачу и в её порядке.
fn follow(names: &mut Vec<(Uuid, String, f32)>, order: &[Uuid]) {
    let place: HashMap<Uuid, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    names.retain(|n| place.contains_key(&n.0));
    names.sort_by_key(|n| place[&n.0]);
    names.dedup_by_key(|n| n.0);
}

/// Лучший результат и подсказка — как у приложений без сети ([`Query::verdict`]).
fn verdict(
    query: &Query,
    artists: &[(Uuid, String, f32)],
    albums: &[(Uuid, String, f32)],
    titles: &[(String, f32)],
) -> (Option<SearchTop>, Option<String>) {
    let (top, suggestion) = query.verdict(artists, albums, titles);
    let top = top.map(|t| match t {
        tm::Top::Artist(id) => SearchTop {
            kind: DeezerKind::Artist,
            id,
        },
        tm::Top::Album(id) => SearchTop {
            kind: DeezerKind::Album,
            id,
        },
    });
    (top, suggestion)
}

/// Поиск по каталогу для людей: Deezer, варианты запроса и известные серверу артисты.
pub async fn catalog(
    db: &PgPool,
    deezer: &Deezer,
    q: &str,
    types: Types,
    limit: u32,
) -> Result<Found, CatalogError> {
    let query = Query::new(q);
    let ((direct, errors), known) = tokio::join!(fetch(deezer, q, types, limit), async {
        match types.artist {
            true => known_artists(db, &query).await,
            false => Ok(Vec::new()),
        }
    });
    let known = known?;
    let asked = usize::from(types.track) + usize::from(types.album) + usize::from(types.artist);
    if errors.len() == asked
        && known.is_empty()
        && let Some(e) = errors.into_iter().next()
    {
        return Err(e.into());
    }

    let (best, best_artist) = best_of(&query, &direct);
    let letters = q.chars().filter(|c| c.is_alphanumeric()).count();
    let mut pools = vec![direct];
    // Артисты важнее всего: трек с названием «Оксимирон» не отменяет поиска самого Oxxxymiron
    let fuzzy = letters >= 3
        && match types.artist {
            true => best_artist < STRONG,
            false => best < STRONG,
        };
    if fuzzy {
        let variants = variants(&query, best, &known);
        let calls = variants.iter().enumerate().map(|(i, v)| {
            // Второй вариант — только артисты: лимит Deezer общий на все запросы сервера
            let t = match i {
                0 => types,
                _ => Types {
                    track: false,
                    album: false,
                    artist: types.artist,
                },
            };
            fetch(deezer, v, t, limit.min(VARIANT_LIMIT))
        });
        for (pool, errors) in join_all(calls).await {
            for e in errors {
                tracing::debug!(error = %e, "search variant failed");
            }
            pools.push(pool);
        }
    }

    let (mut tracks, mut albums, mut artists) =
        (Ranking::default(), Ranking::default(), Ranking::default());
    let mut artist_names = Vec::new();
    let mut album_names = Vec::new();
    let mut titles = Vec::new();
    let mut tx = db.begin().await?;
    for pool in &pools {
        for t in &pool.tracks {
            let id = save_listed_track(&mut tx, t, Isrc::Known(t.isrc.as_deref()), None).await?;
            tracks.add(id, query.score_track(&t.title, &main_artist(t)), 0);
            titles.push((t.title.clone(), query.score(&t.title)));
        }
        for a in &pool.albums {
            let Some(id) = save_album_detail(&mut tx, a, None).await? else {
                continue;
            };
            let artist: Vec<&str> = a.artist.iter().map(|x| x.name.as_str()).collect();
            albums.add(id, query.score_track(&a.title, &artist), 0);
            album_names.push((id, a.title.clone(), query.score(&a.title)));
        }
        for a in &pool.artists {
            let id = upsert_artist(&mut tx, a).await?;
            let score = query.score(&a.name);
            let fans = i64::from(a.nb_fan.unwrap_or(0));
            artists.add(id, score, fans);
            artist_names.push((id, a.name.clone(), with_fans(score, fans)));
        }
    }
    tx.commit().await?;
    for k in &known {
        artists.add(k.id, k.score, k.fans);
        artist_names.push((k.id, k.name.clone(), with_fans(k.score, k.fans)));
    }

    let limit = limit as usize;
    // Запрос нашёлся как есть — порядок Deezer; иначе — по сходству с запросом
    let mut hits = Hits {
        tracks: ids(tracks.sorted(fuzzy, false, limit)),
        albums: ids(albums.sorted(fuzzy, false, limit)),
        artists: ids(artists.sorted(fuzzy, true, limit)),
    };
    follow(&mut artist_names, &hits.artists);
    follow(&mut album_names, &hits.albums);
    let (top, suggestion) = verdict(&query, &artist_names, &album_names, &titles);
    // Артист нашёлся не по написанию («капсайз» — Capsize): его треки — первыми, раньше чужих песен
    // с таким названием
    if fuzzy
        && let Some(SearchTop {
            kind: DeezerKind::Artist,
            id,
        }) = top
    {
        let own = sqlx::query_scalar!(
            "SELECT DISTINCT track_id FROM track_artists WHERE artist_id = $1 AND track_id = ANY($2)",
            id,
            &hits.tracks,
        )
        .fetch_all(db)
        .await?;
        hits.tracks.sort_by_key(|t| !own.contains(t));
    }
    Ok(Found {
        hits,
        top,
        suggestion,
    })
}

/// Сколько кандидатов из базы оценивается для одного вида: дальше — уже совсем далёкое.
const LIBRARY_CANDIDATES: i64 = 300;

/// Поиск по медиатеке: лайки, сохранённые альбомы, подписки и всё, что уже на сервере. База отбирает
/// кандидатов — каждое слово запроса (или его же в другой раскладке) находится в названии с артистами
/// куском или с опечаткой, либо совпадает звучание; порядок — по сходству с запросом.
pub async fn library(
    db: &PgPool,
    user_id: Uuid,
    q: &str,
    types: Types,
    limit: i64,
) -> Result<Found, sqlx::Error> {
    let query = Query::new(q);
    let latins = query.latins();
    let sounds = query.sounds();
    let main = latins.first().cloned().unwrap_or_default();
    let limit = limit.max(0) as usize;
    let rank = |list: Vec<(Uuid, f32)>| {
        let mut r = Ranking::default();
        for (id, score) in list {
            r.add(id, score, 0);
        }
        ids(r.sorted(true, false, limit))
    };
    let mut found = Found::default();
    let (mut artist_names, mut album_names, mut titles) = (Vec::new(), Vec::new(), Vec::new());
    if types.track {
        let rows = sqlx::query!(
            r#"WITH docs AS (
                   SELECT t.id,
                          t.title || CASE WHEN t.title_version <> '' THEN ' ' || t.title_version ELSE '' END AS title,
                          t.latin || ' ' || COALESCE(string_agg(a.latin, ' '), '') AS latin,
                          t.sound || ' ' || COALESCE(string_agg(a.sound, ' '), '') AS sound,
                          COALESCE(array_agg(a.name ORDER BY ta.position) FILTER (WHERE a.id IS NOT NULL), '{}') AS artists
                   FROM tracks t
                   LEFT JOIN recordings r ON r.id = t.recording_id
                   LEFT JOIN track_artists ta ON ta.track_id = t.id
                   LEFT JOIN artists a ON a.id = ta.artist_id
                   WHERE r.audio_status = 'ready'
                      OR EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $1 AND l.track_id = t.id)
                      OR EXISTS (SELECT 1 FROM saved_albums s WHERE s.user_id = $1 AND s.album_id = t.album_id)
                   GROUP BY t.id
               )
               SELECT d.id AS "id!", d.title AS "title!", d.artists AS "artists!" FROM docs d
               WHERE EXISTS (SELECT 1 FROM unnest($2::text[]) f
                             WHERE (SELECT bool_and(strpos(d.latin, w) > 0 OR word_similarity(w, d.latin) >= 0.6)
                                    FROM unnest(string_to_array(f, ' ')) w WHERE w <> ''))
                  OR EXISTS (SELECT 1 FROM unnest($3::text[]) k WHERE strpos(' ' || d.sound, ' ' || k) > 0)
               ORDER BY word_similarity($4, d.latin) DESC, d.id
               LIMIT $5"#,
            user_id,
            &latins,
            &sounds,
            main,
            LIBRARY_CANDIDATES,
        )
        .fetch_all(db)
        .await?;
        let scored = rows
            .into_iter()
            .map(|r| {
                titles.push((r.title.clone(), query.score(&r.title)));
                (r.id, query.score_track(&r.title, &r.artists))
            })
            .collect();
        found.hits.tracks = rank(scored);
    }
    if types.album {
        let rows = sqlx::query!(
            r#"WITH docs AS (
                   SELECT al.id, al.title, a.name AS artist,
                          al.latin || ' ' || a.latin AS latin, al.sound || ' ' || a.sound AS sound
                   FROM albums al JOIN artists a ON a.id = al.artist_id
                   WHERE EXISTS (SELECT 1 FROM saved_albums s WHERE s.user_id = $1 AND s.album_id = al.id)
                      OR EXISTS (SELECT 1 FROM tracks t JOIN recordings r ON r.id = t.recording_id
                                 WHERE t.album_id = al.id AND r.audio_status = 'ready')
               )
               SELECT d.id AS "id!", d.title AS "title!", d.artist AS "artist!" FROM docs d
               WHERE EXISTS (SELECT 1 FROM unnest($2::text[]) f
                             WHERE (SELECT bool_and(strpos(d.latin, w) > 0 OR word_similarity(w, d.latin) >= 0.6)
                                    FROM unnest(string_to_array(f, ' ')) w WHERE w <> ''))
                  OR EXISTS (SELECT 1 FROM unnest($3::text[]) k WHERE strpos(' ' || d.sound, ' ' || k) > 0)
               ORDER BY word_similarity($4, d.latin) DESC, d.id
               LIMIT $5"#,
            user_id,
            &latins,
            &sounds,
            main,
            LIBRARY_CANDIDATES,
        )
        .fetch_all(db)
        .await?;
        let scored = rows
            .into_iter()
            .map(|r| {
                album_names.push((r.id, r.title.clone(), query.score(&r.title)));
                (r.id, query.score_track(&r.title, &[&r.artist]))
            })
            .collect();
        found.hits.albums = rank(scored);
    }
    if types.artist {
        let rows = sqlx::query!(
            r#"SELECT a.id, a.name FROM artists a
               WHERE (EXISTS (SELECT 1 FROM followed_artists f WHERE f.user_id = $1 AND f.artist_id = a.id)
                      OR EXISTS (SELECT 1 FROM track_artists ta JOIN liked_tracks l ON l.track_id = ta.track_id
                                 WHERE ta.artist_id = a.id AND l.user_id = $1)
                      OR EXISTS (SELECT 1 FROM track_artists ta JOIN tracks t ON t.id = ta.track_id
                                 JOIN recordings r ON r.id = t.recording_id
                                 WHERE ta.artist_id = a.id AND r.audio_status = 'ready'))
                 AND (EXISTS (SELECT 1 FROM unnest($2::text[]) f
                              WHERE (SELECT bool_and(strpos(a.latin, w) > 0 OR word_similarity(w, a.latin) >= 0.6)
                                     FROM unnest(string_to_array(f, ' ')) w WHERE w <> ''))
                      OR EXISTS (SELECT 1 FROM unnest($3::text[]) k WHERE strpos(' ' || a.sound, ' ' || k) > 0))
               ORDER BY word_similarity($4, a.latin) DESC, a.id
               LIMIT $5"#,
            user_id,
            &latins,
            &sounds,
            main,
            LIBRARY_CANDIDATES,
        )
        .fetch_all(db)
        .await?;
        let scored = rows
            .into_iter()
            .map(|r| {
                let score = query.score(&r.name);
                artist_names.push((r.id, r.name, score));
                (r.id, score)
            })
            .collect();
        found.hits.artists = rank(scored);
    }
    follow(&mut artist_names, &found.hits.artists);
    follow(&mut album_names, &found.hits.albums);
    (found.top, found.suggestion) = verdict(&query, &artist_names, &album_names, &titles);
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_stay_within_budget() {
        let q = Query::new("капсайз");
        assert_eq!(variants(&q, 0.5, &[]), ["capsiz"]);
        let known = [Known {
            id: Uuid::nil(),
            name: "Capsize".into(),
            fans: 1,
            score: 0.93,
        }];
        assert_eq!(variants(&q, 0.5, &known), ["Capsize", "capsiz"]);
        // Не та раскладка — сначала исправленная
        let q = Query::new("сфзышяу");
        assert_eq!(variants(&q, 0.1, &[]), ["capsize"]);
        // Латиница, которая и так что-то нашла, — без вариантов
        assert!(variants(&Query::new("capsze"), 0.84, &[]).is_empty());
        assert_eq!(
            variants(&Query::new("skriptonit"), 0.3, &[]),
            ["скриптонит"]
        );
    }
}
