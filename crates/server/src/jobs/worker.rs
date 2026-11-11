//! Исполнитель очереди (бинарник `dalywave-worker`). На каждую полосу — свой цикл: берёт задачи, пока
//! полоса позволяет, и спит до сигнала NOTIFY или до ближайшей отложенной задачи. Обработчики
//! регистрируются по виду задачи в [`Registry`]; задача без обработчика закрывается сразу.

use std::{
    collections::HashMap, fmt, future::Future, panic::AssertUnwindSafe, sync::Arc, time::Duration,
};

use dalywave_protocol::jobs::{SyncDiscographyPayload, kind};
use futures_util::{FutureExt, future::BoxFuture};
use serde_json::Value;
use sqlx::{PgPool, postgres::PgListener};
use tokio::{
    sync::{Notify, watch},
    task::JoinSet,
};
use tracing::Instrument;

use super::{Claim, JobRow, NewJob, lanes};
use crate::{
    acquire,
    catalog::CatalogError,
    config::Config,
    deezer::{Deezer, DeezerError},
    jobs,
    storage::{Storage, StorageError},
};

/// Всё, что нужно обработчикам задач.
pub struct Context {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub deezer: Arc<Deezer>,
    pub storage: Arc<Storage>,
    /// Для картинок: переходы по редиректам разрешены, таймаут побольше
    pub http: reqwest::Client,
}

impl Context {
    pub fn new(
        db: PgPool,
        config: Arc<Config>,
        deezer: Arc<Deezer>,
        storage: Arc<Storage>,
    ) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("dalywave/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            db,
            config,
            deezer,
            storage,
            http,
        })
    }
}

/// Неудача обработчика: повторять ли задачу позже.
#[derive(Debug)]
pub struct JobError {
    pub message: String,
    pub retry: bool,
}

impl JobError {
    pub fn retry(message: impl fmt::Display) -> Self {
        Self {
            message: message.to_string(),
            retry: true,
        }
    }

    /// Повтор не поможет: неверные данные, объекта нет в источнике.
    pub fn permanent(message: impl fmt::Display) -> Self {
        Self {
            message: message.to_string(),
            retry: false,
        }
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<sqlx::Error> for JobError {
    fn from(e: sqlx::Error) -> Self {
        Self::retry(format!("база: {e}"))
    }
}

impl From<anyhow::Error> for JobError {
    fn from(e: anyhow::Error) -> Self {
        Self::retry(format!("{e:#}"))
    }
}

impl From<DeezerError> for JobError {
    fn from(e: DeezerError) -> Self {
        let retry = e.is_temporary();
        Self {
            message: e.to_string(),
            retry,
        }
    }
}

impl From<CatalogError> for JobError {
    fn from(e: CatalogError) -> Self {
        match e {
            CatalogError::Deezer(e) => e.into(),
            CatalogError::Db(e) => e.into(),
        }
    }
}

impl From<StorageError> for JobError {
    fn from(e: StorageError) -> Self {
        Self::retry(format!("хранилище: {e}"))
    }
}

impl From<serde_json::Error> for JobError {
    fn from(e: serde_json::Error) -> Self {
        Self::permanent(format!("неверные данные задачи: {e}"))
    }
}

pub type JobResult = Result<Value, JobError>;

/// Обработчик задач одного вида. Подходит любая `async fn(Arc<Context>, JobRow) -> JobResult`.
pub trait Handler: Send + Sync + 'static {
    fn run(&self, ctx: Arc<Context>, job: JobRow) -> BoxFuture<'static, JobResult>;
}

impl<F, Fut> Handler for F
where
    F: Fn(Arc<Context>, JobRow) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = JobResult> + Send + 'static,
{
    fn run(&self, ctx: Arc<Context>, job: JobRow) -> BoxFuture<'static, JobResult> {
        Box::pin(self(ctx, job))
    }
}

/// Обработчики по видам задач. Этап 1b добавит сюда `acquire`.
#[derive(Default, Clone)]
pub struct Registry {
    handlers: HashMap<String, Arc<dyn Handler>>,
}

impl Registry {
    pub fn register(&mut self, kind: &str, handler: impl Handler) -> &mut Self {
        self.handlers.insert(kind.to_string(), Arc::new(handler));
        self
    }

    pub fn get(&self, kind: &str) -> Option<Arc<dyn Handler>> {
        self.handlers.get(kind).cloned()
    }

    pub fn kinds(&self) -> Vec<&str> {
        let mut kinds: Vec<&str> = self.handlers.keys().map(String::as_str).collect();
        kinds.sort_unstable();
        kinds
    }
}

#[derive(Debug, Clone)]
pub struct Settings {
    /// Как часто смотреть в очередь без сигнала NOTIFY.
    pub poll: Duration,
    pub heartbeat: Duration,
    /// Без пульса дольше — задача возвращается в очередь.
    pub stale_after: Duration,
    pub reap_every: Duration,
    /// Сверка подписок и чистка истории.
    pub schedule_every: Duration,
    /// Сколько ждать начатые задачи при остановке.
    pub grace: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(5),
            heartbeat: Duration::from_secs(15),
            stale_after: Duration::from_secs(120),
            reap_every: Duration::from_secs(60),
            schedule_every: Duration::from_secs(3600),
            grace: Duration::from_secs(30),
        }
    }
}

/// Что стало с задачей после запуска.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// Вернётся в очередь позже.
    Retry(String),
    Failed(String),
    /// Задачу отменили или забрали, пока она шла.
    Lost,
}

pub struct Worker {
    ctx: Arc<Context>,
    registry: Arc<Registry>,
    id: String,
    settings: Settings,
}

impl Worker {
    pub fn new(ctx: Arc<Context>, registry: Registry, settings: Settings) -> Self {
        let host = std::env::var("HOSTNAME")
            .ok()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "worker".into());
        Self {
            ctx,
            registry: Arc::new(registry),
            id: format!("{host}:{}", std::process::id()),
            settings,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Берёт одну задачу полосы и выполняет её до конца. Для тестов и ручного запуска.
    pub async fn run_once(&self, lane: &str) -> anyhow::Result<Option<(JobRow, Outcome)>> {
        match jobs::claim(&self.ctx.db, lane, &self.id).await? {
            Claim::Job(job) => {
                let job = *job;
                let outcome = self.execute(job.clone()).await;
                Ok(Some((job, outcome)))
            }
            _ => Ok(None),
        }
    }

    /// Работает до сигнала `shutdown`, затем даёт начатым задачам доработать.
    pub async fn run(self, shutdown: impl Future<Output = ()>) -> anyhow::Result<()> {
        let this = Arc::new(self);
        let lanes: Vec<String> = sqlx::query_scalar!("SELECT lane FROM job_lanes ORDER BY lane")
            .fetch_all(&this.ctx.db)
            .await?;
        let wake: Arc<HashMap<String, Arc<Notify>>> = Arc::new(
            lanes
                .iter()
                .map(|l| (l.clone(), Arc::new(Notify::new())))
                .collect(),
        );
        let (stop_tx, stop_rx) = watch::channel(false);
        let mut tasks = JoinSet::new();
        tasks.spawn(listen(this.ctx.db.clone(), wake.clone(), stop_rx.clone()));
        tasks.spawn(this.clone().maintenance(stop_rx.clone()));
        for lane in &lanes {
            let notify = wake.get(lane).cloned().unwrap_or_default();
            tasks.spawn(
                this.clone()
                    .lane_loop(lane.clone(), notify, stop_rx.clone()),
            );
        }
        tracing::info!(id = %this.id, lanes = lanes.len(), kinds = ?this.registry.kinds(), "worker started");
        shutdown.await;
        tracing::info!("worker stopping");
        let _ = stop_tx.send(true);
        while tasks.join_next().await.is_some() {}
        match jobs::release(&this.ctx.db, &this.id).await {
            Ok(0) => {}
            Ok(n) => tracing::warn!(released = n, "unfinished jobs returned to the queue"),
            Err(e) => tracing::error!(error = %e, "could not release unfinished jobs"),
        }
        tracing::info!("worker stopped");
        Ok(())
    }

    async fn lane_loop(
        self: Arc<Self>,
        lane: String,
        wake: Arc<Notify>,
        mut stop: watch::Receiver<bool>,
    ) {
        let mut running: JoinSet<()> = JoinSet::new();
        loop {
            if *stop.borrow() {
                break;
            }
            while running.try_join_next().is_some() {}
            let wait = match jobs::claim(&self.ctx.db, &lane, &self.id).await {
                Ok(Claim::Job(job)) => {
                    let me = self.clone();
                    running.spawn(async move {
                        me.execute(*job).await;
                    });
                    continue;
                }
                Ok(Claim::Empty(next)) => next.unwrap_or(self.settings.poll),
                Ok(Claim::Wait(d)) => d,
                Ok(Claim::Full | Claim::Paused) => self.settings.poll,
                Err(e) => {
                    tracing::warn!(%lane, error = %e, "could not claim a job");
                    self.settings.poll
                }
            };
            let wait = wait.clamp(Duration::from_millis(20), self.settings.poll);
            tokio::select! {
                _ = wake.notified() => {}
                Some(_) = running.join_next(), if !running.is_empty() => {}
                _ = tokio::time::sleep(wait) => {}
                _ = stop.changed() => break,
            }
        }
        let grace = tokio::time::sleep(self.settings.grace);
        tokio::pin!(grace);
        loop {
            tokio::select! {
                next = running.join_next() => if next.is_none() { break },
                _ = &mut grace => {
                    tracing::warn!(%lane, left = running.len(), "jobs did not finish in time, aborting");
                    running.abort_all();
                    break;
                }
            }
        }
    }

    async fn execute(&self, job: JobRow) -> Outcome {
        let span = tracing::info_span!("job", id = %job.id, kind = %job.kind, lane = %job.lane, attempt = job.attempts);
        self.execute_inner(job).instrument(span).await
    }

    async fn execute_inner(&self, job: JobRow) -> Outcome {
        let db = &self.ctx.db;
        let Some(handler) = self.registry.get(&job.kind) else {
            // Вид задачи из следующего этапа: закрываем без повторов, чтобы её было видно в админке
            let message = format!("обработчик задач «{}» ещё не реализован", job.kind);
            tracing::warn!("{message}");
            if let Err(e) = jobs::fail(db, job.id, &self.id, &message, false).await {
                tracing::error!(error = %e, "could not close the job");
            }
            if job.kind == kind::ACQUIRE
                && let Err(e) = acquire::abandoned(db, &job, &message).await
            {
                tracing::error!(error = %e, "could not close the acquisition");
            }
            return Outcome::Failed(message);
        };
        let started = std::time::Instant::now();
        let work = AssertUnwindSafe(handler.run(self.ctx.clone(), job.clone())).catch_unwind();
        let heartbeat = async {
            let mut tick = tokio::time::interval(self.settings.heartbeat);
            tick.tick().await;
            loop {
                tick.tick().await;
                match jobs::heartbeat(db, job.id, &self.id).await {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(e) => tracing::warn!(error = %e, "heartbeat failed"),
                }
            }
        };
        let res = tokio::select! {
            res = work => res,
            () = heartbeat => {
                tracing::info!("job was cancelled or taken away, stopped");
                return Outcome::Lost;
            }
        };
        let ms = started.elapsed().as_millis() as u64;
        let (closed, outcome) = match res {
            Ok(Ok(result)) => {
                tracing::info!(ms, "job done");
                (
                    jobs::complete(db, job.id, &self.id, result).await,
                    Outcome::Done,
                )
            }
            Ok(Err(e)) => {
                tracing::warn!(ms, retry = e.retry, error = %e.message, "job failed");
                let closed = jobs::fail(db, job.id, &self.id, &e.message, e.retry).await;
                let outcome = match &closed {
                    Ok(Some(j)) if j.status == "queued" => Outcome::Retry(e.message),
                    _ => Outcome::Failed(e.message),
                };
                (closed, outcome)
            }
            Err(_) => {
                let message = "обработчик упал с паникой".to_string();
                tracing::error!(ms, "{message}");
                let closed = jobs::fail(db, job.id, &self.id, &message, true).await;
                (closed, Outcome::Retry(message))
            }
        };
        match closed {
            Ok(Some(_)) => outcome,
            Ok(None) => Outcome::Lost,
            Err(e) => {
                tracing::error!(error = %e, "could not record the job result");
                outcome
            }
        }
    }

    /// Возврат зависших задач раз в минуту; раз в час — сверка подписок и чистка истории.
    async fn maintenance(self: Arc<Self>, mut stop: watch::Receiver<bool>) {
        let mut reap = tokio::time::interval(self.settings.reap_every);
        let mut schedule = tokio::time::interval(self.settings.schedule_every);
        reap.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        schedule.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = reap.tick() => {
                    if let Err(e) = jobs::reap(&self.ctx.db, self.settings.stale_after).await {
                        tracing::warn!(error = %e, "reaper failed");
                    }
                }
                _ = schedule.tick() => {
                    match schedule_discography(&self.ctx.db).await {
                        Ok(0) => {}
                        Ok(n) => tracing::info!(artists = n, "discography sync scheduled"),
                        Err(e) => tracing::warn!(error = %e, "could not schedule discography sync"),
                    }
                    match jobs::cleanup(&self.ctx.db).await {
                        Ok(0) => {}
                        Ok(n) => tracing::info!(deleted = n, "old jobs cleaned up"),
                        Err(e) => tracing::warn!(error = %e, "job cleanup failed"),
                    }
                }
                _ = stop.changed() => break,
            }
        }
    }
}

/// Ставит сверку дискографии подписанным артистам, которых не сверяли больше суток.
pub async fn schedule_discography(db: &PgPool) -> Result<usize, sqlx::Error> {
    let artists = sqlx::query_scalar!(
        "SELECT DISTINCT a.id FROM followed_artists f JOIN artists a ON a.id = f.artist_id
         WHERE a.discography_synced_at IS NULL OR a.discography_synced_at < now() - interval '24 hours'",
    )
    .fetch_all(db)
    .await?;
    let mut created = 0;
    let mut conn = db.acquire().await?;
    for artist_id in artists {
        let job = jobs::enqueue(
            &mut conn,
            NewJob::new(
                kind::SYNC_DISCOGRAPHY,
                lanes::MAINTENANCE,
                SyncDiscographyPayload { artist_id },
            )
            .dedupe(format!("sync_discography:{artist_id}")),
        )
        .await?;
        created += usize::from(job.created);
    }
    Ok(created)
}

/// Будит циклы полос по NOTIFY. Без связи с базой циклы всё равно опрашивают очередь по таймеру.
async fn listen(
    db: PgPool,
    wake: Arc<HashMap<String, Arc<Notify>>>,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        let mut listener = match PgListener::connect_with(&db).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error = %e, "job listener could not connect");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => continue,
                    _ = stop.changed() => return,
                }
            }
        };
        if let Err(e) = listener.listen(jobs::CHANNEL).await {
            tracing::warn!(error = %e, "job listener could not subscribe");
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => continue,
                _ = stop.changed() => return,
            }
        }
        loop {
            let next = tokio::select! {
                next = listener.recv() => next,
                _ = stop.changed() => return,
            };
            match next {
                Ok(n) => {
                    if let Some(notify) = wake.get(n.payload()) {
                        notify.notify_one();
                    }
                }
                Err(sqlx::Error::PoolClosed) => return,
                Err(e) => {
                    tracing::warn!(error = %e, "job listener failed, reconnecting");
                    break;
                }
            }
        }
    }
}
