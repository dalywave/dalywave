//! Сборка ответов каталога из базы. Все обработчики сначала выбирают id в нужном порядке, а потом
//! получают готовые DTO отсюда — так трек везде выглядит одинаково, с лайком и состоянием аудио.

use std::collections::HashMap;

use dalywave_protocol::catalog::{
    AlbumRef, AlbumSummary, ArtistRef, ArtistRole, ArtistSummary, AudioStatus, ImageSet,
    RecordType, Track, TrackArtist,
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::sha256;

/// Вид картинки в адресе `/api/images/{kind}/…`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Albums,
    Artists,
}

impl ImageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Albums => "albums",
            Self::Artists => "artists",
        }
    }
}

/// Адреса картинки на нашем сервере. `v` — отпечаток исходного адреса: новая обложка у Deezer даёт
/// новый адрес, и навсегда закешированная старая копия не мешает.
pub fn image_set(
    kind: ImageKind,
    id: Uuid,
    key: Option<&str>,
    source: Option<&str>,
) -> Option<ImageSet> {
    let origin = source.or(key)?;
    let v = hex::encode(&sha256(origin)[..4]);
    let url = |size: u32| format!("/api/images/{}/{id}/{size}.jpg?v={v}", kind.as_str());
    Some(ImageSet {
        small: url(250),
        medium: url(500),
        large: url(1000),
    })
}

pub fn palette(value: Option<Value>) -> Option<Vec<String>> {
    value.and_then(|v| serde_json::from_value(v).ok())
}

struct TrackRow {
    id: Uuid,
    deezer_id: Option<i64>,
    title: String,
    title_version: String,
    duration_ms: i32,
    explicit: bool,
    disc_number: i16,
    track_position: Option<i16>,
    recording_id: Option<Uuid>,
    isrc: Option<String>,
    audio_status: Option<String>,
    album_id: Option<Uuid>,
    album_title: Option<String>,
    album_cover_key: Option<String>,
    album_cover_source: Option<String>,
    liked: bool,
    artist_ids: Vec<Uuid>,
    artist_names: Vec<String>,
    artist_roles: Vec<String>,
}

/// Треки по id в том же порядке; неизвестные id пропускаются.
pub async fn tracks(db: &PgPool, ids: &[Uuid], user_id: Uuid) -> Result<Vec<Track>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as!(
        TrackRow,
        r#"SELECT t.id, t.deezer_id, t.title, t.title_version, t.duration_ms, t.explicit, t.disc_number,
                  t.track_position, t.recording_id, t.isrc, r.audio_status AS "audio_status?",
                  al.id AS "album_id?", al.title AS "album_title?", al.cover_key AS album_cover_key,
                  al.cover_source AS album_cover_source,
                  EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = $2 AND l.track_id = t.id) AS "liked!",
                  ARRAY(SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id
                        ORDER BY ta.position, ta.artist_id) AS "artist_ids!",
                  ARRAY(SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                        WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id) AS "artist_names!",
                  ARRAY(SELECT ta.role FROM track_artists ta WHERE ta.track_id = t.id
                        ORDER BY ta.position, ta.artist_id) AS "artist_roles!"
           FROM unnest($1::uuid[]) WITH ORDINALITY AS ids(id, ord)
           JOIN tracks t ON t.id = ids.id
           LEFT JOIN recordings r ON r.id = t.recording_id
           LEFT JOIN albums al ON al.id = t.album_id
           ORDER BY ids.ord"#,
        ids,
        user_id,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let artists = r
                .artist_ids
                .iter()
                .zip(&r.artist_names)
                .zip(&r.artist_roles)
                .map(|((id, name), role)| TrackArtist {
                    id: *id,
                    name: name.clone(),
                    role: role.parse().unwrap_or(ArtistRole::Main),
                })
                .collect();
            let album = r.album_id.zip(r.album_title).map(|(id, title)| AlbumRef {
                image: image_set(
                    ImageKind::Albums,
                    id,
                    r.album_cover_key.as_deref(),
                    r.album_cover_source.as_deref(),
                ),
                id,
                title,
            });
            Track {
                id: r.id,
                deezer_id: r.deezer_id,
                title: r.title,
                version: r.title_version,
                duration_ms: r.duration_ms,
                explicit: r.explicit,
                disc_number: r.disc_number,
                track_position: r.track_position,
                artists,
                album,
                recording_id: r.recording_id,
                isrc: r.isrc,
                audio_status: r
                    .audio_status
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(AudioStatus::None),
                liked: r.liked,
            }
        })
        .collect())
}

pub async fn track(db: &PgPool, id: Uuid, user_id: Uuid) -> Result<Option<Track>, sqlx::Error> {
    Ok(tracks(db, &[id], user_id).await?.into_iter().next())
}

/// Альбомы по id в том же порядке.
pub async fn albums(
    db: &PgPool,
    ids: &[Uuid],
    user_id: Uuid,
) -> Result<Vec<AlbumSummary>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query!(
        r#"SELECT al.id, al.deezer_id, al.title, al.record_type, al.release_date, al.explicit, al.tracks_count,
                  al.duration_ms, al.cover_key, al.cover_source, al.cover_palette,
                  a.id AS artist_id, a.name AS artist_name,
                  EXISTS (SELECT 1 FROM saved_albums s WHERE s.user_id = $2 AND s.album_id = al.id) AS "saved!"
           FROM unnest($1::uuid[]) WITH ORDINALITY AS ids(id, ord)
           JOIN albums al ON al.id = ids.id
           JOIN artists a ON a.id = al.artist_id
           ORDER BY ids.ord"#,
        ids,
        user_id,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AlbumSummary {
            image: image_set(
                ImageKind::Albums,
                r.id,
                r.cover_key.as_deref(),
                r.cover_source.as_deref(),
            ),
            palette: palette(r.cover_palette),
            id: r.id,
            deezer_id: r.deezer_id,
            title: r.title,
            artist: ArtistRef {
                id: r.artist_id,
                name: r.artist_name,
            },
            record_type: r.record_type.parse().unwrap_or(RecordType::Album),
            release_date: r.release_date,
            explicit: r.explicit,
            tracks_count: r.tracks_count,
            duration_ms: r.duration_ms,
            saved: r.saved,
        })
        .collect())
}

pub async fn album(
    db: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<AlbumSummary>, sqlx::Error> {
    Ok(albums(db, &[id], user_id).await?.into_iter().next())
}

/// Артисты по id в том же порядке.
pub async fn artists(
    db: &PgPool,
    ids: &[Uuid],
    user_id: Uuid,
) -> Result<Vec<ArtistSummary>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query!(
        r#"SELECT a.id, a.deezer_id, a.name, a.picture_key, a.picture_source, a.fans, a.albums_count,
                  EXISTS (SELECT 1 FROM followed_artists f WHERE f.user_id = $2 AND f.artist_id = a.id) AS "followed!"
           FROM unnest($1::uuid[]) WITH ORDINALITY AS ids(id, ord)
           JOIN artists a ON a.id = ids.id
           ORDER BY ids.ord"#,
        ids,
        user_id,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ArtistSummary {
            image: image_set(
                ImageKind::Artists,
                r.id,
                r.picture_key.as_deref(),
                r.picture_source.as_deref(),
            ),
            id: r.id,
            deezer_id: r.deezer_id,
            name: r.name,
            fans: r.fans,
            albums_count: r.albums_count,
            followed: r.followed,
        })
        .collect())
}

pub async fn artist(
    db: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<ArtistSummary>, sqlx::Error> {
    Ok(artists(db, &[id], user_id).await?.into_iter().next())
}

/// Сколько треков дискографии артистов на сервере и сколько всего: (готово, всего).
/// Сингл и альбом с одним ISRC — одна запись. Список релизов Deezer отдаёт без числа треков, поэтому
/// незагруженный релиз считается хотя бы за один трек; у подписок альбомы загружаются, и счёт точный.
pub async fn discography_progress(
    db: &PgPool,
    artist_ids: &[Uuid],
) -> Result<HashMap<Uuid, (i64, i64)>, sqlx::Error> {
    if artist_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query!(
        r#"SELECT r.artist_id AS "artist_id!",
                  count(DISTINCT t.recording_id) FILTER (WHERE rec.audio_status = 'ready') AS "ready!",
                  count(DISTINCT COALESCE(t.recording_id, t.id)) FILTER (WHERE al.fetched_at IS NOT NULL) AS "known!",
                  (SELECT COALESCE(sum(GREATEST(COALESCE(a2.tracks_count, 0), 1)), 0)
                   FROM artist_releases r2 JOIN albums a2 ON a2.id = r2.album_id
                   WHERE r2.artist_id = r.artist_id AND a2.fetched_at IS NULL)::bigint AS "pending!"
           FROM artist_releases r
           JOIN albums al ON al.id = r.album_id
           LEFT JOIN tracks t ON t.album_id = al.id
           LEFT JOIN recordings rec ON rec.id = t.recording_id
           WHERE r.artist_id = ANY($1)
           GROUP BY r.artist_id"#,
        artist_ids,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.artist_id, (r.ready, r.known + r.pending)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_urls_are_versioned() {
        let id = Uuid::nil();
        assert!(image_set(ImageKind::Albums, id, None, None).is_none());
        let a = image_set(
            ImageKind::Albums,
            id,
            None,
            Some("https://cdn/x/1000x1000.jpg"),
        )
        .unwrap();
        assert!(
            a.medium
                .starts_with(&format!("/api/images/albums/{id}/500.jpg?v="))
        );
        assert!(a.small.contains("/250.jpg?v=") && a.large.contains("/1000.jpg?v="));
        // Копия той же картинки — тот же адрес
        let mirrored = image_set(
            ImageKind::Albums,
            id,
            Some("images/albums/x"),
            Some("https://cdn/x/1000x1000.jpg"),
        )
        .unwrap();
        assert_eq!(a, mirrored);
        let other = image_set(
            ImageKind::Albums,
            id,
            None,
            Some("https://cdn/y/1000x1000.jpg"),
        )
        .unwrap();
        assert_ne!(a.medium, other.medium);
    }
}
