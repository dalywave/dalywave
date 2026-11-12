//! Скачивание аудио записи: постановка, выдача исполнителю, приём результата.
//!
//! Задачу `acquire` выполняет тот, кто её взял. Домашний загрузчик (`fetcher worker` на Mac)
//! берёт её через `/api/fetcher/*` ([`claim_for_downloader`], отчёт — [`result::report`]); воркер — через
//! серверный fetcher ([`server::run`]), только с `server` в `DALYWAVE_FETCHER_MODE`. Оба пути сходятся
//! в [`result::examine`] и [`result::record`]: проверка файла в хранилище, строка `audio_files`, основной
//! файл записи, `recordings.audio_status`, события `acquisition.updated` и `library.changed`.

pub mod result;
pub mod server;

use std::time::Duration;

use chrono::Utc;
use dalywave_protocol::{
    catalog::AudioStatus,
    fetcher::{FetcherJob, UploadTarget},
    jobs::{AcquirePayload, AcquireReason, Acquisition, AcquisitionStatus, kind},
    realtime::{AcquisitionUpdate, LibraryChange, ServerEvent},
    stream::{AudioCodec, AudioContainer},
    sync::SyncEntity,
};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    catalog,
    deezer::Deezer,
    downloaders,
    error::AppError,
    jobs::{self, Claim, ClaimOptions, JobRow, Kinds, NewJob, lanes},
    realtime::{self, Audience},
    storage::{Storage, UPLOAD_PUT_TTL, keys},
};

/// Приоритет запуска трека: выше всего остального в полосе «Сейчас».
pub const PLAY_PRIORITY: i16 = 10;
/// Решения админки в «Спорных» — тоже «сейчас», но после запусков.
pub const MANUAL_PRIORITY: i16 = 5;
/// Сколько кандидатов fetcher пробует скачать.
pub const CANDIDATE_ATTEMPTS: i16 = 3;
/// Задача, которую не удалось выдать (хранилище не подписало ссылку), ждёт столько.
const GIVE_BACK_DELAY: Duration = Duration::from_secs(30);

/// Разрешён ли переход состояния аудио записи.
///
/// `none → queued → acquiring → ready | disputed | unavailable | failed`; неудачные можно поставить снова,
/// сбой посреди скачивания возвращает запись в очередь; `ready` достижимо отовсюду (загрузка своего файла,
/// ручной выбор в «Спорных»); «нет в источниках» админ может отметить и для спорной или неудачной;
/// сброс в `none` — из любого, кроме `ready` (файл есть — запись готова).
pub fn can_transition(from: AudioStatus, to: AudioStatus) -> bool {
    use AudioStatus::*;
    match to {
        None => from != Ready,
        Queued => matches!(from, None | Failed | Unavailable | Disputed | Acquiring),
        Acquiring => matches!(from, Queued | None | Failed),
        Ready => true,
        Disputed => matches!(from, Acquiring | Queued),
        Unavailable => matches!(from, Acquiring | Queued | Disputed | Failed | None),
        Failed => matches!(from, Acquiring | Queued),
    }
}

/// Меняет состояние аудио записи, если переход разрешён. `false` — запрещён (или записи нет).
pub async fn set_audio_status(
    conn: &mut PgConnection,
    recording_id: Uuid,
    to: AudioStatus,
) -> Result<bool, sqlx::Error> {
    let from = sqlx::query_scalar!(
        "SELECT audio_status FROM recordings WHERE id = $1 FOR UPDATE",
        recording_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(from) = from.and_then(|s| s.parse::<AudioStatus>().ok()) else {
        return Ok(false);
    };
    if from == to {
        return Ok(true);
    }
    if !can_transition(from, to) {
        tracing::warn!(%recording_id, %from, %to, "audio status transition refused");
        return Ok(false);
    }
    sqlx::query!(
        "UPDATE recordings SET audio_status = $2, status_changed_at = now() WHERE id = $1",
        recording_id,
        to.as_str(),
    )
    .execute(&mut *conn)
    .await?;
    Ok(true)
}

/// Как [`set_audio_status`], но готовую запись не трогает: у неё есть файл, и неудача новой попытки
/// (замена файла, выбор в «Спорных») его не отменяет.
pub async fn set_audio_status_unless_ready(
    conn: &mut PgConnection,
    recording_id: Uuid,
    to: AudioStatus,
) -> Result<bool, sqlx::Error> {
    let ready = sqlx::query_scalar!(
        r#"SELECT audio_status = 'ready' AS "ready!" FROM recordings WHERE id = $1"#,
        recording_id
    )
    .fetch_optional(&mut *conn)
    .await?
    .unwrap_or(false);
    if ready {
        return Ok(false);
    }
    set_audio_status(conn, recording_id, to).await
}

/// Что попросили скачать.
#[derive(Debug, Clone)]
pub struct Request {
    pub track_id: Uuid,
    pub reason: AcquireReason,
    pub lane: &'static str,
    pub requested_by: Option<Uuid>,
    pub priority: i16,
    /// Выбор в «Спорных»: эти видео пробуются первыми и принимаются без совпадения отпечатка.
    pub prefer_video_ids: Vec<String>,
    pub exclude_video_ids: Vec<String>,
    /// Ссылка из «Спорных» (SoundCloud, Bandcamp, …) вместо поиска.
    pub direct_url: Option<String>,
    /// Ставить, даже если запись готова или ждёт решения в админке.
    pub force: bool,
    /// Жалоба «Неверная запись»: новый файл заменяет основной, даже если тот «лучше».
    pub replace: bool,
}

impl Request {
    pub fn new(track_id: Uuid, reason: AcquireReason, lane: &'static str) -> Self {
        Self {
            track_id,
            reason,
            lane,
            requested_by: None,
            priority: 0,
            prefer_video_ids: Vec::new(),
            exclude_video_ids: Vec::new(),
            direct_url: None,
            force: false,
            replace: false,
        }
    }

    pub fn by(mut self, user: Uuid) -> Self {
        self.requested_by = Some(user);
        self
    }

    pub fn priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enqueue {
    Queued {
        acquisition_id: Uuid,
        job_id: Uuid,
    },
    /// У записи уже идёт скачивание (его задача могла переехать в более важную полосу).
    Active {
        acquisition_id: Uuid,
    },
    /// Файл уже на сервере.
    Ready,
    /// Ставить нечего: `no_track`, `no_recording` (ISRC трека ещё неизвестен — сначала загрузить трек
    /// целиком), `no_deezer_id`, `needs_review` (спорная или не найденная запись ждёт решения в админке).
    Skipped(&'static str),
}

/// Ставит запись трека на скачивание: строка в `acquisitions`, задача `acquire`, статус `queued`.
/// Одна запись — одно активное скачивание: повторная просьба возвращает идущее, а если оно стоит
/// в менее важной полосе (дискографии), а трек хотят сейчас, — переносит его задачу.
pub async fn enqueue(db: &PgPool, req: Request) -> Result<Enqueue, sqlx::Error> {
    let mut tx = db.begin().await?;
    let track = sqlx::query!(
        r#"SELECT t.deezer_id, t.recording_id, r.audio_status AS "audio_status?"
           FROM tracks t LEFT JOIN recordings r ON r.id = t.recording_id
           WHERE t.id = $1"#,
        req.track_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(track) = track else {
        return Ok(Enqueue::Skipped("no_track"));
    };
    let (Some(recording_id), Some(status)) = (track.recording_id, track.audio_status) else {
        return Ok(Enqueue::Skipped("no_recording"));
    };
    let Some(deezer_track_id) = track.deezer_id else {
        return Ok(Enqueue::Skipped("no_deezer_id"));
    };
    let status = status.parse::<AudioStatus>().unwrap_or(AudioStatus::None);
    if !req.force {
        match status {
            AudioStatus::Ready => return Ok(Enqueue::Ready),
            AudioStatus::Disputed | AudioStatus::Unavailable => {
                return Ok(Enqueue::Skipped("needs_review"));
            }
            _ => {}
        }
    }
    let mut created = None;
    for _ in 0..2 {
        let inserted = sqlx::query_scalar!(
            "INSERT INTO acquisitions (recording_id, track_id, reason, lane, requested_by)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (recording_id) WHERE status IN ('queued', 'matching', 'downloading', 'verifying', 'processing')
             DO NOTHING
             RETURNING id",
            recording_id,
            req.track_id,
            req.reason.as_str(),
            req.lane,
            req.requested_by,
        )
        .fetch_optional(&mut *tx)
        .await?;
        if inserted.is_some() {
            created = inserted;
            break;
        }
        let active = sqlx::query!(
            r#"SELECT a.id, a.job_id, j.status AS "job_status?" FROM acquisitions a
               LEFT JOIN jobs j ON j.id = a.job_id
               WHERE a.recording_id = $1 AND a.status IN ('queued', 'matching', 'downloading', 'verifying', 'processing')"#,
            recording_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        if let (Some(job_id), Some("queued" | "running")) =
            (active.job_id, active.job_status.as_deref())
        {
            promote(&mut tx, job_id, req.lane, req.priority).await?;
            tx.commit().await?;
            return Ok(Enqueue::Active {
                acquisition_id: active.id,
            });
        }
        // Скачивание осталось без живой задачи (её закрыли или удалили мимо него): закрываем его,
        // иначе запись навсегда «в очереди», и ставим заново
        sqlx::query!(
            "UPDATE acquisitions SET status = 'failed', error = COALESCE(error, 'задача скачивания пропала'),
                    updated_at = now(), finished_at = now()
             WHERE id = $1",
            active.id,
        )
        .execute(&mut *tx)
        .await?;
    }
    let Some(acquisition_id) = created else {
        return Err(sqlx::Error::Protocol(format!(
            "could not create an acquisition for recording {recording_id}"
        )));
    };
    let payload = AcquirePayload {
        acquisition_id,
        recording_id,
        track_id: req.track_id,
        deezer_track_id,
        prefer_video_ids: req.prefer_video_ids,
        exclude_video_ids: req.exclude_video_ids,
        direct_url: req.direct_url,
        replace: req.replace,
    };
    let new_job = NewJob::new(kind::ACQUIRE, req.lane, &payload)
        .dedupe(format!("acquire:{recording_id}"))
        .priority(req.priority);
    let mut job = jobs::enqueue(&mut tx, new_job.clone()).await?;
    if !job.created {
        // Задача от закрытого скачивания этой записи (отменённого или решённого в админке) ещё жива:
        // её данные о другом скачивании, поэтому её место занимает новая
        jobs::cancel(&mut tx, job.id).await?;
        job = jobs::enqueue(&mut tx, new_job).await?;
    }
    sqlx::query!(
        "UPDATE acquisitions SET job_id = $2 WHERE id = $1",
        acquisition_id,
        job.id
    )
    .execute(&mut *tx)
    .await?;
    set_audio_status_unless_ready(&mut tx, recording_id, AudioStatus::Queued).await?;
    publish(&mut tx, acquisition_id).await?;
    tx.commit().await?;
    Ok(Enqueue::Queued {
        acquisition_id,
        job_id: job.id,
    })
}

/// [`enqueue`], а если ISRC трека ещё неизвестен (трек пришёл из популярного артиста), — сначала
/// загрузить трек целиком из Deezer.
pub async fn request(db: &PgPool, deezer: &Deezer, req: Request) -> Result<Enqueue, AppError> {
    let res = enqueue(db, req.clone()).await?;
    if res != Enqueue::Skipped("no_recording") {
        return Ok(res);
    }
    catalog::ensure_track(db, deezer, req.track_id).await?;
    Ok(enqueue(db, req).await?)
}

/// Задача стоит в менее важной полосе (или с меньшим приоритетом), а запись нужна раньше: переносим.
/// Трек хотят сыграть — отложенный повтор тоже не ждёт.
async fn promote(
    conn: &mut PgConnection,
    job_id: Uuid,
    lane: &str,
    priority: i16,
) -> Result<(), sqlx::Error> {
    let order: Vec<String> = lanes::ACQUIRE.iter().map(|l| l.to_string()).collect();
    let moved = sqlx::query_scalar!(
        "UPDATE jobs SET lane = $2, priority = GREATEST(priority, $3), updated_at = now(),
                run_after = CASE WHEN $2 = 'now' THEN LEAST(run_after, now()) ELSE run_after END
         WHERE id = $1 AND status = 'queued'
           AND (array_position($4::text[], lane) > array_position($4::text[], $2) OR (lane = $2 AND priority < $3))
         RETURNING id",
        job_id,
        lane,
        priority,
        &order,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if moved.is_some() {
        sqlx::query!(
            "UPDATE acquisitions SET lane = $2, updated_at = now() WHERE job_id = $1",
            job_id,
            lane
        )
        .execute(&mut *conn)
        .await?;
        jobs::wake(conn, lane).await?;
    }
    Ok(())
}

/// Меняет шаг скачивания и рассылает `acquisition.updated`.
pub async fn set_status(
    conn: &mut PgConnection,
    acquisition_id: Uuid,
    status: AcquisitionStatus,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    let finished = !status.is_active();
    sqlx::query!(
        "UPDATE acquisitions SET status = $2, error = COALESCE($3, error), updated_at = now(),
                finished_at = CASE WHEN $4 THEN now() ELSE NULL END
         WHERE id = $1",
        acquisition_id,
        status.as_str(),
        error,
        finished,
    )
    .execute(&mut *conn)
    .await?;
    publish(conn, acquisition_id).await
}

/// Задача `acquire` закрыта без обработчика: скачивание помечается неудачным, а запись возвращается
/// в `none` — «скачается при запуске». С этапа 1b так бывает, только если у воркера нет обработчика.
pub async fn abandoned(db: &PgPool, job: &JobRow, error: &str) -> Result<(), sqlx::Error> {
    let Ok(payload) = job.payload::<AcquirePayload>() else {
        return Ok(());
    };
    let mut tx = db.begin().await?;
    set_status(
        &mut tx,
        payload.acquisition_id,
        AcquisitionStatus::Failed,
        Some(error),
    )
    .await?;
    let status = sqlx::query_scalar!(
        "SELECT audio_status FROM recordings WHERE id = $1",
        payload.recording_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    if status.as_deref() == Some(AudioStatus::Queued.as_str()) {
        set_audio_status(&mut tx, payload.recording_id, AudioStatus::None).await?;
        publish(&mut tx, payload.acquisition_id).await?;
    }
    tx.commit().await
}

pub(crate) async fn publish(
    conn: &mut PgConnection,
    acquisition_id: Uuid,
) -> Result<(), sqlx::Error> {
    let row = sqlx::query!(
        "SELECT a.id, a.recording_id, a.track_id, a.status, a.error, r.audio_status
         FROM acquisitions a JOIN recordings r ON r.id = a.recording_id
         WHERE a.id = $1",
        acquisition_id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = row {
        let event = ServerEvent::AcquisitionUpdated(AcquisitionUpdate {
            id: row.id,
            recording_id: row.recording_id,
            track_id: row.track_id,
            status: row.status.parse().unwrap_or(AcquisitionStatus::Failed),
            audio_status: row.audio_status.parse().unwrap_or(AudioStatus::None),
            error: row.error,
        });
        realtime::publish(&mut *conn, Audience::All, event).await;
    }
    Ok(())
}

/// Запись сменила состояние аудио: тем, у кого она в медиатеке, пора запросить `/api/sync`.
pub(crate) async fn recording_changed(
    conn: &mut PgConnection,
    recording_id: Uuid,
) -> Result<(), sqlx::Error> {
    let users = sqlx::query_scalar!(
        "SELECT u.id FROM users u
         WHERE EXISTS (SELECT 1 FROM tracks t WHERE t.recording_id = $1 AND t.id IN (SELECT user_track_ids(u.id)))",
        recording_id,
    )
    .fetch_all(&mut *conn)
    .await?;
    for user in users {
        realtime::publish(
            &mut *conn,
            Audience::User(user),
            ServerEvent::LibraryChanged(LibraryChange {
                entities: vec![SyncEntity::Recordings],
            }),
        )
        .await;
    }
    Ok(())
}

/// Сколько скачиваний впереди задачи: полосы по важности, внутри — приоритет и время. Отложенные
/// и стоящие на паузе не считаются. 0 — следующая или уже идёт; `None` — задача закончилась.
pub async fn queue_position(
    conn: &mut PgConnection,
    job_id: Uuid,
) -> Result<Option<i64>, sqlx::Error> {
    let order: Vec<String> = lanes::ACQUIRE.iter().map(|l| l.to_string()).collect();
    sqlx::query_scalar!(
        r#"SELECT CASE WHEN me.status = 'running' THEN 0 ELSE (
                  SELECT count(*) FROM jobs j JOIN job_lanes l ON l.lane = j.lane
                  WHERE j.kind = 'acquire' AND j.status = 'queued' AND j.id <> me.id AND NOT l.paused
                    AND j.run_after <= now()
                    AND (array_position($2::text[], j.lane) < array_position($2::text[], me.lane)
                         OR (j.lane = me.lane AND (j.priority > me.priority
                             OR (j.priority = me.priority AND (j.run_after, j.id) < (me.run_after, me.id)))))
              ) END AS "position!"
           FROM jobs me WHERE me.id = $1 AND me.status IN ('queued', 'running')"#,
        job_id,
        &order,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Скачивание для интерфейса: шаг, место в очереди, есть ли кому качать.
pub async fn view(
    db: &PgPool,
    server_fetcher: bool,
    id: Uuid,
) -> Result<Option<Acquisition>, sqlx::Error> {
    let mut conn = db.acquire().await?;
    let row = sqlx::query!(
        "SELECT a.id, a.recording_id, a.track_id, a.status, a.reason, a.lane, a.error, a.attempts,
                a.job_id, a.created_at, a.updated_at, a.finished_at, r.audio_status
         FROM acquisitions a JOIN recordings r ON r.id = a.recording_id
         WHERE a.id = $1",
        id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status = row.status.parse().unwrap_or(AcquisitionStatus::Failed);
    let queue_position = match (status.is_active(), row.job_id) {
        (true, Some(job)) => queue_position(&mut conn, job).await?,
        _ => None,
    };
    let online = downloaders::online(&mut *conn).await?;
    Ok(Some(Acquisition {
        id: row.id,
        recording_id: row.recording_id,
        track_id: row.track_id,
        status,
        audio_status: row.audio_status.parse().unwrap_or(AudioStatus::None),
        reason: row.reason.parse().unwrap_or(AcquireReason::Manual),
        lane: row.lane,
        error: row.error,
        attempts: row.attempts,
        queue_position,
        downloaders_online: online,
        waiting_for_downloader: status.is_active() && online == 0 && !server_fetcher,
        created_at: row.created_at,
        updated_at: row.updated_at,
        finished_at: row.finished_at,
    }))
}

/// Файл, который играет плеер: основной файл готовой записи.
#[derive(Debug, Clone)]
pub struct PlayableFile {
    pub id: Uuid,
    pub storage_key: String,
    pub codec: AudioCodec,
    pub container: AudioContainer,
    pub bitrate_kbps: Option<i32>,
    pub duration_ms: i32,
    pub loudness_lufs: Option<f32>,
    pub true_peak_db: Option<f32>,
    pub waveform: Option<Vec<i16>>,
}

pub async fn playable_file(
    db: &PgPool,
    recording_id: Uuid,
) -> Result<Option<PlayableFile>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT f.id, f.storage_key, f.codec, f.container, f.bitrate_kbps, f.duration_ms,
                f.loudness_lufs, f.true_peak_db, f.waveform
         FROM recordings r
         JOIN audio_files f ON f.id = COALESCE(r.primary_file_id,
             (SELECT id FROM audio_files WHERE recording_id = r.id ORDER BY created_at DESC LIMIT 1))
         WHERE r.id = $1 AND r.audio_status = 'ready'",
        recording_id,
    )
    .fetch_optional(db)
    .await?;
    Ok(row.and_then(|r| {
        Some(PlayableFile {
            id: r.id,
            codec: r.codec.parse().ok()?,
            container: r.container.parse().ok()?,
            storage_key: r.storage_key,
            bitrate_kbps: r.bitrate_kbps,
            duration_ms: r.duration_ms,
            loudness_lufs: r.loudness_lufs,
            true_peak_db: r.true_peak_db,
            waveform: r.waveform,
        })
    }))
}

/// Начало попытки: исполнитель взял задачу.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// Новый ключ файла: на него выдаётся подписанная ссылка.
    Started { upload_key: String },
    /// Делать нечего: скачивание уже закрыто (`closed`) или запись успела стать готовой (`ready`).
    Skip(&'static str),
}

/// Исполнитель взял задачу: новая попытка, ключ файла, шаг «поиск», запись «скачивается».
pub async fn start_attempt(
    db: &PgPool,
    payload: &AcquirePayload,
    executor: &str,
) -> Result<Start, sqlx::Error> {
    let mut tx = db.begin().await?;
    let row = sqlx::query!(
        "SELECT a.status, r.audio_status FROM acquisitions a JOIN recordings r ON r.id = a.recording_id
         WHERE a.id = $1 FOR UPDATE OF a",
        payload.acquisition_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        return Ok(Start::Skip("closed"));
    };
    let active = row
        .status
        .parse::<AcquisitionStatus>()
        .is_ok_and(AcquisitionStatus::is_active);
    if !active {
        return Ok(Start::Skip("closed"));
    }
    if row.audio_status == AudioStatus::Ready.as_str() && !payload.is_manual() {
        set_status(
            &mut tx,
            payload.acquisition_id,
            AcquisitionStatus::Ready,
            None,
        )
        .await?;
        tx.commit().await?;
        return Ok(Start::Skip("ready"));
    }
    let upload_key = keys::audio(payload.recording_id, Uuid::now_v7(), "m4a");
    sqlx::query!(
        "UPDATE acquisitions SET status = 'matching', worker = $2, attempts = attempts + 1, upload_key = $3,
                updated_at = now(), finished_at = NULL
         WHERE id = $1",
        payload.acquisition_id,
        executor,
        upload_key,
    )
    .execute(&mut *tx)
    .await?;
    set_audio_status_unless_ready(&mut tx, payload.recording_id, AudioStatus::Acquiring).await?;
    publish(&mut tx, payload.acquisition_id).await?;
    tx.commit().await?;
    Ok(Start::Started { upload_key })
}

/// Задача отдана загрузчику.
#[derive(Debug)]
pub enum Claimed {
    Job(Box<FetcherJob>),
    /// Задач нет; если какая-то полоса скоро освободится — через сколько.
    Nothing(Option<Duration>),
}

/// Берёт задачу `acquire` для домашнего загрузчика: полосы по важности, с их паузами и интервалами.
pub async fn claim_for_downloader(
    db: &PgPool,
    storage: &Storage,
    executor: &str,
    lease: Duration,
) -> Result<Claimed, AppError> {
    let opts = ClaimOptions {
        kinds: Kinds::only(&[kind::ACQUIRE]),
        lease: Some(lease),
    };
    let mut soonest: Option<Duration> = None;
    // Задачи закрытых скачиваний закрываются по пути, и берётся следующая; предел — от зацикливания
    let mut closed = 0;
    let mut lane_index = 0;
    while lane_index < lanes::ACQUIRE.len() && closed <= 20 {
        let lane = lanes::ACQUIRE[lane_index];
        let job = match jobs::claim_with(db, lane, executor, opts.clone()).await? {
            Claim::Job(job) => *job,
            Claim::Empty(next) => {
                soonest = min_opt(soonest, next);
                lane_index += 1;
                continue;
            }
            Claim::Wait(wait) => {
                soonest = min_opt(soonest, Some(wait));
                lane_index += 1;
                continue;
            }
            Claim::Full | Claim::Paused => {
                lane_index += 1;
                continue;
            }
        };
        let payload = match job.payload::<AcquirePayload>() {
            Ok(p) => p,
            Err(e) => {
                let message = format!("неверные данные задачи: {e}");
                jobs::fail(db, job.id, executor, &message, false).await?;
                closed += 1;
                continue;
            }
        };
        let upload_key = match start_attempt(db, &payload, executor).await? {
            Start::Started { upload_key } => upload_key,
            Start::Skip(reason) => {
                jobs::complete(db, job.id, executor, json!({ "skipped": reason })).await?;
                closed += 1;
                continue;
            }
        };
        let url = match storage.presign_put(&upload_key, UPLOAD_PUT_TTL).await {
            Ok(url) => url,
            Err(e) => {
                let message = format!("хранилище не подписало ссылку: {e}");
                result::give_back(db, &job, &payload, executor, &message, GIVE_BACK_DELAY).await?;
                return Err(AppError::Unavailable(
                    "хранилище недоступно, повторите позже".into(),
                ));
            }
        };
        let now = Utc::now();
        let lease_expires_at = job
            .lease_expires_at
            .unwrap_or_else(|| now + chrono::Duration::from_std(lease).unwrap_or_default());
        return Ok(Claimed::Job(Box::new(FetcherJob {
            id: job.id,
            kind: kind::ACQUIRE.into(),
            deezer_track_id: payload.deezer_track_id,
            max_attempts: candidate_attempts(&payload),
            prefer_video_ids: payload.prefer_video_ids.clone(),
            exclude_video_ids: payload.exclude_video_ids.clone(),
            direct_url: payload.direct_url.clone(),
            upload: UploadTarget {
                url: url.to_string(),
                method: "PUT".into(),
                headers: Default::default(),
                expires_at: now + chrono::Duration::from_std(UPLOAD_PUT_TTL).unwrap_or_default(),
            },
            lease_expires_at,
        })));
    }
    Ok(Claimed::Nothing(soonest))
}

/// Ручной выбор пробуется один: если выбранное видео не скачалось, подставлять другое нельзя.
pub fn candidate_attempts(payload: &AcquirePayload) -> i16 {
    if payload.is_manual() {
        1
    } else {
        CANDIDATE_ATTEMPTS
    }
}

fn min_opt(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Аренды, которые кончились: задачи снова в очереди (или закрыты, если попытки кончились), а их
/// скачивания — вслед за ними.
pub async fn reap_leases(db: &PgPool) -> Result<usize, sqlx::Error> {
    let reaped = jobs::reap(db, None).await?;
    after_reap(db, &reaped).await?;
    Ok(reaped.len())
}

/// Скачивания задач, которые вернул в очередь сборщик зависших или освободил отзыв загрузчика.
pub async fn after_reap(db: &PgPool, reaped: &[JobRow]) -> Result<(), sqlx::Error> {
    for job in reaped.iter().filter(|j| j.kind == kind::ACQUIRE) {
        result::after_close(db, job).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_status_transitions() {
        use AudioStatus::*;
        assert!(can_transition(None, Queued));
        assert!(can_transition(Queued, Acquiring));
        assert!(can_transition(Acquiring, Ready));
        assert!(can_transition(Acquiring, Disputed));
        assert!(can_transition(Failed, Queued));
        assert!(can_transition(Acquiring, Queued), "сбой — снова в очередь");
        assert!(can_transition(Disputed, Ready), "ручной выбор в «Спорных»");
        assert!(can_transition(Disputed, Unavailable), "решение админки");
        assert!(can_transition(Queued, None), "сброс");
        assert!(!can_transition(Ready, None), "файл есть — запись готова");
        assert!(!can_transition(Ready, Queued));
        assert!(!can_transition(None, Disputed));
        assert!(!can_transition(Ready, Failed));
        assert!(!can_transition(Ready, Unavailable));
    }

    #[test]
    fn manual_choice_is_tried_alone() {
        let mut p = AcquirePayload {
            acquisition_id: Uuid::nil(),
            recording_id: Uuid::nil(),
            track_id: Uuid::nil(),
            deezer_track_id: 1,
            prefer_video_ids: vec![],
            exclude_video_ids: vec![],
            direct_url: None,
            replace: false,
        };
        assert_eq!(candidate_attempts(&p), CANDIDATE_ATTEMPTS);
        p.prefer_video_ids.push("dQw4w9WgXcQ".into());
        assert_eq!(candidate_attempts(&p), 1);
        assert_eq!(
            min_opt(Some(Duration::from_secs(3)), Some(Duration::from_secs(1))),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            min_opt(None, Some(Duration::from_secs(2))),
            Some(Duration::from_secs(2))
        );
        assert_eq!(min_opt(None, None), None);
    }
}
