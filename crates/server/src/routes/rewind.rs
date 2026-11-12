//! «Итоги года» для историй и плейлист «Твои лучшие треки {год}». Логика — [`crate::rewind`].
//! Итоги — событие сервера: пока оно закрыто, слушатель получает 404 `not_open`, а администратор
//! может открыть любой год для предпросмотра. Настройки события — `/api/admin/rewind`.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use chrono::Utc;
use dalywave_protocol::{
    ErrorBody,
    auth::Permission,
    library::PlaylistSummary,
    rewind::{
        Rewind, RewindEvent, RewindEventPatch, RewindMode, RewindMore, RewindSoundtrack,
        RewindYears, SaveRewindMore,
    },
};
use serde_json::json;

use crate::{
    audit,
    auth::Principal,
    error::{AppError, AppResult, bad},
    extract::{Json, Path},
    reco::Settings,
    rewind::{
        self, Access,
        event::{self, mode_str, style_setting_str},
        local_date, local_year,
    },
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rewind", get(years))
        .route("/rewind/{year}", get(year))
        .route("/rewind/{year}/playlist", post(playlist))
        .route("/rewind/{year}/soundtrack", get(soundtrack))
        .route("/rewind/{year}/more", get(more).post(save_more))
        .route("/rewind/{year}/seen", post(seen))
}

/// Под `/api/admin`, за правом admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new().route("/rewind", get(event_settings).patch(update_event))
}

fn not_open() -> AppError {
    AppError::Missing {
        code: "not_open",
        message: "итоги года сейчас закрыты".into(),
    }
}

/// Можно ли этому слушателю открыть итоги года: событие открыто (или он администратор) и год наступил.
async fn check_year(st: &AppState, principal: &Principal, year: i32) -> AppResult<()> {
    let admin = principal.can(Permission::Admin);
    match rewind::can_open(&st.db, year, Utc::now(), admin).await? {
        Access::Open => Ok(()),
        Access::NotOpen => Err(not_open()),
        Access::NoSuchYear => Err(AppError::NotFound),
    }
}

#[utoipa::path(
    get,
    path = "/api/rewind",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Годы с прослушиваниями (новые первыми), открытый год, стиль историй \
                                      и посмотрены ли они", body = RewindYears),
        (status = 404, description = "`not_open` — итоги сейчас закрыты (администратору видно всегда)", body = ErrorBody),
    ),
)]
pub async fn years(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<RewindYears>> {
    let admin = principal.can(Permission::Admin);
    rewind::years(&st.db, principal.user.id, Utc::now(), admin)
        .await?
        .map(Json)
        .ok_or_else(not_open)
}

#[utoipa::path(
    get,
    path = "/api/rewind/{year}",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    responses(
        (status = 200, description = "Итоги года; текущий — «по сей день» (`complete: false`). Без прослушиваний — \
                                      нули и пустые списки", body = Rewind),
        (status = 404, description = "Год ещё не наступил; `not_open` — итоги закрыты", body = ErrorBody),
    ),
)]
pub async fn year(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
) -> AppResult<Json<Rewind>> {
    check_year(&st, &principal, year).await?;
    rewind::get(&st.db, principal.user.id, year, Utc::now())
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[utoipa::path(
    post,
    path = "/api/rewind/{year}/playlist",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    responses(
        (status = 201, description = "Плейлист «Твои лучшие треки {год}» создан", body = PlaylistSummary),
        (status = 200, description = "Уже был — обновлён по свежим итогам", body = PlaylistSummary),
        (status = 400, description = "За этот год ещё нечего собрать", body = ErrorBody),
        (status = 404, description = "Год ещё не наступил; `not_open` — итоги закрыты", body = ErrorBody),
    ),
)]
pub async fn playlist(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
) -> AppResult<(StatusCode, Json<PlaylistSummary>)> {
    check_year(&st, &principal, year).await?;
    let (summary, created) = rewind::save_playlist(&st.db, principal.user.id, year)
        .await?
        .ok_or_else(|| bad("за этот год ещё нечего собрать"))?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(summary)))
}

#[utoipa::path(
    get,
    path = "/api/rewind/{year}/soundtrack",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    responses(
        (status = 200, description = "Музыка историй: трек и место начала у слайдов. Треки не на сервере \
                                      ставятся на скачивание вне очереди", body = RewindSoundtrack),
        (status = 404, description = "Год ещё не наступил; `not_open` — итоги закрыты", body = ErrorBody),
    ),
)]
pub async fn soundtrack(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
) -> AppResult<Json<RewindSoundtrack>> {
    check_year(&st, &principal, year).await?;
    let user = principal.user.id;
    let track = rewind::soundtrack::get(&st.db, user, year, Utc::now())
        .await?
        .ok_or(AppError::NotFound)?;
    // Скачивание — в фоне: истории открываются сразу, звук догоняет к своему слайду
    let cues = track.cues.clone();
    let bg = st.clone();
    tokio::spawn(async move {
        rewind::soundtrack::prefetch(&bg.db, Some(&bg.deezer), user, &cues).await;
    });
    Ok(Json(track))
}

#[utoipa::path(
    get,
    path = "/api/rewind/{year}/more",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    responses(
        (status = 200, description = "«Ещё послушать»: новое, похожее на любимое за год", body = RewindMore),
        (status = 404, description = "Год ещё не наступил; `not_open` — итоги закрыты", body = ErrorBody),
    ),
)]
pub async fn more(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
) -> AppResult<Json<RewindMore>> {
    check_year(&st, &principal, year).await?;
    let user = principal.user.id;
    Ok(Json(RewindMore {
        items: rewind::more::generate(&st.db, user, year, rewind::more::SIZE).await?,
        playlist_id: rewind::more::playlist(&st.db, user, year).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/api/rewind/{year}/more",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    request_body = SaveRewindMore,
    responses(
        (status = 201, description = "Плейлист «Ещё послушать {год}» создан", body = PlaylistSummary),
        (status = 200, description = "Уже был — заменён показанной подборкой", body = PlaylistSummary),
        (status = 400, description = "Нет треков или больше 100", body = ErrorBody),
        (status = 404, description = "Год ещё не наступил; `not_open` — итоги закрыты", body = ErrorBody),
    ),
)]
pub async fn save_more(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
    Json(body): Json<SaveRewindMore>,
) -> AppResult<(StatusCode, Json<PlaylistSummary>)> {
    check_year(&st, &principal, year).await?;
    if body.track_ids.len() > 100 {
        return Err(bad("не больше 100 треков"));
    }
    let (summary, created) = rewind::more::save(&st.db, principal.user.id, year, &body.track_ids)
        .await?
        .ok_or_else(|| bad("сохранять нечего"))?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(summary)))
}

#[utoipa::path(
    post,
    path = "/api/rewind/{year}/seen",
    tag = "library",
    security(("session" = []), ("device" = [])),
    params(("year" = i32, Path, description = "Год по времени слушателя")),
    responses(
        (status = 204, description = "Отмечено: истории открывали — карточка «Твои итоги готовы» больше не зовёт"),
        (status = 404, description = "Год ещё не наступил; `not_open` — итоги закрыты", body = ErrorBody),
    ),
)]
pub async fn seen(
    State(st): State<AppState>,
    principal: Principal,
    Path(year): Path<i32>,
) -> AppResult<StatusCode> {
    check_year(&st, &principal, year).await?;
    event::mark_seen(&st.db, principal.user.id, year).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_event(st: &AppState) -> AppResult<RewindEvent> {
    let ev = event::load(&st.db).await?;
    let off = Settings::load(&st.db).await?.utc_offset_min;
    let now = Utc::now();
    let current = local_year(now, off);
    // Годы, за которые на сервере вообще что-то играло, — для выбора года ручного запуска
    let mut years = sqlx::query_scalar!(
        r#"SELECT DISTINCT extract(year FROM started_at + make_interval(mins => $1))::int AS "year!"
           FROM play_events ORDER BY 1 DESC"#,
        i32::from(off),
    )
    .fetch_all(&st.db)
    .await?;
    years.retain(|y| (rewind::FIRST_YEAR..=current).contains(y));
    if !years.contains(&current) {
        years.insert(0, current);
    }
    Ok(RewindEvent {
        style: ev.style,
        mode: ev.mode,
        year: ev.year,
        open_year: ev.open_year(local_date(now, off)),
        opens_at: ev.opens_at(now, off),
        closes_at: ev.closes_at(now, off),
        years,
        updated_at: ev.updated_at,
    })
}

#[utoipa::path(
    get,
    path = "/api/admin/rewind",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "«Итоги года» как событие: стиль, режим, что открыто сейчас", body = RewindEvent),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn event_settings(State(st): State<AppState>) -> AppResult<Json<RewindEvent>> {
    Ok(Json(load_event(&st).await?))
}

#[utoipa::path(
    patch,
    path = "/api/admin/rewind",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = RewindEventPatch,
    responses(
        (status = 200, description = "Событие после изменения", body = RewindEvent),
        (status = 400, description = "Год — от 2000 до текущего", body = ErrorBody),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn update_event(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<RewindEventPatch>,
) -> AppResult<Json<RewindEvent>> {
    let off = Settings::load(&st.db).await?.utc_offset_min;
    let current = local_year(Utc::now(), off);
    if patch
        .year
        .is_some_and(|y| !(rewind::FIRST_YEAR..=current).contains(&y))
    {
        return Err(bad(format!("год — от {} до {current}", rewind::FIRST_YEAR)));
    }
    let was = event::load(&st.db).await?;
    let style = patch.style.unwrap_or(was.style);
    let mode = patch.mode.unwrap_or(was.mode);
    // «Открыто сейчас» без года — текущий год
    let year = match (patch.year, mode) {
        (Some(y), _) => Some(y),
        (None, RewindMode::Open) => Some(was.year.unwrap_or(current)),
        (None, _) => was.year,
    };
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "UPDATE rewind_event SET style = $1, mode = $2, year = $3, updated_at = now() WHERE id",
        style_setting_str(style),
        mode_str(mode),
        year,
    )
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::SETTINGS_CHANGED,
        Some("rewind".into()),
        json!({ "rewind": { "style": style_setting_str(style), "mode": mode_str(mode), "year": year } }),
    )
    .await?;
    tx.commit().await?;
    // Итоги открыли — музыку историй на сервер сразу, а не ночью: иначе первый просмотр тянет
    // через все слайды единственный готовый трек
    if let (RewindMode::Open, Some(year)) = (mode, year)
        && (was.mode != RewindMode::Open || was.year != Some(year))
    {
        let bg = st.clone();
        tokio::spawn(async move {
            match rewind::soundtrack::prefetch_year(&bg.db, &bg.deezer, year, Utc::now()).await {
                Ok(n) => tracing::info!(year, queued = n, "year-in-review music prefetched"),
                Err(e) => {
                    tracing::warn!(year, error = %e, "could not prefetch year-in-review music")
                }
            }
        });
    }
    Ok(Json(load_event(&st).await?))
}
