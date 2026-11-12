//! Схема локальной базы. Версия — `PRAGMA user_version`; изменения только вперёд.

use rusqlite::Connection;

/// Медиатека и каталог — копия строк сервера с их `rev`; `search` — нормализованный текст для поиска.
const V1: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE artists (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, deezer_id INTEGER, name TEXT NOT NULL,
    image TEXT, search TEXT NOT NULL DEFAULT ''
);
CREATE TABLE albums (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, deezer_id INTEGER, title TEXT NOT NULL,
    artist_id TEXT NOT NULL, record_type TEXT NOT NULL, release_date TEXT, label TEXT,
    genres TEXT NOT NULL DEFAULT '[]', explicit INTEGER NOT NULL DEFAULT 0, tracks_count INTEGER,
    duration_ms INTEGER, image TEXT, palette TEXT, search TEXT NOT NULL DEFAULT ''
);
CREATE INDEX albums_artist ON albums (artist_id);
CREATE TABLE tracks (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, deezer_id INTEGER, title TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '', album_id TEXT, disc_number INTEGER NOT NULL DEFAULT 1,
    track_position INTEGER, duration_ms INTEGER NOT NULL, explicit INTEGER NOT NULL DEFAULT 0,
    isrc TEXT, recording_id TEXT, search TEXT NOT NULL DEFAULT ''
);
CREATE INDEX tracks_album ON tracks (album_id);
CREATE INDEX tracks_recording ON tracks (recording_id);
-- Участники трека по порядку: основные, затем приглашённые
CREATE TABLE track_artists (
    track_id TEXT NOT NULL, position INTEGER NOT NULL, artist_id TEXT NOT NULL, role TEXT NOT NULL,
    PRIMARY KEY (track_id, position)
);
CREATE INDEX track_artists_artist ON track_artists (artist_id);
CREATE TABLE recordings (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, isrc TEXT, duration_ms INTEGER NOT NULL,
    audio_status TEXT NOT NULL, primary_file_id TEXT
);

CREATE TABLE liked_tracks (track_id TEXT PRIMARY KEY, rev INTEGER NOT NULL, liked_at TEXT NOT NULL);
CREATE TABLE saved_albums (album_id TEXT PRIMARY KEY, rev INTEGER NOT NULL, saved_at TEXT NOT NULL);
CREATE TABLE followed_artists (
    artist_id TEXT PRIMARY KEY, rev INTEGER NOT NULL, policy TEXT NOT NULL, notify INTEGER NOT NULL,
    followed_at TEXT NOT NULL
);
CREATE TABLE blocked_items (
    kind TEXT NOT NULL, ref_id TEXT NOT NULL, rev INTEGER NOT NULL, created_at TEXT NOT NULL,
    PRIMARY KEY (kind, ref_id)
);
CREATE TABLE playlists (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, title TEXT NOT NULL, description TEXT NOT NULL,
    kind TEXT NOT NULL, pinned INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
    search TEXT NOT NULL DEFAULT ''
);
CREATE TABLE playlist_items (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, playlist_id TEXT NOT NULL, track_id TEXT NOT NULL,
    position REAL NOT NULL, added_at TEXT NOT NULL
);
CREATE INDEX playlist_items_playlist ON playlist_items (playlist_id, position);
-- Закреплённое на всех устройствах человека (с сервера)
CREATE TABLE device_downloads (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, device_id TEXT NOT NULL, kind TEXT NOT NULL, ref_id TEXT
);
-- Набор волны этого устройства
CREATE TABLE device_sets (
    track_id TEXT PRIMARY KEY, rev INTEGER NOT NULL, kind TEXT NOT NULL, score REAL NOT NULL
);
CREATE TABLE track_features (
    recording_id TEXT PRIMARY KEY, rev INTEGER NOT NULL, energy REAL, valence REAL, danceability REAL,
    bpm REAL, key_camelot TEXT, voice REAL, language TEXT, approachability REAL, relaxed REAL, party REAL
);

-- Только устройство. Закреплённое: `pending` — изменение ещё не дошло до сервера (`add` | `remove`)
CREATE TABLE pins (
    kind TEXT NOT NULL, ref_id TEXT NOT NULL DEFAULT '', server_id TEXT, created_at TEXT NOT NULL,
    pending TEXT, PRIMARY KEY (kind, ref_id)
);
-- Файлы треков на диске. `path` — относительно папки файлов
CREATE TABLE files (
    track_id TEXT PRIMARY KEY, recording_id TEXT, file_id TEXT, path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL DEFAULT 0, sha256 TEXT, codec TEXT, container TEXT,
    duration_ms INTEGER, loudness_lufs REAL, true_peak_db REAL, waveform TEXT,
    state TEXT NOT NULL, received INTEGER NOT NULL DEFAULT 0, error TEXT, attempts INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL, downloaded_at TEXT, last_played_at TEXT
);
CREATE INDEX files_state ON files (state);
-- Что играло на устройстве: волна без сети учитывает пропуски и не повторяет недавнее
CREATE TABLE plays (
    track_id TEXT NOT NULL, at TEXT NOT NULL, played_ms INTEGER NOT NULL, duration_ms INTEGER NOT NULL,
    skipped INTEGER NOT NULL, wave INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX plays_track ON plays (track_id, at);
CREATE INDEX plays_at ON plays (at);
";

/// Забыть медиатеку и курсор (выход, смена сервера): следующая синхронизация — полная.
pub const RESET_LIBRARY: &str = "
DELETE FROM artists; DELETE FROM albums; DELETE FROM tracks; DELETE FROM track_artists;
DELETE FROM recordings; DELETE FROM liked_tracks; DELETE FROM saved_albums; DELETE FROM followed_artists;
DELETE FROM blocked_items; DELETE FROM playlists; DELETE FROM playlist_items; DELETE FROM device_downloads;
DELETE FROM device_sets; DELETE FROM track_features; DELETE FROM wave_presets;
DELETE FROM meta WHERE key LIKE 'sync.%';
";

/// Сохранённые сочетания настроек волны. Курсор сбрасывается: сочетания, созданные до обновления
/// приложения, придут только полной синхронизацией.
const V2: &str = "
CREATE TABLE wave_presets (
    id TEXT PRIMARY KEY, rev INTEGER NOT NULL, name TEXT NOT NULL, icon TEXT NOT NULL,
    settings TEXT NOT NULL, position REAL NOT NULL, created_at TEXT NOT NULL
);
DELETE FROM meta WHERE key = 'sync.cursor';
";

const MIGRATIONS: &[&str] = &[V1, V2];

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}
