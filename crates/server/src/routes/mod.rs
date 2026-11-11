pub mod admin;
pub mod auth;
pub mod catalog;
pub mod events;
pub mod home;
pub mod images;
pub mod library;
pub mod playlists;
pub mod queue;
pub mod sync;
pub mod system;
pub mod ws;

use axum::{
    Router,
    extract::{DefaultBodyLimit, Request},
    http::{HeaderName, HeaderValue, header},
    middleware::from_fn_with_state,
};
use dalywave_protocol::auth::Permission;
use tower_http::{
    compression::CompressionLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::Level;

use crate::{
    auth::{need, require_access},
    error::AppError,
    security, spa,
    state::AppState,
};

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

async fn not_found() -> AppError {
    AppError::NotFound
}

async fn method_not_allowed() -> AppError {
    AppError::MethodNotAllowed
}

/// Весь API под `/api`, остальное — интерфейс из STATIC_DIR.
///
/// Разделы закрыты слоями: каталог — любой ролью (`require_access`), медиатека — правом library,
/// события прослушивания — правом play, админка — admin. Человек без ролей получает 403 `no_access`,
/// как требует экран «ждём доступа». Картинки открыты всем, WebSocket проверяет вход сам.
pub fn router(state: AppState) -> Router {
    let admin = admin::routes()
        .merge(queue::routes())
        .route_layer(from_fn_with_state(state.clone(), need(Permission::Admin)));
    let catalog = catalog::routes().route_layer(from_fn_with_state(state.clone(), require_access));
    let library = library::routes()
        .merge(playlists::routes())
        .merge(home::routes())
        .merge(sync::routes())
        .route_layer(from_fn_with_state(state.clone(), need(Permission::Library)));
    let play =
        events::routes().route_layer(from_fn_with_state(state.clone(), need(Permission::Play)));
    let api = Router::new()
        .merge(system::routes())
        .merge(images::routes())
        .merge(ws::routes())
        .merge(catalog)
        .merge(library)
        .merge(play)
        .nest("/auth", auth::routes())
        .nest("/admin", admin)
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(security::API_BODY_LIMIT))
        .layer(from_fn_with_state(state.clone(), security::guard));

    let app = Router::new().nest("/api", api);
    let app = match &state.config.static_dir {
        Some(dir) => app.fallback_service(spa::router(dir)),
        None => app.fallback(not_found),
    };

    app.layer(CompressionLayer::new())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(PropagateRequestIdLayer::new(REQUEST_ID))
        .layer(
            TraceLayer::new_for_http()
                // Только путь: в query у возврата от daly.rest лежат одноразовые коды
                .make_span_with(|req: &Request| {
                    let id = req.headers().get(REQUEST_ID).and_then(|v| v.to_str().ok()).unwrap_or_default();
                    tracing::info_span!("request", method = %req.method(), path = %req.uri().path(), request_id = %id)
                })
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(SetRequestIdLayer::new(REQUEST_ID, MakeRequestUuid))
        .with_state(state)
}
