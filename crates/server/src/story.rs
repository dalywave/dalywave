//! Видео-история трека (вертикальное 9:16, 15 с): картинку — обложку, волну и строку текста —
//! рисует приложение, звук — 15 с с припева — даёт сервер.
//!
//! - `GET /api/tracks/{id}/story/audio` — отрывок в WAV (PCM 44,1 кГц, стерео) с мягким входом и
//!   выходом: приложение строит по нему волну и, если браузер умеет кодировать AAC (WebCodecs),
//!   собирает MP4 само.
//! - `POST /api/tracks/{id}/story` — браузер AAC не кодирует: приложение присылает видео без звука,
//!   сервер добавляет тот же отрывок (AAC) и отдаёт готовый MP4. H.264 копируется как есть, другое
//!   (VP8/VP9 из MediaRecorder) — перекодируется в H.264.
//!
//! Откуда звук — как у «Послушать припев» (`/hook`): файл записи на сервере с места припева, иначе —
//! превью Deezer (оно и так с припева).

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::LazyLock,
    time::Duration,
};

use tokio::{process::Command, sync::Semaphore};
use uuid::Uuid;

use crate::{
    acquire, chorus,
    deezer::DeezerError,
    error::{AppError, AppResult},
    state::AppState,
};

/// Длина истории.
pub const STORY_MS: i64 = 15_000;
/// Видео без звука от приложения — не больше (15 с 1080×1920 H.264 — 5–15 МБ).
pub const MAX_VIDEO_BYTES: usize = 64 * 1024 * 1024;
const TOOL_TIMEOUT: Duration = Duration::from_secs(90);
const FADE_IN: &str = "afade=t=in:st=0:d=0.25";

/// ffmpeg разом — не больше двух: сборка тяжёлая, а API делит машину с остальным.
static SLOTS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

fn exe(name: &str) -> String {
    std::env::var(format!("DALYWAVE_{}", name.to_ascii_uppercase()))
        .unwrap_or_else(|_| name.to_string())
}

/// Мягкий вход и выход отрывка длиной [`STORY_MS`].
fn fades() -> String {
    let len = STORY_MS as f64 / 1000.0;
    format!("{FADE_IN},afade=t=out:st={:.2}:d=1", len - 1.0)
}

/// Откуда брать звук истории.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Файл записи в хранилище, с места припева.
    File { key: String, start_ms: i64 },
    /// Превью Deezer (уже с припева).
    Preview { url: String },
}

pub async fn source(st: &AppState, track_id: Uuid) -> AppResult<Source> {
    let row = sqlx::query!(
        r#"SELECT t.deezer_id, t.recording_id, r.audio_status AS "audio_status?"
           FROM tracks t LEFT JOIN recordings r ON r.id = t.recording_id WHERE t.id = $1"#,
        track_id
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if row.audio_status.as_deref() == Some("ready")
        && let Some(rec) = row.recording_id
        && let Some(file) = acquire::playable_file(&st.db, rec).await?
    {
        let start_ms = chorus::starts(&st.db, &[track_id], STORY_MS)
            .await?
            .remove(&track_id)
            .map_or(0, |(start, _)| start);
        // Место припева — по длительности трека в каталоге; у файла она бывает короче
        let start_ms = start_ms.min(i64::from(file.duration_ms) - STORY_MS).max(0);
        return Ok(Source::File {
            key: file.storage_key,
            start_ms,
        });
    }
    let preview = match row.deezer_id {
        Some(d) => match st.deezer.track(d).await {
            Ok(t) => t.preview,
            Err(DeezerError::NotFound) => None,
            Err(e) => {
                tracing::warn!(%track_id, error = %e, "deezer preview lookup failed");
                return Err(AppError::Unavailable("Deezer не ответил".into()));
            }
        },
        None => None,
    };
    preview
        .map(|url| Source::Preview { url })
        .ok_or(AppError::Missing {
            code: "no_audio",
            message: "у трека нет звука для истории".into(),
        })
}

/// Звук источника — во временный файл; начало отрывка, мс.
async fn fetch(st: &AppState, source: &Source, dir: &Path) -> AppResult<(PathBuf, i64)> {
    let path = dir.join("source");
    match source {
        Source::File { key, start_ms } => {
            st.storage
                .download_to(key, &path)
                .await
                .map_err(|e| AppError::Unavailable(format!("хранилище не отдало файл: {e}")))?;
            Ok((path, *start_ms))
        }
        Source::Preview { url } => {
            let res = st
                .http
                .get(url)
                .timeout(Duration::from_secs(20))
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .map_err(|e| AppError::Unavailable(format!("превью не скачалось: {e}")))?;
            let bytes = res
                .bytes()
                .await
                .map_err(|e| AppError::Unavailable(format!("превью не скачалось: {e}")))?;
            tokio::fs::write(&path, &bytes)
                .await
                .map_err(|e| AppError::Internal(e.into()))?;
            Ok((path, 0))
        }
    }
}

async fn run(name: &str, args: &[String]) -> AppResult<Vec<u8>> {
    let child = Command::new(exe(name))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AppError::Unavailable(format!("на сервере нет {name}")),
            _ => AppError::Internal(anyhow::anyhow!("{name} не запустился: {e}")),
        })?;
    let out = tokio::time::timeout(TOOL_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal(anyhow::anyhow!("{name} не уложился в срок")))?
        .map_err(|e| AppError::Internal(anyhow::anyhow!("{name}: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: String = err
            .trim()
            .chars()
            .rev()
            .take(300)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return Err(AppError::Unprocessable {
            code: "story_failed",
            message: format!("{name}: {tail}"),
        });
    }
    Ok(out.stdout)
}

fn secs(ms: i64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

/// Аргументы ffmpeg: отрывок звука в WAV.
pub fn audio_args(input: &Path, start_ms: i64, output: &Path) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostdin".into(),
        "-y".into(),
        "-ss".into(),
        secs(start_ms),
        "-t".into(),
        secs(STORY_MS),
        "-i".into(),
        input.display().to_string(),
        "-vn".into(),
        "-af".into(),
        fades(),
        "-ac".into(),
        "2".into(),
        "-ar".into(),
        "44100".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        "-f".into(),
        "wav".into(),
        output.display().to_string(),
    ]
}

/// Аргументы ffmpeg: видео без звука + отрывок → MP4 (H.264 копией или перекодом, AAC).
pub fn mux_args(
    video: &Path,
    h264: bool,
    audio: &Path,
    start_ms: i64,
    output: &Path,
) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostdin".into(),
        "-y".into(),
        "-i".into(),
        video.display().to_string(),
        "-ss".into(),
        secs(start_ms),
        "-t".into(),
        secs(STORY_MS),
        "-i".into(),
        audio.display().to_string(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        "1:a:0".into(),
    ];
    if h264 {
        a.extend(["-c:v".into(), "copy".into()]);
    } else {
        a.extend(
            [
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "19",
                "-pix_fmt",
                "yuv420p",
                "-profile:v",
                "high",
                "-r",
                "30",
            ]
            .map(String::from),
        );
    }
    a.extend([
        "-af".to_string(),
        fades(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-ac".into(),
        "2".into(),
        "-ar".into(),
        "44100".into(),
        "-t".into(),
        secs(STORY_MS),
        "-movflags".into(),
        "+faststart".into(),
        "-f".into(),
        "mp4".into(),
        output.display().to_string(),
    ]);
    a
}

/// Отрывок звука истории: WAV.
pub async fn audio_wav(st: &AppState, track_id: Uuid) -> AppResult<Vec<u8>> {
    let source = source(st, track_id).await?;
    let _slot = SLOTS
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let dir = tempfile::tempdir().map_err(|e| AppError::Internal(e.into()))?;
    let (input, start_ms) = fetch(st, &source, dir.path()).await?;
    let out = dir.path().join("story.wav");
    run("ffmpeg", &audio_args(&input, start_ms, &out)).await?;
    tokio::fs::read(&out)
        .await
        .map_err(|e| AppError::Internal(e.into()))
}

/// Кодек первого видеопотока (`h264`, `vp9`…); нет видео — ошибка.
async fn video_codec(path: &Path) -> AppResult<String> {
    let out = run(
        "ffprobe",
        &[
            "-v".into(),
            "error".into(),
            "-select_streams".into(),
            "v:0".into(),
            "-show_entries".into(),
            "stream=codec_name".into(),
            "-of".into(),
            "csv=p=0".into(),
            path.display().to_string(),
        ],
    )
    .await?;
    let codec = String::from_utf8_lossy(&out).trim().to_string();
    if codec.is_empty() {
        return Err(AppError::Unprocessable {
            code: "not_a_video",
            message: "в файле нет видео".into(),
        });
    }
    Ok(codec)
}

/// Видео без звука от приложения + отрывок → готовый MP4.
pub async fn mux(st: &AppState, track_id: Uuid, video: &[u8]) -> AppResult<Vec<u8>> {
    if video.is_empty() {
        return Err(AppError::BadRequest("нет видео".into()));
    }
    let source = source(st, track_id).await?;
    let _slot = SLOTS
        .acquire()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let dir = tempfile::tempdir().map_err(|e| AppError::Internal(e.into()))?;
    let video_path = dir.path().join("video");
    tokio::fs::write(&video_path, video)
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    let codec = video_codec(&video_path).await?;
    let (input, start_ms) = fetch(st, &source, dir.path()).await?;
    let out = dir.path().join("story.mp4");
    run(
        "ffmpeg",
        &mux_args(&video_path, codec == "h264", &input, start_ms, &out),
    )
    .await?;
    tokio::fs::read(&out)
        .await
        .map_err(|e| AppError::Internal(e.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_is_cut_from_the_hook_with_fades() {
        let a = audio_args(Path::new("/t/in"), 57_900, Path::new("/t/out.wav"));
        let s = a.join(" ");
        assert!(s.contains("-ss 57.900 -t 15.000 -i /t/in"), "{s}");
        assert!(
            s.contains("afade=t=in:st=0:d=0.25,afade=t=out:st=14.00:d=1"),
            "{s}"
        );
        assert!(s.ends_with("-c:a pcm_s16le -f wav /t/out.wav"), "{s}");
    }

    #[test]
    fn h264_is_copied_other_codecs_are_reencoded() {
        let copy = mux_args(Path::new("v"), true, Path::new("a"), 0, Path::new("o")).join(" ");
        assert!(copy.contains("-map 0:v:0 -map 1:a:0 -c:v copy"), "{copy}");
        assert!(copy.contains("-c:a aac -b:a 192k"), "{copy}");
        assert!(copy.contains("-movflags +faststart -f mp4 o"), "{copy}");
        let vp9 = mux_args(Path::new("v"), false, Path::new("a"), 1000, Path::new("o")).join(" ");
        assert!(vp9.contains("-c:v libx264"), "{vp9}");
        assert!(vp9.contains("-pix_fmt yuv420p"), "{vp9}");
        assert!(vp9.contains("-ss 1.000 -t 15.000 -i a"), "{vp9}");
    }
}
