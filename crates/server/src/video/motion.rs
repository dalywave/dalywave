//! Живая обложка альбома: задача воркера `album_motion` ищет альбом в Apple Music
//! ([`super::musickit`]), берёт подходящий вариант HLS и перекладывает его в короткий MP4
//! (H.264, без звука, `faststart`) — квадратный и, если есть, вертикальный. Нет обложки — строка
//! `none`, перепроверка через месяц.

use std::{path::Path, sync::Arc, time::Duration};

use dalywave_protocol::jobs::kind;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::{RECHECK_DAYS, keys, musickit::AlbumQuery};
use crate::{
    import::audio::{self, AudioError},
    jobs::{
        self as queue, JobRow, NewJob,
        worker::{Context, Done, JobError, JobResult, Outcome},
    },
};

pub const LANE: &str = "motion";
/// Видео длиннее не нужно: петля обложки — 10–30 секунд.
const MAX_SECONDS: u32 = 30;
/// Сверху по ширине (квадрат — 720×720): больше в плеере не видно.
const MAX_WIDTH: u32 = 720;
/// Файл тяжелее — второй проход с меньшим размером и качеством.
const MAX_BYTES: u64 = 6 * 1024 * 1024;
/// Нет ffmpeg на этой машине — задача ждёт, не тратя попыток.
const TOOLS_PAUSE: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPayload {
    pub album_id: Uuid,
}

/// Поставить поиск живых обложек (одна задача на альбом).
pub async fn request(db: &PgPool, album_ids: &[Uuid]) -> Result<(), sqlx::Error> {
    let mut conn = db.acquire().await?;
    for &album_id in album_ids {
        queue::enqueue(
            &mut conn,
            NewJob::new(kind::ALBUM_MOTION, LANE, MotionPayload { album_id })
                .dedupe(format!("album_motion:{album_id}")),
        )
        .await?;
    }
    Ok(())
}

/// Записать «живой обложки нет». `recheck_in_days` — когда спросить Apple снова.
async fn remember_none(
    db: &PgPool,
    album_id: Uuid,
    apple_album_id: Option<&str>,
    error: Option<&str>,
    recheck_in_days: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO album_motion (album_id, state, apple_album_id, checked_at, error)
         VALUES ($1, 'none', $2, now() - make_interval(days => $3), $4)
         ON CONFLICT (album_id) DO UPDATE SET state = 'none', apple_album_id = EXCLUDED.apple_album_id,
             square_key = NULL, tall_key = NULL, size_bytes = 0, checked_at = EXCLUDED.checked_at,
             error = EXCLUDED.error",
        album_id,
        apple_album_id,
        RECHECK_DAYS - recheck_in_days.clamp(0, RECHECK_DAYS),
        error,
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn run(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: MotionPayload = job.payload()?;
    let Some(musickit) = ctx.musickit.clone() else {
        return Ok(json!({ "skipped": "no_musickit" }).into());
    };
    let Some(album) = sqlx::query!("SELECT title, upc FROM albums WHERE id = $1", p.album_id)
        .fetch_optional(&ctx.db)
        .await?
    else {
        return Ok(json!({ "skipped": "gone" }).into());
    };
    let isrcs = sqlx::query_scalar!(
        r#"SELECT t.isrc AS "isrc!" FROM tracks t
           WHERE t.album_id = $1 AND t.isrc IS NOT NULL
           ORDER BY t.disc_number, t.track_position LIMIT 3"#,
        p.album_id
    )
    .fetch_all(&ctx.db)
    .await?;
    let query = AlbumQuery {
        title: album.title,
        upc: album.upc,
        isrcs,
    };
    let found = match musickit.find(&query).await {
        Ok(f) => f,
        Err(e) if e.transient() => return Err(JobError::retry(e)),
        // Ключ не принят и прочие отказы: «нет» до завтра, а не на месяц — ключ могут поправить
        Err(e) => {
            remember_none(&ctx.db, p.album_id, None, Some(&e.to_string()), 1).await?;
            return Ok(json!({ "state": "none", "error": e.to_string() }).into());
        }
    };
    if !found.has_video() {
        remember_none(
            &ctx.db,
            p.album_id,
            found.apple_album_id.as_deref(),
            None,
            RECHECK_DAYS,
        )
        .await?;
        return Ok(json!({ "state": "none", "appleAlbumId": found.apple_album_id }).into());
    }

    let dir = tempfile::tempdir().map_err(|e| JobError::retry(format!("временная папка: {e}")))?;
    let check_id = Uuid::now_v7();
    let mut stored: Vec<(&str, String, u64)> = Vec::new();
    for (shape, source) in [("square", &found.square), ("tall", &found.tall)] {
        let Some(source) = source else { continue };
        let out = dir.path().join(format!("{shape}.mp4"));
        let variant = match pick_variant(&ctx.http, source).await {
            Ok(v) => v,
            Err(e) => return Err(JobError::retry(format!("плейлист HLS: {e}"))),
        };
        match transcode(&variant, &out).await {
            Ok(()) => {}
            Err(AudioError::Missing(tool)) => {
                let msg = format!("на сервере нет {tool}");
                ctx.cooldowns.pause(kind::ALBUM_MOTION, TOOLS_PAUSE);
                let worker = job.locked_by.clone().unwrap_or_default();
                queue::postpone(&ctx.db, job.id, &worker, &msg, TOOLS_PAUSE).await?;
                return Ok(Done::Settled(Outcome::Retry(msg)));
            }
            Err(AudioError::Unreadable(e) | AudioError::Failed(e)) => {
                return Err(JobError::retry(format!("ffmpeg: {e}")));
            }
        }
        let key = keys::motion(p.album_id, check_id, shape);
        let size = ctx.storage.put_file(&key, &out, "video/mp4").await?;
        stored.push((shape, key, size));
    }
    let key_of = |shape: &str| stored.iter().find(|s| s.0 == shape).map(|s| s.1.clone());
    let size: u64 = stored.iter().map(|s| s.2).sum();
    let saved = sqlx::query!(
        "INSERT INTO album_motion (album_id, state, apple_album_id, square_key, tall_key, size_bytes, checked_at)
         VALUES ($1, 'ready', $2, $3, $4, $5, now())
         ON CONFLICT (album_id) DO UPDATE SET state = 'ready', apple_album_id = EXCLUDED.apple_album_id,
             square_key = EXCLUDED.square_key, tall_key = EXCLUDED.tall_key,
             size_bytes = EXCLUDED.size_bytes, checked_at = now(), error = NULL",
        p.album_id,
        found.apple_album_id,
        key_of("square"),
        key_of("tall"),
        size as i64,
    )
    .execute(&ctx.db)
    .await;
    match saved {
        Ok(_) => Ok(json!({ "state": "ready", "bytes": size, "files": stored.len() }).into()),
        // Альбом удалили, пока шла перекладка: файлы никому не нужны
        Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => {
            for (_, key, _) in &stored {
                let _ = ctx.storage.delete(key).await;
            }
            Ok(json!({ "skipped": "gone" }).into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Вариант из главного плейлиста HLS: самый узкий не меньше [`MAX_WIDTH`], иначе самый широкий.
/// Не главный плейлист (сразу сегменты) — он сам.
async fn pick_variant(http: &reqwest::Client, master: &str) -> Result<String, String> {
    let res = http
        .get(master)
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status()));
    }
    let text = res.text().await.map_err(|e| e.without_url().to_string())?;
    let base = url::Url::parse(master).map_err(|e| e.to_string())?;
    Ok(match choose_variant(&text, MAX_WIDTH) {
        Some(uri) => base.join(&uri).map_err(|e| e.to_string())?.to_string(),
        None => master.to_string(),
    })
}

/// Разбор главного плейлиста: `#EXT-X-STREAM-INF:…RESOLUTION=WxH…` и адрес строкой ниже.
pub fn choose_variant(m3u8: &str, min_width: u32) -> Option<String> {
    let mut variants: Vec<(u32, String)> = Vec::new();
    let mut lines = m3u8.lines().map(str::trim);
    while let Some(line) = lines.next() {
        let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") else {
            continue;
        };
        let width = attrs
            .split(',')
            .find_map(|a| a.strip_prefix("RESOLUTION="))
            .and_then(|r| r.split('x').next())
            .and_then(|w| w.parse::<u32>().ok())
            .unwrap_or(0);
        if let Some(uri) = lines
            .by_ref()
            .find(|l| !l.is_empty() && !l.starts_with('#'))
        {
            variants.push((width, uri.to_string()));
        }
    }
    let wide_enough = variants
        .iter()
        .filter(|v| v.0 >= min_width)
        .min_by_key(|v| v.0);
    wide_enough
        .or_else(|| variants.iter().max_by_key(|v| v.0))
        .map(|v| v.1.clone())
}

/// HLS (или любой вход ffmpeg) → MP4: до [`MAX_SECONDS`], без звука, по ширине до [`MAX_WIDTH`];
/// тяжелее [`MAX_BYTES`] — ещё раз мельче.
pub async fn transcode(input: &str, output: &Path) -> Result<(), AudioError> {
    for (width, crf) in [(MAX_WIDTH, 25), (540, 30)] {
        let filter = format!("scale='min({width},iw)':-2:flags=lanczos,fps=30");
        let seconds = MAX_SECONDS.to_string();
        let crf = crf.to_string();
        let out = output.as_os_str();
        let flags = [
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            input,
            "-t",
            &seconds,
            "-an",
            "-sn",
            "-vf",
            &filter,
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            &crf,
            "-profile:v",
            "high",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ];
        let args: Vec<&std::ffi::OsStr> = flags
            .iter()
            .map(std::ffi::OsStr::new)
            .chain(std::iter::once(out))
            .collect();
        let res = audio::run("ffmpeg", &args).await?;
        if !res.status.success() {
            return Err(AudioError::Failed(audio::tail(&res.stderr)));
        }
        let size = std::fs::metadata(output)
            .map(|m| m.len())
            .map_err(|e| AudioError::Failed(e.to_string()))?;
        if size <= MAX_BYTES {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_is_the_narrowest_wide_enough() {
        let master = "#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=500000,RESOLUTION=480x480,CODECS=\"avc1.4d401e\"
v480/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1500000,RESOLUTION=1080x1080
v1080/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=720x720
v720/index.m3u8
";
        assert_eq!(
            choose_variant(master, 720).as_deref(),
            Some("v720/index.m3u8")
        );
        assert_eq!(
            choose_variant(master, 2000).as_deref(),
            Some("v1080/index.m3u8")
        );
        assert_eq!(choose_variant("#EXTM3U\n#EXTINF:4,\nseg0.ts\n", 720), None);
    }
}
