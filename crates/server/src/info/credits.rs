//! Кто сделал трек: авторы музыки и слов, продюсеры и звукорежиссёры из MusicBrainz.
//!
//! Запись ищется по ISRC (несколько записей с одним кодом — ближайшая по длительности), её
//! отношения читаются одним запросом вместе с произведением и его авторами. Ответ хранится в
//! `recording_credits` по ISRC: найденное — 90 дней, «MusicBrainz не знает» — 14.
//!
//! Имя ведёт на страницу артиста, только если MusicBrainz знает страницу этого человека в Deezer:
//! однофамильцев по имени не сопоставляем. Ссылки ищутся в фоне после первого показа (по запросу
//! в секунду), а пока — просто имена.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{LazyLock, Mutex},
};

use chrono::{Duration, Utc};
use dalywave_protocol::info::{CreditPerson, CreditRole, TrackCredits};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    InfoError,
    musicbrainz::{MusicBrainz, Recording, Relation},
};
use crate::{catalog, deezer::Deezer};

const FOUND_DAYS: i64 = 90;
const NOT_FOUND_DAYS: i64 = 14;
/// Сколько записей с одним ISRC смотреть, пока у какой-нибудь не найдутся отношения.
const MAX_LOOKUPS: usize = 2;
/// Запись MusicBrainz длиннее или короче трека больше чем на столько — другая версия.
const LENGTH_SLACK_MS: i64 = 15_000;
/// Сколько людей трека искать в фоне (каждый — запрос в секунду).
const RESOLVE_LIMIT: usize = 16;

/// Человек записи с ролями — так хранится в `recording_credits.people`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub id: Uuid,
    pub name: String,
    pub roles: Vec<CreditRole>,
}

/// Роль по отношению MusicBrainz. Ассистенты и исполнительные продюсеры не показываются.
fn role(rel: &Relation, work_level: bool) -> Option<CreditRole> {
    let has = |a: &str| rel.attributes.iter().any(|x| x == a);
    if has("assistant") || has("executive") {
        return None;
    }
    if work_level {
        return match rel.kind.as_str() {
            "composer" => Some(CreditRole::Composer),
            "lyricist" | "librettist" => Some(CreditRole::Lyricist),
            "writer" => Some(CreditRole::Writer),
            "arranger" => Some(CreditRole::Arranger),
            _ => None,
        };
    }
    match rel.kind.as_str() {
        "producer" => Some(CreditRole::Producer),
        "engineer" | "audio" | "sound" => Some(CreditRole::Engineer),
        "recording" => Some(CreditRole::Recording),
        "mix" => Some(CreditRole::Mix),
        "mastering" => Some(CreditRole::Mastering),
        "programming" => Some(CreditRole::Programming),
        "arranger" | "instrument arranger" | "vocal arranger" | "orchestrator" => {
            Some(CreditRole::Arranger)
        }
        "composer" => Some(CreditRole::Composer),
        "lyricist" => Some(CreditRole::Lyricist),
        "writer" => Some(CreditRole::Writer),
        _ => None,
    }
}

/// Люди записи по порядку появления: сначала авторы произведения, потом работавшие над записью.
pub fn people(rec: &Recording) -> Vec<Person> {
    let mut order: Vec<Uuid> = Vec::new();
    let mut found: HashMap<Uuid, (String, BTreeSet<CreditRole>)> = HashMap::new();
    let mut add = |id: Uuid, name: &str, r: CreditRole| {
        let entry = found.entry(id).or_insert_with(|| {
            order.push(id);
            (name.to_string(), BTreeSet::new())
        });
        entry.1.insert(r);
    };
    for rel in &rec.relations {
        if let Some(work) = rel.work.as_ref().filter(|_| rel.kind == "performance") {
            for wr in &work.relations {
                if let (Some(a), Some(r)) = (&wr.artist, role(wr, true)) {
                    add(a.id, &a.name, r);
                }
            }
        }
    }
    for rel in &rec.relations {
        if let (Some(a), Some(r)) = (&rel.artist, role(rel, false)) {
            add(a.id, &a.name, r);
        }
    }
    order
        .into_iter()
        .filter_map(|id| {
            let (name, roles) = found.remove(&id)?;
            Some(Person {
                id,
                name,
                roles: roles.into_iter().collect(),
            })
        })
        .collect()
}

/// Записи-кандидаты: уверенные совпадения поиска, ближайшие по длительности первыми.
pub fn candidates(recs: &[Recording], duration_ms: i64) -> Vec<Uuid> {
    let mut good: Vec<(i64, usize, Uuid)> = recs
        .iter()
        .enumerate()
        .filter(|(_, r)| r.score.unwrap_or(100) >= 90)
        .filter_map(|(i, r)| {
            let diff = match r.length {
                Some(len) if duration_ms > 0 => (len - duration_ms).abs(),
                _ => LENGTH_SLACK_MS,
            };
            (diff <= LENGTH_SLACK_MS).then_some((diff, i, r.id))
        })
        .collect();
    good.sort();
    good.into_iter().map(|(_, _, id)| id).collect()
}

/// Люди записи с этим ISRC из MusicBrainz. `None` — MusicBrainz такой записи не знает.
pub async fn fetch(
    mb: &MusicBrainz,
    isrc: &str,
    duration_ms: i64,
) -> Result<Option<Vec<Person>>, InfoError> {
    let recs = mb.recordings_by_isrc(isrc).await?;
    let ids = candidates(&recs, duration_ms);
    if ids.is_empty() {
        return Ok(None);
    }
    for id in ids.into_iter().take(MAX_LOOKUPS) {
        if let Some(rec) = mb.recording(id).await? {
            let found = people(&rec);
            if !found.is_empty() {
                return Ok(Some(found));
            }
        }
    }
    Ok(Some(Vec::new()))
}

/// Люди записи: из `recording_credits` или из MusicBrainz. Сбой MusicBrainz при сохранённом ответе —
/// сохранённый ответ.
pub async fn load(
    db: &PgPool,
    mb: &MusicBrainz,
    isrc: &str,
    duration_ms: i64,
) -> Result<Vec<Person>, LoadError> {
    let row = sqlx::query!(
        "SELECT found, people, fetched_at FROM recording_credits WHERE isrc = $1",
        isrc
    )
    .fetch_optional(db)
    .await?;
    let stored = row
        .as_ref()
        .map(|r| serde_json::from_value::<Vec<Person>>(r.people.clone()).unwrap_or_default());
    if let Some(r) = &row {
        let ttl = Duration::days(if r.found { FOUND_DAYS } else { NOT_FOUND_DAYS });
        if r.fetched_at > Utc::now() - ttl {
            return Ok(stored.unwrap_or_default());
        }
    }
    let fetched = match fetch(mb, isrc, duration_ms).await {
        Ok(f) => f,
        Err(e) => {
            return match stored {
                Some(s) => {
                    tracing::warn!(isrc, error = %e, "musicbrainz failed, showing stored credits");
                    Ok(s)
                }
                None => Err(LoadError::Source(e)),
            };
        }
    };
    let found = fetched.is_some();
    let people = fetched.unwrap_or_default();
    let mut tx = db.begin().await?;
    sqlx::query!(
        "INSERT INTO recording_credits (isrc, found, people, fetched_at) VALUES ($1, $2, $3, now())
         ON CONFLICT (isrc) DO UPDATE SET found = $2, people = $3, fetched_at = now()",
        isrc,
        found,
        serde_json::to_value(&people).unwrap_or_default(),
    )
    .execute(&mut *tx)
    .await?;
    for p in &people {
        sqlx::query!(
            "INSERT INTO music_people (mbid, name) VALUES ($1, $2)
             ON CONFLICT (mbid) DO UPDATE SET name = $2",
            p.id,
            p.name,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(people)
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Source(#[from] InfoError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Ответ API: авторы отдельно от записи, у кого найдена страница — ссылка.
pub async fn view(
    db: &PgPool,
    track_id: Uuid,
    people: &[Person],
    label: Option<String>,
    year: Option<i32>,
) -> Result<(TrackCredits, Vec<Uuid>), sqlx::Error> {
    let ids: Vec<Uuid> = people.iter().map(|p| p.id).collect();
    let links: HashMap<Uuid, (Option<Uuid>, bool)> = sqlx::query!(
        "SELECT mbid, artist_id, checked_at IS NOT NULL AS \"checked!\" FROM music_people WHERE mbid = ANY($1)",
        &ids
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.mbid, (r.artist_id, r.checked)))
    .collect();
    let unchecked: Vec<Uuid> = ids
        .iter()
        .filter(|id| !links.get(*id).is_some_and(|l| l.1))
        .take(RESOLVE_LIMIT)
        .copied()
        .collect();
    let person = |p: &Person, writing: bool| -> Option<CreditPerson> {
        let roles: Vec<CreditRole> = p
            .roles
            .iter()
            .copied()
            .filter(|r| r.is_writing() == writing)
            .collect();
        (!roles.is_empty()).then(|| CreditPerson {
            id: p.id,
            name: p.name.clone(),
            roles,
            artist_id: links.get(&p.id).and_then(|l| l.0),
        })
    };
    let mut production: Vec<CreditPerson> =
        people.iter().filter_map(|p| person(p, false)).collect();
    // Продюсеры первыми, затем звукорежиссёры по этапам: запись, сведение, мастеринг
    production.sort_by_key(|p| p.roles.iter().min().copied());
    Ok((
        TrackCredits {
            track_id,
            writing: people.iter().filter_map(|p| person(p, true)).collect(),
            production,
            label,
            year,
            resolving: !unchecked.is_empty(),
        },
        unchecked,
    ))
}

/// Люди, чьи ссылки ищутся прямо сейчас: второй показ того же трека не ставит их снова.
static RESOLVING: LazyLock<Mutex<HashSet<Uuid>>> = LazyLock::new(Default::default);

/// Ищет страницы людей в Deezer по ссылкам MusicBrainz и запоминает артиста каталога. Страница без
/// релизов не считается: на ней нечего послушать. Сбой MusicBrainz — остальные в другой раз.
pub async fn resolve_people(db: &PgPool, mb: &MusicBrainz, deezer: &Deezer, ids: Vec<Uuid>) {
    let mine: Vec<Uuid> = {
        let mut busy = RESOLVING.lock().unwrap_or_else(|e| e.into_inner());
        ids.into_iter().filter(|id| busy.insert(*id)).collect()
    };
    for (i, id) in mine.iter().enumerate() {
        let artist_id = match resolve_one(db, mb, deezer, *id).await {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(mbid = %id, error = %e, "could not resolve a credited person");
                let mut busy = RESOLVING.lock().unwrap_or_else(|e| e.into_inner());
                for left in &mine[i..] {
                    busy.remove(left);
                }
                return;
            }
        };
        let saved = sqlx::query!(
            "UPDATE music_people SET artist_id = $2, checked_at = now() WHERE mbid = $1",
            id,
            artist_id,
        )
        .execute(db)
        .await;
        if let Err(e) = saved {
            tracing::warn!(mbid = %id, error = %e, "could not save a credited person");
        }
        RESOLVING
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }
}

async fn resolve_one(
    db: &PgPool,
    mb: &MusicBrainz,
    deezer: &Deezer,
    id: Uuid,
) -> Result<Option<Uuid>, String> {
    let Some(artist) = mb.artist(id).await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    for deezer_id in artist.deezer_ids().into_iter().take(2) {
        let found = match deezer.artist(deezer_id).await {
            Ok(a) => a,
            Err(crate::deezer::DeezerError::NotFound) => continue,
            Err(e) => return Err(e.to_string()),
        };
        if found.nb_album.unwrap_or(0) == 0 {
            continue;
        }
        let mut conn = db.acquire().await.map_err(|e| e.to_string())?;
        let ours = catalog::upsert_artist(&mut conn, &found)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(Some(ours));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording() -> Recording {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/musicbrainz/recording_bad_guy.json"
        ))
        .unwrap()
    }

    #[test]
    fn writers_come_from_the_work_and_engineers_from_the_recording() {
        let found = people(&recording());
        let names: Vec<(&str, Vec<CreditRole>)> = found
            .iter()
            .map(|p| (p.name.as_str(), p.roles.clone()))
            .collect();
        assert_eq!(
            names,
            vec![
                (
                    "Billie Eilish",
                    vec![CreditRole::Writer, CreditRole::Producer]
                ),
                ("FINNEAS", vec![CreditRole::Writer, CreditRole::Producer]),
                ("Rob Kinelski", vec![CreditRole::Mix]),
            ],
            "ассистент сведения и полевая запись не показываются"
        );
    }

    #[test]
    fn closest_recording_first() {
        let search: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/musicbrainz/search_isrc_bad_guy.json"
        ))
        .unwrap();
        let recs: Vec<Recording> = serde_json::from_value(search["recordings"].clone()).unwrap();
        let ids = candidates(&recs, 194_088);
        assert_eq!(ids.len(), 2, "запись на 175 секунд — другая версия");
        assert_eq!(ids[0].to_string(), "28c07f52-4dd2-4991-ab6f-435b1b313a0b");
        // Длительность трека неизвестна — все уверенные совпадения по порядку
        assert_eq!(candidates(&recs, 0).len(), 3);
    }
}
