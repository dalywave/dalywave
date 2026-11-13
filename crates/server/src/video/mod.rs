//! Видео без звука: живые обложки альбомов ([`motion`], Apple Music) и клипы ленты открытий
//! ([`clips`], ищет и нарезает загрузчик на Mac). Файлы — в хранилище под `video/`, связи — в
//! `album_motion` и `track_clips`; «видео нет» тоже запоминается и перепроверяется раз в месяц.

pub mod clips;
pub mod motion;
pub mod musickit;

use std::collections::HashMap;

use dalywave_protocol::video::{AlbumMotion, VideoState};
use sqlx::PgPool;
use uuid::Uuid;

use crate::storage::{AUDIO_GET_TTL, Storage};

/// «Видео нет» перепроверяется через столько дней.
pub const RECHECK_DAYS: i32 = 30;

pub mod keys {
    use uuid::Uuid;

    /// Живая обложка: у каждой проверки своя папка — обновлённое видео меняет ссылку.
    pub fn motion(album_id: Uuid, check_id: Uuid, shape: &str) -> String {
        format!("video/motion/{album_id}/{check_id}-{shape}.mp4")
    }

    pub fn clip(track_id: Uuid, job_id: Uuid) -> String {
        format!("video/clips/{track_id}/{job_id}.mp4")
    }
}

/// Подписанная ссылка на видео; хранилище не ответило — без ссылки (обложка останется картинкой).
pub async fn signed(storage: &Storage, key: Option<&str>) -> Option<String> {
    let key = key?;
    match storage.presign_get(key, AUDIO_GET_TTL).await {
        Ok(url) => Some(url.to_string()),
        Err(e) => {
            tracing::warn!(%key, error = %e, "could not sign a video link");
            None
        }
    }
}

/// Живые обложки альбомов: готовые — со ссылками; неизвестные и устаревшие «нет» ставятся на поиск
/// (если ключ MusicKit задан), до тех пор — `pending`.
pub async fn motions(
    db: &PgPool,
    storage: &Storage,
    enabled: bool,
    album_ids: &[Uuid],
) -> Result<HashMap<Uuid, AlbumMotion>, sqlx::Error> {
    let mut out = HashMap::new();
    if album_ids.is_empty() {
        return Ok(out);
    }
    let rows = sqlx::query!(
        r#"SELECT album_id, state, square_key, tall_key,
                  checked_at < now() - make_interval(days => $2) AS "stale!"
           FROM album_motion WHERE album_id = ANY($1)"#,
        album_ids,
        RECHECK_DAYS,
    )
    .fetch_all(db)
    .await?;
    let mut known: HashMap<Uuid, _> = rows.into_iter().map(|r| (r.album_id, r)).collect();
    let mut wanted = Vec::new();
    for &id in album_ids {
        if out.contains_key(&id) {
            continue;
        }
        let view = match known.remove(&id) {
            Some(r) if r.state == "ready" => AlbumMotion {
                album_id: id,
                state: VideoState::Ready,
                square: signed(storage, r.square_key.as_deref()).await,
                tall: signed(storage, r.tall_key.as_deref()).await,
            },
            Some(r) if !r.stale || !enabled => none(id),
            _ if !enabled => none(id),
            _ => {
                wanted.push(id);
                AlbumMotion {
                    album_id: id,
                    state: VideoState::Pending,
                    square: None,
                    tall: None,
                }
            }
        };
        out.insert(id, view);
    }
    if !wanted.is_empty() {
        motion::request(db, &wanted).await?;
    }
    Ok(out)
}

fn none(album_id: Uuid) -> AlbumMotion {
    AlbumMotion {
        album_id,
        state: VideoState::None,
        square: None,
        tall: None,
    }
}
