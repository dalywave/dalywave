//! Очередь задач на Postgres. Постановка с `dedupe_key` (одно и то же не ставится дважды), выборка
//! `FOR UPDATE SKIP LOCKED` с учётом паузы, параллельности и интервала полосы, пульс исполнителя,
//! повтор с растущей задержкой до `max_attempts`, возврат зависших задач в очередь.
//! Исполнитель — бинарник `dalywave-worker` ([`worker`]); обработчики по видам — [`handlers`].

pub mod handlers;
pub mod worker;

use std::time::Duration;

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    jobs::{Job, JobStatus},
    realtime::{JobUpdate, ServerEvent},
};
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::realtime::{self, Audience};

/// Канал NOTIFY с именем полосы, где появилась задача: воркер просыпается сразу, а не по таймеру.
pub const CHANNEL: &str = "dalywave_jobs";
/// Первая пауза перед повтором; дальше удваивается.
const RETRY_BASE: Duration = Duration::from_secs(30);
const RETRY_MAX: Duration = Duration::from_secs(3600);

pub mod lanes {
    pub const NOW: &str = "now";
    pub const WAVE: &str = "wave";
    pub const LIBRARY: &str = "library";
    pub const DISCOGRAPHY: &str = "discography";
    pub const DISCOVERY: &str = "discovery";
    pub const ANALYSIS: &str = "analysis";
    pub const SCOUT: &str = "scout";
    pub const MIRROR: &str = "mirror";
    pub const MAINTENANCE: &str = "maintenance";
}

#[derive(Debug, Clone)]
pub struct JobRow {
    pub id: Uuid,
    pub kind: String,
    pub lane: String,
    pub priority: i16,
    pub payload: Value,
    pub status: String,
    pub attempts: i16,
    pub max_attempts: i16,
    pub run_after: DateTime<Utc>,
    pub locked_by: Option<String>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub result: Option<Value>,
    pub dedupe_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

fn parse_status(s: &str) -> JobStatus {
    // Колонка ограничена CHECK теми же значениями
    s.parse().unwrap_or(JobStatus::Failed)
}

impl JobRow {
    pub fn status(&self) -> JobStatus {
        parse_status(&self.status)
    }

    pub fn view(self) -> Job {
        Job {
            status: parse_status(&self.status),
            id: self.id,
            kind: self.kind,
            lane: self.lane,
            priority: self.priority,
            attempts: self.attempts,
            max_attempts: self.max_attempts,
            run_after: self.run_after,
            locked_by: self.locked_by,
            heartbeat_at: self.heartbeat_at,
            last_error: self.last_error,
            payload: self.payload,
            result: self.result,
            dedupe_key: self.dedupe_key,
            created_at: self.created_at,
            updated_at: self.updated_at,
            finished_at: self.finished_at,
        }
    }

    pub fn event(&self) -> ServerEvent {
        ServerEvent::JobUpdated(JobUpdate {
            id: self.id,
            kind: self.kind.clone(),
            lane: self.lane.clone(),
            status: self.status(),
            attempts: self.attempts,
            max_attempts: self.max_attempts,
            last_error: self.last_error.clone(),
            run_after: self.run_after,
        })
    }

    pub fn payload<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}

/// Новая задача.
#[derive(Debug, Clone)]
pub struct NewJob {
    pub kind: &'static str,
    pub lane: &'static str,
    pub payload: Value,
    pub priority: i16,
    pub max_attempts: i16,
    pub dedupe_key: Option<String>,
}

impl NewJob {
    pub fn new(kind: &'static str, lane: &'static str, payload: impl Serialize) -> Self {
        Self {
            kind,
            lane,
            payload: serde_json::to_value(payload).unwrap_or(Value::Null),
            priority: 0,
            max_attempts: 3,
            dedupe_key: None,
        }
    }

    /// Пока задача с этим ключом в очереди или в работе, вторая такая не ставится.
    pub fn dedupe(mut self, key: impl Into<String>) -> Self {
        self.dedupe_key = Some(key.into());
        self
    }

    pub fn priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }

    pub fn max_attempts(mut self, attempts: i16) -> Self {
        self.max_attempts = attempts.max(1);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enqueued {
    pub id: Uuid,
    /// `false` — такая задача уже стояла (`dedupe_key`), вернулся её id.
    pub created: bool,
}

/// Ставит задачу в очередь и будит воркер. В транзакции воркер проснётся после коммита.
pub async fn enqueue(conn: &mut PgConnection, job: NewJob) -> Result<Enqueued, sqlx::Error> {
    // Задача-двойник может закончиться между вставкой и поиском: тогда пробуем вставить ещё раз
    for _ in 0..3 {
        let inserted = sqlx::query_scalar!(
            "INSERT INTO jobs (kind, lane, priority, payload, max_attempts, dedupe_key)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (dedupe_key) WHERE dedupe_key IS NOT NULL AND status IN ('queued', 'running') DO NOTHING
             RETURNING id",
            job.kind,
            job.lane,
            job.priority,
            job.payload,
            job.max_attempts,
            job.dedupe_key,
        )
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(id) = inserted {
            sqlx::query("SELECT pg_notify($1, $2)")
                .bind(CHANNEL)
                .bind(job.lane)
                .execute(&mut *conn)
                .await?;
            return Ok(Enqueued { id, created: true });
        }
        let existing = sqlx::query_scalar!(
            "SELECT id FROM jobs WHERE dedupe_key = $1 AND status IN ('queued', 'running')",
            job.dedupe_key,
        )
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(id) = existing {
            return Ok(Enqueued { id, created: false });
        }
    }
    Err(sqlx::Error::Protocol(format!(
        "could not enqueue {} with dedupe key {:?}",
        job.kind, job.dedupe_key
    )))
}

/// Итог попытки взять задачу.
#[derive(Debug)]
pub enum Claim {
    Job(Box<JobRow>),
    /// Задач нет; если есть отложенные — когда ближайшая.
    Empty(Option<Duration>),
    /// Полоса занята на всю параллельность.
    Full,
    /// Пауза между задачами полосы ещё не прошла.
    Wait(Duration),
    /// Полоса на паузе или её нет.
    Paused,
}

/// Берёт следующую задачу полосы. Строка полосы блокируется на время выборки, поэтому параллельность
/// и интервал соблюдаются для всех процессов-исполнителей сразу.
pub async fn claim(db: &PgPool, lane: &str, worker: &str) -> Result<Claim, sqlx::Error> {
    let mut tx = db.begin().await?;
    let Some(l) = sqlx::query!(
        r#"SELECT paused, concurrency,
                  COALESCE(GREATEST(0, EXTRACT(EPOCH FROM (last_claimed_at + make_interval(secs => min_interval_ms / 1000.0) - now())) * 1000), 0)::bigint AS "wait_ms!",
                  (SELECT count(*) FROM jobs WHERE lane = $1 AND status = 'running') AS "running!"
           FROM job_lanes WHERE lane = $1 FOR UPDATE"#,
        lane,
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(Claim::Paused);
    };
    if l.paused {
        return Ok(Claim::Paused);
    }
    if l.running >= i64::from(l.concurrency) {
        return Ok(Claim::Full);
    }
    if l.wait_ms > 0 {
        return Ok(Claim::Wait(Duration::from_millis(l.wait_ms as u64)));
    }
    let job = sqlx::query_as!(
        JobRow,
        "UPDATE jobs SET status = 'running', attempts = attempts + 1, locked_by = $2, locked_at = now(),
                heartbeat_at = now(), updated_at = now()
         WHERE id = (
            SELECT id FROM jobs
            WHERE lane = $1 AND status = 'queued' AND run_after <= now()
            ORDER BY priority DESC, run_after, id
            LIMIT 1
            FOR UPDATE SKIP LOCKED
         )
         RETURNING id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                   heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at",
        lane,
        worker,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(job) = job else {
        let next = sqlx::query_scalar!(
            r#"SELECT (EXTRACT(EPOCH FROM (min(run_after) - now())) * 1000)::bigint AS "ms"
               FROM jobs WHERE lane = $1 AND status = 'queued'"#,
            lane,
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(Claim::Empty(
            next.map(|ms| Duration::from_millis(ms.max(0) as u64)),
        ));
    };
    sqlx::query!(
        "UPDATE job_lanes SET last_claimed_at = now() WHERE lane = $1",
        lane
    )
    .execute(&mut *tx)
    .await?;
    realtime::publish(&mut *tx, Audience::Admins, job.event()).await;
    tx.commit().await?;
    Ok(Claim::Job(Box::new(job)))
}

/// Пульс исполнителя. `false` — задача больше не его: отменена или возвращена в очередь.
pub async fn heartbeat(db: &PgPool, id: Uuid, worker: &str) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!(
        "UPDATE jobs SET heartbeat_at = now() WHERE id = $1 AND status = 'running' AND locked_by = $2",
        id,
        worker,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected() == 1)
}

pub async fn complete(
    db: &PgPool,
    id: Uuid,
    worker: &str,
    result: Value,
) -> Result<Option<JobRow>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let job = sqlx::query_as!(
        JobRow,
        "UPDATE jobs SET status = 'done', result = $3, last_error = NULL, finished_at = now(), updated_at = now()
         WHERE id = $1 AND status = 'running' AND locked_by = $2
         RETURNING id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                   heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at",
        id,
        worker,
        result,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(job) = &job {
        realtime::publish(&mut *tx, Audience::Admins, job.event()).await;
    }
    tx.commit().await?;
    Ok(job)
}

/// Пауза перед повтором: 30 с, 1 мин, 2 мин … до часа, ±20 %, чтобы повторы не сбивались в кучу.
pub fn retry_delay(attempt: i16) -> Duration {
    let exp = u32::try_from(attempt.max(1) - 1).unwrap_or(0).min(16);
    let base = RETRY_BASE
        .saturating_mul(2u32.saturating_pow(exp))
        .min(RETRY_MAX);
    let jitter = crate::crypto::random_bytes::<1>().map_or(128, |b| b[0]);
    // 0.8 … 1.2
    base.mul_f64(0.8 + f64::from(jitter) / 255.0 * 0.4)
}

/// Неудача. С `retry` и неистраченными попытками задача вернётся в очередь позже, иначе — `failed`.
pub async fn fail(
    db: &PgPool,
    id: Uuid,
    worker: &str,
    error: &str,
    retry: bool,
) -> Result<Option<JobRow>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let attempts = sqlx::query_scalar!(
        "SELECT attempts FROM jobs WHERE id = $1 AND status = 'running' AND locked_by = $2 FOR UPDATE",
        id,
        worker,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(attempts) = attempts else {
        return Ok(None);
    };
    let delay_ms = retry.then(|| retry_delay(attempts).as_millis() as i64);
    let error: String = error.chars().take(2000).collect();
    let job = sqlx::query_as!(
        JobRow,
        "UPDATE jobs SET
            status = CASE WHEN $3::bigint IS NOT NULL AND attempts < max_attempts THEN 'queued' ELSE 'failed' END,
            run_after = CASE WHEN $3::bigint IS NOT NULL AND attempts < max_attempts
                             THEN now() + make_interval(secs => $3::bigint / 1000.0) ELSE run_after END,
            finished_at = CASE WHEN $3::bigint IS NOT NULL AND attempts < max_attempts THEN NULL ELSE now() END,
            last_error = $2, updated_at = now()
         WHERE id = $1
         RETURNING id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                   heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at",
        id,
        error,
        delay_ms,
    )
    .fetch_one(&mut *tx)
    .await?;
    if job.status() == JobStatus::Queued {
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(CHANNEL)
            .bind(&job.lane)
            .execute(&mut *tx)
            .await?;
    }
    realtime::publish(&mut *tx, Audience::Admins, job.event()).await;
    tx.commit().await?;
    Ok(Some(job))
}

/// Возвращает в очередь задачи, у которых исполнитель перестал слать пульс. Попытка засчитана
/// при взятии, поэтому задача с исчерпанными попытками закрывается как `failed`.
pub async fn reap(db: &PgPool, stale_after: Duration) -> Result<Vec<JobRow>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let jobs = sqlx::query_as!(
        JobRow,
        "UPDATE jobs SET
            status = CASE WHEN attempts < max_attempts THEN 'queued' ELSE 'failed' END,
            finished_at = CASE WHEN attempts < max_attempts THEN NULL ELSE now() END,
            last_error = 'исполнитель перестал отвечать', locked_by = NULL, updated_at = now()
         WHERE status = 'running' AND COALESCE(heartbeat_at, locked_at, updated_at) < now() - make_interval(secs => $1)
         RETURNING id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                   heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at",
        stale_after.as_secs_f64(),
    )
    .fetch_all(&mut *tx)
    .await?;
    for job in &jobs {
        tracing::warn!(id = %job.id, kind = %job.kind, status = %job.status, "reaped a stale job");
        realtime::publish(&mut *tx, Audience::Admins, job.event()).await;
    }
    tx.commit().await?;
    Ok(jobs)
}

/// Остановка воркера: недоделанные задачи возвращаются в очередь, попытка не засчитывается.
pub async fn release(db: &PgPool, worker: &str) -> Result<u64, sqlx::Error> {
    let res = sqlx::query!(
        "UPDATE jobs SET status = 'queued', attempts = GREATEST(attempts - 1, 0), locked_by = NULL,
                run_after = now(), updated_at = now()
         WHERE status = 'running' AND locked_by = $1",
        worker,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// Отмена задачи из очереди или из работы. `None` — задачи нет или она уже закончилась.
pub async fn cancel(conn: &mut PgConnection, id: Uuid) -> Result<Option<JobRow>, sqlx::Error> {
    let job = sqlx::query_as!(
        JobRow,
        "UPDATE jobs SET status = 'cancelled', finished_at = now(), updated_at = now()
         WHERE id = $1 AND status IN ('queued', 'running')
         RETURNING id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                   heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at",
        id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(job) = &job {
        realtime::publish(&mut *conn, Audience::Admins, job.event()).await;
    }
    Ok(job)
}

/// Повтор закрытой задачи: попытки с нуля, сразу в очередь. `Err` с нарушением уникальности —
/// такая же задача уже стоит в очереди.
pub async fn retry(conn: &mut PgConnection, id: Uuid) -> Result<Option<JobRow>, sqlx::Error> {
    let job = sqlx::query_as!(
        JobRow,
        "UPDATE jobs SET status = 'queued', attempts = 0, run_after = now(), locked_by = NULL,
                finished_at = NULL, result = NULL, updated_at = now()
         WHERE id = $1 AND status IN ('failed', 'cancelled', 'done')
         RETURNING id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                   heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at",
        id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(job) = &job {
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(CHANNEL)
            .bind(&job.lane)
            .execute(&mut *conn)
            .await?;
        realtime::publish(&mut *conn, Audience::Admins, job.event()).await;
    }
    Ok(job)
}

/// Чистка истории: выполненные храним неделю, неудачные и отменённые — месяц.
pub async fn cleanup(db: &PgPool) -> Result<u64, sqlx::Error> {
    let res = sqlx::query!(
        "DELETE FROM jobs
         WHERE (status = 'done' AND finished_at < now() - interval '7 days')
            OR (status IN ('failed', 'cancelled') AND finished_at < now() - interval '30 days')",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

pub async fn get(db: &PgPool, id: Uuid) -> Result<Option<JobRow>, sqlx::Error> {
    sqlx::query_as!(
        JobRow,
        "SELECT id, kind, lane, priority, payload, status, attempts, max_attempts, run_after, locked_by,
                heartbeat_at, last_error, result, dedupe_key, created_at, updated_at, finished_at
         FROM jobs WHERE id = $1",
        id,
    )
    .fetch_optional(db)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_grows_and_caps() {
        let secs = |a| retry_delay(a).as_secs_f64();
        assert!((24.0..=36.0).contains(&secs(1)), "{}", secs(1));
        assert!((48.0..=72.0).contains(&secs(2)));
        assert!((96.0..=144.0).contains(&secs(3)));
        assert!(secs(30) <= 3600.0 * 1.2);
        assert!(secs(0) <= 36.0);
    }
}
