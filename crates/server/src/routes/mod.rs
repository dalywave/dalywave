pub mod acquisitions;
pub mod admin;
pub mod admin_catalog;
pub mod analysis;
pub mod auth;
pub mod catalog;
pub mod collab;
pub mod connect;
pub mod describe;
pub mod events;
pub mod fetcher;
pub mod friends;
pub mod home;
pub mod images;
pub mod imports;
pub mod info;
pub mod jam;
pub mod library;
pub mod lyrics;
pub mod notifications;
pub mod offline;
pub mod overlay;
pub mod playlist_folders;
pub mod playlist_set;
pub mod playlists;
pub mod queue;
pub mod reco;
pub mod recognize;
pub mod rewind;
pub mod settings;
pub mod share;
pub mod sources;
pub mod stream;
pub mod sync;
pub mod system;
pub mod taste;
pub mod tv;
pub mod updates;
pub mod versions;
pub mod video;
pub mod wave;
pub mod wave_presets;
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
/// события прослушивания, поток трека и волна — правом play, админка — admin. Человек без ролей получает
/// 403 `no_access`, как требует экран «ждём доступа». Картинки открыты всем, WebSocket проверяет вход
/// сам, `/api/fetcher/*` — только токен загрузчика `dwk_…` (право `downloader`), `/api/releases*` —
/// только токен конвейера выпусков (`DALYWAVE_RELEASE_TOKEN`).
pub fn router(state: AppState) -> Router {
    let admin = admin::routes()
        .merge(queue::routes())
        .merge(acquisitions::routes())
        .merge(sources::routes())
        .merge(reco::routes())
        .merge(admin_catalog::routes())
        .merge(rewind::admin_routes())
        .route_layer(from_fn_with_state(state.clone(), need(Permission::Admin)));
    let catalog = catalog::routes()
        .merge(wave::catalog_routes())
        .merge(lyrics::routes())
        .merge(share::routes())
        .merge(info::routes())
        .route_layer(from_fn_with_state(state.clone(), require_access));
    let library = library::routes()
        .merge(playlists::routes())
        .merge(playlist_folders::routes())
        .merge(playlist_set::routes())
        .merge(collab::routes())
        .merge(friends::routes())
        .merge(home::routes())
        .merge(taste::routes())
        .merge(rewind::routes())
        .merge(recognize::routes())
        .merge(sync::routes())
        .merge(settings::routes())
        .merge(imports::routes())
        .merge(notifications::routes())
        .route_layer(from_fn_with_state(state.clone(), need(Permission::Library)));
    let play = events::routes()
        .merge(stream::routes())
        .merge(analysis::routes())
        .merge(wave::routes())
        .merge(wave_presets::routes())
        .merge(describe::routes())
        .merge(connect::routes())
        .merge(offline::routes())
        .merge(jam::host_routes())
        .merge(info::play_routes())
        .merge(versions::routes())
        .merge(tv::party_routes())
        .merge(overlay::routes())
        .merge(video::routes())
        .route_layer(from_fn_with_state(state.clone(), need(Permission::Play)));
    let api = Router::new()
        .merge(system::routes())
        .merge(updates::routes())
        .merge(images::routes())
        // Оверлей для стрима: по секретной ссылке `dwo_…`, без входа
        .merge(overlay::public_routes())
        .merge(ws::routes())
        // «Слушать вместе»: гости без аккаунта — свой токен `dwj_…`, проверка в каждом обработчике
        .merge(jam::guest_routes())
        .merge(catalog)
        .merge(library)
        .merge(play)
        .merge(fetcher::routes())
        .merge(video::fetcher_routes())
        .nest("/auth", auth::routes().merge(tv::auth_routes()))
        .nest("/admin", admin)
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(security::API_BODY_LIMIT))
        .layer(from_fn_with_state(state.clone(), security::guard))
        // Снаружи защитного слоя: preflight отвечается сразу, а заголовки CORS есть и у ошибок
        .layer(security::cors(&state.config));

    let app = Router::new().nest("/api", api);
    let app = match &state.config.static_dir {
        // Страницы трека, альбома, артиста и плейлиста — с тегами превью для мессенджеров
        Some(dir) => app.merge(share::pages()).fallback_service(spa::router(dir)),
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
