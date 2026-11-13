//! Разделы «Каталог», «Хранилище», «Импорт» и «Настройки» админки: записи на сервере с техническими
//! данными и перекачкой, занятое место, история импортов, настройки сервера. Сюда же — свежая ссылка
//! на превью Deezer для «Спорных». Весь роутер закрыт правом admin (см. `routes::router`).

use std::collections::HashMap;

use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::{
    ErrorBody,
    admin::{
        AdminImport, AdminImportList, AdminRecording, AdminRecordingPage, AudioFileInfo,
        RecordingsQuery, ReplaceAudio, ServerSettings, ServerSettingsPatch, StorageArtist,
        StorageDay, StorageGroup, StorageOverview, TrackPreview,
    },
    catalog::{ArtistRef, AudioStatus},
    jobs::{AcquireReason, AdminAcquisition},
    library::DownloadPolicy,
};
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use super::acquisitions::{Pasted, admin_view, parse_pasted};
use crate::{
    acquire::{self, Enqueue, MANUAL_PRIORITY, Request},
    audit,
    auth::Principal,
    catalog::views,
    cursor,
    deezer::DeezerError,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    jobs::lanes,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/recordings", get(recordings))
        .route("/recordings/{id}/reacquire", post(reacquire))
        .route("/tracks/{id}/preview", get(preview))
        .route("/storage", get(storage))
        .route("/imports", get(imports))
        .route("/settings", get(settings).patch(update_settings))
}

/// Политика подписки по умолчанию из настроек сервера.
pub async fn default_follow_policy(conn: &mut PgConnection) -> Result<DownloadPolicy, sqlx::Error> {
    let raw = sqlx::query_scalar!("SELECT follow_policy FROM server_settings WHERE id")
        .fetch_optional(conn)
        .await?;
    Ok(raw
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

struct RecordingRow {
    id: Uuid,
    isrc: Option<String>,
    duration_ms: i32,
    audio_status: String,
    status_changed_at: DateTime<Utc>,
    track_id: Option<Uuid>,
    tracks: i64,
    files: i64,
    file_id: Option<Uuid>,
    codec: Option<String>,
    container: Option<String>,
    lossless: Option<bool>,
    bitrate_kbps: Option<i32>,
    sample_rate: Option<i32>,
    file_duration_ms: Option<i32>,
    size_bytes: Option<i64>,
    source: Option<String>,
    source_ref: Option<String>,
    source_title: Option<String>,
    loudness_lufs: Option<f32>,
    true_peak_db: Option<f32>,
    match_score: Option<f32>,
    verified: Option<bool>,
    verify_similarity: Option<f32>,
    acquired_by: Option<String>,
    file_created_at: Option<DateTime<Utc>>,
}

impl RecordingRow {
    fn file(&self) -> Option<AudioFileInfo> {
        Some(AudioFileInfo {
            id: self.file_id?,
            codec: self.codec.clone()?,
            container: self.container.clone()?,
            lossless: self.lossless?,
            bitrate_kbps: self.bitrate_kbps,
            sample_rate: self.sample_rate,
            duration_ms: self.file_duration_ms?,
            size_bytes: self.size_bytes?,
            source: self.source.clone()?,
            source_ref: self.source_ref.clone(),
            source_title: self.source_title.clone(),
            loudness_lufs: self.loudness_lufs,
            true_peak_db: self.true_peak_db,
            match_score: self.match_score,
            verified: self.verified?,
            verify_similarity: self.verify_similarity,
            acquired_by: self.acquired_by.clone(),
            created_at: self.file_created_at?,
        })
    }
}

/// `%` и `_` в строке поиска — обычные символы, а не шаблон LIKE.
fn like_pattern(q: &str) -> String {
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

#[utoipa::path(
    get,
    path = "/api/admin/recordings",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(RecordingsQuery),
    responses(
        (status = 200, description = "Записи с файлом или запросом на скачивание, недавно менявшиеся первыми", body = AdminRecordingPage),
        (status = 400, description = "Неверный cursor, limit или источник", body = ErrorBody),
    ),
)]
pub async fn recordings(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<RecordingsQuery>,
) -> AppResult<Json<AdminRecordingPage>> {
    let limit = cursor::limit(q.limit, 50, 200)?;
    let after = cursor::decode_opt(q.cursor.as_deref())?;
    let source = q.source.as_deref().filter(|s| !s.is_empty());
    if source.is_some_and(|s| {
        !matches!(
            s,
            "youtube" | "youtube_music" | "soundcloud" | "bandcamp" | "upload" | "other"
        )
    }) {
        return Err(bad("неизвестный источник"));
    }
    let search =
        q.q.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| like_pattern(&s.chars().take(100).collect::<String>()));
    let mut rows = sqlx::query_as!(
        RecordingRow,
        r#"SELECT r.id, r.isrc, r.duration_ms, r.audio_status, r.status_changed_at,
                  (SELECT t.id FROM tracks t WHERE t.recording_id = r.id
                   ORDER BY (t.album_id IS NULL), t.id LIMIT 1) AS "track_id?",
                  (SELECT count(*) FROM tracks t WHERE t.recording_id = r.id) AS "tracks!",
                  (SELECT count(*) FROM audio_files x WHERE x.recording_id = r.id) AS "files!",
                  f.id AS "file_id?", f.codec AS "codec?", f.container AS "container?", f.lossless AS "lossless?",
                  f.bitrate_kbps, f.sample_rate, f.duration_ms AS "file_duration_ms?", f.size_bytes AS "size_bytes?",
                  f.source AS "source?", f.source_ref, f.source_title, f.loudness_lufs, f.true_peak_db,
                  f.match_score, f.verified AS "verified?", f.verify_similarity, f.acquired_by,
                  f.created_at AS "file_created_at?"
           FROM recordings r
           LEFT JOIN audio_files f ON f.id = r.primary_file_id
           WHERE (($1::text IS NULL AND r.audio_status <> 'none') OR r.audio_status = $1)
             AND ($2::text IS NULL OR f.source = $2)
             AND ($3::bool IS NULL OR f.verified = $3)
             AND ($4::text IS NULL OR EXISTS (
                   SELECT 1 FROM tracks t WHERE t.recording_id = r.id
                     AND (search_norm(t.title) LIKE search_norm($4)
                          OR EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                                     WHERE ta.track_id = t.id AND search_norm(a.name) LIKE search_norm($4)))))
             AND ($5::timestamptz IS NULL OR (r.status_changed_at, r.id) < ($5, $6))
           ORDER BY r.status_changed_at DESC, r.id DESC
           LIMIT $7"#,
        q.status.map(|s| s.as_str()),
        source,
        q.verified,
        search,
        after.map(|(at, _)| at),
        after.map(|(_, id)| id).unwrap_or_default(),
        limit + 1,
    )
    .fetch_all(&st.db)
    .await?;
    let next_cursor = cursor::page(&mut rows, limit, |r| (r.status_changed_at, r.id));
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &ids, principal.user.id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let items = rows
        .iter()
        .map(|r| AdminRecording {
            id: r.id,
            isrc: r.isrc.clone(),
            duration_ms: r.duration_ms,
            audio_status: r.audio_status.parse().unwrap_or(AudioStatus::None),
            status_changed_at: r.status_changed_at,
            track: r.track_id.and_then(|id| tracks.get(&id).cloned()),
            tracks: r.tracks,
            file: r.file(),
            files: r.files,
        })
        .collect();
    Ok(Json(AdminRecordingPage { items, next_cursor }))
}

#[utoipa::path(
    post,
    path = "/api/admin/recordings/{id}/reacquire",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id записи")),
    request_body = ReplaceAudio,
    responses(
        (status = 200, description = "Новое скачивание вне очереди. Со ссылкой файл заменит основной, \
                                      без неё — заменит, только если окажется лучше", body = AdminAcquisition),
        (status = 400, description = "Неверная ссылка или у записи нет трека Deezer", body = ErrorBody),
        (status = 404, description = "Нет такой записи", body = ErrorBody),
        (status = 409, description = "Скачивание записи уже идёт (`acquisition_active`)", body = ErrorBody),
    ),
)]
pub async fn reacquire(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<ReplaceAudio>,
) -> AppResult<Json<AdminAcquisition>> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM recordings WHERE id = $1) AS "e!""#,
        id
    )
    .fetch_one(&st.db)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let track_id = sqlx::query_scalar!(
        "SELECT id FROM tracks WHERE recording_id = $1 AND deezer_id IS NOT NULL
         ORDER BY (album_id IS NULL), id LIMIT 1",
        id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| bad("у записи нет трека Deezer — скачать её заново нельзя"))?;
    let admin = principal.user.id;
    let mut request = Request::new(track_id, AcquireReason::Manual, lanes::NOW)
        .by(admin)
        .priority(MANUAL_PRIORITY);
    request.force = true;
    let url = body.url.as_deref().map(str::trim).filter(|u| !u.is_empty());
    if let Some(raw) = url {
        match parse_pasted(raw)? {
            Pasted::Video(video) => request.prefer_video_ids = vec![video],
            Pasted::Direct(url) => request.direct_url = Some(url),
        }
    }
    let acquisition_id = match acquire::enqueue(&st.db, request).await? {
        Enqueue::Queued { acquisition_id, .. } => acquisition_id,
        Enqueue::Active { .. } => {
            return Err(AppError::Conflict {
                code: "acquisition_active",
                message: "скачивание этой записи уже идёт: дождитесь его или отмените задачу"
                    .into(),
            });
        }
        Enqueue::Ready | Enqueue::Skipped(_) => {
            return Err(bad("запись нельзя поставить на скачивание"));
        }
    };
    audit::record(
        &st.db,
        Some(admin),
        audit::RECORDING_REACQUIRE,
        Some(id.to_string()),
        json!({ "url": url, "acquisitionId": acquisition_id }),
    )
    .await?;
    Ok(Json(admin_view(&st, acquisition_id, admin).await?))
}

#[utoipa::path(
    get,
    path = "/api/admin/tracks/{id}/preview",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Свежая подписанная ссылка на превью Deezer (живёт недолго)", body = TrackPreview),
        (status = 404, description = "Нет такого трека", body = ErrorBody),
        (status = 503, description = "Deezer не ответил", body = ErrorBody),
    ),
)]
pub async fn preview(
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TrackPreview>> {
    let deezer_id = sqlx::query_scalar!("SELECT deezer_id FROM tracks WHERE id = $1", id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let Some(deezer_id) = deezer_id else {
        return Ok(Json(TrackPreview { url: None }));
    };
    let url = match st.deezer.track(deezer_id).await {
        Ok(t) => t.preview.filter(|u| !u.is_empty()),
        Err(DeezerError::NotFound) => None,
        Err(e) => {
            tracing::warn!(error = %e, deezer_id, "deezer preview lookup failed");
            return Err(AppError::Unavailable(
                "Deezer не ответил, попробуйте ещё раз".into(),
            ));
        }
    };
    Ok(Json(TrackPreview { url }))
}

#[utoipa::path(
    get,
    path = "/api/admin/storage",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Занятое место по видам, кодекам и источникам, тяжёлые артисты, прирост", body = StorageOverview),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn storage(State(st): State<AppState>) -> AppResult<Json<StorageOverview>> {
    // Своя музыка лежит в своих папках, а не в хранилище: её файлы здесь не считаются
    let k = sqlx::query!(
        r#"SELECT
              (SELECT count(*) FROM audio_files WHERE source <> 'local') AS "audio_files!",
              (SELECT COALESCE(sum(size_bytes), 0) FROM audio_files WHERE source <> 'local')::bigint AS "audio_bytes!",
              (SELECT count(*) FROM albums WHERE cover_key IS NOT NULL) AS "covers!",
              (SELECT COALESCE(sum(cover_bytes), 0) FROM albums)::bigint AS "cover_bytes!",
              (SELECT count(*) FROM artists WHERE picture_key IS NOT NULL) AS "photos!",
              (SELECT COALESCE(sum(picture_bytes), 0) FROM artists)::bigint AS "photo_bytes!",
              (SELECT count(*) FROM audio_files f WHERE f.source <> 'local' AND NOT EXISTS
                 (SELECT 1 FROM recordings r WHERE r.primary_file_id = f.id)) AS "replaced!",
              (SELECT COALESCE(sum(size_bytes), 0) FROM audio_files f WHERE f.source <> 'local' AND NOT EXISTS
                 (SELECT 1 FROM recordings r WHERE r.primary_file_id = f.id))::bigint AS "replaced_bytes!""#
    )
    .fetch_one(&st.db)
    .await?;
    let codecs = sqlx::query!(
        r#"SELECT codec AS key, count(*) AS "files!", COALESCE(sum(size_bytes), 0)::bigint AS "bytes!"
           FROM audio_files WHERE source <> 'local' GROUP BY codec ORDER BY 3 DESC"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| StorageGroup {
        key: r.key,
        files: r.files,
        bytes: r.bytes,
    })
    .collect();
    let sources = sqlx::query!(
        r#"SELECT source AS key, count(*) AS "files!", COALESCE(sum(size_bytes), 0)::bigint AS "bytes!"
           FROM audio_files WHERE source <> 'local' GROUP BY source ORDER BY 3 DESC"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| StorageGroup {
        key: r.key,
        files: r.files,
        bytes: r.bytes,
    })
    .collect();
    // Запись относится к главному артисту одного из её треков, чтобы не считать файл дважды
    let artists = sqlx::query!(
        r#"WITH rec_artist AS (
              SELECT DISTINCT ON (t.recording_id) t.recording_id, ta.artist_id
              FROM tracks t JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'main'
              WHERE t.recording_id IN (SELECT DISTINCT recording_id FROM audio_files WHERE source <> 'local')
              ORDER BY t.recording_id, (t.album_id IS NULL), ta.position, t.id
           )
           SELECT a.id, a.name, count(f.id) AS "files!", COALESCE(sum(f.size_bytes), 0)::bigint AS "bytes!"
           FROM audio_files f
           JOIN rec_artist ra ON ra.recording_id = f.recording_id
           JOIN artists a ON a.id = ra.artist_id
           WHERE f.source <> 'local'
           GROUP BY a.id, a.name
           ORDER BY 4 DESC, a.name
           LIMIT 12"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| StorageArtist {
        artist: ArtistRef {
            id: r.id,
            name: r.name,
        },
        files: r.files,
        bytes: r.bytes,
    })
    .collect();
    let growth = sqlx::query!(
        r#"SELECT to_char(d.day, 'YYYY-MM-DD') AS "day!", count(f.id) AS "files!",
                  COALESCE(sum(f.size_bytes), 0)::bigint AS "bytes!"
           FROM generate_series((now() AT TIME ZONE 'UTC')::date - 29, (now() AT TIME ZONE 'UTC')::date,
                                interval '1 day') AS d(day)
           LEFT JOIN audio_files f ON (f.created_at AT TIME ZONE 'UTC')::date = d.day::date AND f.source <> 'local'
           GROUP BY d.day ORDER BY d.day"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| StorageDay {
        day: r.day,
        files: r.files,
        bytes: r.bytes,
    })
    .collect();
    Ok(Json(StorageOverview {
        total_bytes: k.audio_bytes + k.cover_bytes + k.photo_bytes,
        kinds: vec![
            StorageGroup {
                key: "audio".into(),
                files: k.audio_files,
                bytes: k.audio_bytes,
            },
            StorageGroup {
                key: "covers".into(),
                files: k.covers,
                bytes: k.cover_bytes,
            },
            StorageGroup {
                key: "photos".into(),
                files: k.photos,
                bytes: k.photo_bytes,
            },
        ],
        codecs,
        sources,
        artists,
        growth,
        replaced_files: k.replaced,
        replaced_bytes: k.replaced_bytes,
    }))
}

#[utoipa::path(
    get,
    path = "/api/admin/imports",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Импорты всех людей, новые первыми (до 100)", body = AdminImportList),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn imports(State(st): State<AppState>) -> AppResult<Json<AdminImportList>> {
    let items = sqlx::query!(
        r#"SELECT i.id, u.name AS user_name, i.kind, i.title, i.status, i.total, i.matched,
                  (SELECT count(*) FROM import_items x WHERE x.import_id = i.id AND x.status = 'unmatched') AS "unmatched!",
                  i.phase, i.processed, i.error, i.stats, i.created_at, i.finished_at
           FROM imports i JOIN users u ON u.id = i.user_id
           ORDER BY i.created_at DESC
           LIMIT 100"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| AdminImport {
        id: r.id,
        user_name: r.user_name,
        kind: r.kind,
        title: r.title,
        status: r.status,
        total: r.total,
        matched: r.matched,
        unmatched: r.unmatched,
        phase: r.phase,
        processed: r.processed,
        error: r.error,
        stats: serde_json::from_value(r.stats).unwrap_or_default(),
        created_at: r.created_at,
        finished_at: r.finished_at,
    })
    .collect();
    Ok(Json(AdminImportList { items }))
}

async fn load_settings(st: &AppState) -> AppResult<ServerSettings> {
    let row = sqlx::query!(
        "SELECT s.follow_policy, s.updated_at, r.utc_offset_min, r.updated_at AS reco_updated_at
         FROM server_settings s CROSS JOIN reco_settings r
         WHERE s.id AND r.id"
    )
    .fetch_one(&st.db)
    .await?;
    let mut conn = st.db.acquire().await?;
    let clips = crate::video::clip_settings::view(&mut conn).await?;
    Ok(ServerSettings {
        follow_policy: serde_json::from_value(row.follow_policy).unwrap_or_default(),
        utc_offset_min: row.utc_offset_min,
        clips,
        updated_at: row.updated_at.max(row.reco_updated_at),
    })
}

#[utoipa::path(
    get,
    path = "/api/admin/settings",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Настройки сервера", body = ServerSettings),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn settings(State(st): State<AppState>) -> AppResult<Json<ServerSettings>> {
    Ok(Json(load_settings(&st).await?))
}

#[utoipa::path(
    patch,
    path = "/api/admin/settings",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = ServerSettingsPatch,
    responses(
        (status = 200, description = "Настройки после изменения", body = ServerSettings),
        (status = 400, description = "Часовой пояс — от −720 до 840 минут; неверные настройки клипов \
                                      (или потолок места задан переменной окружения)", body = ErrorBody),
    ),
)]
pub async fn update_settings(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<ServerSettingsPatch>,
) -> AppResult<Json<ServerSettings>> {
    if patch
        .utc_offset_min
        .is_some_and(|m| !(-720..=840).contains(&m))
    {
        return Err(bad("часовой пояс — от −720 до 840 минут"));
    }
    let policy = patch
        .follow_policy
        .map(serde_json::to_value)
        .transpose()
        .map_err(anyhow::Error::from)?;
    let mut tx = st.db.begin().await?;
    if let Some(policy) = &policy {
        sqlx::query!(
            "UPDATE server_settings SET follow_policy = $1, updated_at = now() WHERE id",
            policy
        )
        .execute(&mut *tx)
        .await?;
    }
    if let Some(offset) = patch.utc_offset_min {
        sqlx::query!(
            "UPDATE reco_settings SET utc_offset_min = $1, updated_at = now() WHERE id",
            offset
        )
        .execute(&mut *tx)
        .await?;
    }
    if let Some(clips) = &patch.clips {
        crate::video::clip_settings::update(&mut tx, clips).await?;
    }
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::SETTINGS_CHANGED,
        None,
        json!({ "followPolicy": policy, "utcOffsetMin": patch.utc_offset_min, "clips": patch.clips }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(load_settings(&st).await?))
}
