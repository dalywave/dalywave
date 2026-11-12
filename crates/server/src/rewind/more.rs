//! «Ещё послушать» в конце историй: новое для слушателя (ни разу не игравшее и не лайкнутое), похожее по
//! звучанию на любимое за год. Соседи каждого из любимых треков идут по очереди — подборка не сводится
//! к одному из них; от артиста — не больше двух треков. Без анализа звука — популярное у похожих
//! артистов самых слушаемых. Сохраняется плейлистом «Ещё послушать {год}» только по просьбе.

use std::collections::{HashMap, HashSet};

use dalywave_protocol::{catalog::Track, library::PlaylistSummary};
use sqlx::PgPool;
use uuid::Uuid;

use super::{generated_playlist, top_track_ids, write_playlist};
use crate::{
    catalog::views,
    reco::{self, Cand, rank},
};

/// `rules.generator` плейлиста «Ещё послушать {год}».
pub const GENERATOR: &str = "rewind_more";
/// Треков в подборке.
pub const SIZE: usize = 30;
/// От скольких любимых треков года искать соседей.
const SEEDS: i64 = 12;
/// Соседей у каждого.
const PER_SEED: i64 = 16;
const PER_ARTIST: usize = 2;

/// По одному из каждого списка по кругу, без повторов.
pub fn interleave(lists: &[Vec<Uuid>]) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
    for i in 0..longest {
        for list in lists {
            if let Some(id) = list.get(i)
                && seen.insert(*id)
            {
                out.push(*id);
            }
        }
    }
    out
}

/// Отбор из кандидатов по порядку: годное к скачиванию, без исключённого, одна запись — один раз,
/// не больше [`PER_ARTIST`] от артиста.
pub fn select(cands: Vec<Cand>, limit: usize) -> Vec<Uuid> {
    let mut recordings = HashSet::new();
    let mut per_artist: HashMap<Uuid, usize> = HashMap::new();
    let mut out = Vec::new();
    for c in cands {
        if out.len() >= limit {
            break;
        }
        if c.excluded || c.completed > 0 || c.liked || !rank::acquirable(&c) {
            continue;
        }
        if let Some(r) = c.recording_id
            && !recordings.insert(r)
        {
            continue;
        }
        let artist = c.artist_ids.first().copied().unwrap_or(c.track_id);
        let n = per_artist.entry(artist).or_default();
        if *n >= PER_ARTIST {
            continue;
        }
        *n += 1;
        out.push(c.track_id);
    }
    out
}

async fn by_sound(db: &PgPool, user_id: Uuid, seeds: &[Uuid]) -> Result<Vec<Uuid>, sqlx::Error> {
    let fit = rank::Fit {
        unheard: true,
        ..Default::default()
    };
    let mut lists = Vec::new();
    let mut tx = db.begin().await?;
    reco::widen_search(&mut tx, PER_SEED * 2).await?;
    for seed in seeds {
        if let Some(v) = reco::track_sound(&mut *tx, *seed).await? {
            let near = reco::nearest_fitting(&mut tx, &v, PER_SEED, user_id, &fit).await?;
            lists.push(near.into_iter().map(|n| n.track_id).collect::<Vec<_>>());
        }
    }
    tx.commit().await?;
    Ok(interleave(&lists))
}

/// Та же песня другим релизом (сингл и альбом — разные записи): слушатель её уже знает.
async fn known_by_name(
    db: &PgPool,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<HashSet<Uuid>, sqlx::Error> {
    Ok(sqlx::query_scalar!(
        r#"SELECT t.id FROM tracks t
           WHERE t.id = ANY($1) AND EXISTS (
               SELECT 1 FROM track_artists a1
               JOIN track_artists a2 ON a2.artist_id = a1.artist_id AND a2.role = 'main'
               JOIN tracks t2 ON t2.id = a2.track_id AND t2.id <> t.id AND lower(t2.title) = lower(t.title)
               WHERE a1.track_id = t.id AND a1.role = 'main'
                 AND (EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = $2 AND p.track_id = t2.id)
                      OR EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $2 AND l.track_id = t2.id)))"#,
        ids,
        user_id,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .collect())
}

/// Кандидаты по порядку без того, что слушатель уже знает под другим релизом.
async fn fresh(db: &PgPool, user_id: Uuid, ids: &[Uuid]) -> Result<Vec<Cand>, sqlx::Error> {
    let known = known_by_name(db, user_id, ids).await?;
    Ok(reco::candidates(db, ids, user_id, None, None)
        .await?
        .into_iter()
        .filter(|c| !known.contains(&c.track_id))
        .collect())
}

async fn by_related(db: &PgPool, user_id: Uuid, seeds: &[Uuid]) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"WITH top AS (
               SELECT DISTINCT ON (ta.artist_id) ta.artist_id, x.n
               FROM unnest($1::uuid[]) WITH ORDINALITY AS x(t, n)
               JOIN track_artists ta ON ta.track_id = x.t AND ta.role = 'main'
               ORDER BY ta.artist_id, x.n
           )
           SELECT tt.track_id AS "track_id!" FROM top
           JOIN artist_related r ON r.artist_id = top.artist_id
           JOIN artist_top_tracks tt ON tt.artist_id = r.related_id
           WHERE NOT EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = $2 AND p.track_id = tt.track_id)
           ORDER BY tt.position, top.n, r.position
           LIMIT $3"#,
        seeds,
        user_id,
        (SIZE * 4) as i64,
    )
    .fetch_all(db)
    .await
}

/// Подборка «Ещё послушать» за год: треки по порядку.
pub async fn generate(
    db: &PgPool,
    user_id: Uuid,
    year: i32,
    limit: usize,
) -> Result<Vec<Track>, sqlx::Error> {
    let seeds = top_track_ids(db, user_id, year, SEEDS).await?;
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let mut picked = select(
        fresh(db, user_id, &by_sound(db, user_id, &seeds).await?).await?,
        limit,
    );
    if picked.len() < limit / 2 {
        let extra = fresh(db, user_id, &by_related(db, user_id, &seeds).await?).await?;
        let have: HashSet<Uuid> = picked.iter().copied().collect();
        let more = select(
            extra
                .into_iter()
                .filter(|c| !have.contains(&c.track_id))
                .collect(),
            limit - picked.len(),
        );
        picked.extend(more);
    }
    let mut tracks: HashMap<Uuid, Track> = views::tracks(db, &picked, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    Ok(picked.iter().filter_map(|id| tracks.remove(id)).collect())
}

/// Сохранённый плейлист «Ещё послушать {год}».
pub async fn playlist(db: &PgPool, user_id: Uuid, year: i32) -> Result<Option<Uuid>, sqlx::Error> {
    generated_playlist(db, user_id, GENERATOR, year).await
}

/// Сохранить показанную подборку: неизвестные треки отбрасываются. `None` — сохранять нечего.
pub async fn save(
    db: &PgPool,
    user_id: Uuid,
    year: i32,
    track_ids: &[Uuid],
) -> Result<Option<(PlaylistSummary, bool)>, sqlx::Error> {
    let known: HashSet<Uuid> =
        sqlx::query_scalar!("SELECT id FROM tracks WHERE id = ANY($1)", track_ids)
            .fetch_all(db)
            .await?
            .into_iter()
            .collect();
    let mut seen = HashSet::new();
    let tracks: Vec<Uuid> = track_ids
        .iter()
        .copied()
        .filter(|id| known.contains(id) && seen.insert(*id))
        .collect();
    if tracks.is_empty() {
        return Ok(None);
    }
    let title = format!("Ещё послушать {year}");
    let description = format!("Новое, похожее на то, что ты любил в {year}");
    write_playlist(
        db,
        user_id,
        (GENERATOR, year),
        (&title, &description),
        &tracks,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn neighbours_take_turns() {
        let lists = vec![
            vec![id(1), id(2), id(3)],
            vec![id(10), id(1)],
            vec![],
            vec![id(20)],
        ];
        assert_eq!(
            interleave(&lists),
            vec![id(1), id(10), id(20), id(2), id(3)]
        );
        assert!(interleave(&[]).is_empty());
    }

    fn cand(n: u128, artist: u128) -> Cand {
        Cand {
            track_id: id(n),
            recording_id: Some(id(n + 1000)),
            deezer_id: Some(n as i64),
            artist_ids: vec![id(artist)],
            ..Default::default()
        }
    }

    #[test]
    fn selection_skips_heard_excluded_and_crowded_artists() {
        let mut heard = cand(2, 50);
        heard.completed = 3;
        let mut excluded = cand(3, 51);
        excluded.excluded = true;
        let mut same_recording = cand(4, 52);
        same_recording.recording_id = Some(id(1001));
        let mut gone = cand(5, 53);
        gone.deezer_id = None;
        let list = vec![
            cand(1, 50),
            heard,
            excluded,
            same_recording,
            gone,
            cand(6, 50),
            cand(7, 50),
            cand(8, 54),
        ];
        assert_eq!(select(list.clone(), 10), vec![id(1), id(6), id(8)]);
        assert_eq!(select(list, 2), vec![id(1), id(6)]);
    }
}
