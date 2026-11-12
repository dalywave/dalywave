//! Умный офлайн: что закреплено на устройстве, набор волны устройства и ссылки на файлы для загрузки.
//!
//! Всё — только для входа токеном приложения (`device_id` у сессии): сайт файлов не хранит.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, post},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    jobs::AcquireReason,
    offline::{
        DeviceDownload, DeviceDownloadList, DeviceSet, DeviceSetRequest, DownloadKind, FileLink,
        FileLinks, FileLinksRequest, NewDeviceDownload,
    },
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    acquire::{self, Enqueue, Request},
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    jobs::lanes,
    reco::device_set,
    state::AppState,
    storage::AUDIO_GET_TTL,
};

/// Ссылок за один запрос — не больше.
const MAX_FILES: usize = 200;
/// Сколько неготовых треков закреплённого ставить на скачивание за раз: остальное — со следующим запросом.
const MAX_ACQUIRE: usize = 50;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/devices/current/downloads",
            post(add_download).get(list_downloads),
        )
        .route("/devices/current/downloads/{id}", delete(remove_download))
        .route("/devices/current/set", post(rebuild_set))
        .route("/devices/current/files", post(files))
}

fn device_of(principal: &Principal) -> AppResult<Uuid> {
    principal
        .device_id
        .ok_or_else(|| bad("загрузки на устройство есть только в приложении"))
}

/// Треки закреплённого: альбом, плейлист, дискография артиста, все лайки или один трек.
async fn pin_tracks(
    db: &PgPool,
    user: Uuid,
    kind: DownloadKind,
    ref_id: Option<Uuid>,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let r = ref_id.unwrap_or_default();
    let ids = match kind {
        DownloadKind::Track => vec![r],
        DownloadKind::Album => {
            sqlx::query_scalar!(
                "SELECT id FROM tracks WHERE album_id = $1 ORDER BY disc_number, track_position",
                r
            )
            .fetch_all(db)
            .await?
        }
        DownloadKind::Playlist => {
            sqlx::query_scalar!(
                "SELECT i.track_id FROM playlist_items i JOIN playlists p ON p.id = i.playlist_id
                 WHERE p.id = $1 AND p.user_id = $2 ORDER BY i.position",
                r,
                user
            )
            .fetch_all(db)
            .await?
        }
        DownloadKind::Artist => {
            sqlx::query_scalar!(
                r#"SELECT t.id AS "id!" FROM artist_releases ar JOIN tracks t ON t.album_id = ar.album_id
                   WHERE ar.artist_id = $1"#,
                r
            )
            .fetch_all(db)
            .await?
        }
        DownloadKind::Liked => {
            sqlx::query_scalar!(
                "SELECT track_id FROM liked_tracks WHERE user_id = $1 ORDER BY created_at DESC",
                user
            )
            .fetch_all(db)
            .await?
        }
    };
    Ok(ids)
}

async fn ready_count(db: &PgPool, ids: &[Uuid]) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM tracks t JOIN recordings r ON r.id = t.recording_id
           WHERE t.id = ANY($1) AND r.audio_status = 'ready'"#,
        ids
    )
    .fetch_one(db)
    .await
}

struct PinRow {
    id: Uuid,
    kind: String,
    ref_id: Option<Uuid>,
    created_at: chrono::DateTime<Utc>,
}

async fn view(db: &PgPool, user: Uuid, row: PinRow) -> AppResult<DeviceDownload> {
    let kind: DownloadKind = row
        .kind
        .parse()
        .map_err(|_| anyhow::anyhow!("неизвестный вид загрузки {}", row.kind))?;
    let ids = pin_tracks(db, user, kind, row.ref_id).await?;
    Ok(DeviceDownload {
        id: row.id,
        kind,
        ref_id: row.ref_id,
        created_at: row.created_at,
        tracks_ready: ready_count(db, &ids).await?,
        tracks_total: ids.len() as i64,
    })
}

/// Закреплённое, которого ещё нет на сервере, — на скачивание в полосу «Медиатека».
async fn acquire_missing(st: &AppState, user: Uuid, ids: &[Uuid]) -> Vec<Uuid> {
    let missing = match sqlx::query_scalar!(
        r#"SELECT t.id AS "id!" FROM unnest($1::uuid[]) WITH ORDINALITY AS x(id, ord)
           JOIN tracks t ON t.id = x.id
           LEFT JOIN recordings r ON r.id = t.recording_id
           WHERE r.audio_status IS DISTINCT FROM 'ready'
             AND r.audio_status IS DISTINCT FROM 'unavailable'
             AND r.audio_status IS DISTINCT FROM 'disputed'
           ORDER BY x.ord"#,
        ids
    )
    .fetch_all(&st.db)
    .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "could not list tracks to acquire for a device");
            return Vec::new();
        }
    };
    let mut pending = Vec::new();
    for id in missing.into_iter().take(MAX_ACQUIRE) {
        let req = Request::new(id, AcquireReason::Manual, lanes::LIBRARY).by(user);
        match acquire::request(&st.db, &st.deezer, req).await {
            Ok(Enqueue::Queued { .. } | Enqueue::Active { .. }) => pending.push(id),
            Ok(_) => {}
            Err(e) => tracing::warn!(track = %id, error = %e, "could not acquire a pinned track"),
        }
    }
    pending
}

#[utoipa::path(
    get,
    path = "/api/devices/current/downloads",
    tag = "offline",
    security(("device" = [])),
    responses(
        (status = 200, description = "Что закреплено на этом устройстве и сколько из этого на сервере", body = DeviceDownloadList),
        (status = 400, description = "Вход не токеном приложения", body = ErrorBody),
    ),
)]
pub async fn list_downloads(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<DeviceDownloadList>> {
    let device = device_of(&principal)?;
    let rows = sqlx::query_as!(
        PinRow,
        "SELECT id, kind, ref_id, created_at FROM device_downloads WHERE device_id = $1 ORDER BY created_at",
        device
    )
    .fetch_all(&st.db)
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(view(&st.db, principal.user.id, row).await?);
    }
    Ok(Json(DeviceDownloadList { items }))
}

#[utoipa::path(
    post,
    path = "/api/devices/current/downloads",
    tag = "offline",
    security(("device" = [])),
    request_body = NewDeviceDownload,
    responses(
        (status = 200, description = "Закреплено (повторный запрос возвращает то же); недостающее на сервере ставится на скачивание", body = DeviceDownload),
        (status = 400, description = "Нет refId или вход не токеном приложения", body = ErrorBody),
        (status = 404, description = "Нет такого трека, альбома, плейлиста или артиста", body = ErrorBody),
    ),
)]
pub async fn add_download(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewDeviceDownload>,
) -> AppResult<Json<DeviceDownload>> {
    let device = device_of(&principal)?;
    let user = principal.user.id;
    let ref_id = match (body.kind, body.ref_id) {
        (DownloadKind::Liked, _) => None,
        (_, None) => return Err(bad("нужен refId")),
        (_, Some(r)) => Some(r),
    };
    if let Some(r) = ref_id {
        let exists = match body.kind {
            DownloadKind::Track => {
                sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM tracks WHERE id = $1) AS "e!""#,
                    r
                )
                .fetch_one(&st.db)
                .await?
            }
            DownloadKind::Album => {
                sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM albums WHERE id = $1) AS "e!""#,
                    r
                )
                .fetch_one(&st.db)
                .await?
            }
            DownloadKind::Playlist => sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM playlists WHERE id = $1 AND user_id = $2) AS "e!""#,
                r,
                user
            )
            .fetch_one(&st.db)
            .await?,
            DownloadKind::Artist => {
                sqlx::query_scalar!(
                    r#"SELECT EXISTS (SELECT 1 FROM artists WHERE id = $1) AS "e!""#,
                    r
                )
                .fetch_one(&st.db)
                .await?
            }
            DownloadKind::Liked => true,
        };
        if !exists {
            return Err(AppError::NotFound);
        }
    }
    let nil = Uuid::nil();
    let row = sqlx::query_as!(
        PinRow,
        "WITH ins AS (
             INSERT INTO device_downloads (device_id, kind, ref_id) VALUES ($1, $2, $3)
             ON CONFLICT (device_id, kind, coalesce(ref_id, '00000000-0000-0000-0000-000000000000')) DO NOTHING
             RETURNING id, kind, ref_id, created_at
         )
         SELECT id AS \"id!\", kind AS \"kind!\", ref_id, created_at AS \"created_at!\" FROM ins
         UNION ALL
         SELECT id, kind, ref_id, created_at FROM device_downloads
         WHERE device_id = $1 AND kind = $2 AND coalesce(ref_id, $4) = coalesce($3, $4)
         LIMIT 1",
        device,
        body.kind.as_str(),
        ref_id,
        nil,
    )
    .fetch_one(&st.db)
    .await?;
    let ids = pin_tracks(&st.db, user, body.kind, ref_id).await?;
    acquire_missing(&st, user, &ids).await;
    Ok(Json(view(&st.db, user, row).await?))
}

#[utoipa::path(
    delete,
    path = "/api/devices/current/downloads/{id}",
    tag = "offline",
    security(("device" = [])),
    params(("id" = Uuid, Path, description = "id закреплённого")),
    responses(
        (status = 204, description = "Откреплено (или уже не было)"),
        (status = 400, description = "Вход не токеном приложения", body = ErrorBody),
    ),
)]
pub async fn remove_download(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let device = device_of(&principal)?;
    sqlx::query!(
        "DELETE FROM device_downloads WHERE id = $1 AND device_id = $2",
        id,
        device
    )
    .execute(&st.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/devices/current/set",
    tag = "offline",
    security(("device" = [])),
    request_body = DeviceSetRequest,
    responses(
        (status = 200, description = "Набор волны устройства пересобран; строки придут синхронизацией (`deviceSets`)", body = DeviceSet),
        (status = 400, description = "Вход не токеном приложения", body = ErrorBody),
    ),
)]
pub async fn rebuild_set(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<DeviceSetRequest>,
) -> AppResult<Json<DeviceSet>> {
    let device = device_of(&principal)?;
    Ok(Json(
        device_set::build(&st, principal.user.id, device, &body).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/devices/current/files",
    tag = "offline",
    security(("device" = [])),
    request_body = FileLinksRequest,
    responses(
        (status = 200, description = "Подписанные ссылки на файлы (6 часов) с размером и хешем; чего нет на сервере — `pending`, оно ставится на скачивание", body = FileLinks),
        (status = 400, description = "Больше 200 треков или вход не токеном приложения", body = ErrorBody),
        (status = 503, description = "Хранилище не подписало ссылку", body = ErrorBody),
    ),
)]
pub async fn files(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<FileLinksRequest>,
) -> AppResult<Json<FileLinks>> {
    device_of(&principal)?;
    if body.track_ids.len() > MAX_FILES {
        return Err(bad(format!("не больше {MAX_FILES} треков за раз")));
    }
    let quality = crate::routes::settings::of(&st.db, principal.user.id)
        .await?
        .quality_download;
    let rows = sqlx::query!(
        r#"SELECT t.id AS track_id, r.id AS recording_id, f.id AS file_id, f.storage_key, f.codec, f.container,
                  f.duration_ms, f.loudness_lufs, f.true_peak_db, f.waveform, f.size_bytes, f.sha256
           FROM unnest($1::uuid[]) WITH ORDINALITY AS x(id, ord)
           JOIN tracks t ON t.id = x.id
           JOIN recordings r ON r.id = t.recording_id
           JOIN audio_files f ON f.id = COALESCE(
               -- «Загрузки: Обычное» — lossless основного файла заменяет лучший файл с потерями
               CASE WHEN $2 = 'normal' AND (SELECT p.lossless FROM audio_files p WHERE p.id = r.primary_file_id)
                    THEN (SELECT id FROM audio_files WHERE recording_id = r.id AND NOT lossless
                          ORDER BY verified DESC, bitrate_kbps DESC NULLS LAST, created_at DESC LIMIT 1) END,
               r.primary_file_id,
               (SELECT id FROM audio_files WHERE recording_id = r.id ORDER BY created_at DESC LIMIT 1))
           WHERE r.audio_status = 'ready'
           ORDER BY x.ord"#,
        &body.track_ids,
        quality.as_str(),
    )
    .fetch_all(&st.db)
    .await?;
    let expires_at = Utc::now() + AUDIO_GET_TTL;
    let mut items = Vec::with_capacity(rows.len());
    for r in rows {
        let (Ok(codec), Ok(container)) = (
            r.codec.parse::<dalywave_protocol::stream::AudioCodec>(),
            r.container
                .parse::<dalywave_protocol::stream::AudioContainer>(),
        ) else {
            continue;
        };
        let url = st
            .storage
            .presign_get(&r.storage_key, AUDIO_GET_TTL)
            .await
            .map_err(|e| {
                AppError::Unavailable(format!("хранилище не подписало ссылку на аудио: {e}"))
            })?;
        items.push(FileLink {
            track_id: r.track_id,
            recording_id: r.recording_id,
            file_id: r.file_id,
            url: url.to_string(),
            expires_at,
            size_bytes: r.size_bytes,
            sha256: r.sha256,
            mime_type: container.mime(codec),
            codec,
            container,
            duration_ms: r.duration_ms,
            loudness_lufs: r.loudness_lufs,
            true_peak_db: r.true_peak_db,
            waveform: r.waveform,
        });
    }
    let have: std::collections::HashSet<Uuid> = items.iter().map(|i| i.track_id).collect();
    let missing: Vec<Uuid> = body
        .track_ids
        .iter()
        .copied()
        .filter(|id| !have.contains(id))
        .collect();
    let pending = if missing.is_empty() {
        Vec::new()
    } else {
        acquire_missing(&st, principal.user.id, &missing).await
    };
    Ok(Json(FileLinks { items, pending }))
}
