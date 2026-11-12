//! Импорт музыки: архив данных Spotify, списки и свои файлы. Загрузки идут потоком во временный файл
//! с пределом размера (тело — сам файл, не JSON: такие запросы браузер шлёт только после preflight,
//! а CORS для чужих сайтов закрыт). Сопоставление и раскладка — в фоне ([`crate::import`]); интерфейс
//! спрашивает импорт, пока он идёт.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    routing::{get, post, put},
};
use chrono::{Datelike, Utc};
use dalywave_protocol::{
    ErrorBody,
    imports::{
        FilesImportRequest, ImportItem, ImportItemFix, ImportItemKind, ImportItemPage,
        ImportItemsQuery, ImportKind, ImportList, ImportPhase, ImportSummary, ImportTarget,
        ListFormat, ListImportRequest, UploadQuery, YandexImportRequest, YoutubeImportRequest,
    },
};
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog, cursor,
    error::{AppError, AppResult, bad},
    extract::{Json, Path, Query},
    import::{
        self, NewItem, Options,
        collection::Collection,
        files, lists,
        spotify::{self, ArchiveError, Limits},
        yandex::{self, Yandex, YandexError},
        youtube::{self, YoutubeError},
    },
    jobs as queue,
    state::AppState,
};

/// Списки до 20 000 строк: CSV такого размера — пара мегабайт.
const LIST_BODY_LIMIT: usize = 4 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/imports", get(list))
        .route(
            "/imports/spotify",
            post(upload_spotify).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/imports/list",
            post(import_list).layer(DefaultBodyLimit::max(LIST_BODY_LIMIT)),
        )
        .route("/imports/yandex", post(import_yandex))
        .route("/imports/youtube", post(import_youtube))
        .route(
            "/imports/youtube/takeout",
            post(upload_takeout).layer(DefaultBodyLimit::disable()),
        )
        .route("/imports/files", post(start_files))
        .route("/imports/{id}", get(one).delete(remove))
        .route("/imports/{id}/items", get(items))
        .route("/imports/{id}/items/{item_id}", put(fix_item))
        .route(
            "/imports/{id}/files",
            put(upload_file).layer(DefaultBodyLimit::disable()),
        )
        .route("/imports/{id}/finish", post(finish_files))
}

/// Тело запроса — во временный файл, не больше `limit`. Первые байты — чтобы узнать формат.
async fn receive(
    headers: &HeaderMap,
    body: Body,
    limit: u64,
    dir: &FsPath,
) -> AppResult<(PathBuf, u64, Vec<u8>)> {
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > limit) {
        return Err(AppError::PayloadTooLarge);
    }
    let path = dir.join("upload");
    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let mut stream = body.into_data_stream();
    let mut size = 0u64;
    let mut head = Vec::with_capacity(64);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| bad("загрузка прервалась — попробуйте ещё раз"))?;
        size += chunk.len() as u64;
        if size > limit {
            return Err(AppError::PayloadTooLarge);
        }
        if head.len() < 64 {
            let take = (64 - head.len()).min(chunk.len());
            head.extend_from_slice(&chunk[..take]);
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
    }
    file.flush()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if size == 0 {
        return Err(bad("файл пустой"));
    }
    Ok((path, size, head))
}

fn temp_dir() -> AppResult<tempfile::TempDir> {
    tempfile::tempdir().map_err(|e| AppError::Internal(e.into()))
}

/// «11 ноября» — название импорта по умолчанию.
fn today_ru() -> String {
    const MONTHS: [&str; 12] = [
        "января",
        "февраля",
        "марта",
        "апреля",
        "мая",
        "июня",
        "июля",
        "августа",
        "сентября",
        "октября",
        "ноября",
        "декабря",
    ];
    let d = Utc::now().date_naive();
    format!("{} {}", d.day(), MONTHS[d.month0() as usize])
}

async fn owned(
    st: &AppState,
    user: Uuid,
    id: Uuid,
) -> AppResult<(ImportKind, String, ImportPhase)> {
    let row = sqlx::query!(
        "SELECT kind, status, phase FROM imports WHERE id = $1 AND user_id = $2",
        id,
        user
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok((
        row.kind.parse().unwrap_or(ImportKind::Text),
        row.status,
        row.phase.parse().unwrap_or(ImportPhase::Done),
    ))
}

#[utoipa::path(
    get,
    path = "/api/imports",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Свои импорты, новые первыми (до 50)", body = ImportList)),
)]
pub async fn list(State(st): State<AppState>, principal: Principal) -> AppResult<Json<ImportList>> {
    Ok(Json(ImportList {
        items: import::summaries(&st.db, Some(principal.user.id), None, 50).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/api/imports/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id импорта")),
    responses(
        (status = 200, description = "Импорт: шаг, прогресс, итог", body = ImportSummary),
        (status = 404, description = "Нет такого импорта", body = ErrorBody),
    ),
)]
pub async fn one(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ImportSummary>> {
    Ok(Json(import::summary(&st.db, principal.user.id, id).await?))
}

#[utoipa::path(
    post,
    path = "/api/imports/spotify",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(UploadQuery),
    request_body(content = Vec<u8>, content_type = "application/zip", description = "Архив `my_spotify_data.zip` (или один JSON из него), до 500 МБ"),
    responses(
        (status = 202, description = "Архив разобран, сопоставление идёт в фоне", body = ImportSummary),
        (status = 400, description = "Не архив Spotify или он повреждён", body = ErrorBody),
        (status = 409, description = "Импорт архива уже идёт", body = ErrorBody),
        (status = 413, description = "Больше 500 МБ", body = ErrorBody),
    ),
)]
pub async fn upload_spotify(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> AppResult<(StatusCode, Json<ImportSummary>)> {
    let user = principal.user.id;
    let busy = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM imports WHERE user_id = $1 AND kind = 'spotify_archive'
                          AND status = 'running') AS "busy!""#,
        user
    )
    .fetch_one(&st.db)
    .await?;
    if busy {
        return Err(AppError::Conflict {
            code: "import_running",
            message: "Архив уже переносится — дождитесь конца или отмените его".into(),
        });
    }
    let dir = temp_dir()?;
    let (path, size, head) = receive(&headers, body, import::MAX_ARCHIVE_BYTES, dir.path()).await?;
    let name = q
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "my_spotify_data.zip".into());
    let parse_name = name.clone();
    let archive = tokio::task::spawn_blocking(move || -> Result<spotify::Archive, ArchiveError> {
        if head.starts_with(b"PK") {
            let file =
                std::fs::File::open(&path).map_err(|e| ArchiveError::Broken(e.to_string()))?;
            spotify::parse_zip(std::io::BufReader::new(file), Limits::default())
        } else if head
            .iter()
            .find(|b| !b.is_ascii_whitespace())
            .is_some_and(|b| *b == b'[' || *b == b'{')
        {
            if size > Limits::default().max_entry_bytes {
                return Err(ArchiveError::EntryTooLarge(parse_name));
            }
            let bytes = std::fs::read(&path).map_err(|e| ArchiveError::Broken(e.to_string()))?;
            spotify::parse_json(&parse_name, &bytes)
        } else {
            Err(ArchiveError::NotZip)
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .map_err(|e| AppError::BadRequest(e.to_string()))?;
    tracing::info!(
        tracks = archive.tracks.len(),
        albums = archive.albums.len(),
        artists = archive.artists.len(),
        playlists = archive.playlists.len(),
        plays = archive.plays.len(),
        extended = archive.extended,
        "spotify archive parsed"
    );
    let options = Options {
        target: Some(ImportTarget::Library),
        file_name: Some(name),
        ..Options::default()
    };
    let mut tx = st.db.begin().await?;
    let id = import::create(
        &mut tx,
        user,
        ImportKind::SpotifyArchive,
        import::kind_title(ImportKind::SpotifyArchive),
        &options,
        ImportPhase::Matching,
        Some(size as i64),
    )
    .await?;
    import::store_archive(&mut tx, id, &archive).await?;
    import::enqueue_match(&mut tx, id).await?;
    tx.commit().await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(import::summary(&st.db, user, id).await?),
    ))
}

/// Медиатека другого сервиса уже собрана: импорт, позиции, сопоставление в фоне.
async fn start_collection(
    st: &AppState,
    user: Uuid,
    kind: ImportKind,
    title: &str,
    collection: &Collection,
    size: Option<i64>,
    file_name: Option<String>,
) -> AppResult<(StatusCode, Json<ImportSummary>)> {
    tracing::info!(
        kind = kind.as_str(),
        tracks = collection.tracks.len(),
        liked = collection.tracks.iter().filter(|t| t.liked).count(),
        playlists = collection.playlists.len(),
        "import source collected"
    );
    let options = Options {
        target: Some(ImportTarget::Library),
        file_name,
        ..Options::default()
    };
    let mut tx = st.db.begin().await?;
    let id = import::create(
        &mut tx,
        user,
        kind,
        title,
        &options,
        ImportPhase::Matching,
        size,
    )
    .await?;
    import::store_collection(&mut tx, id, collection).await?;
    import::enqueue_match(&mut tx, id).await?;
    tx.commit().await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(import::summary(&st.db, user, id).await?),
    ))
}

fn import_title(title: Option<String>, kind: ImportKind) -> String {
    title
        .map(|t| t.trim().chars().take(200).collect::<String>())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| import::kind_title(kind).to_string())
}

#[utoipa::path(
    post,
    path = "/api/imports/yandex",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = YandexImportRequest,
    responses(
        (status = 202, description = "Треки собраны, сопоставление идёт в фоне", body = ImportSummary),
        (status = 400, description = "Нет ни ссылки, ни ключа входа, или в плейлисте нет треков", body = ErrorBody),
        (status = 422, description = "Ссылка не «Яндекс Музыки» (`yandex_link`), ключ не похож на ключ (`yandex_token`), \
            нужен ключ входа (`yandex_login_required`), ключ не подошёл (`yandex_token_rejected`), \
            плейлиста нет или он скрыт (`yandex_not_found`), Яндекс не отвечает стране сервера (`yandex_region`)", body = ErrorBody),
        (status = 503, description = "«Яндекс Музыка» не ответила", body = ErrorBody),
    ),
)]
pub async fn import_yandex(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<YandexImportRequest>,
) -> AppResult<(StatusCode, Json<ImportSummary>)> {
    let user = principal.user.id;
    let link = match body.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => Some(
            yandex::parse_link(u).ok_or_else(|| AppError::Unprocessable {
                code: "yandex_link",
                message: "Это не ссылка на плейлист «Яндекс Музыки»".into(),
            })?,
        ),
        None => None,
    };
    let token = match body
        .token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(t) => Some(
            yandex::parse_token(t).ok_or_else(|| AppError::Unprocessable {
                code: "yandex_token",
                message: "Ключ не похож на настоящий — скопируйте его целиком".into(),
            })?,
        ),
        None => None,
    };
    if link.is_none() && token.is_none() {
        return Err(bad("вставьте ссылку на плейлист"));
    }
    // Ключ живёт только здесь: в базу, задачи и журналы он не попадает
    let fetched = Yandex::new(&st.http, &st.config.yandex_music.url, token.as_deref())
        .fetch(link.as_ref())
        .await
        .map_err(|e| {
            let code = match e {
                YandexError::LoginRequired => "yandex_login_required",
                YandexError::TokenRejected => "yandex_token_rejected",
                YandexError::NotFound => "yandex_not_found",
                YandexError::Region => "yandex_region",
                YandexError::Unavailable(_) | YandexError::Format => {
                    tracing::warn!(error = %e, "yandex music did not answer");
                    return AppError::Unavailable(
                        "«Яндекс Музыка» не ответила — попробуйте позже".into(),
                    );
                }
            };
            let message = match e {
                YandexError::Region => {
                    "«Яндекс Музыка» не отдаёт данные серверу из его страны".into()
                }
                e => capitalize(&e.to_string()),
            };
            AppError::Unprocessable { code, message }
        })?;
    drop(token);
    if fetched.collection.is_empty() {
        return Err(bad("в плейлисте нет треков"));
    }
    let title = import_title(fetched.title, ImportKind::Yandex);
    start_collection(
        &st,
        user,
        ImportKind::Yandex,
        &title,
        &fetched.collection,
        None,
        None,
    )
    .await
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[utoipa::path(
    post,
    path = "/api/imports/youtube",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = YoutubeImportRequest,
    responses(
        (status = 202, description = "Треки плейлиста собраны, сопоставление идёт в фоне", body = ImportSummary),
        (status = 400, description = "В плейлисте нет треков", body = ErrorBody),
        (status = 422, description = "Не ссылка на плейлист (`youtube_link`) или плейлист закрыт (`youtube_not_found`)", body = ErrorBody),
        (status = 503, description = "YouTube Music недоступен", body = ErrorBody),
    ),
)]
pub async fn import_youtube(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<YoutubeImportRequest>,
) -> AppResult<(StatusCode, Json<ImportSummary>)> {
    let user = principal.user.id;
    let id = youtube::playlist_id(&body.url).ok_or_else(|| AppError::Unprocessable {
        code: "youtube_link",
        message: "Нужна ссылка на плейлист YouTube или YouTube Music".into(),
    })?;
    let playlist = youtube::fetch_playlist(&st.http, &st.config.fetcher.url, &id)
        .await
        .map_err(|e| match e {
            YoutubeError::NotFound => AppError::Unprocessable {
                code: "youtube_not_found",
                message: "Плейлист не открылся — он закрыт или удалён".into(),
            },
            YoutubeError::Unavailable(m) => {
                tracing::warn!(error = %m, "youtube music playlist failed");
                AppError::Unavailable("YouTube Music сейчас недоступен — попробуйте позже".into())
            }
        })?;
    let collection = youtube::playlist_collection(&playlist);
    if collection.is_empty() {
        return Err(bad("в плейлисте нет треков"));
    }
    let title = import_title(Some(playlist.title), ImportKind::YoutubePlaylist);
    start_collection(
        &st,
        user,
        ImportKind::YoutubePlaylist,
        &title,
        &collection,
        None,
        None,
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/imports/youtube/takeout",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(UploadQuery),
    request_body(content = Vec<u8>, content_type = "application/zip", description = "Архив Google Takeout «YouTube и YouTube Music», до 500 МБ"),
    responses(
        (status = 202, description = "Архив разобран, сопоставление идёт в фоне", body = ImportSummary),
        (status = 400, description = "Не архив Takeout или в нём нет музыки", body = ErrorBody),
        (status = 413, description = "Больше 500 МБ", body = ErrorBody),
    ),
)]
pub async fn upload_takeout(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> AppResult<(StatusCode, Json<ImportSummary>)> {
    let user = principal.user.id;
    let dir = temp_dir()?;
    let (path, size, head) = receive(&headers, body, import::MAX_ARCHIVE_BYTES, dir.path()).await?;
    if !head.starts_with(b"PK") {
        return Err(bad("нужен ZIP-архив из Google Takeout"));
    }
    let collection = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&path).map_err(|e| ArchiveError::Broken(e.to_string()))?;
        youtube::parse_takeout(std::io::BufReader::new(file), Limits::default())
    })
    .await
    .map_err(|e| AppError::Internal(e.into()))?
    .map_err(|e| match e {
        ArchiveError::Empty => bad(
            "в архиве нет музыки YouTube: в Takeout отметьте «YouTube и YouTube Music» — плейлисты и музыкальную библиотеку",
        ),
        e => AppError::BadRequest(e.to_string()),
    })?;
    let name = q
        .name
        .map(|n| n.trim().chars().take(300).collect::<String>())
        .filter(|n| !n.is_empty());
    start_collection(
        &st,
        user,
        ImportKind::YoutubeTakeout,
        import::kind_title(ImportKind::YoutubeTakeout),
        &collection,
        Some(size as i64),
        name,
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/imports/list",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = ListImportRequest,
    responses(
        (status = 202, description = "Список разобран, поиск треков идёт в фоне", body = ImportSummary),
        (status = 400, description = "В тексте не нашлось ни одного трека", body = ErrorBody),
    ),
)]
pub async fn import_list(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<ListImportRequest>,
) -> AppResult<(StatusCode, Json<ImportSummary>)> {
    let user = principal.user.id;
    let (format, lines) = lists::parse(&body.text, body.format);
    if lines.is_empty() {
        return Err(bad(
            "не нашли в тексте ни одного трека — по строке на трек, «Артист — Название»",
        ));
    }
    let target = match body.target {
        Some(ImportTarget::Likes) => ImportTarget::Likes,
        _ => ImportTarget::Playlist,
    };
    let title: String = body
        .playlist_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(200).collect())
        .unwrap_or_else(|| format!("Импорт {}", today_ru()));
    let kind = match format {
        ListFormat::Text => ImportKind::Text,
        ListFormat::Csv => ImportKind::Csv,
        ListFormat::M3u => ImportKind::M3u,
    };
    let options = Options {
        target: Some(target),
        playlist_title: (target == ImportTarget::Playlist).then(|| title.clone()),
        ..Options::default()
    };
    let display = if target == ImportTarget::Likes {
        "В «Мне нравится»".to_string()
    } else {
        title.clone()
    };
    let mut tx = st.db.begin().await?;
    let id = import::create(
        &mut tx,
        user,
        kind,
        &display,
        &options,
        ImportPhase::Matching,
        Some(body.text.len() as i64),
    )
    .await?;
    import::store_lines(&mut tx, id, &lines, target, &title).await?;
    import::enqueue_match(&mut tx, id).await?;
    tx.commit().await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(import::summary(&st.db, user, id).await?),
    ))
}

#[utoipa::path(
    post,
    path = "/api/imports/files",
    tag = "library",
    security(("session" = []), ("device" = [])),
    request_body = FilesImportRequest,
    responses((status = 201, description = "Импорт файлов открыт: дальше файлы по одному, затем finish", body = ImportSummary)),
)]
pub async fn start_files(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<FilesImportRequest>,
) -> AppResult<(StatusCode, Json<ImportSummary>)> {
    let user = principal.user.id;
    let title: String = body
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(200).collect())
        .unwrap_or_else(|| import::kind_title(ImportKind::Files).to_string());
    let mut tx = st.db.begin().await?;
    let id = import::create(
        &mut tx,
        user,
        ImportKind::Files,
        &title,
        &Options {
            target: Some(ImportTarget::Library),
            ..Options::default()
        },
        ImportPhase::Uploading,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(import::summary(&st.db, user, id).await?),
    ))
}

#[utoipa::path(
    put,
    path = "/api/imports/{id}/files",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id импорта файлов"), UploadQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "Аудиофайл: FLAC, ALAC/M4A, MP3, Opus, Ogg, WAV, AIFF — до 1 ГБ"),
    responses(
        (status = 201, description = "Файл принят и ждёт обработки", body = ImportItem),
        (status = 400, description = "Не аудиофайл или формат не поддерживается", body = ErrorBody),
        (status = 409, description = "Импорт уже закрыт для новых файлов", body = ErrorBody),
        (status = 413, description = "Файл больше 1 ГБ", body = ErrorBody),
    ),
)]
pub async fn upload_file(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Query(q): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> AppResult<(StatusCode, Json<ImportItem>)> {
    let user = principal.user.id;
    let (kind, status, phase) = owned(&st, user, id).await?;
    if kind != ImportKind::Files {
        return Err(bad("в этот импорт файлы не загружаются"));
    }
    if status != "running" || phase != ImportPhase::Uploading {
        return Err(AppError::Conflict {
            code: "import_closed",
            message: "Этот импорт уже закрыт — начните новый".into(),
        });
    }
    let name = q
        .name
        .map(|n| n.trim().chars().take(300).collect::<String>())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| bad("нужно имя файла (name)"))?;
    let ext = files::extension(&name).ok_or_else(|| {
        bad("такой формат не подходит: нужны FLAC, ALAC/M4A, MP3, Opus, Ogg, WAV или AIFF")
    })?;
    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM import_items WHERE import_id = $1"#,
        id
    )
    .fetch_one(&st.db)
    .await?;
    if count >= import::MAX_FILES {
        return Err(bad("в одном импорте не больше 5000 файлов"));
    }
    let dir = temp_dir()?;
    let (path, size, head) = receive(&headers, body, import::MAX_FILE_BYTES, dir.path()).await?;
    if !files::sniff(&head) {
        return Err(bad("это не аудиофайл"));
    }
    let item_id = Uuid::now_v7();
    let key = import::upload_key(id, item_id, &ext);
    st.storage
        .put_file(&key, &path, "application/octet-stream")
        .await
        .map_err(|e| AppError::Unavailable(format!("хранилище не приняло файл: {e}")))?;
    let (artist, title) = lists::from_file_name(&name);
    let mut tx = st.db.begin().await?;
    import::insert_items(
        &mut tx,
        id,
        &[NewItem {
            id: item_id,
            kind: Some(ImportItemKind::File),
            position: count as i32,
            artist,
            title,
            source_ref: Some(name.clone()),
            ..NewItem::default()
        }],
    )
    .await?;
    sqlx::query!(
        "UPDATE import_items SET file_key = $2, file_size = $3 WHERE id = $1",
        item_id,
        key,
        size as i64,
    )
    .execute(&mut *tx)
    .await?;
    import::refresh_counts(&mut tx, id).await?;
    files::enqueue(&mut tx, item_id, None).await?;
    tx.commit().await?;
    let item = one_item(&st, user, id, item_id).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(
    post,
    path = "/api/imports/{id}/finish",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id импорта файлов")),
    responses(
        (status = 200, description = "Файлы загружены; импорт закончится, когда все обработаются", body = ImportSummary),
        (status = 404, description = "Нет такого импорта", body = ErrorBody),
    ),
)]
pub async fn finish_files(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ImportSummary>> {
    let user = principal.user.id;
    owned(&st, user, id).await?;
    sqlx::query!(
        "UPDATE imports SET phase = 'matching', updated_at = now() WHERE id = $1 AND phase = 'uploading'",
        id
    )
    .execute(&st.db)
    .await?;
    files::maybe_finish(&st.db, id).await?;
    Ok(Json(import::summary(&st.db, user, id).await?))
}

#[utoipa::path(
    get,
    path = "/api/imports/{id}/items",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id импорта"), ImportItemsQuery),
    responses(
        (status = 200, description = "Позиции по порядку источника", body = ImportItemPage),
        (status = 404, description = "Нет такого импорта", body = ErrorBody),
    ),
)]
pub async fn items(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Query(q): Query<ImportItemsQuery>,
) -> AppResult<Json<ImportItemPage>> {
    let limit = cursor::limit(q.limit, 50, 200)?;
    Ok(Json(
        import::items(
            &st.db,
            principal.user.id,
            id,
            q.status,
            q.cursor.as_deref(),
            limit,
        )
        .await?,
    ))
}

async fn one_item(
    st: &AppState,
    user: Uuid,
    import_id: Uuid,
    item_id: Uuid,
) -> AppResult<ImportItem> {
    let position = sqlx::query_scalar!(
        "SELECT position FROM import_items WHERE id = $1 AND import_id = $2",
        item_id,
        import_id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    // Курсор «после предыдущей позиции» — ровно эта
    let cursor = format!("{}_{}", position - 1, Uuid::max());
    import::items(&st.db, user, import_id, None, Some(&cursor), 5)
        .await?
        .items
        .into_iter()
        .find(|i| i.id == item_id)
        .ok_or(AppError::NotFound)
}

#[utoipa::path(
    put,
    path = "/api/imports/{id}/items/{itemId}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(
        ("id" = Uuid, Path, description = "id импорта"),
        ("itemId" = Uuid, Path, description = "id позиции"),
    ),
    request_body = ImportItemFix,
    responses(
        (status = 200, description = "Позиция исправлена; если импорт уже разложен — найденное сразу в медиатеке", body = ImportItem),
        (status = 400, description = "Не указан трек или он не подходит к позиции", body = ErrorBody),
        (status = 404, description = "Нет такого импорта, позиции или трека", body = ErrorBody),
    ),
)]
pub async fn fix_item(
    State(st): State<AppState>,
    principal: Principal,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
    Json(fix): Json<ImportItemFix>,
) -> AppResult<Json<ImportItem>> {
    let user = principal.user.id;
    let (_, status, phase) = owned(&st, user, id).await?;
    let item = sqlx::query!(
        "SELECT kind, status, artist, title, source_ref, file_key FROM import_items WHERE id = $1 AND import_id = $2",
        item_id,
        id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let kind: ImportItemKind = item.kind.parse().unwrap_or(ImportItemKind::Track);
    if fix.skip {
        let mut tx = st.db.begin().await?;
        sqlx::query!(
            "UPDATE import_items SET status = 'skipped', resolved_by = 'manual', updated_at = now()
             WHERE id = $1 AND status <> 'matched'",
            item_id
        )
        .execute(&mut *tx)
        .await?;
        import::refresh_counts(&mut tx, id).await?;
        tx.commit().await?;
        if kind == ImportItemKind::File {
            if let Some(key) = &item.file_key
                && let Err(e) = st.storage.delete(key).await
            {
                tracing::warn!(%key, error = %e, "could not delete a skipped upload");
            }
            files::maybe_finish(&st.db, id).await?;
        }
        return Ok(Json(one_item(&st, user, id, item_id).await?));
    }

    let deezer_of = async |table: &str, our: Uuid| -> AppResult<Option<i64>> {
        let v = match table {
            "albums" => {
                sqlx::query_scalar!("SELECT deezer_id FROM albums WHERE id = $1", our)
                    .fetch_optional(&st.db)
                    .await?
            }
            "artists" => {
                sqlx::query_scalar!("SELECT deezer_id FROM artists WHERE id = $1", our)
                    .fetch_optional(&st.db)
                    .await?
            }
            _ => {
                sqlx::query_scalar!("SELECT deezer_id FROM tracks WHERE id = $1", our)
                    .fetch_optional(&st.db)
                    .await?
            }
        };
        v.ok_or(AppError::NotFound)
    };
    let (track_id, album_id, artist_id, deezer_id) = match kind {
        ImportItemKind::Track | ImportItemKind::File => {
            let track = match (fix.track_id, fix.deezer_id) {
                (Some(t), _) => t,
                (None, Some(dz)) => catalog::resolve_track(&st.db, &st.deezer, dz)
                    .await
                    .map_err(|e| match e {
                        catalog::CatalogError::Deezer(crate::deezer::DeezerError::NotFound) => {
                            AppError::NotFound
                        }
                        e => AppError::Unavailable(e.to_string()),
                    })?,
                (None, None) => return Err(bad("выберите трек")),
            };
            let dz = deezer_of("tracks", track).await?;
            (Some(track), None, None, dz)
        }
        ImportItemKind::Album => {
            let album = match (fix.album_id, fix.deezer_id) {
                (Some(a), _) => a,
                (None, Some(dz)) => catalog::resolve_album(&st.db, &st.deezer, dz)
                    .await
                    .map_err(|e| AppError::Unavailable(e.to_string()))?,
                (None, None) => return Err(bad("выберите альбом")),
            };
            let dz = deezer_of("albums", album).await?;
            (None, Some(album), None, dz)
        }
        ImportItemKind::Artist => {
            let artist = match (fix.artist_id, fix.deezer_id) {
                (Some(a), _) => a,
                (None, Some(dz)) => catalog::resolve_artist(&st.db, &st.deezer, dz)
                    .await
                    .map_err(|e| AppError::Unavailable(e.to_string()))?,
                (None, None) => return Err(bad("выберите артиста")),
            };
            let dz = deezer_of("artists", artist).await?;
            (None, None, Some(artist), dz)
        }
    };

    let mut tx = st.db.begin().await?;
    if kind == ImportItemKind::File {
        // Файл кладётся к выбранному треку в фоне: позиция снова «в работе»
        if item.file_key.is_none() {
            return Err(bad(
                "у этой позиции больше нет файла — загрузите его заново",
            ));
        }
        sqlx::query!(
            "UPDATE import_items SET status = 'pending', track_id = $2, resolved_by = 'manual', updated_at = now()
             WHERE id = $1",
            item_id,
            track_id,
        )
        .execute(&mut *tx)
        .await?;
        files::enqueue(&mut tx, item_id, track_id).await?;
    } else {
        sqlx::query!(
            "UPDATE import_items SET status = 'matched', track_id = $2, album_id = $3, artist_id = $4,
                    confidence = 1, resolved_by = 'manual', candidates = '[]', applied = false, updated_at = now()
             WHERE id = $1",
            item_id,
            track_id,
            album_id,
            artist_id,
        )
        .execute(&mut *tx)
        .await?;
        let key = import::cache_key(kind, &item.artist, &item.title, item.source_ref.as_deref());
        import::remember(&mut tx, &key, kind, deezer_id, Some(1.0), &[], true).await?;
    }
    import::refresh_counts(&mut tx, id).await?;
    tx.commit().await?;
    // Импорт уже разложен (или раскладывается) — исправленное сразу в медиатеке, на своём месте
    let applied = matches!(phase, ImportPhase::Applying | ImportPhase::Done) || status != "running";
    if kind != ImportItemKind::File && applied {
        import::apply(&st.db, &st.deezer, id, Some(item_id)).await?;
    }
    Ok(Json(one_item(&st, user, id, item_id).await?))
}

#[utoipa::path(
    delete,
    path = "/api/imports/{id}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("id" = Uuid, Path, description = "id импорта")),
    responses(
        (status = 204, description = "Идущий импорт остановлен, законченный — убран из истории (медиатека не меняется)"),
        (status = 404, description = "Нет такого импорта", body = ErrorBody),
    ),
)]
pub async fn remove(
    State(st): State<AppState>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let user = principal.user.id;
    let (_, status, _) = owned(&st, user, id).await?;
    let keys = sqlx::query_scalar!(
        r#"SELECT file_key AS "key!" FROM import_items
           WHERE import_id = $1 AND file_key IS NOT NULL AND status <> 'matched'"#,
        id
    )
    .fetch_all(&st.db)
    .await?;
    let mut tx = st.db.begin().await?;
    if status == "running" {
        sqlx::query!(
            "UPDATE imports SET status = 'cancelled', finished_at = now(), updated_at = now() WHERE id = $1",
            id
        )
        .execute(&mut *tx)
        .await?;
        let jobs = sqlx::query_scalar!(
            "SELECT j.id FROM jobs j
             WHERE j.status IN ('queued', 'running') AND j.kind IN ('import_match', 'import_apply', 'import_file')
               AND (j.payload ->> 'importId' = $1::uuid::text
                    OR j.payload ->> 'itemId' IN (SELECT i.id::text FROM import_items i WHERE i.import_id = $1::uuid))",
            id
        )
        .fetch_all(&mut *tx)
        .await?;
        for job in jobs {
            queue::cancel(&mut tx, job).await?;
        }
        sqlx::query!("DELETE FROM import_plays WHERE import_id = $1", id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query!("DELETE FROM imports WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    for key in keys {
        if let Err(e) = st.storage.delete(&key).await {
            tracing::warn!(%key, error = %e, "could not delete an upload");
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_date() {
        let d = today_ru();
        assert!(d.split(' ').count() == 2, "{d}");
    }
}
