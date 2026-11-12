//! «Поделиться»: карточка ссылки, разбор чужой ссылки в сущность каталога и страницы с превью
//! (`/track/{id}`, `/album/{id}`, `/artist/{id}`, `/playlist/{id}`) для мессенджеров.

use axum::{
    Router,
    extract::State,
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use dalywave_protocol::{
    ErrorBody,
    catalog::{DeezerKind, Track},
    share::{LinkSource, ResolveLinkRequest, ResolvedLink, ShareCard, ShareKind},
};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::{self, Types, views},
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    links::{self, FetchError, Parsed, Wanted},
    share,
    spa::HTML_CSP,
    state::AppState,
};

/// Боевой адрес: его ссылки узнаются и на сервере с другим `PUBLIC_URL` (локально, в тестах).
const PRODUCTION_HOST: &str = "wave.daly.rest";
/// Сколько вариантов из поиска Deezer сверять с названием.
const CANDIDATES: u32 = 10;

/// Под слоем каталога (`require_access`).
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/share/{kind}/{id}", get(card))
        .route("/resolve-link", post(resolve_link))
}

/// Страницы с превью — вне `/api`, без входа. Подключаются, только когда есть собранный интерфейс.
pub fn pages() -> Router<AppState> {
    Router::new()
        .route("/track/{id}", get(track_page))
        .route("/album/{id}", get(album_page))
        .route("/artist/{id}", get(artist_page))
        .route("/playlist/{id}", get(playlist_page))
}

fn unsupported() -> AppError {
    AppError::Unprocessable {
        code: "unsupported_link",
        message: "Эту ссылку не получается открыть".into(),
    }
}

#[utoipa::path(
    get,
    path = "/api/share/{kind}/{id}",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    params(
        ("kind" = ShareKind, Path, description = "track, album, artist или playlist"),
        ("id" = Uuid, Path, description = "id сущности"),
    ),
    responses(
        (status = 200, description = "Ссылка, название, подпись и публичная обложка", body = ShareCard),
        (status = 404, description = "Нет такого (или плейлист чужой)", body = ErrorBody),
    ),
)]
pub async fn card(
    State(st): State<AppState>,
    principal: Principal,
    Path((kind, id)): Path<(ShareKind, Uuid)>,
) -> AppResult<Json<ShareCard>> {
    share::card(
        &st.db,
        &st.config.public_url,
        kind,
        id,
        Some(principal.user.id),
    )
    .await?
    .map(Json)
    .ok_or(AppError::NotFound)
}

#[utoipa::path(
    post,
    path = "/api/resolve-link",
    tag = "catalog",
    security(("session" = []), ("device" = [])),
    request_body = ResolveLinkRequest,
    responses(
        (status = 200, description = "Что нашлось в каталоге по ссылке", body = ResolvedLink),
        (status = 404, description = "Ссылка понятна, но в каталоге такого нет", body = ErrorBody),
        (status = 422, description = "`unsupported_link`: не ссылка на музыку из известных сервисов", body = ErrorBody),
        (status = 503, description = "Сервис по ссылке или Deezer не ответил", body = ErrorBody),
    ),
)]
pub async fn resolve_link(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<ResolveLinkRequest>,
) -> AppResult<Json<ResolvedLink>> {
    if body.url.chars().count() > links::MAX_TEXT {
        return Err(bad("слишком длинный текст"));
    }
    let url = links::extract_url(&body.url).ok_or_else(unsupported)?;
    let public_host = url::Url::parse(&st.config.public_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let own = [public_host.as_str(), PRODUCTION_HOST];
    let mut parsed = links::parse(&url, &own).ok_or_else(unsupported)?;
    if let Parsed::DeezerShort(short) = &parsed {
        let target = st.links.redirect(short).await.map_err(fetch_error)?;
        parsed = match links::parse(&target, &own) {
            Some(p @ Parsed::Deezer { .. }) => p,
            _ => return Err(unsupported()),
        };
    }
    let user = principal.user.id;
    let resolved = match parsed {
        Parsed::Own { kind, id } => own_entity(&st, user, kind, id).await?,
        Parsed::Deezer { kind, id } => deezer_entity(&st, user, kind, id).await?,
        Parsed::DeezerShort(_) => return Err(unsupported()),
        Parsed::Spotify { kind, url } => {
            let (landed, html) = st.links.page(&url).await.map_err(fetch_error)?;
            let kind = match kind {
                Some(k) => k,
                // spotify.link → open.spotify.com/…: вид — по итоговому адресу
                None => match links::parse(&landed, &own) {
                    Some(Parsed::Spotify { kind: Some(k), .. }) => k,
                    _ => return Err(unsupported()),
                },
            };
            let wanted = links::spotify(&html, kind).ok_or(AppError::NotFound)?;
            search(&st, user, LinkSource::Spotify, kind, wanted).await?
        }
        Parsed::Apple { kind, url } => {
            let (_, html) = st.links.page(&url).await.map_err(fetch_error)?;
            let mut wanted = links::apple(&html).ok_or(AppError::NotFound)?;
            if kind == ShareKind::Artist {
                wanted.artist = None;
            }
            search(&st, user, LinkSource::Apple, kind, wanted).await?
        }
        Parsed::Bandcamp { kind, url } => {
            let (_, html) = st.links.page(&url).await.map_err(fetch_error)?;
            let wanted = links::bandcamp(&html).ok_or(AppError::NotFound)?;
            search(&st, user, LinkSource::Bandcamp, kind, wanted).await?
        }
        Parsed::Youtube { url } => {
            let o = st
                .links
                .oembed(links::YOUTUBE_OEMBED, &url)
                .await
                .map_err(fetch_error)?;
            let wanted = links::youtube(&o).ok_or(AppError::NotFound)?;
            search(&st, user, LinkSource::Youtube, ShareKind::Track, wanted).await?
        }
        Parsed::Soundcloud { url } => {
            let o = st
                .links
                .oembed(links::SOUNDCLOUD_OEMBED, &url)
                .await
                .map_err(fetch_error)?;
            let wanted = links::soundcloud(&o).ok_or(AppError::NotFound)?;
            search(&st, user, LinkSource::Soundcloud, ShareKind::Track, wanted).await?
        }
    };
    Ok(Json(resolved))
}

/// 404 у страницы — ссылка битая или приватная: это «не нашлось», а не «сервис упал».
fn fetch_error(e: FetchError) -> AppError {
    match e {
        FetchError::Status(404 | 410) => AppError::NotFound,
        FetchError::Host => unsupported(),
        e => AppError::Unavailable(format!("{e}, попробуйте позже")),
    }
}

fn empty(source: LinkSource, kind: ShareKind, query: Option<String>) -> ResolvedLink {
    ResolvedLink {
        source,
        kind,
        query,
        track: None,
        album: None,
        artist: None,
        playlist_id: None,
    }
}

async fn own_entity(
    st: &AppState,
    user: Uuid,
    kind: ShareKind,
    id: Uuid,
) -> AppResult<ResolvedLink> {
    let mut out = empty(LinkSource::Dalywave, kind, None);
    match kind {
        ShareKind::Track => {
            catalog::ensure_track(&st.db, &st.deezer, id).await?;
            out.track = views::track(&st.db, id, user).await?;
        }
        ShareKind::Album => {
            catalog::ensure_album(&st.db, &st.deezer, id).await?;
            out.album = views::album(&st.db, id, user).await?;
        }
        ShareKind::Artist => out.artist = views::artist(&st.db, id, user).await?,
        ShareKind::Playlist => {
            out.playlist_id = sqlx::query_scalar!(
                "SELECT id FROM playlists WHERE id = $1 AND user_id = $2",
                id,
                user
            )
            .fetch_optional(&st.db)
            .await?;
        }
    }
    found(out)
}

fn found(out: ResolvedLink) -> AppResult<ResolvedLink> {
    let any = out.track.is_some()
        || out.album.is_some()
        || out.artist.is_some()
        || out.playlist_id.is_some();
    if any {
        Ok(out)
    } else {
        Err(AppError::NotFound)
    }
}

async fn deezer_entity(
    st: &AppState,
    user: Uuid,
    kind: DeezerKind,
    deezer_id: i64,
) -> AppResult<ResolvedLink> {
    let out = match kind {
        DeezerKind::Track => {
            let id = catalog::resolve_track(&st.db, &st.deezer, deezer_id).await?;
            ResolvedLink {
                track: views::track(&st.db, id, user).await?,
                ..empty(LinkSource::Deezer, ShareKind::Track, None)
            }
        }
        DeezerKind::Album => {
            let id = catalog::resolve_album(&st.db, &st.deezer, deezer_id).await?;
            ResolvedLink {
                album: views::album(&st.db, id, user).await?,
                ..empty(LinkSource::Deezer, ShareKind::Album, None)
            }
        }
        DeezerKind::Artist => {
            let id = catalog::resolve_artist(&st.db, &st.deezer, deezer_id).await?;
            ResolvedLink {
                artist: views::artist(&st.db, id, user).await?,
                ..empty(LinkSource::Deezer, ShareKind::Artist, None)
            }
        }
    };
    found(out)
}

fn track_artists(t: &Track) -> Vec<&str> {
    t.artists.iter().map(|a| a.name.as_str()).collect()
}

/// Поиск в Deezer по названию и исполнителю; берётся первый вариант, у которого сходится название
/// (и исполнитель, если он известен). Не сошлось — 404: молча подставить чужой трек хуже.
async fn search(
    st: &AppState,
    user: Uuid,
    source: LinkSource,
    kind: ShareKind,
    wanted: Wanted,
) -> AppResult<ResolvedLink> {
    if wanted.title.trim().is_empty() {
        return Err(AppError::NotFound);
    }
    let query = wanted.query();
    let types = Types {
        track: kind == ShareKind::Track,
        album: kind == ShareKind::Album,
        artist: kind == ShareKind::Artist,
    };
    if kind == ShareKind::Playlist {
        return Err(unsupported());
    }
    let hits = catalog::search(&st.db, &st.deezer, &query, types, CANDIDATES).await?;
    let mut out = empty(source, kind, Some(query));
    let artist_ok = |names: &[&str]| {
        wanted
            .artist
            .as_deref()
            .is_none_or(|a| links::artist_matches(a, names))
    };
    match kind {
        ShareKind::Track => {
            let tracks = views::tracks(&st.db, &hits.tracks, user).await?;
            let matching: Vec<&Track> = tracks
                .iter()
                .filter(|t| {
                    links::title_matches(&wanted.title, &t.title) && artist_ok(&track_artists(t))
                })
                .collect();
            // Та же версия первой: у «Take On Me» без пометок есть и «(2017 Acoustic)» выше в поиске
            let wanted_full = links::normalize(&wanted.title);
            out.track = matching
                .iter()
                .find(|t| links::normalize(&format!("{} {}", t.title, t.version)) == wanted_full)
                .or_else(|| matching.first())
                .copied()
                // «Артист - Название» на YouTube бывает и наоборот
                .or_else(|| {
                    let artist = wanted.artist.as_deref()?;
                    tracks.iter().find(|t| {
                        links::title_matches(artist, &t.title)
                            && links::artist_matches(&wanted.title, &track_artists(t))
                    })
                })
                .cloned();
        }
        ShareKind::Album => {
            let albums = views::albums(&st.db, &hits.albums, user).await?;
            out.album = albums.into_iter().find(|a| {
                links::title_matches(&wanted.title, &a.title)
                    && artist_ok(&[a.artist.name.as_str()])
            });
        }
        ShareKind::Artist => {
            let artists = views::artists(&st.db, &hits.artists, user).await?;
            out.artist = artists
                .into_iter()
                .find(|a| links::artist_matches(&wanted.title, &[a.name.as_str()]));
        }
        ShareKind::Playlist => return Err(unsupported()),
    }
    found(out)
}

// ---- страницы с превью ----

async fn track_page(st: State<AppState>, id: axum::extract::Path<String>) -> Response {
    page(st, ShareKind::Track, id).await
}

async fn album_page(st: State<AppState>, id: axum::extract::Path<String>) -> Response {
    page(st, ShareKind::Album, id).await
}

async fn artist_page(st: State<AppState>, id: axum::extract::Path<String>) -> Response {
    page(st, ShareKind::Artist, id).await
}

async fn playlist_page(st: State<AppState>, id: axum::extract::Path<String>) -> Response {
    page(st, ShareKind::Playlist, id).await
}

/// index.html интерфейса с тегами превью. Неизвестный id — index.html как есть: «не найдено»
/// покажет сам интерфейс.
async fn page(
    State(st): State<AppState>,
    kind: ShareKind,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let Some(dir) = st.config.static_dir.as_ref() else {
        return AppError::NotFound.into_response();
    };
    let index = match tokio::fs::read_to_string(dir.join("index.html")).await {
        Ok(index) => index,
        Err(e) => {
            tracing::warn!(error = %e, "index.html is missing");
            return AppError::NotFound.into_response();
        }
    };
    let card = match id.parse::<Uuid>() {
        Ok(id) => share::card(&st.db, &st.config.public_url, kind, id, None)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "share card for preview failed");
                None
            }),
        Err(_) => None,
    };
    let html = match &card {
        Some(card) => share::inject(&index, card),
        None => index,
    };
    let mut res = Html(html).into_response();
    let headers = res.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(HTML_CSP),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    res
}
