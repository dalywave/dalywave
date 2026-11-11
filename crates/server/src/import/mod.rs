//! Импорт музыки: архив данных Spotify, списки и свои аудиофайлы.
//!
//! Путь импорта: API разбирает загруженное (архив — потоком во временный файл, с пределами против
//! архивов-бомб) и пишет позиции в `import_items`, историю — в `import_plays`; задача `import_match`
//! (полоса «Импорт») сопоставляет позиции с каталогом — кеш `import_matches`, свой каталог, поиск Deezer
//! в пределах общего лимита запросов; `import_apply` раскладывает найденное в медиатеку: лайки,
//! плейлисты (те же названия и порядок), альбомы, подписки (политика по умолчанию), прослушивания
//! (`play_events.source = 'import'`: вкус их видит, «Продолжить» — нет), затем ставит пересчёт вкуса
//! и разведчика. Несопоставленное человек исправляет сам — исправление применяется сразу.
//! Свои файлы обрабатывает `import_file` ([`files`]).

pub mod audio;
pub mod files;
pub mod jobs;
pub mod lists;
pub mod matching;
pub mod spotify;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    imports::{
        ImportCandidate, ImportFileInfo, ImportItem, ImportItemKind, ImportItemPage,
        ImportItemStatus, ImportJobPayload, ImportKind, ImportPhase, ImportStats, ImportStatus,
        ImportSummary, ImportTarget,
    },
    jobs::{AcquireReason, SyncDiscographyPayload, kind},
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    acquire,
    catalog::views,
    deezer::Deezer,
    error::{AppError, AppResult, bad},
    jobs::{self as queue, NewJob, lanes},
    realtime::{self, Audience},
    reco,
};

/// Полоса очереди для задач импорта.
pub const LANE: &str = "import";
/// Архив Spotify больше не бывает: даже расширенная история за десять лет — десятки мегабайт.
pub const MAX_ARCHIVE_BYTES: u64 = 500 * 1024 * 1024;
/// Один аудиофайл: 24 бит / 192 кГц FLAC на 20 минут — около 700 МБ.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// Файлов в одном импорте.
pub const MAX_FILES: i64 = 5000;
/// Ключ загруженного файла до обработки.
pub fn upload_key(import_id: Uuid, item_id: Uuid, ext: &str) -> String {
    format!("imports/{import_id}/{item_id}.{ext}")
}

/// Плейлист архива: позиции по порядку и созданный плейлист.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePlaylist {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub items: Vec<Uuid>,
    #[serde(default)]
    pub playlist_id: Option<Uuid>,
}

/// Настройки импорта (`imports.options`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub target: Option<ImportTarget>,
    pub playlist_title: Option<String>,
    pub file_name: Option<String>,
}

/// Новая позиция импорта.
#[derive(Debug, Clone, Default)]
pub struct NewItem {
    pub id: Uuid,
    pub kind: Option<ImportItemKind>,
    pub position: i32,
    pub priority: i32,
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
    pub source_ref: Option<String>,
    pub liked: bool,
    pub saved: bool,
    pub followed: bool,
    pub plays: i32,
}

pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    kind: ImportKind,
    title: &str,
    options: &Options,
    phase: ImportPhase,
    size_bytes: Option<i64>,
) -> Result<Uuid, sqlx::Error> {
    let options = serde_json::to_value(options).unwrap_or_else(|_| json!({}));
    sqlx::query_scalar!(
        "INSERT INTO imports (user_id, kind, title, options, phase, size_bytes)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        user_id,
        kind.as_str(),
        title,
        options,
        phase.as_str(),
        size_bytes,
    )
    .fetch_one(conn)
    .await
}

pub async fn insert_items(
    conn: &mut PgConnection,
    import_id: Uuid,
    items: &[NewItem],
) -> Result<(), sqlx::Error> {
    for chunk in items.chunks(1000) {
        let ids: Vec<Uuid> = chunk.iter().map(|i| i.id).collect();
        let kinds: Vec<String> = chunk
            .iter()
            .map(|i| i.kind.unwrap_or(ImportItemKind::Track).as_str().to_string())
            .collect();
        let positions: Vec<i32> = chunk.iter().map(|i| i.position).collect();
        let priorities: Vec<i32> = chunk.iter().map(|i| i.priority).collect();
        let artists: Vec<String> = chunk.iter().map(|i| clip(&i.artist, 500)).collect();
        let titles: Vec<String> = chunk.iter().map(|i| clip(&i.title, 500)).collect();
        let albums: Vec<Option<String>> = chunk
            .iter()
            .map(|i| i.album.as_deref().map(|a| clip(a, 500)))
            .collect();
        let durations: Vec<Option<i32>> = chunk.iter().map(|i| i.duration_ms).collect();
        let isrcs: Vec<Option<String>> = chunk.iter().map(|i| i.isrc.clone()).collect();
        let refs: Vec<Option<String>> = chunk
            .iter()
            .map(|i| i.source_ref.as_deref().map(|r| clip(r, 1000)))
            .collect();
        let liked: Vec<bool> = chunk.iter().map(|i| i.liked).collect();
        let saved: Vec<bool> = chunk.iter().map(|i| i.saved).collect();
        let followed: Vec<bool> = chunk.iter().map(|i| i.followed).collect();
        let plays: Vec<i32> = chunk.iter().map(|i| i.plays).collect();
        sqlx::query!(
            "INSERT INTO import_items (id, import_id, kind, position, priority, artist, title, album, duration_ms,
                                       isrc, source_ref, liked, saved, followed, plays)
             SELECT x.id, $1, x.kind, x.position, x.priority, x.artist, x.title, x.album, x.duration_ms, x.isrc,
                    x.source_ref, x.liked, x.saved, x.followed, x.plays
             FROM unnest($2::uuid[], $3::text[], $4::int4[], $5::int4[], $6::text[], $7::text[], $8::text[],
                         $9::int4[], $10::text[], $11::text[], $12::bool[], $13::bool[], $14::bool[], $15::int4[])
                  AS x(id, kind, position, priority, artist, title, album, duration_ms, isrc, source_ref, liked,
                       saved, followed, plays)",
            import_id,
            &ids,
            &kinds,
            &positions,
            &priorities,
            &artists,
            &titles,
            &albums as &[Option<String>],
            &durations as &[Option<i32>],
            &isrcs as &[Option<String>],
            &refs as &[Option<String>],
            &liked,
            &saved,
            &followed,
            &plays,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Позиции и история архива Spotify. Возвращает число позиций.
pub async fn store_archive(
    conn: &mut PgConnection,
    import_id: Uuid,
    archive: &spotify::Archive,
) -> Result<i32, sqlx::Error> {
    let mut in_playlist = vec![false; archive.tracks.len()];
    for p in &archive.playlists {
        for i in &p.tracks {
            in_playlist[*i] = true;
        }
    }
    let mut items = Vec::new();
    let mut track_ids = Vec::with_capacity(archive.tracks.len());
    // Медиатека раньше истории, частое раньше редкого: первые минуты импорта дают самое нужное
    for (n, t) in archive.tracks.iter().enumerate() {
        let id = Uuid::now_v7();
        track_ids.push(id);
        let priority = if t.liked { 1000 } else { 0 }
            + if in_playlist[n] { 500 } else { 0 }
            + (t.plays.min(400) as i32);
        items.push(NewItem {
            id,
            kind: Some(ImportItemKind::Track),
            position: n as i32,
            priority,
            artist: t.artist.clone(),
            title: t.title.clone(),
            album: t.album.clone(),
            source_ref: t.uri.clone(),
            liked: t.liked,
            plays: t.plays.min(i32::MAX as u32) as i32,
            ..NewItem::default()
        });
    }
    let base = archive.tracks.len() as i32;
    for (n, a) in archive.albums.iter().enumerate() {
        items.push(NewItem {
            id: Uuid::now_v7(),
            kind: Some(ImportItemKind::Album),
            position: base + n as i32,
            priority: 900,
            artist: a.artist.clone(),
            title: a.title.clone(),
            source_ref: a.uri.clone(),
            saved: true,
            ..NewItem::default()
        });
    }
    let base = base + archive.albums.len() as i32;
    for (n, a) in archive.artists.iter().enumerate() {
        items.push(NewItem {
            id: Uuid::now_v7(),
            kind: Some(ImportItemKind::Artist),
            position: base + n as i32,
            priority: 800,
            artist: a.name.clone(),
            title: a.name.clone(),
            source_ref: a.uri.clone(),
            followed: true,
            ..NewItem::default()
        });
    }
    insert_items(conn, import_id, &items).await?;

    for chunk in archive.plays.chunks(5000) {
        let items: Vec<Uuid> = chunk.iter().map(|p| track_ids[p.track]).collect();
        let ended: Vec<DateTime<Utc>> = chunk.iter().map(|p| p.ended_at).collect();
        let played: Vec<i32> = chunk
            .iter()
            .map(|p| p.played_ms.min(i32::MAX as u32) as i32)
            .collect();
        let reasons: Vec<String> = chunk.iter().map(|p| p.end_reason.to_string()).collect();
        let shuffle: Vec<Option<bool>> = chunk.iter().map(|p| p.shuffle).collect();
        let platform: Vec<Option<String>> =
            chunk.iter().map(|p| p.platform.map(String::from)).collect();
        sqlx::query!(
            "INSERT INTO import_plays (import_id, item_id, ended_at, played_ms, end_reason, shuffle, platform)
             SELECT $1, x.item, x.ended, x.played, x.reason, x.shuffle, x.platform
             FROM unnest($2::uuid[], $3::timestamptz[], $4::int4[], $5::text[], $6::bool[], $7::text[])
                  AS x(item, ended, played, reason, shuffle, platform)",
            import_id,
            &items,
            &ended,
            &played,
            &reasons,
            &shuffle as &[Option<bool>],
            &platform as &[Option<String>],
        )
        .execute(&mut *conn)
        .await?;
    }

    let playlists: Vec<SourcePlaylist> = archive
        .playlists
        .iter()
        .map(|p| SourcePlaylist {
            name: p.name.clone(),
            description: p.description.clone(),
            items: p.tracks.iter().map(|i| track_ids[*i]).collect(),
            playlist_id: None,
        })
        .collect();
    let total = items.len() as i32;
    sqlx::query!(
        "UPDATE imports SET total = $2, playlists = $3, updated_at = now() WHERE id = $1",
        import_id,
        total,
        serde_json::to_value(&playlists).unwrap_or_else(|_| json!([])),
    )
    .execute(&mut *conn)
    .await?;
    Ok(total)
}

/// Строки списка. В «Мне нравится» — лайки; в плейлист — позиции по порядку.
pub async fn store_lines(
    conn: &mut PgConnection,
    import_id: Uuid,
    lines: &[lists::Line],
    target: ImportTarget,
    playlist_title: &str,
) -> Result<i32, sqlx::Error> {
    let items: Vec<NewItem> = lines
        .iter()
        .enumerate()
        .map(|(n, l)| NewItem {
            id: Uuid::now_v7(),
            kind: Some(ImportItemKind::Track),
            position: n as i32,
            artist: l.artist.clone(),
            title: l.title.clone(),
            album: l.album.clone(),
            duration_ms: l.duration_ms,
            isrc: l.isrc.clone(),
            source_ref: Some(l.raw.clone()),
            liked: target == ImportTarget::Likes,
            ..NewItem::default()
        })
        .collect();
    insert_items(conn, import_id, &items).await?;
    let playlists = if target == ImportTarget::Playlist {
        vec![SourcePlaylist {
            name: playlist_title.to_string(),
            items: items.iter().map(|i| i.id).collect(),
            ..SourcePlaylist::default()
        }]
    } else {
        Vec::new()
    };
    let total = items.len() as i32;
    sqlx::query!(
        "UPDATE imports SET total = $2, playlists = $3, updated_at = now() WHERE id = $1",
        import_id,
        total,
        serde_json::to_value(&playlists).unwrap_or_else(|_| json!([])),
    )
    .execute(&mut *conn)
    .await?;
    Ok(total)
}

/// Ставит сопоставление импорта.
pub async fn enqueue_match(conn: &mut PgConnection, import_id: Uuid) -> Result<(), sqlx::Error> {
    queue::enqueue(
        conn,
        NewJob::new(kind::IMPORT_MATCH, LANE, ImportJobPayload { import_id })
            .dedupe(format!("import_match:{import_id}"))
            .max_attempts(10),
    )
    .await?;
    Ok(())
}

pub async fn enqueue_apply(conn: &mut PgConnection, import_id: Uuid) -> Result<(), sqlx::Error> {
    queue::enqueue(
        conn,
        NewJob::new(kind::IMPORT_APPLY, LANE, ImportJobPayload { import_id })
            .dedupe(format!("import_apply:{import_id}"))
            .priority(5)
            .max_attempts(5),
    )
    .await?;
    Ok(())
}

/// Пересчитывает счётчики импорта по позициям.
pub async fn refresh_counts(conn: &mut PgConnection, import_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE imports i SET
            processed = c.processed, matched = c.matched,
            total = GREATEST(i.total, c.total), updated_at = now()
         FROM (SELECT count(*) FILTER (WHERE status <> 'pending')::int AS processed,
                      count(*) FILTER (WHERE status = 'matched')::int AS matched,
                      count(*)::int AS total
               FROM import_items WHERE import_id = $1) c
         WHERE i.id = $1",
        import_id,
    )
    .execute(conn)
    .await?;
    Ok(())
}

// ───────────── Просмотр ─────────────

struct SummaryRow {
    id: Uuid,
    kind: String,
    title: String,
    status: String,
    phase: String,
    total: i32,
    processed: i32,
    matched: i32,
    unmatched: i64,
    stats: Value,
    error: Option<String>,
    playlists: Value,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

impl SummaryRow {
    fn view(self) -> ImportSummary {
        let playlists: Vec<SourcePlaylist> =
            serde_json::from_value(self.playlists).unwrap_or_default();
        ImportSummary {
            id: self.id,
            kind: self.kind.parse().unwrap_or(ImportKind::Text),
            title: self.title,
            status: self.status.parse().unwrap_or(ImportStatus::Failed),
            phase: self.phase.parse().unwrap_or(ImportPhase::Done),
            total: self.total,
            processed: self.processed,
            matched: self.matched,
            unmatched: self.unmatched,
            stats: serde_json::from_value(self.stats).unwrap_or_default(),
            error: self.error,
            playlist_id: (playlists.len() == 1)
                .then(|| playlists[0].playlist_id)
                .flatten(),
            created_at: self.created_at,
            finished_at: self.finished_at,
        }
    }
}

/// Импорты человека, новые первыми; `id` — только этот.
pub async fn summaries(
    db: &PgPool,
    user_id: Option<Uuid>,
    id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<ImportSummary>, sqlx::Error> {
    let rows = sqlx::query_as!(
        SummaryRow,
        r#"SELECT i.id, i.kind, i.title, i.status, i.phase, i.total, i.processed, i.matched,
                  (SELECT count(*) FROM import_items x WHERE x.import_id = i.id AND x.status = 'unmatched') AS "unmatched!",
                  i.stats, i.error, i.playlists, i.created_at, i.finished_at
           FROM imports i
           WHERE ($1::uuid IS NULL OR i.user_id = $1) AND ($2::uuid IS NULL OR i.id = $2)
           ORDER BY i.created_at DESC
           LIMIT $3"#,
        user_id,
        id,
        limit,
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(SummaryRow::view).collect())
}

pub async fn summary(db: &PgPool, user_id: Uuid, id: Uuid) -> AppResult<ImportSummary> {
    summaries(db, Some(user_id), Some(id), 1)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

struct ItemRow {
    id: Uuid,
    kind: String,
    position: i32,
    artist: String,
    title: String,
    album: Option<String>,
    status: String,
    confidence: Option<f32>,
    track_id: Option<Uuid>,
    candidates: Value,
    plays: i32,
    liked: bool,
    file_meta: Option<Value>,
}

/// Позиции импорта по порядку источника.
pub async fn items(
    db: &PgPool,
    user_id: Uuid,
    import_id: Uuid,
    status: Option<ImportItemStatus>,
    cursor: Option<&str>,
    limit: i64,
) -> AppResult<ImportItemPage> {
    let after = match cursor.filter(|c| !c.is_empty()) {
        Some(c) => {
            let (pos, id) = c.split_once('_').ok_or_else(|| bad("неверный cursor"))?;
            Some((
                pos.parse::<i32>().map_err(|_| bad("неверный cursor"))?,
                id.parse::<Uuid>().map_err(|_| bad("неверный cursor"))?,
            ))
        }
        None => None,
    };
    let owned = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM imports WHERE id = $1 AND user_id = $2) AS "e!""#,
        import_id,
        user_id
    )
    .fetch_one(db)
    .await?;
    if !owned {
        return Err(AppError::NotFound);
    }
    let mut rows = sqlx::query_as!(
        ItemRow,
        "SELECT id, kind, position, artist, title, album, status, confidence, track_id, candidates, plays, liked,
                file_meta
         FROM import_items
         WHERE import_id = $1 AND ($2::text IS NULL OR status = $2)
           AND ($3::int4 IS NULL OR (position, id) > ($3, $4))
         ORDER BY position, id
         LIMIT $5",
        import_id,
        status.map(|s| s.as_str()),
        after.map(|(p, _)| p),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
    )
    .fetch_all(db)
    .await?;
    let next_cursor = if rows.len() as i64 > limit {
        rows.truncate(limit as usize);
        rows.last().map(|r| format!("{}_{}", r.position, r.id))
    } else {
        None
    };
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(db, &ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let items = rows
        .into_iter()
        .map(|r| ImportItem {
            id: r.id,
            kind: r.kind.parse().unwrap_or(ImportItemKind::Track),
            position: r.position,
            artist: r.artist,
            title: r.title,
            album: r.album,
            status: r.status.parse().unwrap_or(ImportItemStatus::Pending),
            confidence: r.confidence,
            track: r.track_id.and_then(|id| tracks.get(&id).cloned()),
            candidates: serde_json::from_value::<Vec<ImportCandidate>>(r.candidates)
                .unwrap_or_default(),
            plays: r.plays,
            liked: r.liked,
            file: r
                .file_meta
                .and_then(|m| serde_json::from_value::<ImportFileInfo>(m).ok()),
        })
        .collect();
    Ok(ImportItemPage { items, next_cursor })
}

// ───────────── Раскладка в медиатеку ─────────────

async fn changed(conn: &mut PgConnection, user_id: Uuid, entities: Vec<SyncEntity>) {
    if entities.is_empty() {
        return;
    }
    realtime::publish(
        conn,
        Audience::User(user_id),
        ServerEvent::LibraryChanged(LibraryChange { entities }),
    )
    .await;
}

/// Раскладывает найденные и ещё не разложенные позиции (или одну `only`). Повтор безопасен:
/// разложенное помечается `applied`, лайки и подписки вставляются без дублей, прослушивания —
/// с устойчивым `client_event_id`.
pub async fn apply(
    db: &PgPool,
    deezer: &Deezer,
    import_id: Uuid,
    only: Option<Uuid>,
) -> Result<ImportStats, sqlx::Error> {
    let Some(imp) = sqlx::query!(
        "SELECT user_id, kind, playlists FROM imports WHERE id = $1",
        import_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(ImportStats::default());
    };
    let user = imp.user_id;
    let mut playlists: Vec<SourcePlaylist> =
        serde_json::from_value(imp.playlists).unwrap_or_default();
    let mut stats = ImportStats::default();
    let mut entities = Vec::new();
    let mut tx = db.begin().await?;

    // Лайки
    let liked = sqlx::query_scalar!(
        r#"INSERT INTO liked_tracks (user_id, track_id)
           SELECT DISTINCT $1::uuid, i.track_id FROM import_items i
           WHERE i.import_id = $2 AND i.status = 'matched' AND i.liked AND NOT i.applied AND i.track_id IS NOT NULL
             AND ($3::uuid IS NULL OR i.id = $3)
           ON CONFLICT DO NOTHING
           RETURNING track_id"#,
        user,
        import_id,
        only,
    )
    .fetch_all(&mut *tx)
    .await?;
    stats.likes = liked.len() as i64;
    if !liked.is_empty() {
        entities.push(SyncEntity::LikedTracks);
    }

    // Плейлисты: те же названия, позиции — по месту в источнике (исправленное позже встаёт на своё место)
    let matched: HashMap<Uuid, Uuid> = sqlx::query!(
        "SELECT id, track_id AS \"track_id!\" FROM import_items
         WHERE import_id = $1 AND status = 'matched' AND NOT applied AND track_id IS NOT NULL
           AND kind IN ('track', 'file') AND ($2::uuid IS NULL OR id = $2)",
        import_id,
        only,
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|r| (r.id, r.track_id))
    .collect();
    let mut playlists_changed = false;
    for p in playlists.iter_mut() {
        let mut tracks = Vec::new();
        let mut positions = Vec::new();
        for (n, item) in p.items.iter().enumerate() {
            if let Some(track) = matched.get(item) {
                tracks.push(*track);
                positions.push((n + 1) as f64);
            }
        }
        if tracks.is_empty() {
            continue;
        }
        let playlist_id = match p.playlist_id {
            Some(id)
                if sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM playlists WHERE id = $1 AND user_id = $2) AS "e!""#,
                    id,
                    user
                )
                .fetch_one(&mut *tx)
                .await? =>
            {
                id
            }
            _ => {
                // Тот же плейлист из прошлого импорта (архив загрузили ещё раз) — дополняется, а не дублируется
                let earlier = sqlx::query_scalar!(
                    r#"SELECT pl.id FROM imports i
                       CROSS JOIN LATERAL jsonb_array_elements(i.playlists) AS e
                       JOIN playlists pl ON pl.id = (e ->> 'playlistId')::uuid AND pl.user_id = i.user_id
                       WHERE $3 = 'spotify_archive' AND i.user_id = $1 AND i.id <> $2 AND i.kind = $3 AND e ->> 'name' = $4
                       ORDER BY i.created_at DESC
                       LIMIT 1"#,
                    user,
                    import_id,
                    imp.kind,
                    p.name,
                )
                .fetch_optional(&mut *tx)
                .await?;
                let id = match earlier {
                    Some(id) => id,
                    None => {
                        stats.playlists += 1;
                        sqlx::query_scalar!(
                            "INSERT INTO playlists (user_id, title, description) VALUES ($1, $2, $3) RETURNING id",
                            user,
                            clip(&p.name, 200),
                            clip(&p.description, 2000),
                        )
                        .fetch_one(&mut *tx)
                        .await?
                    }
                };
                p.playlist_id = Some(id);
                id
            }
        };
        // Уже лежащие в плейлисте треки не повторяются (повтор внутри самого источника сохраняется)
        let added = sqlx::query!(
            "INSERT INTO playlist_items (playlist_id, track_id, position)
             SELECT $1, t, p FROM unnest($2::uuid[], $3::float8[]) AS x(t, p)
             WHERE NOT EXISTS (SELECT 1 FROM playlist_items i WHERE i.playlist_id = $1 AND i.track_id = x.t)",
            playlist_id,
            &tracks,
            &positions,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if added == 0 {
            continue;
        }
        sqlx::query!(
            "UPDATE playlists SET updated_at = now() WHERE id = $1",
            playlist_id
        )
        .execute(&mut *tx)
        .await?;
        stats.playlist_tracks += added as i64;
        playlists_changed = true;
    }
    if playlists_changed {
        entities.extend([SyncEntity::Playlists, SyncEntity::PlaylistItems]);
        sqlx::query!(
            "UPDATE imports SET playlists = $2 WHERE id = $1",
            import_id,
            serde_json::to_value(&playlists).unwrap_or_else(|_| json!([])),
        )
        .execute(&mut *tx)
        .await?;
    }

    // Сохранённые альбомы
    let albums = sqlx::query_scalar!(
        r#"INSERT INTO saved_albums (user_id, album_id)
           SELECT DISTINCT $1::uuid, i.album_id FROM import_items i
           WHERE i.import_id = $2 AND i.status = 'matched' AND i.saved AND NOT i.applied AND i.album_id IS NOT NULL
             AND ($3::uuid IS NULL OR i.id = $3)
           ON CONFLICT DO NOTHING
           RETURNING album_id"#,
        user,
        import_id,
        only,
    )
    .fetch_all(&mut *tx)
    .await?;
    stats.albums = albums.len() as i64;
    if !albums.is_empty() {
        entities.push(SyncEntity::SavedAlbums);
    }

    // Подписки — с политикой докачки по умолчанию из настроек сервера
    let artists = sqlx::query_scalar!(
        r#"SELECT DISTINCT i.artist_id AS "artist_id!" FROM import_items i
           WHERE i.import_id = $1 AND i.status = 'matched' AND i.followed AND NOT i.applied AND i.artist_id IS NOT NULL
             AND ($2::uuid IS NULL OR i.id = $2)"#,
        import_id,
        only,
    )
    .fetch_all(&mut *tx)
    .await?;
    if !artists.is_empty() {
        let policy = crate::routes::admin_catalog::default_follow_policy(&mut tx).await?;
        let policy = serde_json::to_value(policy).unwrap_or_else(|_| json!({}));
        let followed = sqlx::query_scalar!(
            "INSERT INTO followed_artists (user_id, artist_id, download_policy, notify)
             SELECT $1, a, $3, true FROM unnest($2::uuid[]) AS a
             ON CONFLICT (user_id, artist_id) DO NOTHING
             RETURNING artist_id",
            user,
            &artists,
            policy,
        )
        .fetch_all(&mut *tx)
        .await?;
        for artist_id in &followed {
            queue::enqueue(
                &mut tx,
                NewJob::new(
                    kind::SYNC_DISCOGRAPHY,
                    lanes::MAINTENANCE,
                    SyncDiscographyPayload {
                        artist_id: *artist_id,
                    },
                )
                .dedupe(format!("sync_discography:{artist_id}")),
            )
            .await?;
        }
        stats.artists = followed.len() as i64;
        if !followed.is_empty() {
            entities.push(SyncEntity::FollowedArtists);
        }
    }

    // История: устойчивый id прослушивания — (слушатель, трек, минута конца). Один и тот же архив,
    // загруженный дважды, и короткая история поверх расширенной не дают повторов
    let plays = sqlx::query!(
        r#"INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms,
                                    end_reason, source, platform)
           SELECT md5($1::uuid::text || ':' || i.track_id::text || ':' || to_char(date_trunc('minute', p.ended_at AT TIME ZONE 'UTC'), 'YYYYMMDDHH24MI'))::uuid,
                  $1::uuid, i.track_id, p.ended_at - make_interval(secs => p.played_ms / 1000.0), p.played_ms, t.duration_ms,
                  CASE WHEN p.end_reason <> 'auto' THEN p.end_reason
                       WHEN p.played_ms >= 0.8 * t.duration_ms THEN 'completed'
                       WHEN p.played_ms < 30000 THEN 'skipped'
                       ELSE 'stopped' END,
                  'import', p.platform
           FROM import_plays p
           JOIN import_items i ON i.id = p.item_id AND i.status = 'matched' AND i.track_id IS NOT NULL
           JOIN tracks t ON t.id = i.track_id
           WHERE p.import_id = $2 AND ($3::uuid IS NULL OR i.id = $3)
           ON CONFLICT (client_event_id) DO NOTHING"#,
        user,
        import_id,
        only,
    )
    .execute(&mut *tx)
    .await?;
    stats.plays = plays.rows_affected() as i64;
    sqlx::query!(
        "DELETE FROM import_plays p USING import_items i
         WHERE p.import_id = $1 AND i.id = p.item_id AND i.status = 'matched' AND ($2::uuid IS NULL OR i.id = $2)",
        import_id,
        only,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE import_items SET applied = true, updated_at = now()
         WHERE import_id = $1 AND status = 'matched' AND NOT applied AND ($2::uuid IS NULL OR id = $2)",
        import_id,
        only,
    )
    .execute(&mut *tx)
    .await?;
    add_stats(&mut tx, import_id, &stats).await?;
    changed(&mut tx, user, entities).await;
    tx.commit().await?;

    // Медиатека качается на сервер: лайки и треки сохранённых альбомов — в полосу «Медиатека»,
    // после того, что человек попросил сам
    for track_id in &liked {
        let req = acquire::Request::new(*track_id, AcquireReason::Import, lanes::LIBRARY)
            .by(user)
            .priority(-1);
        if let Err(e) = acquire::request(db, deezer, req).await {
            tracing::warn!(track = %track_id, error = %e, "could not queue an imported like for download");
        }
    }
    for album_id in &albums {
        let tracks = sqlx::query_scalar!("SELECT id FROM tracks WHERE album_id = $1", album_id)
            .fetch_all(db)
            .await?;
        for track_id in tracks {
            let req = acquire::Request::new(track_id, AcquireReason::Import, lanes::LIBRARY)
                .by(user)
                .priority(-1);
            if let Err(e) = acquire::request(db, deezer, req).await {
                tracing::warn!(track = %track_id, error = %e, "could not queue an imported album track");
            }
        }
    }
    Ok(stats)
}

async fn add_stats(
    conn: &mut PgConnection,
    import_id: Uuid,
    add: &ImportStats,
) -> Result<(), sqlx::Error> {
    let current = sqlx::query_scalar!(
        "SELECT stats FROM imports WHERE id = $1 FOR UPDATE",
        import_id
    )
    .fetch_optional(&mut *conn)
    .await?
    .unwrap_or_else(|| json!({}));
    let mut s: ImportStats = serde_json::from_value(current).unwrap_or_default();
    s.likes += add.likes;
    s.playlists += add.playlists;
    s.playlist_tracks += add.playlist_tracks;
    s.albums += add.albums;
    s.artists += add.artists;
    s.plays += add.plays;
    s.files += add.files;
    s.upgraded += add.upgraded;
    sqlx::query!(
        "UPDATE imports SET stats = $2, updated_at = now() WHERE id = $1",
        import_id,
        serde_json::to_value(&s).unwrap_or_else(|_| json!({})),
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Импорт закончен: состояние, время; вкус пересчитывается, разведчик расширяет окрестность.
pub async fn finish(db: &PgPool, import_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    refresh_counts(&mut tx, import_id).await?;
    let user = sqlx::query_scalar!(
        "UPDATE imports SET status = 'done', phase = 'done', finished_at = now(), updated_at = now()
         WHERE id = $1 AND status = 'running'
         RETURNING user_id",
        import_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(user) = user {
        reco::taste::request(&mut tx, user, true).await?;
        queue::enqueue(
            &mut tx,
            NewJob::new(kind::SCOUT, lanes::SCOUT, json!({}))
                .dedupe("scout")
                .priority(reco::scout::PLAN_PRIORITY),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn fail(db: &PgPool, import_id: Uuid, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE imports SET status = 'failed', error = $2, finished_at = now(), updated_at = now()
         WHERE id = $1 AND status = 'running'",
        import_id,
        clip(error, 1000),
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Ключ кеша сопоставления позиции.
pub fn cache_key(
    kind: ImportItemKind,
    artist: &str,
    title: &str,
    source_ref: Option<&str>,
) -> String {
    if let Some(r) =
        source_ref.filter(|r| r.starts_with("spotify:") && !r.starts_with("spotify:local:"))
    {
        return r.to_string();
    }
    let n = matching::normalize;
    match kind {
        ImportItemKind::Artist => format!("n:artist:{}", n(artist)),
        ImportItemKind::Album => format!("n:album:{}|{}", n(artist), n(title)),
        ImportItemKind::Track | ImportItemKind::File => {
            format!("n:track:{}|{}", n(artist), n(title))
        }
    }
}

/// Запоминает сопоставление для повторных импортов.
pub async fn remember(
    conn: &mut PgConnection,
    key: &str,
    kind: ImportItemKind,
    deezer_id: Option<i64>,
    confidence: Option<f32>,
    candidates: &[ImportCandidate],
    manual: bool,
) -> Result<(), sqlx::Error> {
    let kind = match kind {
        ImportItemKind::File => ImportItemKind::Track,
        k => k,
    };
    sqlx::query!(
        "INSERT INTO import_matches (key, kind, deezer_id, confidence, candidates, manual)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (key) DO UPDATE SET
            deezer_id = EXCLUDED.deezer_id, confidence = EXCLUDED.confidence, candidates = EXCLUDED.candidates,
            manual = EXCLUDED.manual, matched_at = now()
         WHERE NOT import_matches.manual OR EXCLUDED.manual",
        key,
        kind.as_str(),
        deezer_id,
        confidence,
        serde_json::to_value(candidates).unwrap_or_else(|_| json!([])),
        manual,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Название импорта по умолчанию.
pub fn kind_title(kind: ImportKind) -> &'static str {
    match kind {
        ImportKind::SpotifyArchive => "Архив Spotify",
        ImportKind::Text => "Список треков",
        ImportKind::Csv => "Таблица CSV",
        ImportKind::M3u => "Плейлист M3U",
        ImportKind::Files => "Свои файлы",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_keys() {
        assert_eq!(
            cache_key(
                ImportItemKind::Track,
                "Lowborn",
                "Devil",
                Some("spotify:track:1")
            ),
            "spotify:track:1"
        );
        assert_eq!(
            cache_key(
                ImportItemKind::Track,
                "Lowborn",
                "Devil!",
                Some("spotify:local:a:b:c:1")
            ),
            "n:track:lowborn|devil"
        );
        assert_eq!(
            cache_key(ImportItemKind::Artist, "5OPKA", "", None),
            "n:artist:5opka"
        );
    }
}
