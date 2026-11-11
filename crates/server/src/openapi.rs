//! Описание API для `/api/openapi.json`. Из него `make api-types` собирает типы интерфейса,
//! поэтому новый обработчик добавляется и сюда.

use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
};

use crate::routes::{
    acquisitions, admin, auth, catalog, events, fetcher, home, images, library, playlists, queue,
    reco, sources, stream, sync, system, updates, wave, ws,
};

#[derive(OpenApi)]
#[openapi(
    info(title = "DalyWave API", description = "Личный музыкальный сервис DalyWave"),
    paths(
        system::healthz,
        system::openapi,
        updates::check,
        auth::providers,
        auth::oidc_start,
        auth::oidc_callback,
        auth::password_login,
        auth::logout,
        auth::me,
        auth::app_start,
        auth::app_token,
        admin::list_users,
        admin::update_user,
        admin::list_sessions,
        admin::revoke_session,
        admin::audit_log,
        queue::overview,
        queue::list_jobs,
        queue::retry_job,
        queue::cancel_job,
        queue::list_lanes,
        queue::update_lane,
        acquisitions::list,
        acquisitions::resolve,
        sources::list_downloaders,
        sources::create_downloader,
        sources::revoke_downloader,
        sources::sources,
        reco::reco_overview,
        reco::update_reco_settings,
        reco::run_reco,
        catalog::search,
        catalog::artist,
        catalog::album,
        catalog::track,
        catalog::lookup_tracks,
        catalog::resolve_deezer,
        wave::similar,
        wave::sound_search,
        wave::create_session,
        wave::session,
        wave::next,
        wave::feedback,
        images::image,
        library::overview,
        library::liked_tracks,
        library::like,
        library::unlike,
        library::saved_albums,
        library::save_album,
        library::unsave_album,
        library::followed_artists,
        library::follow,
        library::unfollow,
        library::blocked,
        library::block,
        library::unblock,
        playlists::list,
        playlists::create,
        playlists::detail,
        playlists::update,
        playlists::remove,
        playlists::pin,
        playlists::unpin,
        playlists::add_items,
        playlists::remove_item,
        playlists::move_item,
        home::home,
        sync::sync,
        events::ingest,
        events::history,
        stream::stream,
        stream::acquisition,
        fetcher::claim,
        fetcher::renew_lease,
        fetcher::report,
        fetcher::release,
        ws::ws,
    ),
    components(schemas(
        dalywave_protocol::ErrorBody,
        dalywave_protocol::realtime::ServerEvent,
        dalywave_protocol::realtime::ClientMessage,
        dalywave_protocol::jobs::AcquirePayload,
        dalywave_protocol::jobs::MirrorImagePayload,
        dalywave_protocol::jobs::RefreshArtistPayload,
        dalywave_protocol::jobs::SyncDiscographyPayload,
        dalywave_protocol::jobs::AnalyzeFullPayload,
        dalywave_protocol::jobs::AnalyzePreviewPayload,
        dalywave_protocol::jobs::ScoutArtistPayload,
        dalywave_protocol::jobs::TastePayload,
        dalywave_protocol::jobs::AcquireReason,
        dalywave_protocol::stream::AudioCodec,
        dalywave_protocol::stream::AudioContainer,
    )),
    modifiers(&Security),
    tags(
        (name = "auth", description = "Вход и сессии"),
        (name = "catalog", description = "Каталог: поиск, артисты, альбомы, треки, картинки"),
        (name = "library", description = "Медиатека, плейлисты, главная и синхронизация приложений"),
        (name = "events", description = "События прослушивания и история"),
        (name = "playback", description = "Воспроизведение: ссылка на аудио трека и состояние его скачивания"),
        (name = "wave", description = "«Моя волна» и радио: сессии, выдача с причинами, реакции"),
        (name = "fetcher", description = "Домашний загрузчик: задания, аренда, отчёты (токен `dwk_…`)"),
        (name = "realtime", description = "WebSocket: события в реальном времени"),
        (name = "admin", description = "Админка: доступ, журнал, очередь задач"),
        (name = "system", description = "Служебное"),
    ),
)]
pub struct ApiDoc;

struct Security;

impl Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Лицензии у личного проекта нет: пустое поле из Cargo.toml только сбивает генераторы
        openapi.info.license = None;
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                crate::session::SESSION_COOKIE,
                "Сессия сайта (HttpOnly cookie)",
            ))),
        );
        components.add_security_scheme(
            "device",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some("Токен приложения `dwd_…`"))
                    .build(),
            ),
        );
        components.add_security_scheme(
            "downloader",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some(
                        "Токен домашнего загрузчика `dwk_…`: только `/api/fetcher/*`",
                    ))
                    .build(),
            ),
        );
    }
}
