//! Файл своей музыки → каталог: артисты, альбом, трек, запись и файл записи.
//!
//! Чей это трек, решается так: ISRC уже есть в каталоге → тот трек; включён Deezer → сопоставление с ним
//! (ISRC, затем название, артист и длительность) — у трека будут обложка в высоком качестве, похожие и
//! разведчик; иначе — свои записи каталога из тегов (`source = 'local'`): артисты по имени, альбом по
//! исполнителю альбома (albumartist; сборники — «Various Artists»), трек по альбому и номеру. Второй файл
//! того же трека (FLAC и MP3 одной песни) становится вторым файлом записи, лучший — основным.

use std::path::Path;

use chrono::NaiveDate;
use dalywave_protocol::catalog::AudioStatus;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::tags::{FileInfo, Tags};
use crate::{
    acquire, catalog,
    deezer::Deezer,
    import::matching::{self, ACCEPT, Wanted},
    jobs as queue,
};

/// Имя исполнителя сборников.
pub const VARIOUS_ARTISTS: &str = "Various Artists";
/// Сопоставление с Deezer без ISRC — только при такой уверенности: своя запись лучше чужой.
const DEEZER_ACCEPT: f32 = 0.86;
/// Расхождение длительности с треком Deezer по ISRC, при котором это уже другая версия.
const ISRC_MAX_DRIFT_MS: i32 = 10_000;

/// Название, исполнители и альбом трека: из тегов, а чего нет — из пути `…/Исполнитель/Альбом/01 Название.flac`.
#[derive(Debug, Clone, PartialEq)]
pub struct Described {
    pub title: String,
    pub artists: Vec<String>,
    pub featured: Vec<String>,
    pub album: String,
    pub album_artist: String,
    pub compilation: bool,
    pub disc: i16,
    pub position: Option<i16>,
    pub release_date: Option<NaiveDate>,
    pub genres: Vec<String>,
}

/// «01 - Название», «01. Название», «1-03 Название» → «Название».
fn title_from_stem(stem: &str) -> String {
    let s = stem.trim();
    let digits = s
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit() || *c == '-')
        .last()
        .map(|(i, _)| i + 1)
        .unwrap_or(0);
    let rest = s[digits..]
        .trim_start_matches([' ', '.', '-', '_', ')'])
        .trim();
    if digits > 0 && !rest.is_empty() {
        rest.to_string()
    } else {
        s.to_string()
    }
}

/// Имя папки, если это не сама папка музыки.
fn folder_name(dir: Option<&Path>, root: &Path) -> Option<String> {
    let dir = dir?;
    if dir == root || !dir.starts_with(root) {
        return None;
    }
    dir.file_name()?
        .to_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn describe(tags: &Tags, path: &Path, root: &Path) -> Described {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let parent = path.parent();
    let grandparent = parent.and_then(Path::parent);
    let title = tags.title.clone().unwrap_or_else(|| title_from_stem(stem));
    let mut artists = tags.artists.clone();
    if artists.is_empty() {
        artists = match &tags.album_artist {
            Some(a) => vec![a.clone()],
            None => vec![
                folder_name(grandparent, root).unwrap_or_else(|| "Неизвестный исполнитель".into()),
            ],
        };
    }
    let compilation = tags.compilation
        || tags.album_artist.as_deref().is_some_and(|a| {
            a.eq_ignore_ascii_case(VARIOUS_ARTISTS) || a.eq_ignore_ascii_case("VA")
        });
    let album_artist = match (&tags.album_artist, compilation) {
        (_, true) => VARIOUS_ARTISTS.to_string(),
        (Some(a), false) => a.clone(),
        (None, false) => artists[0].clone(),
    };
    let album = tags
        .album
        .clone()
        .or_else(|| folder_name(parent, root))
        .unwrap_or_else(|| title.clone());
    let release_date = tags
        .date
        .or_else(|| tags.year.and_then(|y| NaiveDate::from_ymd_opt(y, 1, 1)));
    Described {
        title,
        artists,
        featured: tags.featured.clone(),
        album,
        album_artist,
        compilation,
        disc: tags.disc.and_then(|d| i16::try_from(d).ok()).unwrap_or(1),
        position: tags.track.and_then(|t| i16::try_from(t).ok()),
        release_date,
        genres: tags.genres.clone(),
    }
}

/// Сопоставление с Deezer (только когда он включён). `None` — не нашлось уверенно.
pub async fn match_deezer(
    db: &PgPool,
    deezer: &Deezer,
    d: &Described,
    info: &FileInfo,
) -> Result<Option<Uuid>, crate::deezer::DeezerError> {
    if !deezer.is_enabled() {
        return Ok(None);
    }
    if let Some(isrc) = &info.tags.isrc {
        match deezer.track_by_isrc(isrc).await {
            Ok(t)
                if (t.duration.saturating_mul(1000) - info.duration_ms).abs()
                    <= ISRC_MAX_DRIFT_MS =>
            {
                let id = catalog::save_track(db, &t).await.map_err(|e| {
                    crate::deezer::DeezerError::Decode(format!("сохранение трека: {e}"))
                })?;
                return Ok(Some(id));
            }
            Ok(_) | Err(crate::deezer::DeezerError::NotFound) => {}
            Err(e) => return Err(e),
        }
    }
    let w = Wanted {
        artist: d.artists.join(", "),
        title: d.title.clone(),
        album: Some(d.album.clone()),
        duration_ms: Some(info.duration_ms),
        isrc: info.tags.isrc.clone(),
    };
    let found = matching::find_track(deezer, &w).await?;
    match found.best {
        Some((t, score)) if score >= DEEZER_ACCEPT.max(ACCEPT) => {
            let ids = catalog::save_listed(db, std::slice::from_ref(&t))
                .await
                .map_err(|e| {
                    crate::deezer::DeezerError::Decode(format!("сохранение трека: {e}"))
                })?;
            Ok(ids.first().copied())
        }
        _ => Ok(None),
    }
}

/// Свой артист по имени (без учёта регистра).
async fn local_artist(conn: &mut PgConnection, name: &str) -> Result<Uuid, sqlx::Error> {
    let name = name.trim();
    if let Some(id) = sqlx::query_scalar!(
        "SELECT id FROM artists WHERE source = 'local' AND lower(name) = lower($1) ORDER BY created_at LIMIT 1",
        name
    )
    .fetch_optional(&mut *conn)
    .await?
    {
        return Ok(id);
    }
    sqlx::query_scalar!(
        "INSERT INTO artists (name, source, fetched_at) VALUES ($1, 'local', now()) RETURNING id",
        name
    )
    .fetch_one(conn)
    .await
}

/// Свой альбом: по исполнителю альбома и названию. Новый — с типом, датой и жанрами из тегов.
async fn local_album(
    conn: &mut PgConnection,
    d: &Described,
    artist_id: Uuid,
    total: Option<u32>,
) -> Result<(Uuid, bool), sqlx::Error> {
    if let Some(id) = sqlx::query_scalar!(
        "SELECT id FROM albums WHERE source = 'local' AND artist_id = $1 AND lower(title) = lower($2)
         ORDER BY created_at LIMIT 1",
        artist_id,
        d.album.trim(),
    )
    .fetch_optional(&mut *conn)
    .await?
    {
        // Даты и жанров могло не быть у первого файла
        sqlx::query!(
            "UPDATE albums SET release_date = COALESCE(release_date, $2),
                    genres = CASE WHEN genres = '{}' THEN $3 ELSE genres END,
                    tracks_count = GREATEST(COALESCE(tracks_count, 0), COALESCE($4, 0))
             WHERE id = $1 AND (release_date IS NULL OR genres = '{}' OR COALESCE(tracks_count, 0) < COALESCE($4, 0))",
            id,
            d.release_date,
            &d.genres,
            total.and_then(|t| i32::try_from(t).ok()),
        )
        .execute(&mut *conn)
        .await?;
        return Ok((id, false));
    }
    let record_type = if d.compilation { "compile" } else { "album" };
    let id = sqlx::query_scalar!(
        "INSERT INTO albums (title, artist_id, record_type, release_date, genres, tracks_count, source, fetched_at)
         VALUES ($1, $2, $3, $4, $5, $6, 'local', now())
         RETURNING id",
        d.album.trim(),
        artist_id,
        record_type,
        d.release_date,
        &d.genres,
        total.and_then(|t| i32::try_from(t).ok()),
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok((id, true))
}

/// Участники своего трека и дискография артистов.
async fn set_artists(
    conn: &mut PgConnection,
    track_id: Uuid,
    album_id: Uuid,
    album_artist: Uuid,
    d: &Described,
) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM track_artists WHERE track_id = $1", track_id)
        .execute(&mut *conn)
        .await?;
    let mut position: i16 = 0;
    for (names, role) in [(&d.artists, "main"), (&d.featured, "featured")] {
        for name in names {
            let artist = local_artist(conn, name).await?;
            sqlx::query!(
                "INSERT INTO track_artists (track_id, artist_id, role, position) VALUES ($1, $2, $3, $4)
                 ON CONFLICT DO NOTHING",
                track_id,
                artist,
                role,
                position,
            )
            .execute(&mut *conn)
            .await?;
            position += 1;
            if role == "main" {
                sqlx::query!(
                    "INSERT INTO artist_releases (artist_id, album_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                    artist,
                    album_id
                )
                .execute(&mut *conn)
                .await?;
            }
        }
    }
    sqlx::query!(
        "INSERT INTO artist_releases (artist_id, album_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        album_artist,
        album_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Чей трек из своих: тот же альбом, диск, номер и название — это тот же трек в другом формате.
async fn same_local_track(
    conn: &mut PgConnection,
    album_id: Uuid,
    d: &Described,
) -> Result<Option<(Uuid, Uuid)>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, recording_id AS "recording_id!" FROM tracks
           WHERE source = 'local' AND album_id = $1 AND disc_number = $2
             AND track_position IS NOT DISTINCT FROM $3 AND lower(title) = lower($4)
             AND recording_id IS NOT NULL
           LIMIT 1"#,
        album_id,
        d.disc,
        d.position,
        d.title.trim(),
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|r| (r.id, r.recording_id)))
}

/// Новая запись: ISRC файла, если он ещё ничей.
async fn new_recording(
    conn: &mut PgConnection,
    isrc: Option<&str>,
    duration_ms: i32,
) -> Result<Uuid, sqlx::Error> {
    if let Some(code) = isrc
        && let Some(id) = sqlx::query_scalar!(
            "INSERT INTO recordings (isrc, duration_ms, audio_status) VALUES ($1, $2, 'ready')
             ON CONFLICT (isrc) DO NOTHING RETURNING id",
            code,
            duration_ms
        )
        .fetch_optional(&mut *conn)
        .await?
    {
        return Ok(id);
    }
    sqlx::query_scalar!(
        "INSERT INTO recordings (duration_ms, audio_status) VALUES ($1, 'ready') RETURNING id",
        duration_ms
    )
    .fetch_one(conn)
    .await
}

/// Где оказался файл.
#[derive(Debug, Clone, Copy)]
pub struct Placed {
    pub track_id: Uuid,
    pub recording_id: Uuid,
    pub audio_file_id: Uuid,
    /// Свой альбом, у которого ещё нет обложки: её надо поставить из файла или папки.
    pub needs_cover: Option<Uuid>,
}

/// Трек каталога, уже известный по ISRC (скачанный раньше, найденный в Deezer).
async fn track_by_isrc(conn: &mut PgConnection, isrc: &str) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT t.id FROM tracks t WHERE t.isrc = $1 AND t.recording_id IS NOT NULL
         ORDER BY (t.source = 'local'), t.rank DESC NULLS LAST LIMIT 1",
        isrc
    )
    .fetch_optional(conn)
    .await
}

/// Свои записи каталога для трека из тегов: создать или найти тот же трек.
async fn local_track(
    conn: &mut PgConnection,
    d: &Described,
    info: &FileInfo,
) -> Result<(Uuid, Uuid, Option<Uuid>), sqlx::Error> {
    let album_artist = local_artist(conn, &d.album_artist).await?;
    let (album_id, _) = local_album(conn, d, album_artist, info.tags.track_total).await?;
    let has_cover = sqlx::query_scalar!(
        r#"SELECT cover_key IS NOT NULL AS "has!" FROM albums WHERE id = $1"#,
        album_id
    )
    .fetch_one(&mut *conn)
    .await?;
    let needs_cover = (!has_cover).then_some(album_id);
    if let Some((track_id, recording_id)) = same_local_track(conn, album_id, d).await? {
        return Ok((track_id, recording_id, needs_cover));
    }
    let recording_id = new_recording(conn, info.tags.isrc.as_deref(), info.duration_ms).await?;
    let track_id = sqlx::query_scalar!(
        "INSERT INTO tracks (isrc, recording_id, title, album_id, disc_number, track_position, duration_ms,
                             source, fetched_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'local', now())
         RETURNING id",
        info.tags.isrc,
        recording_id,
        d.title.trim(),
        album_id,
        d.disc,
        d.position,
        info.duration_ms,
    )
    .fetch_one(&mut *conn)
    .await?;
    set_artists(conn, track_id, album_id, album_artist, d).await?;
    Ok((track_id, recording_id, needs_cover))
}

/// Новые теги своего трека: название, альбом, исполнители, номер. Трек (и лайки) остаётся тем же.
pub async fn retag(
    conn: &mut PgConnection,
    track_id: Uuid,
    d: &Described,
    info: &FileInfo,
) -> Result<Option<Uuid>, sqlx::Error> {
    let local = sqlx::query_scalar!(
        r#"SELECT source = 'local' AS "local!" FROM tracks WHERE id = $1"#,
        track_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    if local != Some(true) {
        return Ok(None);
    }
    let album_artist = local_artist(conn, &d.album_artist).await?;
    let (album_id, _) = local_album(conn, d, album_artist, info.tags.track_total).await?;
    sqlx::query!(
        "UPDATE tracks SET title = $2, album_id = $3, disc_number = $4, track_position = $5, duration_ms = $6,
                isrc = COALESCE($7, isrc), updated_at = now()
         WHERE id = $1",
        track_id,
        d.title.trim(),
        album_id,
        d.disc,
        d.position,
        info.duration_ms,
        info.tags.isrc,
    )
    .execute(&mut *conn)
    .await?;
    set_artists(conn, track_id, album_id, album_artist, d).await?;
    let has_cover = sqlx::query_scalar!(
        r#"SELECT cover_key IS NOT NULL AS "has!" FROM albums WHERE id = $1"#,
        album_id
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok((!has_cover).then_some(album_id))
}

/// Сравнение файлов записи: без потерь лучше, дальше — битрейт с поправкой на кодек.
fn quality(codec: &str, lossless: bool, kbps: Option<i32>) -> (bool, i64) {
    let factor = match codec {
        "opus" | "flac" | "alac" | "pcm" => 1.0,
        "vorbis" => 0.85,
        "aac" => 0.75,
        _ => 0.6,
    };
    (
        lossless,
        (f64::from(kbps.unwrap_or(0)) * factor).round() as i64,
    )
}

/// Громкость из ReplayGain тегов, пока свой анализ не посчитан: опорный уровень ReplayGain 2 — −18 LUFS.
pub fn tag_loudness(tags: &Tags) -> (Option<f32>, Option<f32>) {
    let lufs = tags.replay_gain_db.map(|g| -18.0 - g);
    let peak = tags
        .replay_peak
        .map(|p| 20.0 * p.log10())
        .filter(|p| p.is_finite());
    (lufs, peak)
}

/// Новый файл записи. Основной — если основного нет или этот лучше.
#[allow(clippy::too_many_arguments)]
async fn add_file(
    conn: &mut PgConnection,
    recording_id: Uuid,
    key: &str,
    info: &FileInfo,
    size: i64,
    fingerprint: &str,
    name: &str,
) -> Result<Uuid, sqlx::Error> {
    let (lufs, peak) = tag_loudness(&info.tags);
    let file_id = sqlx::query_scalar!(
        "INSERT INTO audio_files (recording_id, storage_key, codec, container, lossless, bitrate_kbps, sample_rate,
                                  channels, bit_depth, duration_ms, size_bytes, sha256, source, source_title,
                                  loudness_lufs, true_peak_db, verified, acquired_by, direct)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'local', $13, $14, $15, true, 'library', $16)
         RETURNING id",
        recording_id,
        key,
        info.codec,
        info.container,
        info.lossless,
        info.bitrate_kbps,
        info.sample_rate,
        info.channels,
        info.bit_depth,
        info.duration_ms,
        size,
        fingerprint,
        name,
        lufs,
        peak,
        info.direct,
    )
    .fetch_one(&mut *conn)
    .await?;
    let current = sqlx::query!(
        "SELECT f.codec, f.lossless, f.bitrate_kbps FROM recordings r JOIN audio_files f ON f.id = r.primary_file_id
         WHERE r.id = $1",
        recording_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let better = current.is_none_or(|c| {
        quality(info.codec, info.lossless, info.bitrate_kbps)
            > quality(&c.codec, c.lossless, c.bitrate_kbps)
    });
    if better {
        sqlx::query!(
            "UPDATE recordings SET primary_file_id = $2 WHERE id = $1",
            recording_id,
            file_id
        )
        .execute(&mut *conn)
        .await?;
    }
    acquire::set_audio_status(&mut *conn, recording_id, AudioStatus::Ready).await?;
    // Ждущее скачивание больше не нужно: файл уже на сервере
    let waiting = sqlx::query_scalar!(
        r#"UPDATE acquisitions SET status = 'cancelled', error = 'есть в своей музыке', updated_at = now(),
                  finished_at = now()
           WHERE recording_id = $1 AND status = 'queued'
           RETURNING job_id"#,
        recording_id
    )
    .fetch_all(&mut *conn)
    .await?;
    for job_id in waiting.into_iter().flatten() {
        queue::cancel(&mut *conn, job_id).await?;
    }
    acquire::recording_changed(&mut *conn, recording_id).await?;
    Ok(file_id)
}

/// Положить новый файл в каталог. `matched` — трек Deezer, найденный до транзакции.
#[allow(clippy::too_many_arguments)]
pub async fn place(
    conn: &mut PgConnection,
    d: &Described,
    info: &FileInfo,
    matched: Option<Uuid>,
    key: &str,
    size: i64,
    fingerprint: &str,
    name: &str,
) -> Result<Placed, sqlx::Error> {
    let known = match info.tags.isrc.as_deref() {
        Some(isrc) => track_by_isrc(conn, isrc).await?,
        None => None,
    };
    let catalog_track = match known.or(matched) {
        Some(t) => sqlx::query_scalar!("SELECT recording_id FROM tracks WHERE id = $1", t)
            .fetch_optional(&mut *conn)
            .await?
            .flatten()
            .map(|r| (t, r)),
        None => None,
    };
    let (track_id, recording_id, needs_cover) = match catalog_track {
        Some((t, r)) => (t, r, None),
        None => local_track(conn, d, info).await?,
    };
    let audio_file_id = add_file(conn, recording_id, key, info, size, fingerprint, name).await?;
    Ok(Placed {
        track_id,
        recording_id,
        audio_file_id,
        needs_cover,
    })
}

/// Файл изменился на месте (перезапись тегов, другая кодировка): свойства файла записи.
pub async fn update_file(
    conn: &mut PgConnection,
    audio_file_id: Uuid,
    info: &FileInfo,
    size: i64,
    fingerprint: &str,
) -> Result<(), sqlx::Error> {
    let (lufs, peak) = tag_loudness(&info.tags);
    sqlx::query!(
        "UPDATE audio_files SET codec = $2, container = $3, lossless = $4, bitrate_kbps = $5, sample_rate = $6,
                channels = $7, bit_depth = $8, duration_ms = $9, size_bytes = $10, sha256 = $11, direct = $12,
                loudness_lufs = COALESCE($13, loudness_lufs), true_peak_db = COALESCE($14, true_peak_db)
         WHERE id = $1",
        audio_file_id,
        info.codec,
        info.container,
        info.lossless,
        info.bitrate_kbps,
        info.sample_rate,
        info.channels,
        info.bit_depth,
        info.duration_ms,
        size,
        fingerprint,
        info.direct,
        lufs,
        peak,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Файл пропал: убрать его из записи. Запись без файлов — снова без аудио; свои треки без файлов,
/// а следом альбомы и артисты без треков уходят из каталога.
pub async fn remove_file(conn: &mut PgConnection, audio_file_id: Uuid) -> Result<(), sqlx::Error> {
    let Some(recording_id) = sqlx::query_scalar!(
        "DELETE FROM audio_files WHERE id = $1 RETURNING recording_id",
        audio_file_id
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(());
    };
    let rest = sqlx::query!(
        "SELECT id, codec, lossless, bitrate_kbps FROM audio_files WHERE recording_id = $1",
        recording_id
    )
    .fetch_all(&mut *conn)
    .await?;
    match rest
        .iter()
        .max_by_key(|f| quality(&f.codec, f.lossless, f.bitrate_kbps))
    {
        Some(best) => {
            sqlx::query!(
                "UPDATE recordings SET primary_file_id = $2 WHERE id = $1 AND primary_file_id IS NULL",
                recording_id,
                best.id
            )
            .execute(&mut *conn)
            .await?;
        }
        None => {
            sqlx::query!(
                "UPDATE recordings SET audio_status = 'none', primary_file_id = NULL, status_changed_at = now()
                 WHERE id = $1",
                recording_id
            )
            .execute(&mut *conn)
            .await?;
            sqlx::query!(
                "DELETE FROM tracks WHERE recording_id = $1 AND source = 'local'",
                recording_id
            )
            .execute(&mut *conn)
            .await?;
            sqlx::query!(
                "DELETE FROM recordings r WHERE r.id = $1 AND NOT EXISTS (SELECT 1 FROM tracks WHERE recording_id = r.id)",
                recording_id
            )
            .execute(&mut *conn)
            .await?;
        }
    }
    acquire::recording_changed(&mut *conn, recording_id).await?;
    Ok(())
}

/// Свои альбомы без треков и свои артисты без треков и альбомов.
pub async fn sweep(conn: &mut PgConnection) -> Result<(u64, u64), sqlx::Error> {
    let albums = sqlx::query!(
        "DELETE FROM albums al WHERE al.source = 'local' AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = al.id)"
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    let artists = sqlx::query!(
        "DELETE FROM artists a WHERE a.source = 'local'
           AND NOT EXISTS (SELECT 1 FROM track_artists WHERE artist_id = a.id)
           AND NOT EXISTS (SELECT 1 FROM albums WHERE artist_id = a.id)"
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    Ok((albums, artists))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn titles_from_file_names() {
        assert_eq!(title_from_stem("01 - Группа крови"), "Группа крови");
        assert_eq!(title_from_stem("1-03 Song"), "Song");
        assert_eq!(title_from_stem("07. Intro"), "Intro");
        assert_eq!(title_from_stem("1979"), "1979");
        assert_eq!(title_from_stem("Song"), "Song");
    }

    #[test]
    fn description_falls_back_to_folders() {
        let root = PathBuf::from("/music");
        let path = PathBuf::from("/music/Кино/Группа крови/01 - Группа крови.flac");
        let d = describe(&Tags::default(), &path, &root);
        assert_eq!(d.title, "Группа крови");
        assert_eq!(d.artists, vec!["Кино"]);
        assert_eq!(d.album, "Группа крови");
        assert_eq!(d.album_artist, "Кино");
        assert_eq!(d.disc, 1);
        // Файл прямо в папке музыки: альбом — по названию трека
        let d = describe(&Tags::default(), &PathBuf::from("/music/track.mp3"), &root);
        assert_eq!(d.album, "track");
        assert_eq!(d.artists, vec!["Неизвестный исполнитель"]);

        let tags = Tags {
            title: Some("Song".into()),
            artists: vec!["A".into()],
            album: Some("Hits".into()),
            album_artist: Some("Various Artists".into()),
            year: Some(1999),
            track: Some(4),
            disc: Some(2),
            ..Tags::default()
        };
        let d = describe(&tags, &path, &root);
        assert!(d.compilation);
        assert_eq!(d.album_artist, VARIOUS_ARTISTS);
        assert_eq!(d.position, Some(4));
        assert_eq!(d.disc, 2);
        assert_eq!(d.release_date, NaiveDate::from_ymd_opt(1999, 1, 1));
    }

    #[test]
    fn lossless_wins() {
        assert!(quality("flac", true, Some(900)) > quality("mp3", false, Some(320)));
        assert!(quality("opus", false, Some(160)) > quality("mp3", false, Some(192)));
        let (lufs, peak) = tag_loudness(&Tags {
            replay_gain_db: Some(-6.0),
            replay_peak: Some(1.0),
            ..Tags::default()
        });
        assert_eq!(lufs, Some(-12.0));
        assert_eq!(peak, Some(0.0));
    }
}
