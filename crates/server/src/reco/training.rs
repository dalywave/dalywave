//! Обучение сети вкуса на компьютерах с приложением. Компьютер (администратора — по умолчанию, остальных —
//! если человек разрешил в настройках, и только на питании без нагрузки) берёт задание: обучающую часть
//! набора и описание обучения. Учит на своём железе тем же `dalywave-taste`, что и сервер, и присылает
//! веса. Сервер не верит ни весам, ни метрикам компьютера: сам собирает ONNX, сам проверяет на своей
//! отложенной части (строки после границы задания компьютер не видел) и включает, только если сеть
//! точнее правил и текущей версии ([`model::check`], [`model::judge`]).
//!
//! Администратор учит общую сеть всех слушателей, остальные — свою (подбор предпочитает её общей).
//! Компьютеры пробуют варианты обучения ([`VARIANTS`]) со своим зерном: при росте данных именно это и
//! даёт пользу — перебор, на который у сервера ночью нет времени.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::{
    auth::Permission,
    reco::TrainTaskView,
    taste::{TrainClaim, TrainOutcome, TrainScope, TrainSubmit},
};
use dalywave_taste::{Spec, TrainReport, wire};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::model::{self, Recipe};
use crate::{
    auth::Principal,
    error::{AppError, AppResult, bad},
    storage::Storage,
};

/// Задание держится за компьютером столько; не прислал — его может взять другой.
pub const LEASE_HOURS: i64 = 6;
/// Одна и та же сеть (общая или своя слушателя) учится не чаще, чем раз в столько часов.
const EVERY_HOURS: i64 = 20;
/// Один компьютер берёт задание не чаще.
const DEVICE_EVERY_HOURS: i64 = 6;

/// Варианты обучения для компьютеров по очереди: как у сервера, пошире, с меньшим сжатием векторов, логистическая.
pub const VARIANTS: usize = 4;

pub fn variant(n: usize, seed: u64) -> Spec {
    let base = model::spec();
    let mut s = match n % VARIANTS {
        0 => base,
        1 => Spec {
            hidden: vec![64, 32],
            ..base
        },
        2 => Spec {
            sound_k: 32,
            semantic_k: 16,
            ..base
        },
        _ => Spec {
            hidden: vec![],
            weight_decay: 1e-2,
            ..base
        },
    };
    s.seed = seed;
    s
}

fn desktop(platform: &str) -> bool {
    matches!(platform, "macos" | "windows" | "linux")
}

/// Выдать задание компьютеру, если пора. `None` — учить нечего (204).
pub async fn claim(
    db: &PgPool,
    principal: &Principal,
    scope: TrainScope,
) -> AppResult<Option<TrainClaim>> {
    let device = principal
        .device_id
        .ok_or_else(|| bad("учить сеть вкуса может только приложение на компьютере"))?;
    let platform = sqlx::query_scalar!(
        "SELECT platform FROM devices WHERE id = $1 AND revoked_at IS NULL",
        device
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::Unauthorized)?;
    if !desktop(&platform) {
        return Err(AppError::Forbidden(
            "учить сеть вкуса можно только на компьютере".into(),
        ));
    }
    let admin = principal.can(Permission::Admin);
    let user = match scope {
        TrainScope::Global if !admin => {
            return Err(AppError::Forbidden(
                "общую сеть учат только компьютеры администраторов".into(),
            ));
        }
        TrainScope::Global => None,
        TrainScope::Auto if admin => None,
        TrainScope::Auto => return Ok(None),
        TrainScope::Personal => Some(principal.user.id),
    };

    sqlx::query!(
        "UPDATE taste_train_tasks SET status = 'expired', finished_at = now()
         WHERE status = 'leased' AND leased_at < now() - make_interval(hours => $1)",
        LEASE_HOURS as i32,
    )
    .execute(db)
    .await?;
    let due = sqlx::query_scalar!(
        r#"SELECT NOT EXISTS (SELECT 1 FROM taste_train_tasks
                              WHERE device_id = $1
                                AND (status = 'leased' OR leased_at > now() - make_interval(hours => $3)))
                  AND NOT EXISTS (SELECT 1 FROM taste_train_tasks
                                  WHERE user_id IS NOT DISTINCT FROM $2
                                    AND (status = 'leased' OR (status = 'done' AND finished_at > now() - make_interval(hours => $4))))
                  AND NOT EXISTS (SELECT 1 FROM model_versions
                                  WHERE kind = 'taste' AND user_id IS NOT DISTINCT FROM $2 AND origin = 'device'
                                    AND created_at > now() - make_interval(hours => $4))
                  AS "due!""#,
        device,
        user,
        DEVICE_EVERY_HOURS as i32,
        EVERY_HOURS as i32,
    )
    .fetch_one(db)
    .await?;
    if !due {
        return Ok(None);
    }
    let data = model::dataset(db, &Recipe::default(), user, None).await?;
    let Some(cut) = data.holdout_cut().filter(|_| data.enough()) else {
        return Ok(None);
    };
    let done = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM taste_train_tasks WHERE user_id IS NOT DISTINCT FROM $1"#,
        user
    )
    .fetch_one(db)
    .await?;
    let seed = u64::from_le_bytes(
        Uuid::now_v7().as_bytes()[8..]
            .try_into()
            .unwrap_or_default(),
    );
    let spec = variant(done as usize, seed);
    let train = data.train_part();
    let package = wire::Package::new(spec.clone(), &train, &model::SCALARS);
    let spec_json = serde_json::to_value(&spec).map_err(anyhow::Error::from)?;
    let task_id = sqlx::query_scalar!(
        "INSERT INTO taste_train_tasks (user_id, device_id, requested_by, holdout_cut, rows, spec)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        user,
        device,
        principal.user.id,
        cut,
        train.rows() as i32,
        spec_json,
    )
    .fetch_one(db)
    .await?;
    tracing::info!(task = %task_id, rows = train.rows(), global = user.is_none(), "taste training task leased to a device");
    Ok(Some(TrainClaim {
        task_id,
        scope: if user.is_none() {
            TrainScope::Global
        } else {
            TrainScope::Personal
        },
        rows: train.rows() as i64,
        package: serde_json::to_value(&package).map_err(anyhow::Error::from)?,
    }))
}

struct Task {
    user_id: Option<Uuid>,
    device_id: Uuid,
    requested_by: Uuid,
    holdout_cut: DateTime<Utc>,
    spec: Value,
    device_name: String,
}

/// Задание этого компьютера, которое ещё ждёт веса; отмечается как «прислано», чтобы второй
/// запрос не проверял то же самое.
async fn take(db: &PgPool, principal: &Principal, id: Uuid) -> AppResult<Task> {
    let task = sqlx::query!(
        r#"SELECT t.user_id, t.device_id, t.requested_by, t.holdout_cut, t.spec, t.status, t.leased_at,
                  t.finished_at, d.name AS device_name
           FROM taste_train_tasks t JOIN devices d ON d.id = t.device_id WHERE t.id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    if Some(task.device_id) != principal.device_id || task.requested_by != principal.user.id {
        return Err(AppError::NotFound);
    }
    // Общую сеть присылает только администратор — права проверяются заново при отправке
    if task.user_id.is_none() && !principal.can(Permission::Admin) {
        return Err(AppError::Forbidden(
            "общую сеть присылают только администраторы".into(),
        ));
    }
    if task.status != "leased"
        || task.finished_at.is_some()
        || task.leased_at < Utc::now() - Duration::hours(LEASE_HOURS)
    {
        return Err(AppError::Conflict {
            code: "task_closed",
            message: "задание уже закрыто или просрочено".into(),
        });
    }
    let taken = sqlx::query!(
        "UPDATE taste_train_tasks SET finished_at = now() WHERE id = $1 AND finished_at IS NULL",
        id
    )
    .execute(db)
    .await?;
    if taken.rows_affected() == 0 {
        return Err(AppError::Conflict {
            code: "task_closed",
            message: "веса этого задания уже присланы".into(),
        });
    }
    Ok(Task {
        user_id: task.user_id,
        device_id: task.device_id,
        requested_by: task.requested_by,
        holdout_cut: task.holdout_cut,
        spec: task.spec,
        device_name: task.device_name,
    })
}

async fn close(db: &PgPool, id: Uuid, status: &str, result: Value) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE taste_train_tasks SET status = $2, result = $3, finished_at = now() WHERE id = $1",
        id,
        status,
        result,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Веса с компьютера: проверка на своей отложенной части и решение.
pub async fn submit(
    db: &PgPool,
    storage: &Storage,
    principal: &Principal,
    id: Uuid,
    body: TrainSubmit,
) -> AppResult<TrainOutcome> {
    let task = take(db, principal, id).await?;
    let refuse = |message: String| async move {
        close(db, id, "failed", json!({ "message": message })).await?;
        Err::<TrainOutcome, AppError>(AppError::Unprocessable {
            code: "bad_weights",
            message,
        })
    };
    let params = match serde_json::from_value::<wire::WireParams>(body.params)
        .map_err(|e| e.to_string())
        .and_then(|w| w.decode())
    {
        Ok(p) => p,
        Err(e) => return refuse(format!("веса не приняты: {e}")).await,
    };
    // Метрики компьютера — только для журнала: решает проверка сервера
    let report: TrainReport = serde_json::from_value(body.report).unwrap_or_default();
    let spec: Spec = serde_json::from_value(task.spec).unwrap_or_default();
    let data = Arc::new(
        model::dataset(db, &Recipe::default(), task.user_id, Some(task.holdout_cut)).await?,
    );
    if data.rows() - data.holdout_from < model::MIN_HOLDOUT {
        return refuse("на отложенной части слишком мало строк".into()).await;
    }
    let (onnx, eval, verdict) = match model::check(db, data, &params, task.user_id).await {
        Ok(x) => x,
        Err(e) => return refuse(format!("веса не прошли проверку: {e}")).await,
    };
    let stored = model::store(
        db,
        storage,
        model::NewVersion {
            params: &params,
            report: &report,
            spec: &spec,
            eval: &eval,
            verdict: &verdict,
            user_id: task.user_id,
            device: Some((task.device_id, task.device_name)),
            onnx,
        },
    )
    .await?;
    close(
        db,
        id,
        "done",
        json!({ "version": stored.version, "active": stored.active, "message": stored.note,
                "requestedBy": task.requested_by }),
    )
    .await?;
    Ok(TrainOutcome {
        version: stored.version,
        active: stored.active,
        note: stored.note,
    })
}

/// Компьютер не смог обучить: задание закрывается, следующий раз — новое.
pub async fn fail(db: &PgPool, principal: &Principal, id: Uuid, message: &str) -> AppResult<()> {
    take(db, principal, id).await?;
    let message: String = message.chars().take(500).collect();
    close(db, id, "failed", json!({ "message": message })).await?;
    Ok(())
}

/// Последние задания для админки.
pub async fn recent(db: &PgPool) -> Result<Vec<TrainTaskView>, sqlx::Error> {
    Ok(sqlx::query!(
        r#"SELECT t.id, d.name AS device_name, u.name AS "user_name?", t.status, t.rows, t.result,
                  t.leased_at, t.finished_at
           FROM taste_train_tasks t
           JOIN devices d ON d.id = t.device_id
           LEFT JOIN users u ON u.id = t.user_id
           ORDER BY t.leased_at DESC, t.id DESC LIMIT 10"#
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| {
        let result = r.result.unwrap_or(Value::Null);
        TrainTaskView {
            id: r.id,
            device_name: r.device_name,
            user_name: r.user_name,
            status: r.status,
            rows: i64::from(r.rows),
            version: result
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string),
            message: result
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string),
            leased_at: r.leased_at,
            finished_at: r.finished_at,
        }
    })
    .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_differ_and_keep_the_seed() {
        let specs: Vec<Spec> = (0..VARIANTS).map(|n| variant(n, 42)).collect();
        for (i, a) in specs.iter().enumerate() {
            assert_eq!(a.seed, 42);
            assert!(a.validate().is_ok());
            for b in &specs[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert_eq!(variant(VARIANTS, 1).hidden, model::spec().hidden);
    }
}
