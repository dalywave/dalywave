//! Каталог — копия Deezer в Postgres, заполняется при первом обращении и освежается по срокам:
//! карточка артиста, похожие и список релизов — 7 дней, популярное — сутки, альбом с треками — 30 дней.
//!
//! Трек и запись — разные сущности: треки с одинаковым ISRC (сингл и он же в альбоме) указывают на одну
//! запись, трек без ISRC получает свою. Пока ISRC неизвестен (трек пришёл из популярного артиста),
//! записи у трека нет; она появится, когда трек загрузится целиком или в составе альбома.

pub mod find;
pub mod views;

use std::{collections::HashSet, time::Duration};

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    catalog::{ArtistRole, RecordType},
    jobs::{AcquireReason, RefreshArtistPayload, kind},
    library::DownloadPolicy,
};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    acquire::{self, Enqueue},
    deezer::{Deezer, DeezerError, types as dz},
    error::AppError,
    jobs::{self, NewJob, lanes},
};

pub const ARTIST_TTL: Duration = Duration::from_secs(7 * 86400);
pub const TOP_TTL: Duration = Duration::from_secs(86400);
pub const RELATED_TTL: Duration = Duration::from_secs(7 * 86400);
pub const RELEASES_TTL: Duration = Duration::from_secs(7 * 86400);
pub const ALBUM_TTL: Duration = Duration::from_secs(30 * 86400);
pub const TOP_LIMIT: u32 = 10;
pub const RELATED_LIMIT: u32 = 20;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error(transparent)]
    Deezer(#[from] DeezerError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl From<CatalogError> for AppError {
    fn from(e: CatalogError) -> Self {
        match e {
            CatalogError::Deezer(DeezerError::NotFound) => AppError::NotFound,
            CatalogError::Deezer(DeezerError::Disabled) => AppError::Conflict {
                code: "deezer_disabled",
                message: "каталог Deezer выключен на этом сервере".into(),
            },
            CatalogError::Deezer(e) => AppError::Unavailable(format!(
                "каталог Deezer сейчас недоступен, попробуйте позже ({e})"
            )),
            CatalogError::Db(e) => AppError::from(e),
        }
    }
}

/// Что известно об ISRC трека из этого ответа Deezer.
#[derive(Debug, Clone, Copy)]
pub enum Isrc<'a> {
    /// Ответ без кодов (популярное артиста, чарт): запись не трогаем.
    Unknown,
    /// Ответ с кодами; `None` — у трека в Deezer кода нет, он получает свою запись.
    Known(Option<&'a str>),
}

/// Главный артист альбома.
#[derive(Debug, Clone, Copy)]
pub enum AlbumArtist {
    /// Из ответа Deezer про сам альбом — верный.
    Known(Uuid),
    /// Догадка (альбом пришёл в списке без артиста): только для новой строки.
    Fallback(Uuid),
}

fn too_old(at: Option<DateTime<Utc>>, ttl: Duration) -> bool {
    at.is_none_or(|at| {
        Utc::now() - at > chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::MAX)
    })
}

pub fn normalize_isrc(code: &str) -> Option<String> {
    let code: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    (code.len() == 12).then_some(code)
}

// ───────────── Запись в базу ─────────────

pub async fn upsert_artist(conn: &mut PgConnection, a: &dz::Artist) -> Result<Uuid, sqlx::Error> {
    let card = a.has_card();
    sqlx::query_scalar!(
        "INSERT INTO artists (deezer_id, name, picture_source, fans, albums_count, fetched_at)
         VALUES ($1, $2, $3, $4, $5, CASE WHEN $6 THEN now() END)
         ON CONFLICT (deezer_id) DO UPDATE SET
            name = EXCLUDED.name,
            picture_source = COALESCE(EXCLUDED.picture_source, artists.picture_source),
            picture_key = CASE WHEN EXCLUDED.picture_source IS DISTINCT FROM artists.picture_source
                                    AND EXCLUDED.picture_source IS NOT NULL
                               THEN NULL ELSE artists.picture_key END,
            picture_bytes = CASE WHEN EXCLUDED.picture_source IS DISTINCT FROM artists.picture_source
                                      AND EXCLUDED.picture_source IS NOT NULL
                                 THEN NULL ELSE artists.picture_bytes END,
            fans = COALESCE(EXCLUDED.fans, artists.fans),
            albums_count = COALESCE(EXCLUDED.albums_count, artists.albums_count),
            fetched_at = COALESCE(EXCLUDED.fetched_at, artists.fetched_at),
            updated_at = now()
         RETURNING id",
        a.id,
        a.name.trim(),
        a.picture(),
        a.nb_fan,
        a.nb_album,
        card,
    )
    .fetch_one(conn)
    .await
}

pub async fn upsert_album(
    conn: &mut PgConnection,
    a: &dz::Album,
    artist: AlbumArtist,
) -> Result<Uuid, sqlx::Error> {
    let (artist_id, authoritative) = match artist {
        AlbumArtist::Known(id) => (id, true),
        AlbumArtist::Fallback(id) => (id, false),
    };
    let record_type = a
        .record_type
        .as_deref()
        .and_then(|t| t.parse::<RecordType>().ok())
        .map(RecordType::as_str);
    let genres = a.genre_names();
    sqlx::query_scalar!(
        "INSERT INTO albums (deezer_id, upc, title, artist_id, record_type, release_date, label, genres,
                             explicit, tracks_count, duration_ms, fans, cover_source)
         VALUES ($1, $2, $3, $4, COALESCE($5, 'album'), $6, $7, $8, COALESCE($9, false), $10, $11, $12, $13)
         ON CONFLICT (deezer_id) DO UPDATE SET
            upc = COALESCE(EXCLUDED.upc, albums.upc),
            title = EXCLUDED.title,
            artist_id = CASE WHEN $14 THEN EXCLUDED.artist_id ELSE albums.artist_id END,
            record_type = CASE WHEN $5::text IS NOT NULL THEN EXCLUDED.record_type ELSE albums.record_type END,
            release_date = COALESCE(EXCLUDED.release_date, albums.release_date),
            label = COALESCE(EXCLUDED.label, albums.label),
            genres = CASE WHEN cardinality(EXCLUDED.genres) > 0 THEN EXCLUDED.genres ELSE albums.genres END,
            explicit = CASE WHEN $9::boolean IS NOT NULL THEN EXCLUDED.explicit ELSE albums.explicit END,
            tracks_count = COALESCE(EXCLUDED.tracks_count, albums.tracks_count),
            duration_ms = COALESCE(EXCLUDED.duration_ms, albums.duration_ms),
            fans = COALESCE(EXCLUDED.fans, albums.fans),
            cover_source = COALESCE(EXCLUDED.cover_source, albums.cover_source),
            cover_key = CASE WHEN EXCLUDED.cover_source IS DISTINCT FROM albums.cover_source
                                  AND EXCLUDED.cover_source IS NOT NULL
                             THEN NULL ELSE albums.cover_key END,
            cover_bytes = CASE WHEN EXCLUDED.cover_source IS DISTINCT FROM albums.cover_source
                                    AND EXCLUDED.cover_source IS NOT NULL
                               THEN NULL ELSE albums.cover_bytes END,
            cover_palette = CASE WHEN EXCLUDED.cover_source IS DISTINCT FROM albums.cover_source
                                      AND EXCLUDED.cover_source IS NOT NULL
                                 THEN NULL ELSE albums.cover_palette END,
            updated_at = now()
         RETURNING id",
        a.id,
        a.upc,
        a.title.trim(),
        artist_id,
        record_type,
        a.release_date(),
        a.label,
        &genres,
        a.explicit_lyrics,
        a.nb_tracks,
        a.duration.map(|s| s.saturating_mul(1000)),
        a.fans,
        a.cover(),
        authoritative,
    )
    .fetch_one(conn)
    .await
}

/// Трек из ответа Deezer.
pub struct TrackIn<'a> {
    pub track: &'a dz::Track,
    pub album_id: Option<Uuid>,
    pub isrc: Isrc<'a>,
    /// Пришёл трек целиком (`track/{id}`): ISRC и все участники известны.
    pub full: bool,
}

async fn recording_by_isrc(
    conn: &mut PgConnection,
    isrc: &str,
    duration_ms: i32,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar!(
        "INSERT INTO recordings (isrc, duration_ms) VALUES ($1, $2)
         ON CONFLICT (isrc) DO UPDATE SET isrc = EXCLUDED.isrc
         RETURNING id",
        isrc,
        duration_ms,
    )
    .fetch_one(conn)
    .await
}

pub async fn upsert_track(
    conn: &mut PgConnection,
    input: TrackIn<'_>,
) -> Result<Uuid, sqlx::Error> {
    let t = input.track;
    let duration_ms = t.duration.saturating_mul(1000);
    let old_recording =
        sqlx::query_scalar!("SELECT recording_id FROM tracks WHERE deezer_id = $1", t.id)
            .fetch_optional(&mut *conn)
            .await?
            .flatten();
    let isrc = match input.isrc {
        Isrc::Known(Some(code)) => normalize_isrc(code),
        _ => None,
    };
    let recording_id = match (input.isrc, &isrc) {
        (Isrc::Known(_), Some(code)) => Some(recording_by_isrc(conn, code, duration_ms).await?),
        // Кода нет (или он кривой): своя запись, если её ещё нет
        (Isrc::Known(_), None) => match old_recording {
            Some(id) => Some(id),
            None => Some(
                sqlx::query_scalar!(
                    "INSERT INTO recordings (duration_ms) VALUES ($1) RETURNING id",
                    duration_ms
                )
                .fetch_one(&mut *conn)
                .await?,
            ),
        },
        (Isrc::Unknown, _) => old_recording,
    };
    let (title, version) = t.split_title();
    let id = sqlx::query_scalar!(
        "INSERT INTO tracks (deezer_id, isrc, recording_id, title, title_version, album_id, disc_number,
                             track_position, duration_ms, explicit, rank, fetched_at)
         VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7::smallint, 1), $8, $9, COALESCE($10, false), $11, CASE WHEN $12 THEN now() END)
         ON CONFLICT (deezer_id) DO UPDATE SET
            isrc = COALESCE(EXCLUDED.isrc, tracks.isrc),
            recording_id = COALESCE(EXCLUDED.recording_id, tracks.recording_id),
            title = EXCLUDED.title,
            title_version = EXCLUDED.title_version,
            album_id = COALESCE(EXCLUDED.album_id, tracks.album_id),
            disc_number = CASE WHEN $7::smallint IS NOT NULL THEN EXCLUDED.disc_number ELSE tracks.disc_number END,
            track_position = COALESCE(EXCLUDED.track_position, tracks.track_position),
            duration_ms = EXCLUDED.duration_ms,
            explicit = CASE WHEN $10::boolean IS NOT NULL THEN EXCLUDED.explicit ELSE tracks.explicit END,
            rank = COALESCE(EXCLUDED.rank, tracks.rank),
            fetched_at = COALESCE(EXCLUDED.fetched_at, tracks.fetched_at),
            updated_at = now()
         RETURNING id",
        t.id,
        isrc,
        recording_id,
        title.trim(),
        version.trim(),
        input.album_id,
        t.disk_number,
        t.track_position,
        duration_ms,
        t.explicit_lyrics,
        t.rank,
        input.full,
    )
    .fetch_one(&mut *conn)
    .await?;
    if let (Some(old), Some(new)) = (old_recording, recording_id)
        && old != new
    {
        // Трек переехал на запись по ISRC: прежняя своя запись без файлов больше никому не нужна
        sqlx::query!(
            "DELETE FROM recordings r WHERE r.id = $1
               AND NOT EXISTS (SELECT 1 FROM tracks WHERE recording_id = r.id)
               AND NOT EXISTS (SELECT 1 FROM audio_files WHERE recording_id = r.id)
               AND NOT EXISTS (SELECT 1 FROM acquisitions WHERE recording_id = r.id)",
            old,
        )
        .execute(&mut *conn)
        .await?;
    }
    set_track_artists(conn, id, t).await?;
    Ok(id)
}

/// Участники трека: основные первыми, затем приглашённые, в порядке Deezer. Список только с главным
/// артистом (треки альбома, поиск) не затирает полный список участников.
async fn set_track_artists(
    conn: &mut PgConnection,
    track_id: Uuid,
    t: &dz::Track,
) -> Result<(), sqlx::Error> {
    let complete = !t.contributors.is_empty();
    let featured = |c: &&dz::Artist| c.role.as_deref() == Some("Featured");
    let people: Vec<(&dz::Artist, ArtistRole)> = if complete {
        t.contributors
            .iter()
            .filter(|c| !featured(c))
            .map(|c| (c, ArtistRole::Main))
            .chain(
                t.contributors
                    .iter()
                    .filter(featured)
                    .map(|c| (c, ArtistRole::Featured)),
            )
            .collect()
    } else {
        t.artist.iter().map(|a| (a, ArtistRole::Main)).collect()
    };
    if people.is_empty() {
        return Ok(());
    }
    let current = sqlx::query!(
        "SELECT artist_id, role FROM track_artists WHERE track_id = $1 ORDER BY position, artist_id",
        track_id
    )
    .fetch_all(&mut *conn)
    .await?;
    if !complete && !current.is_empty() {
        return Ok(());
    }
    let mut wanted: Vec<(Uuid, ArtistRole)> = Vec::with_capacity(people.len());
    for (artist, role) in people {
        let id = upsert_artist(conn, artist).await?;
        if !wanted.iter().any(|(x, _)| *x == id) {
            wanted.push((id, role));
        }
    }
    let same = current.len() == wanted.len()
        && current
            .iter()
            .zip(&wanted)
            .all(|(c, (id, role))| c.artist_id == *id && c.role == role.as_str());
    if same {
        return Ok(());
    }
    let ids: Vec<Uuid> = wanted.iter().map(|(id, _)| *id).collect();
    let roles: Vec<String> = wanted.iter().map(|(_, r)| r.as_str().to_string()).collect();
    let positions: Vec<i16> = (0..wanted.len() as i16).collect();
    sqlx::query!("DELETE FROM track_artists WHERE track_id = $1", track_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "INSERT INTO track_artists (track_id, artist_id, role, position)
         SELECT $1, a, r, p FROM unnest($2::uuid[], $3::text[], $4::smallint[]) AS x(a, r, p)",
        track_id,
        &ids,
        &roles,
        &positions,
    )
    .execute(&mut *conn)
    .await?;
    // Участники входят в синхронизируемую строку трека, а триггер на них не срабатывает
    sqlx::query!(
        "UPDATE tracks SET rev = nextval('sync_rev'), txid = pg_current_xact_id() WHERE id = $1",
        track_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Альбом из ответа Deezer про сам альбом: главный артист берётся оттуда же.
async fn save_album_detail(
    conn: &mut PgConnection,
    album: &dz::Album,
    fallback: Option<Uuid>,
) -> Result<Option<Uuid>, sqlx::Error> {
    let artist = match &album.artist {
        Some(a) => AlbumArtist::Known(upsert_artist(conn, a).await?),
        None => match fallback {
            Some(id) => AlbumArtist::Fallback(id),
            None => return Ok(None),
        },
    };
    Ok(Some(upsert_album(conn, album, artist).await?))
}

/// Трек из списка, где у трека есть альбом (поиск, популярное, чарт): альбом и артист — облегчённые.
async fn save_listed_track(
    conn: &mut PgConnection,
    t: &dz::Track,
    isrc: Isrc<'_>,
    fallback_artist: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let main = match &t.artist {
        Some(a) => Some(upsert_artist(conn, a).await?),
        None => fallback_artist,
    };
    let album_id = match (&t.album, main) {
        (Some(album), Some(artist)) => {
            Some(upsert_album(conn, album, AlbumArtist::Fallback(artist)).await?)
        }
        _ => None,
    };
    upsert_track(
        conn,
        TrackIn {
            track: t,
            album_id,
            isrc,
            full: false,
        },
    )
    .await
}

// ───────────── Загрузка из Deezer ─────────────

/// Альбом с треками: метаданные, жанры и главный артист — из `album/{id}`, ISRC и позиции — из `album/{id}/tracks`.
pub async fn load_album(
    db: &PgPool,
    deezer: &Deezer,
    deezer_id: i64,
) -> Result<Uuid, CatalogError> {
    let (album, tracks) =
        tokio::try_join!(deezer.album(deezer_id), deezer.album_tracks(deezer_id))?;
    let mut tx = db.begin().await?;
    let fallback = match tracks.first().and_then(|t| t.artist.as_ref()) {
        Some(a) => Some(upsert_artist(&mut tx, a).await?),
        None => None,
    };
    let Some(album_id) = save_album_detail(&mut tx, &album, fallback).await? else {
        return Err(DeezerError::Decode(format!("album {deezer_id} has no artist")).into());
    };
    for t in &tracks {
        upsert_track(
            &mut tx,
            TrackIn {
                track: t,
                album_id: Some(album_id),
                isrc: Isrc::Known(t.isrc.as_deref()),
                full: false,
            },
        )
        .await?;
    }
    sqlx::query!(
        "UPDATE albums SET fetched_at = now(), tracks_count = COALESCE(tracks_count, $2) WHERE id = $1",
        album_id,
        tracks.len() as i32,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(album_id)
}

/// Трек целиком: ISRC, позиция и все участники.
pub async fn load_track(
    db: &PgPool,
    deezer: &Deezer,
    deezer_id: i64,
) -> Result<Uuid, CatalogError> {
    let t = deezer.track(deezer_id).await?;
    Ok(save_track(db, &t).await?)
}

/// Трек из ответа `track/{id}`: ISRC (а с ним и запись), позиция и все участники.
pub async fn save_track(db: &PgPool, t: &dz::Track) -> Result<Uuid, sqlx::Error> {
    let mut tx = db.begin().await?;
    let main = match &t.artist {
        Some(a) => Some(upsert_artist(&mut tx, a).await?),
        None => None,
    };
    let album_id = match (&t.album, main) {
        (Some(album), Some(artist)) => {
            Some(upsert_album(&mut tx, album, AlbumArtist::Fallback(artist)).await?)
        }
        _ => None,
    };
    let id = upsert_track(
        &mut tx,
        TrackIn {
            track: t,
            album_id,
            isrc: Isrc::Known(t.isrc.as_deref()),
            full: true,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Треки из списка (радио артиста, чарт жанра) облегчёнными строками; наши id в том же порядке.
/// Коды ISRC в таких списках обычно нет — запись тогда не трогается.
pub async fn save_listed(db: &PgPool, tracks: &[dz::Track]) -> Result<Vec<Uuid>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let mut ids = Vec::with_capacity(tracks.len());
    for t in tracks {
        let isrc = match t.isrc.as_deref() {
            Some(code) => Isrc::Known(Some(code)),
            None => Isrc::Unknown,
        };
        let id = save_listed_track(&mut tx, t, isrc, None).await?;
        push_unique(&mut ids, id);
    }
    tx.commit().await?;
    Ok(ids)
}

/// Части страницы артиста, которые надо загрузить.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistParts {
    pub card: bool,
    pub top: bool,
    pub related: bool,
    pub releases: bool,
}

impl ArtistParts {
    pub const ALL: Self = Self {
        card: true,
        top: true,
        related: true,
        releases: true,
    };

    pub fn any(self) -> bool {
        self.card || self.top || self.related || self.releases
    }
}

pub struct ArtistFreshness {
    pub deezer_id: Option<i64>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub top_fetched_at: Option<DateTime<Utc>>,
    pub related_fetched_at: Option<DateTime<Utc>>,
    pub albums_fetched_at: Option<DateTime<Utc>>,
}

impl ArtistFreshness {
    /// Ни разу не загружалось.
    pub fn missing(&self) -> ArtistParts {
        ArtistParts {
            card: self.fetched_at.is_none(),
            top: self.top_fetched_at.is_none(),
            related: self.related_fetched_at.is_none(),
            releases: self.albums_fetched_at.is_none(),
        }
    }

    /// Загружалось, но срок вышел (или не загружалось вовсе).
    pub fn stale(&self) -> ArtistParts {
        ArtistParts {
            card: too_old(self.fetched_at, ARTIST_TTL),
            top: too_old(self.top_fetched_at, TOP_TTL),
            related: too_old(self.related_fetched_at, RELATED_TTL),
            releases: too_old(self.albums_fetched_at, RELEASES_TTL),
        }
    }
}

pub async fn artist_freshness(
    db: &PgPool,
    artist_id: Uuid,
) -> Result<Option<ArtistFreshness>, sqlx::Error> {
    sqlx::query_as!(
        ArtistFreshness,
        "SELECT deezer_id, fetched_at, top_fetched_at, related_fetched_at, albums_fetched_at
         FROM artists WHERE id = $1",
        artist_id,
    )
    .fetch_optional(db)
    .await
}

/// Загружает части страницы артиста. Удачные части сохраняются, даже если другие не удались;
/// тогда возвращается первая ошибка.
pub async fn refresh_artist(
    db: &PgPool,
    deezer: &Deezer,
    artist_id: Uuid,
    parts: ArtistParts,
) -> Result<ArtistParts, CatalogError> {
    let Some(deezer_id) = artist_freshness(db, artist_id)
        .await?
        .and_then(|f| f.deezer_id)
    else {
        return Ok(ArtistParts::default());
    };
    let card = async {
        match parts.card {
            true => Some(deezer.artist(deezer_id).await),
            false => None,
        }
    };
    let top = async {
        match parts.top {
            true => Some(deezer.artist_top(deezer_id, TOP_LIMIT).await),
            false => None,
        }
    };
    let related = async {
        match parts.related {
            true => Some(deezer.artist_related(deezer_id, RELATED_LIMIT).await),
            false => None,
        }
    };
    let releases = async {
        match parts.releases {
            true => Some(deezer.artist_albums(deezer_id).await),
            false => None,
        }
    };
    let (card, top, related, releases) = tokio::join!(card, top, related, releases);

    let mut done = ArtistParts::default();
    let mut first_error: Option<DeezerError> = None;
    let mut tx = db.begin().await?;
    match top {
        Some(Ok(tracks)) => {
            let mut ids = Vec::with_capacity(tracks.len());
            for t in &tracks {
                let id = save_listed_track(&mut tx, t, Isrc::Unknown, Some(artist_id)).await?;
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            sqlx::query!(
                "DELETE FROM artist_top_tracks WHERE artist_id = $1",
                artist_id
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "INSERT INTO artist_top_tracks (artist_id, position, track_id)
                 SELECT $1, (p - 1)::smallint, t FROM unnest($2::uuid[]) WITH ORDINALITY AS x(t, p)",
                artist_id,
                &ids,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "UPDATE artists SET top_fetched_at = now() WHERE id = $1",
                artist_id
            )
            .execute(&mut *tx)
            .await?;
            done.top = true;
        }
        Some(Err(e)) => first_error = first_error.or(Some(e)),
        None => {}
    }
    match related {
        Some(Ok(artists)) => {
            let mut ids = Vec::with_capacity(artists.len());
            for a in &artists {
                let id = upsert_artist(&mut tx, a).await?;
                if id != artist_id && !ids.contains(&id) {
                    ids.push(id);
                }
            }
            sqlx::query!("DELETE FROM artist_related WHERE artist_id = $1", artist_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query!(
                "INSERT INTO artist_related (artist_id, position, related_id)
                 SELECT $1, (p - 1)::smallint, r FROM unnest($2::uuid[]) WITH ORDINALITY AS x(r, p)",
                artist_id,
                &ids,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "UPDATE artists SET related_fetched_at = now() WHERE id = $1",
                artist_id
            )
            .execute(&mut *tx)
            .await?;
            done.related = true;
        }
        Some(Err(e)) => first_error = first_error.or(Some(e)),
        None => {}
    }
    match releases {
        Some(Ok(albums)) => {
            let mut ids = Vec::with_capacity(albums.len());
            for album in &albums {
                ids.push(upsert_album(&mut tx, album, AlbumArtist::Fallback(artist_id)).await?);
            }
            sqlx::query!(
                "DELETE FROM artist_releases WHERE artist_id = $1 AND NOT (album_id = ANY($2))",
                artist_id,
                &ids,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "INSERT INTO artist_releases (artist_id, album_id)
                 SELECT $1, unnest($2::uuid[]) ON CONFLICT DO NOTHING",
                artist_id,
                &ids,
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "UPDATE artists SET albums_fetched_at = now() WHERE id = $1",
                artist_id
            )
            .execute(&mut *tx)
            .await?;
            done.releases = true;
        }
        Some(Err(e)) => first_error = first_error.or(Some(e)),
        None => {}
    }
    // Карточка — последней: в списках треков артист приходит облегчённым, а полная карточка точнее
    match card {
        Some(Ok(card)) => {
            upsert_artist(&mut tx, &card).await?;
            done.card = true;
        }
        Some(Err(e)) => first_error = first_error.or(Some(e)),
        None => {}
    }
    tx.commit().await?;
    match first_error {
        Some(e) => Err(e.into()),
        None => Ok(done),
    }
}

/// Страница артиста: то, чего нет совсем, загружается сразу; устаревшее показывается как есть,
/// а обновление уходит в очередь (`refresh_artist`). Сбой Deezer не мешает показать то, что уже есть.
pub async fn ensure_artist(db: &PgPool, deezer: &Deezer, artist_id: Uuid) -> Result<(), AppError> {
    let freshness = artist_freshness(db, artist_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if freshness.deezer_id.is_none() {
        return Ok(());
    }
    let missing = freshness.missing();
    if missing.any()
        && let Err(e) = refresh_artist(db, deezer, artist_id, missing).await
    {
        tracing::warn!(%artist_id, error = %e, "could not load the artist from Deezer");
    }
    let stale = freshness.stale();
    let later = ArtistParts {
        card: stale.card && !missing.card,
        top: stale.top && !missing.top,
        related: stale.related && !missing.related,
        releases: stale.releases && !missing.releases,
    };
    if later.any() {
        let mut conn = db.acquire().await?;
        jobs::enqueue(
            &mut conn,
            NewJob::new(
                kind::REFRESH_ARTIST,
                lanes::MAINTENANCE,
                RefreshArtistPayload {
                    artist_id,
                    force: false,
                },
            )
            .dedupe(format!("refresh_artist:{artist_id}"))
            .priority(1),
        )
        .await?;
    }
    Ok(())
}

/// Альбом с треками: загружается, если ещё не загружен или устарел (30 дней).
pub async fn ensure_album(db: &PgPool, deezer: &Deezer, album_id: Uuid) -> Result<(), AppError> {
    let row = sqlx::query!(
        "SELECT deezer_id, fetched_at FROM albums WHERE id = $1",
        album_id
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    if let Some(deezer_id) = row.deezer_id
        && too_old(row.fetched_at, ALBUM_TTL)
        && let Err(e) = load_album(db, deezer, deezer_id).await
    {
        tracing::warn!(%album_id, error = %e, "could not load the album from Deezer");
    }
    Ok(())
}

/// Трек целиком (ISRC, участники): загружается один раз.
pub async fn ensure_track(db: &PgPool, deezer: &Deezer, track_id: Uuid) -> Result<(), AppError> {
    let row = sqlx::query!(
        "SELECT deezer_id, fetched_at FROM tracks WHERE id = $1",
        track_id
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    if let (Some(deezer_id), None) = (row.deezer_id, row.fetched_at)
        && let Err(e) = load_track(db, deezer, deezer_id).await
    {
        tracing::warn!(%track_id, error = %e, "could not load the track from Deezer");
    }
    Ok(())
}

pub async fn resolve_artist(
    db: &PgPool,
    deezer: &Deezer,
    deezer_id: i64,
) -> Result<Uuid, CatalogError> {
    let found = sqlx::query_scalar!(
        "SELECT id FROM artists WHERE deezer_id = $1 AND fetched_at IS NOT NULL",
        deezer_id
    )
    .fetch_optional(db)
    .await?;
    if let Some(id) = found {
        return Ok(id);
    }
    let artist = deezer.artist(deezer_id).await?;
    let mut conn = db.acquire().await?;
    Ok(upsert_artist(&mut conn, &artist).await?)
}

pub async fn resolve_album(
    db: &PgPool,
    deezer: &Deezer,
    deezer_id: i64,
) -> Result<Uuid, CatalogError> {
    let found = sqlx::query_scalar!(
        "SELECT id FROM albums WHERE deezer_id = $1 AND fetched_at IS NOT NULL",
        deezer_id
    )
    .fetch_optional(db)
    .await?;
    match found {
        Some(id) => Ok(id),
        None => load_album(db, deezer, deezer_id).await,
    }
}

pub async fn resolve_track(
    db: &PgPool,
    deezer: &Deezer,
    deezer_id: i64,
) -> Result<Uuid, CatalogError> {
    let found = sqlx::query_scalar!(
        "SELECT id FROM tracks WHERE deezer_id = $1 AND fetched_at IS NOT NULL",
        deezer_id
    )
    .fetch_optional(db)
    .await?;
    match found {
        Some(id) => Ok(id),
        None => load_track(db, deezer, deezer_id).await,
    }
}

// ───────────── Поиск ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Types {
    pub track: bool,
    pub album: bool,
    pub artist: bool,
}

impl Types {
    pub const ALL: Self = Self {
        track: true,
        album: true,
        artist: true,
    };
}

#[derive(Debug, Default)]
pub struct Hits {
    pub tracks: Vec<Uuid>,
    pub albums: Vec<Uuid>,
    pub artists: Vec<Uuid>,
}

fn take<T>(res: Option<Result<Vec<T>, DeezerError>>, errors: &mut Vec<DeezerError>) -> Vec<T> {
    match res {
        Some(Ok(v)) => v,
        Some(Err(e)) => {
            errors.push(e);
            Vec::new()
        }
        None => Vec::new(),
    }
}

fn push_unique(list: &mut Vec<Uuid>, id: Uuid) {
    if !list.contains(&id) {
        list.push(id);
    }
}

/// Поиск по каталогу Deezer как есть — для сопоставления (плейлист по описанию, ссылки): найденное
/// сохраняется облегчёнными строками и отдаётся нашими id. Если не ответил один из видов поиска,
/// остальные всё равно показываются. Поиск для людей — [`find::catalog`].
pub async fn search(
    db: &PgPool,
    deezer: &Deezer,
    q: &str,
    types: Types,
    limit: u32,
) -> Result<Hits, CatalogError> {
    let tracks = async {
        match types.track {
            true => Some(deezer.search_tracks(q, limit).await),
            false => None,
        }
    };
    let albums = async {
        match types.album {
            true => Some(deezer.search_albums(q, limit).await),
            false => None,
        }
    };
    let artists = async {
        match types.artist {
            true => Some(deezer.search_artists(q, limit).await),
            false => None,
        }
    };
    let (tracks, albums, artists) = tokio::join!(tracks, albums, artists);
    let mut errors = Vec::new();
    let tracks = take(tracks, &mut errors);
    let albums = take(albums, &mut errors);
    let artists = take(artists, &mut errors);
    let asked = usize::from(types.track) + usize::from(types.album) + usize::from(types.artist);
    if errors.len() == asked
        && let Some(e) = errors.into_iter().next()
    {
        return Err(e.into());
    }

    let mut hits = Hits::default();
    let mut tx = db.begin().await?;
    for t in &tracks {
        let id = save_listed_track(&mut tx, t, Isrc::Known(t.isrc.as_deref()), None).await?;
        push_unique(&mut hits.tracks, id);
    }
    for a in &albums {
        let Some(id) = save_album_detail(&mut tx, a, None).await? else {
            continue;
        };
        push_unique(&mut hits.albums, id);
    }
    for a in &artists {
        let id = upsert_artist(&mut tx, a).await?;
        push_unique(&mut hits.artists, id);
    }
    tx.commit().await?;
    Ok(hits)
}

// ───────────── Дискография подписок ─────────────

/// Концертная запись — по названию релиза: отдельного типа у Deezer нет.
pub fn is_live(title: &str) -> bool {
    let t = title.to_lowercase();
    [
        "(live",
        "[live",
        "- live",
        "live at ",
        "live in ",
        "live from ",
        "unplugged",
        "концерт",
    ]
    .iter()
    .any(|marker| t.contains(marker))
        || t.ends_with(" live")
}

/// Подходит ли релиз под политику подписки. `own` — главный артист альбома и есть артист подписки.
pub fn wanted(policy: &DownloadPolicy, record_type: RecordType, title: &str, own: bool) -> bool {
    let by_type = match record_type {
        RecordType::Album => policy.albums,
        RecordType::Ep => policy.eps,
        RecordType::Single => policy.singles,
        RecordType::Compile => policy.compilations,
    };
    by_type && (policy.live || !is_live(title)) && (own || policy.appears_on)
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscographyReport {
    pub releases: usize,
    pub matched: usize,
    pub albums_loaded: usize,
    pub queued: usize,
    pub already: usize,
    pub skipped: usize,
}

/// Сверка релизов артиста из подписок: свежий список релизов, загрузка треков подходящих альбомов
/// и постановка их записей на скачивание в полосу «Дискографии». Скачанное и уже стоящее в очереди
/// не трогается, поэтому повторный запуск безопасен.
pub async fn sync_discography(
    db: &PgPool,
    deezer: &Deezer,
    artist_id: Uuid,
) -> Result<DiscographyReport, CatalogError> {
    refresh_artist(
        db,
        deezer,
        artist_id,
        ArtistParts {
            releases: true,
            ..Default::default()
        },
    )
    .await?;
    let followers = sqlx::query!(
        "SELECT user_id, download_policy FROM followed_artists WHERE artist_id = $1 ORDER BY created_at",
        artist_id
    )
    .fetch_all(db)
    .await?;
    let mut report = DiscographyReport::default();
    let Some(requested_by) = followers.first().map(|f| f.user_id) else {
        return Ok(report);
    };
    // Подписчиков может быть несколько: качается всё, что нужно хоть кому-то
    let policies: Vec<DownloadPolicy> = followers
        .iter()
        .map(|f| serde_json::from_value(f.download_policy.clone()).unwrap_or_default())
        .collect();
    let policy = policies
        .iter()
        .fold(None::<DownloadPolicy>, |acc, p| {
            Some(match acc {
                None => *p,
                Some(a) => DownloadPolicy {
                    albums: a.albums || p.albums,
                    eps: a.eps || p.eps,
                    singles: a.singles || p.singles,
                    compilations: a.compilations || p.compilations,
                    live: a.live || p.live,
                    appears_on: a.appears_on || p.appears_on,
                },
            })
        })
        .unwrap_or_default();

    let releases = sqlx::query!(
        "SELECT al.id, al.deezer_id, al.title, al.record_type, al.fetched_at, al.release_date
         FROM artist_releases r JOIN albums al ON al.id = r.album_id
         WHERE r.artist_id = $1
         ORDER BY al.release_date DESC NULLS LAST, al.id",
        artist_id
    )
    .fetch_all(db)
    .await?;
    report.releases = releases.len();
    let today = Utc::now().date_naive();
    let mut seen_recordings = HashSet::new();
    for release in releases {
        let record_type = release.record_type.parse().unwrap_or(RecordType::Album);
        // Тип и «концертность» видны сразу; главного артиста надёжно знает только полный альбом
        if !wanted(&policy, record_type, &release.title, true) {
            continue;
        }
        if release.fetched_at.is_none()
            && let Some(deezer_id) = release.deezer_id
        {
            load_album(db, deezer, deezer_id).await?;
            report.albums_loaded += 1;
        }
        let main_artist =
            sqlx::query_scalar!("SELECT artist_id FROM albums WHERE id = $1", release.id)
                .fetch_one(db)
                .await?;
        if !wanted(
            &policy,
            record_type,
            &release.title,
            main_artist == artist_id,
        ) {
            continue;
        }
        report.matched += 1;
        // Будущий релиз: треков в источниках ещё нет, дождёмся даты выхода
        if release.release_date.is_some_and(|d| d > today) {
            continue;
        }
        let tracks = sqlx::query!(
            "SELECT id, recording_id FROM tracks WHERE album_id = $1 ORDER BY disc_number, track_position NULLS LAST, id",
            release.id
        )
        .fetch_all(db)
        .await?;
        for track in tracks {
            // Сингл и альбом с одним ISRC — одна запись: ставим её один раз
            if let Some(rec) = track.recording_id
                && !seen_recordings.insert(rec)
            {
                continue;
            }
            let res = acquire::enqueue(
                db,
                acquire::Request::new(track.id, AcquireReason::Subscription, lanes::DISCOGRAPHY)
                    .by(requested_by),
            )
            .await?;
            match res {
                Enqueue::Queued { .. } => report.queued += 1,
                Enqueue::Active { .. } | Enqueue::Ready => report.already += 1,
                Enqueue::Skipped(_) => report.skipped += 1,
            }
        }
    }
    sqlx::query!(
        "UPDATE artists SET discography_synced_at = now() WHERE id = $1",
        artist_id
    )
    .execute(db)
    .await?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isrc_is_normalized() {
        assert_eq!(
            normalize_isrc("usl-zj-21-56379").as_deref(),
            Some("USLZJ2156379")
        );
        assert_eq!(
            normalize_isrc("QZ5FN2194847").as_deref(),
            Some("QZ5FN2194847")
        );
        assert_eq!(normalize_isrc("short"), None);
    }

    #[test]
    fn live_releases() {
        assert!(is_live("Live at Wembley"));
        assert!(is_live("Demons (Live)"));
        assert!(is_live("Концерт в Москве"));
        assert!(!is_live("Alive"));
        assert!(!is_live("Delivery"));
        assert!(!is_live("Modern Sadness"));
    }

    #[test]
    fn policy_filters_releases() {
        let p = DownloadPolicy::default();
        assert!(wanted(&p, RecordType::Album, "Modern Sadness", true));
        assert!(wanted(&p, RecordType::Single, "Devil", true));
        assert!(!wanted(&p, RecordType::Compile, "Best Of", true));
        assert!(!wanted(&p, RecordType::Album, "Live at Home", true));
        assert!(
            !wanted(&p, RecordType::Ep, "SUPERNOVA", false),
            "участие выключено"
        );
        let all = DownloadPolicy {
            compilations: true,
            live: true,
            appears_on: true,
            ..p
        };
        assert!(wanted(&all, RecordType::Compile, "Live at Home", false));
    }
}
