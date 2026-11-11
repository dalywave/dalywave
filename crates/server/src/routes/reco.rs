//! Раздел «Рекомендации» админки. Вкус — у каждого слушателя свой (`?userId=`, по умолчанию — тот, кто
//! смотрит): грани с примерами, последние показы с исходами, сессии волны, заготовки, миксы. Индекс
//! каталога общий: анализ звука, находки разведчика, сервис анализа, настройки и запуск задач вне расписания.
//! Весь роутер закрыт правом admin (см. `routes::router`).

use std::collections::HashMap;

use axum::{
    Router,
    extract::{Query, State},
    routing::{get, patch, post},
};
use dalywave_protocol::{
    ErrorBody,
    jobs::kind,
    reco::{
        FacetView, ImpressionView, IndexStats, MixView, MlStatus, RecoListener, RecoOverview,
        RecoQuery, RecoRun, RecoRunResult, RecoSettings, RecoSettingsPatch, RecoTask,
        ScoutSourceStats, WavePoolView, WaveSessionView,
    },
    wave::Reason,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit,
    auth::Principal,
    catalog::views,
    error::{AppError, AppResult, bad},
    extract::Json,
    jobs::{self, NewJob, lanes},
    reco::{Settings, analysis, model, scout, taste},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/reco", get(reco_overview))
        .route("/reco/settings", patch(update_reco_settings))
        .route("/reco/run", post(run_reco))
}

/// Сколько примеров у грани, последних показов и сессий волны.
const EXAMPLES: usize = 5;
const IMPRESSIONS: i64 = 50;
const SESSIONS: i64 = 10;

#[utoipa::path(
    get,
    path = "/api/admin/reco",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    params(RecoQuery),
    responses(
        (status = 200, description = "Вкус слушателя (грани, показы, сессии, заготовки, миксы) и общий индекс \
                                      каталога: анализ, находки разведчика, сервис анализа, настройки", body = RecoOverview),
        (status = 403, description = "Нет права admin", body = ErrorBody),
        (status = 404, description = "Нет такого слушателя", body = ErrorBody),
    ),
)]
pub async fn reco_overview(
    State(st): State<AppState>,
    principal: Principal,
    Query(q): Query<RecoQuery>,
) -> AppResult<Json<RecoOverview>> {
    let user_id = q.user_id.unwrap_or(principal.user.id);
    let listeners = listeners(&st, principal.user.id, user_id).await?;
    let user = listeners
        .iter()
        .find(|l| l.id == user_id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let settings = Settings::load(&st.db).await?;
    let index = index_stats(&st, &settings).await?;
    let model_version =
        sqlx::query_scalar!("SELECT version FROM model_versions WHERE kind = 'taste' AND active")
            .fetch_optional(&st.db)
            .await?;
    let ml = match st.ml.health().await {
        Ok(health) => MlStatus {
            url: st.ml.url().to_string(),
            reachable: true,
            health: Some(health),
            error: None,
        },
        Err(e) => MlStatus {
            url: st.ml.url().to_string(),
            reachable: false,
            health: None,
            error: Some(e.to_string()),
        },
    };
    Ok(Json(RecoOverview {
        facets: facets(&st, user_id).await?,
        impressions: impressions(&st, user_id).await?,
        sessions: sessions(&st, user_id).await?,
        pools: pools(&st, user_id).await?,
        mixes: mixes(&st, user_id).await?,
        user,
        listeners,
        index,
        settings: settings.view(),
        model_version,
        ml,
        models: model::versions(&st.db).await?,
        training: model::training(&st.db).await?,
    }))
}

/// Кого можно выбрать: тот, кто смотрит, выбранный и все, у кого есть лайки или прослушивания.
async fn listeners(st: &AppState, viewer: Uuid, selected: Uuid) -> AppResult<Vec<RecoListener>> {
    Ok(sqlx::query!(
        r#"SELECT u.id, u.name,
                  (SELECT count(*) FROM liked_tracks l WHERE l.user_id = u.id) AS "likes!",
                  (SELECT count(*) FROM play_events p WHERE p.user_id = u.id) AS "plays!",
                  (SELECT count(*) FROM taste_facets f WHERE f.user_id = u.id) AS "facets!"
           FROM users u
           WHERE u.id = $1 OR u.id = $2
              OR (NOT u.disabled AND (EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = u.id)
                                      OR EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = u.id)))
           ORDER BY u.id = $1 DESC, u.name, u.id"#,
        viewer,
        selected,
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| RecoListener {
        id: r.id,
        name: r.name,
        likes: r.likes,
        plays: r.plays,
        facets: r.facets,
    })
    .collect())
}

/// Общий индекс каталога: анализы, окрестность разведчика, очередь, находки по видам.
async fn index_stats(st: &AppState, settings: &Settings) -> AppResult<IndexStats> {
    let s = sqlx::query!(
        r#"SELECT
              (SELECT count(*) FROM track_analysis WHERE source = 'preview') AS "size!",
              (SELECT count(*) FROM track_analysis WHERE source = 'full') AS "full!",
              (SELECT count(*) FROM recordings r WHERE r.audio_status = 'ready'
                 AND NOT EXISTS (SELECT 1 FROM track_analysis a WHERE a.recording_id = r.id AND a.source = 'full'))
                 AS "ready_without!",
              (SELECT count(*) FROM scout_artists) AS "artists!",
              (SELECT count(*) FROM scout_artists WHERE hops = 0) AS "seeds!",
              (SELECT count(*) FROM jobs WHERE kind = 'analyze_full' AND status = 'queued') AS "queued_full!",
              (SELECT count(*) FROM jobs WHERE kind = 'analyze_preview' AND status = 'queued') AS "queued_preview!",
              (SELECT count(*) FROM track_analysis WHERE analyzed_at > now() - interval '24 hours') AS "analyzed!",
              (SELECT count(*) FROM jobs WHERE kind IN ('analyze_full', 'analyze_preview') AND status = 'failed'
                 AND finished_at > now() - interval '24 hours') AS "failed!""#
    )
    .fetch_one(&st.db)
    .await?;
    let by_source = sqlx::query!(
        r#"SELECT source, count(*) AS "found!",
                  count(*) FILTER (WHERE analyzed_at IS NOT NULL) AS "analyzed!",
                  count(*) FILTER (WHERE analyzed_at IS NULL AND skipped IS NULL) AS "pending!"
           FROM scout_tracks GROUP BY source ORDER BY source"#
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| ScoutSourceStats {
        source: r.source,
        found: r.found,
        analyzed: r.analyzed,
        pending: r.pending,
    })
    .collect();
    Ok(IndexStats {
        size: s.size,
        max_index: i64::from(settings.max_index),
        full: s.full,
        ready_without_analysis: s.ready_without,
        by_source,
        artists: s.artists,
        seeds: s.seeds,
        queued_full: s.queued_full,
        queued_preview: s.queued_preview,
        analyzed_24h: s.analyzed,
        failed_24h: s.failed,
    })
}

async fn facets(st: &AppState, user_id: Uuid) -> AppResult<Vec<FacetView>> {
    let rows = sqlx::query!(
        "SELECT f.id, f.user_id, u.name AS user_name, f.position, f.label, f.weight, f.tracks, f.context,
                f.examples, f.updated_at
         FROM taste_facets f JOIN users u ON u.id = f.user_id
         WHERE f.user_id = $1
         ORDER BY f.position",
        user_id,
    )
    .fetch_all(&st.db)
    .await?;
    let ids: Vec<Uuid> = rows
        .iter()
        .flat_map(|f| f.examples.iter().take(EXAMPLES).copied())
        .collect();
    let examples: HashMap<Uuid, _> = views::tracks(&st.db, &ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    Ok(rows
        .into_iter()
        .map(|f| FacetView {
            examples: f
                .examples
                .iter()
                .take(EXAMPLES)
                .filter_map(|id| examples.get(id).cloned())
                .collect(),
            id: f.id,
            user_id: f.user_id,
            user_name: f.user_name,
            position: f.position,
            label: f.label,
            weight: f.weight,
            tracks: f.tracks,
            context: f.context,
            updated_at: f.updated_at,
        })
        .collect())
}

async fn impressions(st: &AppState, user_id: Uuid) -> AppResult<Vec<ImpressionView>> {
    let shown = sqlx::query!(
        "SELECT i.id, u.name AS user_name, i.surface, i.track_id, i.reason, i.score, i.outcome, i.wave_session_id,
                i.shown_at
         FROM recommendation_impressions i JOIN users u ON u.id = i.user_id
         WHERE i.user_id = $1
         ORDER BY i.shown_at DESC, i.id DESC
         LIMIT $2",
        user_id,
        IMPRESSIONS,
    )
    .fetch_all(&st.db)
    .await?;
    let ids: Vec<Uuid> = shown.iter().map(|i| i.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    Ok(shown
        .into_iter()
        .filter_map(|i| {
            Some(ImpressionView {
                track: tracks.get(&i.track_id)?.clone(),
                id: i.id,
                user_name: i.user_name,
                surface: i.surface,
                reason: i
                    .reason
                    .and_then(|r| serde_json::from_value::<Reason>(r).ok()),
                score: i.score,
                outcome: i.outcome,
                wave_session_id: i.wave_session_id,
                shown_at: i.shown_at,
            })
        })
        .collect())
}

async fn sessions(st: &AppState, user_id: Uuid) -> AppResult<Vec<WaveSessionView>> {
    Ok(sqlx::query!(
        r#"SELECT s.id, s.settings, s.state ->> 'title' AS title,
                  COALESCE((s.state ->> 'step')::bigint, 0) AS "served!",
                  count(i.id) FILTER (WHERE i.outcome IN ('like', 'more_like_this')) AS "likes!",
                  count(i.id) FILTER (WHERE i.outcome = 'completed') AS "completed!",
                  count(i.id) FILTER (WHERE i.outcome IN ('skip', 'dislike')) AS "skips!",
                  s.started_at, s.last_active_at
           FROM wave_sessions s
           LEFT JOIN recommendation_impressions i ON i.wave_session_id = s.id
           WHERE s.user_id = $1
           GROUP BY s.id
           ORDER BY s.last_active_at DESC, s.id DESC
           LIMIT $2"#,
        user_id,
        SESSIONS,
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| WaveSessionView {
        id: r.id,
        title: r
            .title
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Моя волна".into()),
        settings: serde_json::from_value(r.settings).unwrap_or_default(),
        served: r.served,
        likes: r.likes,
        completed: r.completed,
        skips: r.skips,
        started_at: r.started_at,
        last_active_at: r.last_active_at,
    })
    .collect())
}

async fn pools(st: &AppState, user_id: Uuid) -> AppResult<Vec<WavePoolView>> {
    Ok(sqlx::query!(
        r#"SELECT p.combo, (array_agg(p.settings ORDER BY p.picked_at DESC))[1] AS "settings!",
                  count(*) AS "tracks!",
                  count(*) FILTER (WHERE r.audio_status = 'ready') AS "ready!",
                  max(p.picked_at) AS "picked_at!"
           FROM wave_pools p
           JOIN tracks t ON t.id = p.track_id
           LEFT JOIN recordings r ON r.id = t.recording_id
           WHERE p.user_id = $1
           GROUP BY p.combo
           ORDER BY count(*) DESC, p.combo"#,
        user_id,
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| WavePoolView {
        combo: r.combo,
        settings: serde_json::from_value(r.settings).unwrap_or_default(),
        tracks: r.tracks,
        ready: r.ready,
        picked_at: r.picked_at,
    })
    .collect())
}

async fn mixes(st: &AppState, user_id: Uuid) -> AppResult<Vec<MixView>> {
    Ok(sqlx::query!(
        r#"SELECT p.id, p.title, p.rules ->> 'generator' AS "generator!", p.updated_at,
                  (SELECT count(*) FROM playlist_items i WHERE i.playlist_id = p.id) AS "tracks!"
           FROM playlists p
           WHERE p.user_id = $1 AND p.kind = 'mix' AND p.rules ->> 'generator' IS NOT NULL
           ORDER BY p.rules ->> 'generator', (p.rules ->> 'slot')::int NULLS LAST, p.id"#,
        user_id,
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|r| MixView {
        id: r.id,
        title: r.title,
        generator: r.generator,
        tracks: r.tracks,
        updated_at: r.updated_at,
    })
    .collect())
}

#[utoipa::path(
    patch,
    path = "/api/admin/reco/settings",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = RecoSettingsPatch,
    responses(
        (status = 200, description = "Настройки после изменения; разведчик применит их в следующий проход", body = RecoSettings),
        (status = 400, description = "Треков в сутки 0–50 000, предел индекса 100–2 000 000, шагов 0–3, \
                                      затухание 0,05–0,95, часовой пояс −720…840 минут", body = ErrorBody),
    ),
)]
pub async fn update_reco_settings(
    State(st): State<AppState>,
    principal: Principal,
    Json(patch): Json<RecoSettingsPatch>,
) -> AppResult<Json<RecoSettings>> {
    if patch
        .tracks_per_day
        .is_some_and(|n| !(0..=50_000).contains(&n))
    {
        return Err(bad("треков в сутки — от 0 до 50 000"));
    }
    if patch
        .max_index
        .is_some_and(|n| !(100..=2_000_000).contains(&n))
    {
        return Err(bad("предел индекса — от 100 до 2 000 000"));
    }
    if patch.max_hops.is_some_and(|n| !(0..=3).contains(&n)) {
        return Err(bad("шагов по похожим — от 0 до 3"));
    }
    if patch
        .hop_decay
        .is_some_and(|d| !d.is_finite() || !(0.05..=0.95).contains(&d))
    {
        return Err(bad("затухание — от 0,05 до 0,95"));
    }
    if patch
        .utc_offset_min
        .is_some_and(|m| !(-720..=840).contains(&m))
    {
        return Err(bad("часовой пояс — от −720 до 840 минут"));
    }
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "UPDATE reco_settings SET tracks_per_day = COALESCE($1, tracks_per_day),
                max_index = COALESCE($2, max_index), max_hops = COALESCE($3, max_hops),
                hop_decay = COALESCE($4, hop_decay), utc_offset_min = COALESCE($5, utc_offset_min),
                updated_at = now()
         WHERE id",
        patch.tracks_per_day,
        patch.max_index,
        patch.max_hops,
        patch.hop_decay,
        patch.utc_offset_min,
    )
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut *tx,
        Some(principal.user.id),
        audit::RECO_SETTINGS_CHANGED,
        None,
        json!({
            "tracksPerDay": patch.tracks_per_day, "maxIndex": patch.max_index, "maxHops": patch.max_hops,
            "hopDecay": patch.hop_decay, "utcOffsetMin": patch.utc_offset_min,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(Settings::load(&st.db).await?.view()))
}

#[utoipa::path(
    post,
    path = "/api/admin/reco/run",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    request_body = RecoRun,
    responses(
        (status = 200, description = "Задачи поставлены (уже стоящие в очереди не дублируются)", body = RecoRunResult),
        (status = 400, description = "Неизвестная задача", body = ErrorBody),
        (status = 404, description = "Нет такого слушателя", body = ErrorBody),
    ),
)]
pub async fn run_reco(
    State(st): State<AppState>,
    principal: Principal,
    Json(body): Json<RecoRun>,
) -> AppResult<Json<RecoRunResult>> {
    let queued = match body.task {
        RecoTask::Scout => {
            let mut conn = st.db.acquire().await?;
            let job = jobs::enqueue(
                &mut conn,
                NewJob::new(kind::SCOUT, lanes::SCOUT, json!({}))
                    .dedupe("scout")
                    .priority(scout::PLAN_PRIORITY),
            )
            .await?;
            i64::from(job.created)
        }
        RecoTask::Taste => {
            let users = match body.user_id {
                Some(user) => {
                    sqlx::query_scalar!("SELECT id FROM users WHERE id = $1", user)
                        .fetch_optional(&st.db)
                        .await?
                        .ok_or(AppError::NotFound)?;
                    vec![user]
                }
                None => {
                    sqlx::query_scalar!(
                        "SELECT u.id FROM users u WHERE NOT u.disabled
                           AND (EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = u.id)
                                OR EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = u.id))"
                    )
                    .fetch_all(&st.db)
                    .await?
                }
            };
            let mut conn = st.db.acquire().await?;
            let mut n = 0;
            for user in users {
                n += i64::from(taste::request(&mut conn, user, true).await?);
            }
            n
        }
        RecoTask::Analysis => analysis::schedule_backfill(&st.db, 5000).await? as i64,
        RecoTask::Model => i64::from(model::request(&st.db).await?),
    };
    audit::record(
        &st.db,
        Some(principal.user.id),
        audit::RECO_RUN,
        Some(body.task.as_str().to_string()),
        json!({ "queued": queued, "userId": body.user_id }),
    )
    .await?;
    Ok(Json(RecoRunResult { queued }))
}
