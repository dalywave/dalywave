//! Встраиваемый плеер: страница `/embed/{kind}/{id}` и отрывки без входа, «Можно встраивать» у
//! плейлиста. Устройство страницы и отрывков — в `crate::embed`.

use std::time::Duration;

use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use base64::Engine;
use dalywave_protocol::{
    ErrorBody,
    export::{EmbedClip, EmbedSettings},
    share::ShareKind,
    sync::SyncEntity,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::Principal,
    embed::{self, ClipAnswer, ClipError, Lang, Theme},
    error::{AppError, AppResult},
    extract::{Json, Path},
    routes::playlists::{changed_playlist, lock_any, owner_only},
    state::{AppState, ClientIp},
};

/// Страниц плеера с одного адреса в минуту (альбом без списка треков спрашивает Deezer).
const PAGES_PER_MINUTE: u32 = 240;
/// Отрывков с одного адреса в минуту.
const CLIPS_PER_MINUTE: u32 = 60;

/// Страницы плеера — вне `/api`, без входа, всегда (и без собранного интерфейса).
pub fn pages() -> Router<AppState> {
    Router::new().route("/embed/{kind}/{id}", get(page))
}

/// Отрывки — под `/api`, без входа (как картинки).
pub fn public_routes() -> Router<AppState> {
    Router::new().route("/embed/tracks/{id}/clip", get(embed_clip))
}

/// «Можно встраивать» — под слоем медиатеки.
pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/playlists/{id}/embed",
        get(embed_settings).put(update_embed_settings),
    )
}

#[derive(Debug, Default, Deserialize)]
pub struct PageQuery {
    theme: Option<String>,
    lang: Option<String>,
}

fn nonce() -> String {
    // Без случайности скрипт просто не запустится: CSP не пропустит пустой nonce
    crate::crypto::random_bytes::<16>()
        .map(|b| base64::engine::general_purpose::STANDARD_NO_PAD.encode(b))
        .unwrap_or_default()
}

fn html(status: StatusCode, body: String, nonce: &str, cache: &'static str) -> Response {
    let mut res = (status, Html(body)).into_response();
    let headers = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&embed::csp(nonce)) {
        headers.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    res
}

async fn page(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    axum::extract::Path((kind, id)): axum::extract::Path<(String, String)>,
    axum::extract::Query(q): axum::extract::Query<PageQuery>,
) -> Response {
    let theme = Theme::parse(q.theme.as_deref());
    let lang = Lang::parse(q.lang.as_deref());
    let nonce = nonce();
    let gone = || {
        html(
            StatusCode::NOT_FOUND,
            embed::render_gone(theme, lang),
            &nonce,
            "no-cache",
        )
    };
    let (Ok(kind), Ok(id)) = (kind.parse::<ShareKind>(), id.parse::<Uuid>()) else {
        return gone();
    };
    if !st
        .limiter
        .check("embed_page", ip, PAGES_PER_MINUTE, Duration::from_secs(60))
    {
        return AppError::TooManyRequests.into_response();
    }
    match embed::load(&st.db, &st.deezer, &st.config.public_url, kind, id, lang).await {
        Ok(Some(card)) => html(
            StatusCode::OK,
            embed::render(&card, theme, lang, &nonce),
            &nonce,
            // Недолго: снятое «Можно встраивать» и новые треки доходят за пару минут
            "public, max-age=120",
        ),
        Ok(None) => gone(),
        Err(e) => {
            tracing::warn!(error = %e, "embed page failed");
            AppError::Db(e).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/embed/tracks/{id}/clip",
    tag = "catalog",
    params(("id" = Uuid, Path, description = "id трека")),
    responses(
        (status = 200, description = "Отрывок 30 с для встраиваемого плеера: с припева (`chorus`) или превью \
                                      Deezer (`preview`). Без входа; ссылка подписана и живёт час", body = EmbedClip),
        (status = 404, description = "Нет такого трека или отрывка у него нет", body = ErrorBody),
        (status = 429, description = "Слишком много отрывков с одного адреса", body = ErrorBody),
        (status = 503, description = "`clip_preparing`: отрывок нарезается — спросить ещё раз позже; или Deezer не ответил", body = ErrorBody),
    ),
)]
pub async fn embed_clip(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if !st
        .limiter
        .check("embed_clip", ip, CLIPS_PER_MINUTE, Duration::from_secs(60))
    {
        return Err(AppError::TooManyRequests);
    }
    let answer = embed::clip(&st.db, &st.storage, &st.deezer, id)
        .await
        .map_err(|e| match e {
            ClipError::Db(e) => AppError::Db(e),
            e => AppError::Unavailable(e.to_string()),
        })?;
    match answer {
        ClipAnswer::Ready(clip) => Ok((
            // Ссылка подписана и живёт час: ответ не кешируется
            [(header::CACHE_CONTROL, "no-store")],
            Json(clip),
        )
            .into_response()),
        ClipAnswer::Preparing => Err(AppError::Unavailable(
            "отрывок готовится, попробуйте чуть позже".into(),
        )),
        ClipAnswer::NotFound => Err(AppError::NotFound),
    }
}

#[utoipa::path(
    get,
    path = "/api/playlists/{id}/embed",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    responses(
        (status = 200, description = "Можно ли встраивать плейлист на чужие сайты", body = EmbedSettings),
        (status = 404, description = "Нет такого плейлиста или он чужой", body = ErrorBody),
    ),
)]
pub async fn embed_settings(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<EmbedSettings>> {
    let embeddable = sqlx::query_scalar!(
        "SELECT p.embeddable FROM playlists p
         WHERE p.id = $1 AND (p.user_id = $2 OR (p.collaborative AND EXISTS (
             SELECT 1 FROM playlist_members m WHERE m.playlist_id = p.id AND m.user_id = $2)))",
        id,
        principal.user.id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(EmbedSettings { embeddable }))
}

#[utoipa::path(
    put,
    path = "/api/playlists/{id}/embed",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id плейлиста")),
    request_body = EmbedSettings,
    responses(
        (status = 200, description = "Сохранено: плейлист открывается во встраиваемом плеере без входа (или нет)", body = EmbedSettings),
        (status = 403, description = "Не свой плейлист", body = ErrorBody),
        (status = 404, description = "Нет такого плейлиста", body = ErrorBody),
    ),
)]
pub async fn update_embed_settings(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(body): Json<EmbedSettings>,
) -> AppResult<Json<EmbedSettings>> {
    let user = principal.user.id;
    let mut tx = st.db.begin().await?;
    if !lock_any(&mut tx, user, id).await?.is_owner(user) {
        return Err(owner_only());
    }
    sqlx::query!(
        "UPDATE playlists SET embeddable = $2 WHERE id = $1",
        id,
        body.embeddable
    )
    .execute(&mut *tx)
    .await?;
    changed_playlist(&mut tx, id, vec![SyncEntity::Playlists]).await;
    tx.commit().await?;
    Ok(Json(body))
}
