//! Настройки слушателя, общие для всех его устройств: «Без нецензурного», качество звука по сети и
//! порядок плейлистов. Строки нет — всё по умолчанию; первая правка её создаёт.

use axum::{Router, extract::State, routing::get};
use dalywave_protocol::{
    ErrorBody,
    realtime::{LibraryChange, ServerEvent},
    settings::{
        CellularQuality, DownloadQuality, PlaylistSort, UserSettings, UserSettingsPatch,
        WifiQuality,
    },
    sync::SyncEntity,
};
use sqlx::PgExecutor;
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::AppResult,
    extract::Json,
    realtime::{self, Audience},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/settings", get(get_settings).patch(update))
}

/// Настройки человека; строки нет — значения по умолчанию.
pub async fn of(db: impl PgExecutor<'_>, user: Uuid) -> Result<UserSettings, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT hide_explicit, quality_wifi, quality_cellular, quality_download, playlist_sort, updated_at
         FROM user_settings WHERE user_id = $1",
        user,
    )
    .fetch_optional(db)
    .await?;
    Ok(match row {
        None => UserSettings::default(),
        Some(r) => UserSettings {
            hide_explicit: r.hide_explicit,
            quality_wifi: r.quality_wifi.parse().unwrap_or(WifiQuality::High),
            quality_cellular: r
                .quality_cellular
                .parse()
                .unwrap_or(CellularQuality::Normal),
            quality_download: r.quality_download.parse().unwrap_or(DownloadQuality::High),
            playlist_sort: r.playlist_sort.parse().unwrap_or(PlaylistSort::Recent),
            updated_at: Some(r.updated_at),
        },
    })
}

#[utoipa::path(
    get,
    path = "/api/settings",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Настройки слушателя (общие для всех устройств)", body = UserSettings)),
)]
pub async fn get_settings(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<UserSettings>> {
    Ok(Json(of(&st.db, principal.user.id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/settings",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = UserSettingsPatch,
    responses(
        (status = 200, description = "Настройки после правки; другие устройства получат `library.changed`", body = UserSettings),
        (status = 400, description = "Неизвестное поле или значение", body = ErrorBody),
    ),
)]
pub async fn update(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<UserSettingsPatch>,
) -> AppResult<Json<UserSettings>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "INSERT INTO user_settings (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
        user
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE user_settings SET
             hide_explicit = COALESCE($2, hide_explicit),
             quality_wifi = COALESCE($3, quality_wifi),
             quality_cellular = COALESCE($4, quality_cellular),
             quality_download = COALESCE($5, quality_download),
             playlist_sort = COALESCE($6, playlist_sort),
             updated_at = now()
         WHERE user_id = $1",
        user,
        patch.hide_explicit,
        patch.quality_wifi.map(WifiQuality::as_str),
        patch.quality_cellular.map(CellularQuality::as_str),
        patch.quality_download.map(DownloadQuality::as_str),
        patch.playlist_sort.map(PlaylistSort::as_str),
    )
    .execute(&mut *tx)
    .await?;
    let settings = of(&mut *tx, user).await?;
    realtime::publish(
        &mut *tx,
        Audience::User(user),
        ServerEvent::LibraryChanged(LibraryChange {
            entities: vec![SyncEntity::Settings],
        }),
    )
    .await;
    tx.commit().await?;
    Ok(Json(settings))
}
