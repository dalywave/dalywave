//! Скачивание аудио записи — шов для этапа 1b.
//!
//! Здесь: постановка (строка `acquisitions` + задача `acquire` с [`AcquirePayload`]), переходы
//! `recordings.audio_status` и события `acquisition.updated`. Обработчик задачи `acquire` (fetcher, проверка,
//! запись `audio_files`, подписанные ссылки) регистрируется в [`crate::jobs::worker::Registry`] на этапе 1b;
//! пока его нет, воркер закрывает такие задачи через [`abandoned`].

use dalywave_protocol::{
    catalog::AudioStatus,
    jobs::{AcquirePayload, AcquireReason, AcquisitionStatus, kind},
    realtime::{AcquisitionUpdate, ServerEvent},
};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    jobs::{self, JobRow, NewJob},
    realtime::{self, Audience},
};

/// Разрешён ли переход состояния аудио записи.
///
/// `none → queued → acquiring → ready | disputed | unavailable | failed`; неудачные можно поставить снова;
/// `ready` достижимо отовсюду (загрузка своего файла, ручной выбор в «Спорных»); сброс в `none` — из любого,
/// кроме `ready` (файл есть — запись готова).
pub fn can_transition(from: AudioStatus, to: AudioStatus) -> bool {
    use AudioStatus::*;
    match to {
        None => from != Ready,
        Queued => matches!(from, None | Failed | Unavailable | Disputed),
        Acquiring => matches!(from, Queued | None | Failed),
        Ready => true,
        Disputed | Unavailable => matches!(from, Acquiring | Queued),
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

/// Что попросили скачать.
#[derive(Debug, Clone)]
pub struct Request {
    pub track_id: Uuid,
    pub reason: AcquireReason,
    pub lane: &'static str,
    pub requested_by: Option<Uuid>,
    pub priority: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enqueue {
    Queued {
        acquisition_id: Uuid,
        job_id: Uuid,
    },
    /// У записи уже идёт скачивание.
    Active {
        acquisition_id: Uuid,
    },
    /// Файл уже на сервере.
    Ready,
    /// Ставить нечего: `no_recording` (ISRC трека ещё неизвестен — сначала загрузить трек целиком),
    /// `no_deezer_id`, `needs_review` (спорная или не найденная запись ждёт решения в админке).
    Skipped(&'static str),
}

/// Ставит запись трека на скачивание: строка в `acquisitions`, задача `acquire`, статус `queued`.
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
    match status.parse::<AudioStatus>().unwrap_or(AudioStatus::None) {
        AudioStatus::Ready => return Ok(Enqueue::Ready),
        AudioStatus::Disputed | AudioStatus::Unavailable => {
            return Ok(Enqueue::Skipped("needs_review"));
        }
        _ => {}
    }
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
    let Some(acquisition_id) = inserted else {
        let active = sqlx::query_scalar!(
            "SELECT id FROM acquisitions
             WHERE recording_id = $1 AND status IN ('queued', 'matching', 'downloading', 'verifying', 'processing')",
            recording_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(Enqueue::Active {
            acquisition_id: active,
        });
    };
    let payload = AcquirePayload {
        acquisition_id,
        recording_id,
        track_id: req.track_id,
        deezer_track_id,
        prefer_video_ids: Vec::new(),
        exclude_video_ids: Vec::new(),
    };
    let job = jobs::enqueue(
        &mut tx,
        NewJob::new(kind::ACQUIRE, req.lane, &payload)
            .dedupe(format!("acquire:{recording_id}"))
            .priority(req.priority),
    )
    .await?;
    sqlx::query!(
        "UPDATE acquisitions SET job_id = $2 WHERE id = $1",
        acquisition_id,
        job.id
    )
    .execute(&mut *tx)
    .await?;
    set_audio_status(&mut tx, recording_id, AudioStatus::Queued).await?;
    publish(&mut tx, acquisition_id).await?;
    tx.commit().await?;
    Ok(Enqueue::Queued {
        acquisition_id,
        job_id: job.id,
    })
}

/// Меняет шаг скачивания (обработчик 1b зовёт на каждом шаге) и рассылает `acquisition.updated`.
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

/// Задача `acquire` закрыта без обработчика (этап 1b ещё не подключён): скачивание помечается
/// неудачным, а запись возвращается в `none` — «скачается при запуске», как и раньше.
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

async fn publish(conn: &mut PgConnection, acquisition_id: Uuid) -> Result<(), sqlx::Error> {
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
        assert!(can_transition(Disputed, Ready), "ручной выбор в «Спорных»");
        assert!(can_transition(Queued, None), "сброс");
        assert!(!can_transition(Ready, None), "файл есть — запись готова");
        assert!(!can_transition(Ready, Queued));
        assert!(!can_transition(None, Disputed));
        assert!(!can_transition(Ready, Failed));
    }
}
