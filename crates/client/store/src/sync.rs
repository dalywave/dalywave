//! Применение страницы синхронизации (`/api/sync`).
//!
//! Строка применяется, только если её `rev` не меньше локального: одна и та же строка может прийти
//! повторно (полнота по id транзакции, `docs/data-model.md`), а старая копия не должна затереть новую.
//! Tombstone удаляет строку с меньшим `rev`. Плейлист уходит вместе с позициями — их tombstone'ов при
//! каскадном удалении сервер не пишет.

use dalywave_protocol::{
    settings::UserSettings,
    sync::{SyncEntity, SyncPage, SyncTombstone},
    wave::WavePreset,
};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;
use uuid::Uuid;

use crate::{Store, StoreResult, normalize};

/// Что изменила страница.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Applied {
    /// Строк записано или удалено.
    pub rows: usize,
    /// Изменилась медиатека: лайки, альбомы, подписки, плейлисты.
    pub library: bool,
    /// Изменился набор волны устройства.
    pub reserve: bool,
    /// Изменилось закреплённое этого устройства.
    pub pins: bool,
}

fn json<T: serde::Serialize>(v: &T) -> StoreResult<String> {
    Ok(serde_json::to_string(v)?)
}

fn key_str(key: &Value, field: &str) -> Option<String> {
    key.get(field).and_then(Value::as_str).map(str::to_string)
}

impl Store {
    /// Применить страницу и запомнить курсор — одной транзакцией: оборванная синхронизация повторится
    /// с прежнего курсора.
    pub fn apply(&mut self, page: &SyncPage) -> StoreResult<Applied> {
        let device = self.device_id()?;
        let tx = self.conn.transaction()?;
        let mut a = Applied::default();
        apply_rows(&tx, page, device, &mut a)?;
        for t in &page.tombstones {
            apply_tombstone(&tx, t, device, &mut a)?;
        }
        tx.execute(
            "INSERT INTO meta (key, value) VALUES ('sync.cursor', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [&page.cursor],
        )?;
        if !page.has_more {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES ('sync.completed_at', ?1)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [chrono::Utc::now().to_rfc3339()],
            )?;
        }
        tx.commit()?;
        Ok(a)
    }
}

impl Store {
    /// Настройки слушателя из последней синхронизации; не приходили — по умолчанию.
    pub fn settings(&self) -> StoreResult<UserSettings> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT settings FROM user_settings WHERE id = 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(raw
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    /// Сохранённые сочетания волны по порядку карточек.
    pub fn wave_presets(&self) -> StoreResult<Vec<WavePreset>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, icon, settings, position, created_at FROM wave_presets ORDER BY position, id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, icon, settings, position, created_at) = row?;
            let (Ok(id), Ok(created_at)) = (id.parse(), created_at.parse()) else {
                continue;
            };
            out.push(WavePreset {
                id,
                name,
                icon,
                settings: serde_json::from_str(&settings).unwrap_or_default(),
                position,
                created_at,
            });
        }
        Ok(out)
    }
}

fn apply_rows(
    tx: &Transaction,
    page: &SyncPage,
    device: Option<Uuid>,
    a: &mut Applied,
) -> StoreResult<()> {
    for r in &page.artists {
        a.rows += tx.execute(
            "INSERT INTO artists (id, rev, deezer_id, name, image, search) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, deezer_id = excluded.deezer_id,
               name = excluded.name, image = excluded.image, search = excluded.search
             WHERE excluded.rev >= artists.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.deezer_id,
                r.name,
                r.image.as_ref().map(json).transpose()?,
                normalize(&r.name)
            ],
        )?;
    }
    for r in &page.albums {
        a.rows += tx.execute(
            "INSERT INTO albums (id, rev, deezer_id, title, artist_id, record_type, release_date, label, genres,
                                 explicit, tracks_count, duration_ms, image, palette, search)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, deezer_id = excluded.deezer_id,
               title = excluded.title, artist_id = excluded.artist_id, record_type = excluded.record_type,
               release_date = excluded.release_date, label = excluded.label, genres = excluded.genres,
               explicit = excluded.explicit, tracks_count = excluded.tracks_count,
               duration_ms = excluded.duration_ms, image = excluded.image, palette = excluded.palette,
               search = excluded.search
             WHERE excluded.rev >= albums.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.deezer_id,
                r.title,
                r.artist_id.to_string(),
                r.record_type.as_str(),
                r.release_date.map(|d| d.to_string()),
                r.label,
                json(&r.genres)?,
                r.explicit,
                r.tracks_count,
                r.duration_ms,
                r.image.as_ref().map(json).transpose()?,
                r.palette.as_ref().map(json).transpose()?,
                normalize(&r.title)
            ],
        )?;
    }
    for r in &page.tracks {
        let n = tx.execute(
            "INSERT INTO tracks (id, rev, deezer_id, title, version, album_id, disc_number, track_position,
                                 duration_ms, explicit, isrc, recording_id, search)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, deezer_id = excluded.deezer_id,
               title = excluded.title, version = excluded.version, album_id = excluded.album_id,
               disc_number = excluded.disc_number, track_position = excluded.track_position,
               duration_ms = excluded.duration_ms, explicit = excluded.explicit, isrc = excluded.isrc,
               recording_id = excluded.recording_id, search = excluded.search
             WHERE excluded.rev >= tracks.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.deezer_id,
                r.title,
                r.version,
                r.album_id.map(|i| i.to_string()),
                r.disc_number,
                r.track_position,
                r.duration_ms,
                r.explicit,
                r.isrc,
                r.recording_id.map(|i| i.to_string()),
                normalize(&format!("{} {}", r.title, r.version))
            ],
        )?;
        if n > 0 {
            let id = r.id.to_string();
            tx.execute("DELETE FROM track_artists WHERE track_id = ?1", [&id])?;
            for (i, ta) in r.artists.iter().enumerate() {
                tx.execute(
                    "INSERT INTO track_artists (track_id, position, artist_id, role) VALUES (?1, ?2, ?3, ?4)",
                    params![id, i as i64, ta.artist_id.to_string(), ta.role.as_str()],
                )?;
            }
        }
        a.rows += n;
    }
    for r in &page.recordings {
        a.rows += tx.execute(
            "INSERT INTO recordings (id, rev, isrc, duration_ms, audio_status, primary_file_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, isrc = excluded.isrc,
               duration_ms = excluded.duration_ms, audio_status = excluded.audio_status,
               primary_file_id = excluded.primary_file_id
             WHERE excluded.rev >= recordings.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.isrc,
                r.duration_ms,
                r.audio_status.as_str(),
                r.primary_file_id.map(|i| i.to_string())
            ],
        )?;
    }
    for r in &page.liked_tracks {
        let n = tx.execute(
            "INSERT INTO liked_tracks (track_id, rev, liked_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (track_id) DO UPDATE SET rev = excluded.rev, liked_at = excluded.liked_at
             WHERE excluded.rev >= liked_tracks.rev",
            params![r.track_id.to_string(), r.rev, r.liked_at.to_rfc3339()],
        )?;
        a.library |= n > 0;
        a.rows += n;
    }
    for r in &page.saved_albums {
        let n = tx.execute(
            "INSERT INTO saved_albums (album_id, rev, saved_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (album_id) DO UPDATE SET rev = excluded.rev, saved_at = excluded.saved_at
             WHERE excluded.rev >= saved_albums.rev",
            params![r.album_id.to_string(), r.rev, r.saved_at.to_rfc3339()],
        )?;
        a.library |= n > 0;
        a.rows += n;
    }
    for r in &page.followed_artists {
        let n = tx.execute(
            "INSERT INTO followed_artists (artist_id, rev, policy, notify, followed_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (artist_id) DO UPDATE SET rev = excluded.rev, policy = excluded.policy,
               notify = excluded.notify, followed_at = excluded.followed_at
             WHERE excluded.rev >= followed_artists.rev",
            params![
                r.artist_id.to_string(),
                r.rev,
                json(&r.policy)?,
                r.notify,
                r.followed_at.to_rfc3339()
            ],
        )?;
        a.library |= n > 0;
        a.rows += n;
    }
    for r in &page.blocked_items {
        a.rows += tx.execute(
            "INSERT INTO blocked_items (kind, ref_id, rev, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (kind, ref_id) DO UPDATE SET rev = excluded.rev, created_at = excluded.created_at
             WHERE excluded.rev >= blocked_items.rev",
            params![
                r.kind.as_str(),
                r.ref_id.to_string(),
                r.rev,
                r.created_at.to_rfc3339()
            ],
        )?;
    }
    for r in &page.playlists {
        let n = tx.execute(
            "INSERT INTO playlists (id, rev, title, description, kind, pinned, created_at, updated_at, search,
                                   folder_id, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, title = excluded.title,
               description = excluded.description, kind = excluded.kind, pinned = excluded.pinned,
               created_at = excluded.created_at, updated_at = excluded.updated_at, search = excluded.search,
               folder_id = excluded.folder_id, position = excluded.position
             WHERE excluded.rev >= playlists.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.title,
                r.description,
                r.kind.as_str(),
                r.pinned,
                r.created_at.to_rfc3339(),
                r.updated_at.to_rfc3339(),
                normalize(&r.title),
                r.folder_id.map(|f| f.to_string()),
                r.position
            ],
        )?;
        a.library |= n > 0;
        a.rows += n;
    }
    for r in &page.playlist_folders {
        let n = tx.execute(
            "INSERT INTO playlist_folders (id, rev, title, position, collapsed, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, title = excluded.title,
               position = excluded.position, collapsed = excluded.collapsed,
               created_at = excluded.created_at, updated_at = excluded.updated_at
             WHERE excluded.rev >= playlist_folders.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.title,
                r.position,
                r.collapsed,
                r.created_at.to_rfc3339(),
                r.updated_at.to_rfc3339()
            ],
        )?;
        a.library |= n > 0;
        a.rows += n;
    }
    if let Some(r) = &page.settings {
        a.rows += tx.execute(
            "INSERT INTO user_settings (id, rev, settings) VALUES (1, ?1, ?2)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, settings = excluded.settings
             WHERE excluded.rev >= user_settings.rev",
            params![r.rev, json(&r.settings)?],
        )?;
    }
    for r in &page.playlist_items {
        let n = tx.execute(
            "INSERT INTO playlist_items (id, rev, playlist_id, track_id, position, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, playlist_id = excluded.playlist_id,
               track_id = excluded.track_id, position = excluded.position, added_at = excluded.added_at
             WHERE excluded.rev >= playlist_items.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.playlist_id.to_string(),
                r.track_id.to_string(),
                r.position,
                r.added_at.to_rfc3339()
            ],
        )?;
        a.library |= n > 0;
        a.rows += n;
    }
    for r in &page.device_downloads {
        let n = tx.execute(
            "INSERT INTO device_downloads (id, rev, device_id, kind, ref_id) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, device_id = excluded.device_id,
               kind = excluded.kind, ref_id = excluded.ref_id
             WHERE excluded.rev >= device_downloads.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.device_id.to_string(),
                r.kind,
                r.ref_id.map(|i| i.to_string())
            ],
        )?;
        a.rows += n;
        // Закреплённое этого устройства с сервера (переустановка, другой экземпляр приложения):
        // локально удалённое, но ещё не отправленное, не возвращается
        if Some(r.device_id) == device {
            let added = tx.execute(
                "INSERT INTO pins (kind, ref_id, server_id, created_at, pending) VALUES (?1, ?2, ?3, ?4, NULL)
                 ON CONFLICT (kind, ref_id) DO UPDATE SET server_id = excluded.server_id
                 WHERE pins.pending IS NOT 'remove'",
                params![
                    r.kind,
                    r.ref_id.map(|i| i.to_string()).unwrap_or_default(),
                    r.id.to_string(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )?;
            a.pins |= added > 0;
        }
    }
    for r in &page.device_sets {
        if device.is_some_and(|d| d != r.device_id) {
            continue;
        }
        let n = tx.execute(
            "INSERT INTO device_sets (track_id, rev, kind, score) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (track_id) DO UPDATE SET rev = excluded.rev, kind = excluded.kind, score = excluded.score
             WHERE excluded.rev >= device_sets.rev",
            params![r.track_id.to_string(), r.rev, r.kind, r.score],
        )?;
        a.reserve |= n > 0;
        a.rows += n;
    }
    for r in &page.wave_presets {
        a.rows += tx.execute(
            "INSERT INTO wave_presets (id, rev, name, icon, settings, position, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (id) DO UPDATE SET rev = excluded.rev, name = excluded.name, icon = excluded.icon,
               settings = excluded.settings, position = excluded.position, created_at = excluded.created_at
             WHERE excluded.rev >= wave_presets.rev",
            params![
                r.id.to_string(),
                r.rev,
                r.name,
                r.icon,
                json(&r.settings)?,
                r.position,
                r.created_at.to_rfc3339()
            ],
        )?;
    }
    for r in &page.track_features {
        a.rows += tx.execute(
            "INSERT INTO track_features (recording_id, rev, energy, valence, danceability, bpm, key_camelot, voice,
                                         language, approachability, relaxed, party)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT (recording_id) DO UPDATE SET rev = excluded.rev, energy = excluded.energy,
               valence = excluded.valence, danceability = excluded.danceability, bpm = excluded.bpm,
               key_camelot = excluded.key_camelot, voice = excluded.voice, language = excluded.language,
               approachability = excluded.approachability, relaxed = excluded.relaxed, party = excluded.party
             WHERE excluded.rev >= track_features.rev",
            params![
                r.recording_id.to_string(),
                r.rev,
                r.energy,
                r.valence,
                r.danceability,
                r.bpm,
                r.key_camelot,
                r.voice,
                r.language,
                r.approachability,
                r.relaxed,
                r.party
            ],
        )?;
    }
    Ok(())
}

fn apply_tombstone(
    tx: &Transaction,
    t: &SyncTombstone,
    device: Option<Uuid>,
    a: &mut Applied,
) -> StoreResult<()> {
    let k = &t.key;
    let n = match t.entity {
        SyncEntity::LikedTracks => tx.execute(
            "DELETE FROM liked_tracks WHERE track_id = ?1 AND rev < ?2",
            params![key_str(k, "trackId"), t.rev],
        )?,
        SyncEntity::SavedAlbums => tx.execute(
            "DELETE FROM saved_albums WHERE album_id = ?1 AND rev < ?2",
            params![key_str(k, "albumId"), t.rev],
        )?,
        SyncEntity::FollowedArtists => tx.execute(
            "DELETE FROM followed_artists WHERE artist_id = ?1 AND rev < ?2",
            params![key_str(k, "artistId"), t.rev],
        )?,
        SyncEntity::BlockedItems => tx.execute(
            "DELETE FROM blocked_items WHERE kind = ?1 AND ref_id = ?2 AND rev < ?3",
            params![key_str(k, "kind"), key_str(k, "refId"), t.rev],
        )?,
        SyncEntity::Playlists => {
            let id = key_str(k, "id");
            let n = tx.execute(
                "DELETE FROM playlists WHERE id = ?1 AND rev < ?2",
                params![id, t.rev],
            )?;
            if n > 0 {
                tx.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [&id])?;
            }
            n
        }
        SyncEntity::PlaylistItems => tx.execute(
            "DELETE FROM playlist_items WHERE id = ?1 AND rev < ?2",
            params![key_str(k, "id"), t.rev],
        )?,
        SyncEntity::DeviceDownloads => {
            let id = key_str(k, "id");
            let n = tx.execute(
                "DELETE FROM device_downloads WHERE id = ?1 AND rev < ?2",
                params![id, t.rev],
            )?;
            // Откреплено на сервере (с другого экземпляра) — откреплено и здесь, если не ждёт отправки
            if key_str(k, "deviceId").and_then(|s| s.parse().ok()) == device && device.is_some() {
                let removed = tx.execute(
                    "DELETE FROM pins WHERE server_id = ?1 AND pending IS NULL",
                    [&id],
                )?;
                a.pins |= removed > 0;
            }
            n
        }
        SyncEntity::DeviceSets => {
            let mine = key_str(k, "deviceId").and_then(|s| s.parse::<Uuid>().ok());
            if device.is_some() && mine != device {
                0
            } else {
                let n = tx.execute(
                    "DELETE FROM device_sets WHERE track_id = ?1 AND rev < ?2",
                    params![key_str(k, "trackId"), t.rev],
                )?;
                a.reserve |= n > 0;
                n
            }
        }
        SyncEntity::Artists => tx.execute(
            "DELETE FROM artists WHERE id = ?1 AND rev < ?2",
            params![key_str(k, "id"), t.rev],
        )?,
        SyncEntity::Albums => tx.execute(
            "DELETE FROM albums WHERE id = ?1 AND rev < ?2",
            params![key_str(k, "id"), t.rev],
        )?,
        SyncEntity::Tracks => {
            let id = key_str(k, "id");
            let n = tx.execute(
                "DELETE FROM tracks WHERE id = ?1 AND rev < ?2",
                params![id, t.rev],
            )?;
            if n > 0 {
                tx.execute("DELETE FROM track_artists WHERE track_id = ?1", [&id])?;
            }
            n
        }
        SyncEntity::Recordings => tx.execute(
            "DELETE FROM recordings WHERE id = ?1 AND rev < ?2",
            params![key_str(k, "id"), t.rev],
        )?,
        SyncEntity::TrackFeatures => tx.execute(
            "DELETE FROM track_features WHERE recording_id = ?1 AND rev < ?2",
            params![key_str(k, "recordingId"), t.rev],
        )?,
        SyncEntity::WavePresets => tx.execute(
            "DELETE FROM wave_presets WHERE id = ?1 AND rev < ?2",
            params![key_str(k, "id"), t.rev],
        )?,
        SyncEntity::PlaylistFolders => {
            let id = key_str(k, "id");
            let n = tx.execute(
                "DELETE FROM playlist_folders WHERE id = ?1 AND rev < ?2",
                params![id, t.rev],
            )?;
            // Плейлисты папки сервер и так пришлёт вне папок; до них — не терять их из виду
            if n > 0 {
                tx.execute(
                    "UPDATE playlists SET folder_id = NULL WHERE folder_id = ?1",
                    [&id],
                )?;
            }
            n
        }
        // Настройки не удаляются: строка живёт, пока жив слушатель
        SyncEntity::Settings => 0,
    };
    if n > 0
        && matches!(
            t.entity,
            SyncEntity::LikedTracks
                | SyncEntity::SavedAlbums
                | SyncEntity::FollowedArtists
                | SyncEntity::Playlists
                | SyncEntity::PlaylistItems
                | SyncEntity::PlaylistFolders
        )
    {
        a.library = true;
    }
    a.rows += n;
    Ok(())
}
