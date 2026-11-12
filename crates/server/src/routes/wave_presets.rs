//! Сохранённые сочетания настроек волны: карточки под «Моей волной». Под них же держатся заготовки
//! волны ([`crate::reco::prep`]). Строки синхронизируются с приложениями, как медиатека.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, put},
};
use chrono::{DateTime, Utc};
use dalywave_protocol::{
    ErrorBody,
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
    wave::{
        NewWavePreset, WavePreset, WavePresetList, WavePresetOrder, WavePresetPatch, WaveSettings,
    },
};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    realtime::{self, Audience},
    routes::playlists::STEP,
    state::AppState,
};

/// Больше — ряд под волной превращается в склад.
pub const MAX_PRESETS: i64 = 24;
const MAX_NAME: usize = 40;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/wave/presets", get(list).post(create))
        .route("/wave/presets/order", put(reorder))
        .route(
            "/wave/presets/{id}",
            axum::routing::patch(update).delete(remove),
        )
}

struct Row {
    id: Uuid,
    name: String,
    icon: String,
    settings: Value,
    position: f64,
    created_at: DateTime<Utc>,
}

impl From<Row> for WavePreset {
    fn from(r: Row) -> Self {
        Self {
            id: r.id,
            name: r.name,
            icon: r.icon,
            settings: serde_json::from_value(r.settings).unwrap_or_default(),
            position: r.position,
            created_at: r.created_at,
        }
    }
}

async fn rows(db: &PgPool, user: Uuid) -> Result<Vec<WavePreset>, sqlx::Error> {
    Ok(sqlx::query_as!(
        Row,
        "SELECT id, name, icon, settings, position, created_at FROM wave_presets
         WHERE user_id = $1 ORDER BY position, id",
        user,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(WavePreset::from)
    .collect())
}

async fn one(db: &PgPool, user: Uuid, id: Uuid) -> AppResult<WavePreset> {
    sqlx::query_as!(
        Row,
        "SELECT id, name, icon, settings, position, created_at FROM wave_presets
         WHERE id = $1 AND user_id = $2",
        id,
        user,
    )
    .fetch_optional(db)
    .await?
    .map(WavePreset::from)
    .ok_or(AppError::NotFound)
}

/// Настройки сочетаний человека по порядку карточек — для заготовок волны.
pub async fn settings_of(db: &PgPool, user: Uuid) -> Result<Vec<WaveSettings>, sqlx::Error> {
    Ok(rows(db, user)
        .await?
        .into_iter()
        .map(|p| p.settings)
        .collect())
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(bad("название сочетания — от 1 до 40 символов"));
    }
    Ok(name)
}

fn clean_icon(icon: &str) -> AppResult<String> {
    let ok = (1..=40).contains(&icon.len())
        && icon
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !ok {
        return Err(bad(
            "иконка — имя Phosphor без префикса: laptop, moon-stars",
        ));
    }
    Ok(icon.to_string())
}

fn clean_settings(s: WaveSettings) -> AppResult<Value> {
    if s.seed.is_some() {
        return Err(bad("радио не сохраняется в сочетание"));
    }
    if s.tempo.is_some_and(|t| !(100..=220).contains(&t)) {
        return Err(bad("темп — от 100 до 220 шагов в минуту"));
    }
    Ok(serde_json::to_value(&s).map_err(anyhow::Error::from)?)
}

async fn changed(conn: &mut PgConnection, user: Uuid) {
    realtime::publish(
        conn,
        Audience::User(user),
        ServerEvent::LibraryChanged(LibraryChange {
            entities: vec![SyncEntity::WavePresets],
        }),
    )
    .await;
}

#[utoipa::path(
    get,
    path = "/api/wave/presets",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Сохранённые сочетания по порядку карточек", body = WavePresetList)),
)]
pub async fn list(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<WavePresetList>> {
    Ok(Json(WavePresetList {
        items: rows(&st.db, principal.user.id).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/api/wave/presets",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    request_body = NewWavePreset,
    responses(
        (status = 201, description = "Сочетание сохранено в конец ряда", body = WavePreset),
        (status = 400, description = "Пустое или длинное название, неверная иконка, радио в настройках, \
                                      сочетаний уже 24", body = ErrorBody),
    ),
)]
pub async fn create(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<NewWavePreset>,
) -> AppResult<(StatusCode, Json<WavePreset>)> {
    let user = principal.user.id;
    let name = clean_name(&body.name)?;
    let icon = clean_icon(&body.icon)?;
    let settings = clean_settings(body.settings)?;
    let mut tx = st.db.begin().await?;
    // Сохранения одного человека — по очереди: предел и место в конце считаются без гонок
    sqlx::query!("SELECT id FROM users WHERE id = $1 FOR UPDATE", user)
        .fetch_one(&mut *tx)
        .await?;
    let have = sqlx::query!(
        r#"SELECT count(*) AS "n!", max(position) AS last FROM wave_presets WHERE user_id = $1"#,
        user,
    )
    .fetch_one(&mut *tx)
    .await?;
    if have.n >= MAX_PRESETS {
        return Err(bad("сочетаний не больше 24 — удалите ненужное"));
    }
    let position = have.last.unwrap_or(0.0) + STEP;
    let id = sqlx::query_scalar!(
        "INSERT INTO wave_presets (user_id, name, icon, settings, position) VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
        user,
        name,
        icon,
        settings,
        position,
    )
    .fetch_one(&mut *tx)
    .await?;
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(one(&st.db, user, id).await?)))
}

#[utoipa::path(
    patch,
    path = "/api/wave/presets/{id}",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id сочетания")),
    request_body = WavePresetPatch,
    responses(
        (status = 200, description = "Сочетание после правки", body = WavePreset),
        (status = 400, description = "Пустое или длинное название, неверная иконка, радио в настройках", body = ErrorBody),
        (status = 404, description = "Нет такого сочетания", body = ErrorBody),
    ),
)]
pub async fn update(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(patch): Json<WavePresetPatch>,
) -> AppResult<Json<WavePreset>> {
    let user = principal.user.id;
    let name = patch.name.as_deref().map(clean_name).transpose()?;
    let icon = patch.icon.as_deref().map(clean_icon).transpose()?;
    let settings = patch.settings.map(clean_settings).transpose()?;
    let mut tx = st.db.begin().await?;
    let updated = sqlx::query!(
        "UPDATE wave_presets SET name = COALESCE($3, name), icon = COALESCE($4, icon),
                settings = COALESCE($5, settings)
         WHERE id = $1 AND user_id = $2",
        id,
        user,
        name,
        icon,
        settings,
    )
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(Json(one(&st.db, user, id).await?))
}

#[utoipa::path(
    delete,
    path = "/api/wave/presets/{id}",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id сочетания")),
    responses(
        (status = 204, description = "Сочетание удалено"),
        (status = 404, description = "Нет такого сочетания", body = ErrorBody),
    ),
)]
pub async fn remove(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let deleted = sqlx::query!(
        "DELETE FROM wave_presets WHERE id = $1 AND user_id = $2",
        id,
        user
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    changed(&mut tx, user).await;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Новый порядок: названные — первыми, остальные — после, как были. Чужие и неизвестные id
/// пропускаются: порядок мог прийти из устаревшего экрана.
pub fn arrange(current: &[Uuid], wanted: &[Uuid]) -> Vec<Uuid> {
    let mut out: Vec<Uuid> = Vec::with_capacity(current.len());
    for id in wanted {
        if current.contains(id) && !out.contains(id) {
            out.push(*id);
        }
    }
    for id in current {
        if !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

#[utoipa::path(
    put,
    path = "/api/wave/presets/order",
    tag = "wave",
    security(("session" = []), ("device" = [])),
    request_body = WavePresetOrder,
    responses((status = 200, description = "Сочетания в новом порядке", body = WavePresetList)),
)]
pub async fn reorder(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<WavePresetOrder>,
) -> AppResult<Json<WavePresetList>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    let current = sqlx::query!(
        "SELECT id, position FROM wave_presets WHERE user_id = $1 ORDER BY position, id FOR UPDATE",
        user,
    )
    .fetch_all(&mut *tx)
    .await?;
    let ids: Vec<Uuid> = current.iter().map(|r| r.id).collect();
    let order = arrange(&ids, &body.ids);
    let mut moved = false;
    for (i, id) in order.iter().enumerate() {
        let position = (i + 1) as f64 * STEP;
        // Строка без перемены места не трогается: её `rev` не растёт, синхронизация не гоняет её зря
        if current
            .iter()
            .any(|r| r.id == *id && r.position == position)
        {
            continue;
        }
        sqlx::query!(
            "UPDATE wave_presets SET position = $2 WHERE id = $1",
            id,
            position
        )
        .execute(&mut *tx)
        .await?;
        moved = true;
    }
    if moved {
        changed(&mut tx, user).await;
    }
    tx.commit().await?;
    Ok(Json(WavePresetList {
        items: rows(&st.db, user).await?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_puts_named_first_and_keeps_the_rest() {
        let [a, b, c, d] = [1, 2, 3, 4].map(Uuid::from_u128);
        let stranger = Uuid::from_u128(9);
        assert_eq!(arrange(&[a, b, c, d], &[c, a]), vec![c, a, b, d]);
        assert_eq!(arrange(&[a, b, c], &[stranger, b, b]), vec![b, a, c]);
        assert_eq!(arrange(&[a, b], &[]), vec![a, b]);
    }

    #[test]
    fn names_and_icons_are_checked() {
        assert_eq!(clean_name("  Ночь   за  рулём ").unwrap(), "Ночь за рулём");
        assert!(clean_name("   ").is_err());
        assert!(clean_name(&"я".repeat(41)).is_err());
        assert!(clean_icon("moon-stars").is_ok());
        assert!(clean_icon("ph:moon").is_err());
        assert!(clean_icon("").is_err());
    }
}
