//! Раздел «Рекомендации» админки: индекс каталога и анализ звука, грани вкуса с примерами, последние
//! показы с причинами, сервис анализа, настройки разведчика и запуск задач вне расписания.
//! Весь роутер закрыт правом admin (см. `routes::router`).

use std::collections::HashMap;

use axum::{
    Router,
    extract::State,
    routing::{get, patch, post},
};
use dalywave_protocol::{
    ErrorBody,
    jobs::kind,
    reco::{
        FacetView, ImpressionView, IndexStats, MlStatus, RecoOverview, RecoRun, RecoRunResult,
        RecoSettings, RecoSettingsPatch, RecoTask, ScoutSourceStats,
    },
    wave::Reason,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit,
    auth::Principal,
    catalog::views,
    error::{AppResult, bad},
    extract::Json,
    jobs::{self, NewJob, lanes},
    reco::{Settings, analysis, scout, taste},
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/reco", get(reco_overview))
        .route("/reco/settings", patch(update_reco_settings))
        .route("/reco/run", post(run_reco))
}

/// Сколько примеров у грани и сколько последних показов.
const EXAMPLES: usize = 5;
const IMPRESSIONS: i64 = 50;

#[utoipa::path(
    get,
    path = "/api/admin/reco",
    tag = "admin",
    security(("session" = []), ("device" = [])),
    responses(
        (status = 200, description = "Индекс и анализ, грани вкуса, последние показы, сервис анализа, настройки", body = RecoOverview),
        (status = 403, description = "Нет права admin", body = ErrorBody),
    ),
)]
pub async fn reco_overview(
    State(st): State<AppState>,
    principal: Principal,
) -> AppResult<Json<RecoOverview>> {
    let viewer = principal.user.id;
    let settings = Settings::load(&st.db).await?;
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
    let model_version =
        sqlx::query_scalar!("SELECT version FROM model_versions WHERE kind = 'taste' AND active")
            .fetch_optional(&st.db)
            .await?;

    let facet_rows = sqlx::query!(
        "SELECT f.id, f.user_id, u.name AS user_name, f.position, f.label, f.weight, f.tracks, f.context,
                f.examples, f.updated_at
         FROM taste_facets f JOIN users u ON u.id = f.user_id
         ORDER BY u.name, f.user_id, f.position"
    )
    .fetch_all(&st.db)
    .await?;
    let example_ids: Vec<Uuid> = facet_rows
        .iter()
        .flat_map(|f| f.examples.iter().take(EXAMPLES).copied())
        .collect();
    let examples: HashMap<Uuid, _> = views::tracks(&st.db, &example_ids, viewer)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let facets = facet_rows
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
        .collect();

    let shown = sqlx::query!(
        "SELECT i.id, u.name AS user_name, i.surface, i.track_id, i.reason, i.score, i.outcome, i.wave_session_id,
                i.shown_at
         FROM recommendation_impressions i JOIN users u ON u.id = i.user_id
         ORDER BY i.id DESC
         LIMIT $1",
        IMPRESSIONS,
    )
    .fetch_all(&st.db)
    .await?;
    let shown_ids: Vec<Uuid> = shown.iter().map(|i| i.track_id).collect();
    let tracks: HashMap<Uuid, _> = views::tracks(&st.db, &shown_ids, viewer)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let impressions = shown
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
        .collect();

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
        index: IndexStats {
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
        },
        settings: settings.view(),
        model_version,
        ml,
        facets,
        impressions,
    }))
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
            let users = sqlx::query_scalar!(
                "SELECT u.id FROM users u WHERE NOT u.disabled
                   AND (EXISTS (SELECT 1 FROM liked_tracks l WHERE l.user_id = u.id)
                        OR EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = u.id))"
            )
            .fetch_all(&st.db)
            .await?;
            let mut conn = st.db.acquire().await?;
            let mut n = 0;
            for user in users {
                n += i64::from(taste::request(&mut conn, user, true).await?);
            }
            n
        }
        RecoTask::Analysis => analysis::schedule_backfill(&st.db, 5000).await? as i64,
    };
    audit::record(
        &st.db,
        Some(principal.user.id),
        audit::RECO_RUN,
        Some(body.task.as_str().to_string()),
        json!({ "queued": queued }),
    )
    .await?;
    Ok(Json(RecoRunResult { queued }))
}
