//! То, что знает только устройство: закреплённое скачанным, файлы на диске, набор волны с признаками
//! звучания и история прослушиваний для волны без сети.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Store, StoreResult};

/// Закреплённое на устройстве: трек, альбом, плейлист, артист или все лайки (`ref_id` пустой).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub kind: String,
    pub ref_id: Option<Uuid>,
    /// id строки `device_downloads` на сервере; `None` — ещё не отправлено.
    pub server_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// `add` | `remove` — изменение ещё не дошло до сервера.
    pub pending: Option<String>,
}

/// Закреплённое с ходом загрузки — для интерфейса.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinState {
    pub kind: String,
    pub ref_id: Option<Uuid>,
    pub tracks_total: usize,
    /// Треков уже на устройстве.
    pub tracks_ready: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileState {
    /// Ждёт своей очереди (или сети, Wi-Fi).
    Queued,
    Downloading,
    /// Скачан и проверен.
    Ready,
    Failed,
}

impl FileState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "downloading" => Self::Downloading,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }
}

/// Файл трека на устройстве.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub track_id: Uuid,
    pub recording_id: Option<Uuid>,
    pub file_id: Option<Uuid>,
    /// Относительно папки файлов.
    pub path: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub codec: Option<String>,
    pub container: Option<String>,
    pub duration_ms: Option<i64>,
    pub loudness_lufs: Option<f32>,
    pub true_peak_db: Option<f32>,
    pub waveform: Option<Vec<i16>>,
    pub state: FileState,
    pub received: u64,
    pub error: Option<String>,
    pub attempts: u32,
    pub downloaded_at: Option<DateTime<Utc>>,
    pub last_played_at: Option<DateTime<Utc>>,
}

/// Файл, который пора качать: всё, что сервер сказал о нём.
#[derive(Debug, Clone, PartialEq)]
pub struct NewFile {
    pub track_id: Uuid,
    pub recording_id: Uuid,
    pub file_id: Uuid,
    pub path: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub codec: String,
    pub container: String,
    pub duration_ms: i64,
    pub loudness_lufs: Option<f32>,
    pub true_peak_db: Option<f32>,
    pub waveform: Option<Vec<i16>>,
}

/// Скачанный файл для плеера.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFile {
    pub path: String,
    pub duration_ms: Option<i64>,
    pub loudness_lufs: Option<f32>,
    pub true_peak_db: Option<f32>,
    pub waveform: Option<Vec<i16>>,
}

/// Трек набора волны устройства.
#[derive(Debug, Clone, PartialEq)]
pub struct ReserveTrack {
    pub track_id: Uuid,
    /// `favorite` | `fresh`
    pub kind: String,
    pub score: f32,
}

/// Всё, что волне без сети нужно о скачанном треке.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackFeatures {
    pub track_id: Uuid,
    pub title: String,
    pub artist_ids: Vec<Uuid>,
    pub artist_names: Vec<String>,
    pub energy: Option<f32>,
    pub valence: Option<f32>,
    pub danceability: Option<f32>,
    pub bpm: Option<f32>,
    pub key_camelot: Option<String>,
    pub voice: Option<f32>,
    pub language: Option<String>,
    pub approachability: Option<f32>,
    pub liked: bool,
    /// Из набора волны: `favorite` | `fresh`; `None` — закреплённое или кэш.
    pub set_kind: Option<String>,
    pub set_score: Option<f32>,
    pub plays_30d: i64,
    /// Ранние пропуски за 30 дней.
    pub skips_30d: i64,
    pub completed: i64,
    pub last_played: Option<DateTime<Utc>>,
    pub blocked: bool,
}

/// Прослушивание на устройстве.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRecord {
    pub track_id: Uuid,
    pub at: DateTime<Utc>,
    pub played_ms: i64,
    pub duration_ms: i64,
    /// Пропущен (не дослушан).
    pub skipped: bool,
    /// Играл в волне.
    pub wave: bool,
}

/// Сколько места занято: закреплённое, умный запас и кэш (остальное скачанное, уходит первым).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub pinned: u64,
    pub reserve: u64,
    pub cache: u64,
    pub files: u64,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.pinned + self.reserve + self.cache
    }
}

fn uuid(s: String) -> Uuid {
    s.parse().unwrap_or_default()
}

fn opt_time(s: Option<String>) -> Option<DateTime<Utc>> {
    s.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc))
}

fn pin_row(r: &Row) -> rusqlite::Result<Pin> {
    let ref_id: String = r.get(1)?;
    Ok(Pin {
        kind: r.get(0)?,
        ref_id: (!ref_id.is_empty()).then(|| uuid(ref_id)),
        server_id: r.get::<_, Option<String>>(2)?.map(uuid),
        created_at: opt_time(Some(r.get(3)?)).unwrap_or_default(),
        pending: r.get(4)?,
    })
}

const FILE_COLS: &str = "track_id, recording_id, file_id, path, size_bytes, sha256, codec, container,
    duration_ms, loudness_lufs, true_peak_db, waveform, state, received, error, attempts, downloaded_at,
    last_played_at";

fn file_row(r: &Row) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        track_id: uuid(r.get(0)?),
        recording_id: r.get::<_, Option<String>>(1)?.map(uuid),
        file_id: r.get::<_, Option<String>>(2)?.map(uuid),
        path: r.get(3)?,
        size_bytes: r.get::<_, i64>(4)?.max(0) as u64,
        sha256: r.get(5)?,
        codec: r.get(6)?,
        container: r.get(7)?,
        duration_ms: r.get(8)?,
        loudness_lufs: r.get(9)?,
        true_peak_db: r.get(10)?,
        waveform: r
            .get::<_, Option<String>>(11)?
            .and_then(|s| serde_json::from_str(&s).ok()),
        state: FileState::parse(&r.get::<_, String>(12)?),
        received: r.get::<_, i64>(13)?.max(0) as u64,
        error: r.get(14)?,
        attempts: r.get::<_, i64>(15)?.max(0) as u32,
        downloaded_at: opt_time(r.get(16)?),
        last_played_at: opt_time(r.get(17)?),
    })
}

impl Store {
    // ---------- закреплённое ----------

    /// Закреплённое (без ожидающего открепления), в порядке закрепления.
    pub fn pins(&self) -> StoreResult<Vec<Pin>> {
        let mut st = self.conn().prepare_cached(
            "SELECT kind, ref_id, server_id, created_at, pending FROM pins
             WHERE pending IS NOT 'remove' ORDER BY created_at, kind, ref_id",
        )?;
        Ok(st.query_map([], pin_row)?.collect::<Result<_, _>>()?)
    }

    /// Изменения закреплённого, которые ещё не дошли до сервера.
    pub fn pending_pins(&self) -> StoreResult<Vec<Pin>> {
        let mut st = self.conn().prepare_cached(
            "SELECT kind, ref_id, server_id, created_at, pending FROM pins WHERE pending IS NOT NULL",
        )?;
        Ok(st.query_map([], pin_row)?.collect::<Result<_, _>>()?)
    }

    pub fn is_pinned(&self, kind: &str, ref_id: Option<Uuid>) -> StoreResult<bool> {
        Ok(self.conn().query_row(
            "SELECT EXISTS (SELECT 1 FROM pins WHERE kind = ?1 AND ref_id = ?2 AND pending IS NOT 'remove')",
            params![kind, ref_id.map(|i| i.to_string()).unwrap_or_default()],
            |r| r.get(0),
        )?)
    }

    /// Закрепить. Без сети — запоминается и уходит на сервер при следующей синхронизации.
    pub fn add_pin(&self, kind: &str, ref_id: Option<Uuid>) -> StoreResult<()> {
        self.conn().execute(
            "INSERT INTO pins (kind, ref_id, server_id, created_at, pending) VALUES (?1, ?2, NULL, ?3, 'add')
             ON CONFLICT (kind, ref_id) DO UPDATE SET
               pending = CASE WHEN pins.server_id IS NULL THEN 'add' ELSE NULL END",
            params![
                kind,
                ref_id.map(|i| i.to_string()).unwrap_or_default(),
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Открепить: отправленное на сервер ждёт удаления там, неотправленное просто забывается.
    pub fn remove_pin(&self, kind: &str, ref_id: Option<Uuid>) -> StoreResult<()> {
        let r = ref_id.map(|i| i.to_string()).unwrap_or_default();
        self.conn().execute(
            "DELETE FROM pins WHERE kind = ?1 AND ref_id = ?2 AND server_id IS NULL",
            params![kind, r],
        )?;
        self.conn().execute(
            "UPDATE pins SET pending = 'remove' WHERE kind = ?1 AND ref_id = ?2",
            params![kind, r],
        )?;
        Ok(())
    }

    /// Сервер принял закрепление.
    pub fn confirm_pin(
        &self,
        kind: &str,
        ref_id: Option<Uuid>,
        server_id: Uuid,
    ) -> StoreResult<()> {
        self.conn().execute(
            "UPDATE pins SET server_id = ?3, pending = NULL WHERE kind = ?1 AND ref_id = ?2 AND pending = 'add'",
            params![
                kind,
                ref_id.map(|i| i.to_string()).unwrap_or_default(),
                server_id.to_string()
            ],
        )?;
        Ok(())
    }

    /// Сервер открепил.
    pub fn forget_pin(&self, kind: &str, ref_id: Option<Uuid>) -> StoreResult<()> {
        self.conn().execute(
            "DELETE FROM pins WHERE kind = ?1 AND ref_id = ?2 AND pending = 'remove'",
            params![kind, ref_id.map(|i| i.to_string()).unwrap_or_default()],
        )?;
        Ok(())
    }

    /// Треки одного закреплённого по порядку.
    pub fn pin_tracks(&self, kind: &str, ref_id: Option<Uuid>) -> StoreResult<Vec<Uuid>> {
        let r = ref_id.map(|i| i.to_string()).unwrap_or_default();
        let sql = match kind {
            "track" => "SELECT id FROM tracks WHERE id = ?1",
            "album" => {
                "SELECT id FROM tracks WHERE album_id = ?1 ORDER BY disc_number, track_position, title"
            }
            "playlist" => {
                "SELECT track_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position"
            }
            // Дискография: альбомы артиста и треки, где он основной
            "artist" => {
                "SELECT t.id FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
                 WHERE al.artist_id = ?1
                    OR t.id IN (SELECT track_id FROM track_artists WHERE artist_id = ?1 AND role = 'main')
                 ORDER BY al.release_date DESC, t.disc_number, t.track_position"
            }
            "liked" => "SELECT track_id FROM liked_tracks WHERE ?1 = ?1 ORDER BY liked_at DESC",
            _ => return Ok(Vec::new()),
        };
        let mut st = self.conn().prepare_cached(sql)?;
        let ids = st
            .query_map([r], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids.into_iter().map(uuid).collect())
    }

    /// Все закреплённые треки без повторов, в порядке закрепления.
    pub fn pinned_tracks(&self) -> StoreResult<Vec<Uuid>> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for pin in self.pins()? {
            for id in self.pin_tracks(&pin.kind, pin.ref_id)? {
                if seen.insert(id) {
                    out.push(id);
                }
            }
        }
        Ok(out)
    }

    /// Закреплённое с ходом загрузки.
    pub fn pin_states(&self) -> StoreResult<Vec<PinState>> {
        let mut out = Vec::new();
        for pin in self.pins()? {
            let ids = self.pin_tracks(&pin.kind, pin.ref_id)?;
            let mut ready = 0;
            let mut bytes = 0;
            for id in &ids {
                if let Some(f) = self.file(*id)?
                    && f.state == FileState::Ready
                {
                    ready += 1;
                    bytes += f.size_bytes;
                }
            }
            out.push(PinState {
                kind: pin.kind,
                ref_id: pin.ref_id,
                tracks_total: ids.len(),
                tracks_ready: ready,
                bytes,
            });
        }
        Ok(out)
    }

    /// Забыть всё закреплённое («Удалить всё»): то, что было на сервере, ждёт открепления там.
    pub fn clear_pins(&self) -> StoreResult<()> {
        self.conn()
            .execute("DELETE FROM pins WHERE server_id IS NULL", [])?;
        self.conn()
            .execute("UPDATE pins SET pending = 'remove'", [])?;
        Ok(())
    }

    // ---------- набор волны ----------

    /// Набор волны устройства: любимое и новое через одно, лучшие первыми — в таком порядке и качается.
    pub fn reserve(&self) -> StoreResult<Vec<ReserveTrack>> {
        let mut st = self.conn().prepare_cached(
            "SELECT s.track_id, s.kind, s.score FROM device_sets s JOIN tracks t ON t.id = s.track_id
             ORDER BY s.score DESC",
        )?;
        let all: Vec<ReserveTrack> = st
            .query_map([], |r| {
                Ok(ReserveTrack {
                    track_id: uuid(r.get(0)?),
                    kind: r.get(1)?,
                    score: r.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        let (fav, fresh): (Vec<_>, Vec<_>) = all.into_iter().partition(|r| r.kind == "favorite");
        let mut out = Vec::with_capacity(fav.len() + fresh.len());
        let (mut a, mut b) = (fav.into_iter(), fresh.into_iter());
        loop {
            match (a.next(), b.next()) {
                (None, None) => break,
                (x, y) => out.extend(x.into_iter().chain(y)),
            }
        }
        Ok(out)
    }

    /// Новое из набора, которое уже сыграно на устройстве: пора заменить свежим.
    pub fn played_fresh(&self) -> StoreResult<usize> {
        Ok(self.conn().query_row(
            "SELECT count(*) FROM device_sets s WHERE s.kind = 'fresh'
               AND EXISTS (SELECT 1 FROM plays p WHERE p.track_id = s.track_id)",
            [],
            |r| r.get::<_, i64>(0),
        )? as usize)
    }

    // ---------- файлы ----------

    pub fn file(&self, track_id: Uuid) -> StoreResult<Option<FileRecord>> {
        Ok(self
            .conn()
            .prepare_cached(&format!(
                "SELECT {FILE_COLS} FROM files WHERE track_id = ?1"
            ))?
            .query_row([track_id.to_string()], file_row)
            .optional()?)
    }

    pub fn files(&self) -> StoreResult<Vec<FileRecord>> {
        let mut st = self.conn().prepare_cached(&format!(
            "SELECT {FILE_COLS} FROM files ORDER BY created_at"
        ))?;
        Ok(st.query_map([], file_row)?.collect::<Result<_, _>>()?)
    }

    /// Скачанный и проверенный файл трека для плеера.
    pub fn ready_file(&self, track_id: Uuid) -> StoreResult<Option<StoredFile>> {
        Ok(self.file(track_id)?.and_then(|f| {
            (f.state == FileState::Ready).then_some(StoredFile {
                path: f.path,
                duration_ms: f.duration_ms,
                loudness_lufs: f.loudness_lufs,
                true_peak_db: f.true_peak_db,
                waveform: f.waveform,
            })
        }))
    }

    /// Начать (или продолжить) загрузку: сведения о файле с сервера. Другой файл записи — заново.
    pub fn queue_file(&self, f: &NewFile) -> StoreResult<()> {
        let id = f.track_id.to_string();
        let same: Option<String> = self
            .conn()
            .query_row(
                "SELECT file_id FROM files WHERE track_id = ?1",
                [&id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if same.as_deref() == Some(&f.file_id.to_string()) {
            self.conn().execute(
                "UPDATE files SET state = CASE WHEN state = 'ready' THEN 'ready' ELSE 'queued' END, error = NULL
                 WHERE track_id = ?1",
                [&id],
            )?;
            return Ok(());
        }
        self.conn().execute(
            "INSERT INTO files (track_id, recording_id, file_id, path, size_bytes, sha256, codec, container,
                                duration_ms, loudness_lufs, true_peak_db, waveform, state, received, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'queued', 0, ?13)
             ON CONFLICT (track_id) DO UPDATE SET recording_id = excluded.recording_id,
               file_id = excluded.file_id, path = excluded.path, size_bytes = excluded.size_bytes,
               sha256 = excluded.sha256, codec = excluded.codec, container = excluded.container,
               duration_ms = excluded.duration_ms, loudness_lufs = excluded.loudness_lufs,
               true_peak_db = excluded.true_peak_db, waveform = excluded.waveform, state = 'queued',
               received = 0, error = NULL, attempts = 0, downloaded_at = NULL",
            params![
                id,
                f.recording_id.to_string(),
                f.file_id.to_string(),
                f.path,
                f.size_bytes as i64,
                f.sha256,
                f.codec,
                f.container,
                f.duration_ms,
                f.loudness_lufs,
                f.true_peak_db,
                f.waveform.as_ref().map(serde_json::to_string).transpose()?,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn set_file_state(
        &self,
        track_id: Uuid,
        state: FileState,
        received: u64,
        error: Option<&str>,
    ) -> StoreResult<()> {
        let attempts = if state == FileState::Failed { 1 } else { 0 };
        let done = (state == FileState::Ready).then(|| Utc::now().to_rfc3339());
        self.conn().execute(
            "UPDATE files SET state = ?2, received = ?3, error = ?4, attempts = attempts + ?5,
                              downloaded_at = coalesce(?6, downloaded_at)
             WHERE track_id = ?1",
            params![
                track_id.to_string(),
                state.as_str(),
                received as i64,
                error,
                attempts,
                done
            ],
        )?;
        Ok(())
    }

    /// Убрать запись о файле (сам файл удаляет вызывающий).
    pub fn delete_file(&self, track_id: Uuid) -> StoreResult<()> {
        self.conn().execute(
            "DELETE FROM files WHERE track_id = ?1",
            [track_id.to_string()],
        )?;
        Ok(())
    }

    /// Трек сыграл: кэш вытесняется по давности прослушивания.
    pub fn touch_file(&self, track_id: Uuid) -> StoreResult<()> {
        self.conn().execute(
            "UPDATE files SET last_played_at = ?2 WHERE track_id = ?1",
            params![track_id.to_string(), Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Занятое место по разделам: закреплённое, умный запас, кэш (недокачанное — по скачанным байтам).
    pub fn usage(&self) -> StoreResult<Usage> {
        let pinned: HashSet<Uuid> = self.pinned_tracks()?.into_iter().collect();
        let reserve: HashSet<Uuid> = self.reserve()?.into_iter().map(|r| r.track_id).collect();
        let mut u = Usage::default();
        for f in self.files()? {
            let bytes = if f.state == FileState::Ready {
                f.size_bytes
            } else {
                f.received
            };
            if bytes == 0 {
                continue;
            }
            u.files += 1;
            if pinned.contains(&f.track_id) {
                u.pinned += bytes;
            } else if reserve.contains(&f.track_id) {
                u.reserve += bytes;
            } else {
                u.cache += bytes;
            }
        }
        Ok(u)
    }

    // ---------- волна без сети ----------

    /// Скачанные треки с признаками звучания и историей — кандидаты волны без сети.
    pub fn playable_features(&self) -> StoreResult<Vec<TrackFeatures>> {
        let since = (Utc::now() - Duration::days(30)).to_rfc3339();
        let mut st = self.conn().prepare_cached(
            "SELECT t.id, t.title, f.energy, f.valence, f.danceability, f.bpm, f.key_camelot, f.voice, f.language,
                    f.approachability,
                    EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = t.id),
                    s.kind, s.score,
                    (SELECT count(*) FROM plays p WHERE p.track_id = t.id AND p.at > ?1),
                    (SELECT count(*) FROM plays p WHERE p.track_id = t.id AND p.at > ?1 AND p.skipped
                       AND p.played_ms < 30000),
                    (SELECT count(*) FROM plays p WHERE p.track_id = t.id AND NOT p.skipped),
                    (SELECT max(p.at) FROM plays p WHERE p.track_id = t.id),
                    EXISTS (SELECT 1 FROM blocked_items WHERE kind = 'track' AND ref_id = t.id)
                      OR EXISTS (SELECT 1 FROM blocked_items b JOIN track_artists ta ON ta.artist_id = b.ref_id
                                 WHERE b.kind = 'artist' AND ta.track_id = t.id)
             FROM files fl
             JOIN tracks t ON t.id = fl.track_id
             LEFT JOIN track_features f ON f.recording_id = t.recording_id
             LEFT JOIN device_sets s ON s.track_id = t.id
             WHERE fl.state = 'ready'",
        )?;
        let mut rows: Vec<TrackFeatures> = st
            .query_map([&since], |r| {
                Ok(TrackFeatures {
                    track_id: uuid(r.get(0)?),
                    title: r.get(1)?,
                    energy: r.get(2)?,
                    valence: r.get(3)?,
                    danceability: r.get(4)?,
                    bpm: r.get(5)?,
                    key_camelot: r.get(6)?,
                    voice: r.get(7)?,
                    language: r.get(8)?,
                    approachability: r.get(9)?,
                    liked: r.get(10)?,
                    set_kind: r.get(11)?,
                    set_score: r.get(12)?,
                    plays_30d: r.get(13)?,
                    skips_30d: r.get(14)?,
                    completed: r.get(15)?,
                    last_played: opt_time(r.get(16)?),
                    blocked: r.get(17)?,
                    ..Default::default()
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut artists = self.conn().prepare_cached(
            "SELECT ta.artist_id, a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
             WHERE ta.track_id = ?1 ORDER BY ta.position",
        )?;
        for t in &mut rows {
            for row in artists.query_map([t.track_id.to_string()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })? {
                let (id, name) = row?;
                t.artist_ids.push(uuid(id));
                t.artist_names.push(name);
            }
        }
        Ok(rows)
    }

    pub fn record_play(&self, p: &PlayRecord) -> StoreResult<()> {
        self.conn().execute(
            "INSERT INTO plays (track_id, at, played_ms, duration_ms, skipped, wave) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                p.track_id.to_string(),
                p.at.to_rfc3339(),
                p.played_ms,
                p.duration_ms,
                p.skipped,
                p.wave
            ],
        )?;
        // История нужна волне на месяц-два вперёд — старое не копится
        self.conn().execute(
            "DELETE FROM plays WHERE at < ?1",
            [(Utc::now() - Duration::days(90)).to_rfc3339()],
        )?;
        self.touch_file(p.track_id)
    }

    /// Последние прослушивания, новые первыми.
    pub fn recent_plays(&self, limit: usize) -> StoreResult<Vec<PlayRecord>> {
        let mut st = self.conn().prepare_cached(
            "SELECT track_id, at, played_ms, duration_ms, skipped, wave FROM plays ORDER BY at DESC LIMIT ?1",
        )?;
        Ok(st
            .query_map([limit as i64], |r| {
                Ok(PlayRecord {
                    track_id: uuid(r.get(0)?),
                    at: opt_time(Some(r.get(1)?)).unwrap_or_default(),
                    played_ms: r.get(2)?,
                    duration_ms: r.get(3)?,
                    skipped: r.get(4)?,
                    wave: r.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }
}
