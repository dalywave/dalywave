//! Результат fetcher — от домашнего загрузчика или от серверного: разбор, проверка файла в хранилище
//! и решение, которое закрывается одной транзакцией вместе с задачей. Принятый файл ставит полный
//! анализ звука записи.
//!
//! Файл принимается, если отпечаток совпал с превью Deezer; если превью нет, а оценка кандидата не ниже
//! 0,9; или если запись выбрал человек в «Спорных». Скачанное, но не совпавшее — «Спорные» (кандидаты
//! с разбором оценки сохраняются); ничего подходящего — «нет в источниках»; сбой — повтор с растущей
//! паузой, а сбой исполнителя (YouTube заподозрил бота) — отсрочка без траты попытки.

use std::time::Duration;

use dalywave_protocol::{
    catalog::AudioStatus,
    jobs::{
        AcquirePayload, AcquisitionCandidate, AcquisitionStatus, AttemptOutcome, CandidateAttempt,
        JobStatus,
    },
    stream::{AudioCodec, AudioContainer},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{publish, recording_changed, set_audio_status, set_audio_status_unless_ready};
use crate::{
    jobs::{self, JobRow},
    storage::Storage,
};

/// Ниже этой оценки fetcher кандидатов не качает; совпавший отпечаток при такой оценке подозрителен.
pub const MIN_SCORE: f64 = 0.6;
/// Без превью проверить нечем: принимается только очень хороший кандидат (так же решает fetcher).
pub const UNVERIFIED_ACCEPT_SCORE: f64 = 0.9;
/// Сбои, которые говорят об отношении YouTube (или Deezer) к исполнителю, а не о записи.
const SYSTEMIC: [&str; 8] = [
    "bot_check",
    "rate_limited",
    "http_403",
    "po_token",
    "network",
    "cookies_missing",
    "search_failed",
    "deezer_error",
];
/// Отсрочка после сбоя загрузчика: он сам отдыхает 15 минут, а задачу за это время может взять другой.
pub const REMOTE_POSTPONE: Duration = Duration::from_secs(120);
/// Самое длинное принимаемое описание формы волны.
const MAX_WAVEFORM: usize = 4096;

// ───────────── Ответ fetcher ─────────────

/// Ответ `POST /v1/acquire` сервиса fetcher (он же `result` в отчёте загрузчика). Незнакомые поля
/// пропускаются: fetcher обновляется отдельно от сервера.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FetcherResult {
    pub status: String,
    pub deezer_track_id: Option<i64>,
    pub chosen: Option<Chosen>,
    pub file: Option<FileInfo>,
    pub upload: Option<UploadReport>,
    pub attempts: Vec<AttemptInfo>,
    pub candidates: Vec<Value>,
    pub error: Option<ErrorInfo>,
    pub timings_ms: Option<Timings>,
    pub versions: Option<Versions>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Chosen {
    pub candidate: CandidateInfo,
    pub download: Option<DownloadInfo>,
    pub verify: Option<VerifyInfo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CandidateInfo {
    pub video_id: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub kind: Option<String>,
    pub source: Option<String>,
    pub score: Option<f64>,
    pub rejected: Option<String>,
    pub breakdown: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadInfo {
    pub video_id: Option<String>,
    pub container: Option<String>,
    pub codec: Option<String>,
    pub abr_kbps: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub size_bytes: Option<i64>,
    pub duration_ms: Option<i64>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VerifyInfo {
    pub verified: bool,
    pub similarity: Option<f64>,
    pub offset_s: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileInfo {
    pub sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub container: Option<String>,
    pub codec: Option<String>,
    pub duration_ms: Option<i64>,
    pub loudness_lufs: Option<f64>,
    pub true_peak_db: Option<f64>,
    pub waveform: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UploadReport {
    pub ok: bool,
    pub status: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AttemptInfo {
    pub video_id: Option<String>,
    pub score: Option<f64>,
    pub source: Option<String>,
    pub download: Option<DownloadInfo>,
    pub verify: Option<VerifyInfo>,
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ErrorInfo {
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Timings {
    pub total: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Versions {
    pub fetcher: Option<String>,
    pub yt_dlp: Option<String>,
}

// ───────────── Решение ─────────────

/// Новый файл записи — строка `audio_files`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewFile {
    pub id: Uuid,
    pub storage_key: String,
    pub codec: AudioCodec,
    pub container: AudioContainer,
    pub bitrate_kbps: Option<i32>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i16>,
    pub duration_ms: i32,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub source: &'static str,
    pub source_ref: Option<String>,
    pub source_title: Option<String>,
    pub match_score: Option<f32>,
    pub verified: bool,
    pub verify_similarity: Option<f32>,
    pub verify_offset_ms: Option<i32>,
    pub loudness_lufs: Option<f32>,
    pub true_peak_db: Option<f32>,
    pub waveform: Option<Vec<i16>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Файл принят: запись готова.
    Accept(Box<NewFile>),
    /// Кандидаты не прошли проверку — «Спорные».
    Dispute(String),
    /// Записи нет в источниках.
    Unavailable(String),
    /// Сбой: повторить позже, попытка засчитана.
    Retry(String),
    /// Сбой исполнителя, а не записи: отложить, попытку не засчитывать.
    Postpone(String),
    /// Повтор не поможет.
    Fail(String),
}

impl Decision {
    /// Скачивание закончилось — задача выполнена, что бы ни решилось о записи.
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            Self::Accept(_) | Self::Dispute(_) | Self::Unavailable(_)
        )
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Accept(_) => None,
            Self::Dispute(m)
            | Self::Unavailable(m)
            | Self::Retry(m)
            | Self::Postpone(m)
            | Self::Fail(m) => Some(m),
        }
    }
}

/// Разобранный результат и решение по нему.
#[derive(Debug, Clone)]
pub struct Examined {
    pub decision: Decision,
    /// Статус ответа fetcher.
    pub status: String,
    pub error_code: Option<String>,
    pub video_id: Option<String>,
    pub yt_dlp: Option<String>,
    pub total_ms: Option<i32>,
    pub candidates: Vec<AcquisitionCandidate>,
    pub chosen: Option<Value>,
    /// Файл загружен, но не принят: его место в хранилище надо освободить.
    pub discard_upload: bool,
}

impl Examined {
    fn failed(status: &str, decision: Decision) -> Self {
        Self {
            decision,
            status: status.into(),
            error_code: None,
            video_id: None,
            yt_dlp: None,
            total_ms: None,
            candidates: Vec::new(),
            chosen: None,
            discard_upload: false,
        }
    }
}

/// Что сервер знает о попытке ещё до результата.
#[derive(Debug, Clone)]
pub struct Expected {
    /// Ключ, на который выдана подписанная ссылка.
    pub upload_key: Option<String>,
    /// Длительность записи у Deezer — если fetcher её не прислал.
    pub duration_ms: i32,
}

pub async fn expected(
    conn: &mut PgConnection,
    acquisition_id: Uuid,
) -> Result<Expected, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT a.upload_key, r.duration_ms FROM acquisitions a JOIN recordings r ON r.id = a.recording_id
         WHERE a.id = $1",
        acquisition_id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(match row {
        Some(r) => Expected {
            upload_key: r.upload_key,
            duration_ms: r.duration_ms,
        },
        None => Expected {
            upload_key: None,
            duration_ms: 0,
        },
    })
}

/// Принять ли выбранный fetcher файл. `Ok(verified)` — принять; `Err` — причина для «Спорных».
pub fn judge(result: &FetcherResult, payload: &AcquirePayload) -> Result<bool, String> {
    let Some(chosen) = &result.chosen else {
        return Err("fetcher не выбрал кандидата".into());
    };
    let video = chosen
        .candidate
        .video_id
        .as_deref()
        .or_else(|| chosen.download.as_ref()?.video_id.as_deref());
    let score = chosen.candidate.score.unwrap_or(0.0);
    let verified = chosen.verify.as_ref().is_some_and(|v| v.verified);
    // Выбор человека в «Спорных» — решение, а не догадка: принимается и без совпадения отпечатка
    let manual = video.is_some_and(|v| {
        payload.prefer_video_ids.iter().any(|p| p == v) || payload.direct_url.as_deref() == Some(v)
    });
    if manual {
        return Ok(verified);
    }
    if verified {
        return if score >= MIN_SCORE {
            Ok(true)
        } else {
            Err(format!(
                "отпечаток совпал, но оценка кандидата {score:.2} слишком низкая"
            ))
        };
    }
    match &chosen.verify {
        None if score >= UNVERIFIED_ACCEPT_SCORE => Ok(false),
        None => Err(format!(
            "проверить нечем (у трека нет превью в Deezer), а оценка {score:.2} ниже {UNVERIFIED_ACCEPT_SCORE}"
        )),
        Some(v) => Err(format!(
            "отпечаток не совпал с превью Deezer: сходство {:.2}",
            v.similarity.unwrap_or(0.0)
        )),
    }
}

/// Решение без проверки хранилища: `None` — файл подходит, его ещё надо найти в хранилище.
fn plan(result: &FetcherResult, payload: &AcquirePayload) -> Result<bool, Decision> {
    match result.status.as_str() {
        "verified" | "unverified" => judge(result, payload).map_err(Decision::Dispute),
        "disputed" => Err(Decision::Dispute(
            "ни один скачанный кандидат не совпал с превью Deezer".into(),
        )),
        "not_found" => Err(Decision::Unavailable(
            "подходящих записей в источниках нет".into(),
        )),
        "error" => {
            let error = result.error.clone().unwrap_or_default();
            let code = error.code.unwrap_or_else(|| "unknown".into());
            let message = error.message.unwrap_or_else(|| "сбой без описания".into());
            let text = format!("{code}: {message}");
            Err(if SYSTEMIC.contains(&code.as_str()) {
                Decision::Postpone(text)
            } else if code == "deezer_not_found" {
                Decision::Unavailable("трека больше нет в Deezer".into())
            } else {
                Decision::Retry(text)
            })
        }
        other => Err(Decision::Retry(format!(
            "неизвестный статус результата: «{other}»"
        ))),
    }
}

pub(crate) fn parse_codec(raw: &str) -> Option<AudioCodec> {
    match raw.to_ascii_lowercase().as_str() {
        "mp4a" | "m4a" => Some(AudioCodec::Aac),
        other => other.parse().ok(),
    }
}

fn parse_container(raw: &str) -> Option<AudioContainer> {
    match raw.to_ascii_lowercase().as_str() {
        "m4a" | "mov" | "isobmff" => Some(AudioContainer::Mp4),
        "opus" => Some(AudioContainer::Ogg),
        other => other.parse().ok(),
    }
}

/// Откуда файл. Кандидаты из поиска YouTube Music (альбом, ISRC, песни) и официальное аудио —
/// YouTube Music; клипы и видео — YouTube; вставленная ссылка — по её адресу.
pub fn file_source(
    candidate: &CandidateInfo,
    video: Option<&str>,
    payload: &AcquirePayload,
) -> &'static str {
    if let (Some(url), Some(video)) = (payload.direct_url.as_deref(), video)
        && url == video
    {
        return source_of_url(url);
    }
    let music_search = matches!(candidate.source.as_deref(), Some("album" | "isrc" | "song"));
    if music_search || candidate.kind.as_deref() == Some("atv") {
        "youtube_music"
    } else {
        "youtube"
    }
}

pub fn source_of_url(url: &str) -> &'static str {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()));
    let Some(host) = host else {
        return "other";
    };
    let host = host.trim_start_matches("www.");
    let under = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    if host == "music.youtube.com" {
        "youtube_music"
    } else if under("youtube.com") || host == "youtu.be" {
        "youtube"
    } else if under("soundcloud.com") {
        "soundcloud"
    } else if under("bandcamp.com") {
        "bandcamp"
    } else {
        "other"
    }
}

fn finite(v: Option<f64>) -> Option<f32> {
    v.filter(|x| x.is_finite()).map(|x| x as f32)
}

/// Огибающая 0..1000 из ответа fetcher; пустая или подозрительно длинная — нет огибающей.
fn waveform(raw: Option<&[f64]>) -> Option<Vec<i16>> {
    let raw = raw?;
    if raw.is_empty() || raw.len() > MAX_WAVEFORM {
        return None;
    }
    Some(
        raw.iter()
            .map(|v| {
                if v.is_finite() {
                    v.round().clamp(0.0, 1000.0) as i16
                } else {
                    0
                }
            })
            .collect(),
    )
}

/// id файла из ключа `audio/{recordingId}/{fileId}.{ext}`.
fn file_id(key: &str) -> Option<Uuid> {
    key.rsplit('/').next()?.split('.').next()?.parse().ok()
}

/// Файл подходит по решению — убедиться, что он лежит в хранилище целиком, и собрать строку.
async fn accept(
    storage: &Storage,
    payload: &AcquirePayload,
    expected: &Expected,
    result: &FetcherResult,
    verified: bool,
) -> Result<NewFile, Decision> {
    let retry = |m: String| Decision::Retry(m);
    let chosen = result
        .chosen
        .as_ref()
        .ok_or_else(|| retry("в результате нет выбранного кандидата".into()))?;
    let file = result
        .file
        .as_ref()
        .ok_or_else(|| retry("в результате нет файла".into()))?;
    match &result.upload {
        Some(u) if u.ok => {}
        Some(u) => {
            let why = u
                .error
                .clone()
                .or_else(|| u.status.map(|s| format!("HTTP {s}")))
                .unwrap_or_else(|| "без описания".into());
            return Err(retry(format!("файл не загрузился в хранилище: {why}")));
        }
        None => return Err(retry("файл не загружен в хранилище".into())),
    }
    let key = expected
        .upload_key
        .clone()
        .ok_or_else(|| retry("у попытки нет ключа файла".into()))?;
    let meta = match storage.head(&key).await {
        Ok(Some(meta)) => meta,
        Ok(None) => return Err(retry("загруженного файла нет в хранилище".into())),
        Err(e) => return Err(retry(format!("хранилище: {e}"))),
    };
    if meta.size == 0 {
        return Err(retry("в хранилище пустой файл".into()));
    }
    if let Some(size) = file.size_bytes
        && u64::try_from(size).ok() != Some(meta.size)
    {
        return Err(retry(format!(
            "в хранилище {} байт, а загрузчик отправил {size}",
            meta.size
        )));
    }
    let download = chosen.download.clone().unwrap_or_default();
    let codec = file
        .codec
        .as_deref()
        .or(download.codec.as_deref())
        .and_then(parse_codec)
        .ok_or_else(|| Decision::Fail("неизвестный кодек файла".into()))?;
    let container = file
        .container
        .as_deref()
        .or(download.container.as_deref())
        .and_then(parse_container)
        .ok_or_else(|| Decision::Fail("неизвестный контейнер файла".into()))?;
    let duration_ms = file
        .duration_ms
        .or(download.duration_ms)
        .filter(|d| *d > 0)
        .and_then(|d| i32::try_from(d).ok())
        .unwrap_or(expected.duration_ms);
    let video = chosen
        .candidate
        .video_id
        .clone()
        .or(download.video_id.clone());
    let verify = chosen.verify.clone().unwrap_or_default();
    Ok(NewFile {
        id: file_id(&key).unwrap_or_else(Uuid::now_v7),
        source: file_source(&chosen.candidate, video.as_deref(), payload),
        storage_key: key,
        codec,
        container,
        bitrate_kbps: download
            .abr_kbps
            .filter(|b| b.is_finite() && *b > 0.0)
            .map(|b| b.round() as i32),
        sample_rate: download.sample_rate.and_then(|s| i32::try_from(s).ok()),
        channels: download.channels.and_then(|c| i16::try_from(c).ok()),
        duration_ms,
        size_bytes: i64::try_from(meta.size).unwrap_or(i64::MAX),
        sha256: file.sha256.clone().filter(|s| s.len() == 64),
        source_title: chosen
            .candidate
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .or(download.title),
        source_ref: video,
        match_score: finite(chosen.candidate.score),
        verified,
        verify_similarity: if chosen.verify.is_some() {
            finite(verify.similarity)
        } else {
            None
        },
        verify_offset_ms: verify
            .offset_s
            .filter(|s| s.is_finite())
            .map(|s| (s * 1000.0).round() as i32),
        loudness_lufs: finite(file.loudness_lufs),
        true_peak_db: finite(file.true_peak_db),
        waveform: waveform(file.waveform.as_deref()),
    })
}

/// Кандидаты по убыванию оценки и попытки скачать их — одним списком для раздела «Спорные».
fn candidates(result: &FetcherResult) -> Vec<AcquisitionCandidate> {
    let mut out: Vec<AcquisitionCandidate> = result
        .candidates
        .iter()
        .filter_map(|raw| serde_json::from_value::<CandidateInfo>(raw.clone()).ok())
        .filter_map(|c| {
            Some(AcquisitionCandidate {
                video_id: c.video_id?,
                url: c.url,
                title: c.title.unwrap_or_default(),
                artists: c.artists,
                album: c.album,
                duration_ms: c.duration_ms,
                kind: c.kind,
                source: c.source,
                score: c.score,
                rejected: c.rejected,
                breakdown: c.breakdown,
                attempt: None,
            })
        })
        .collect();
    for a in &result.attempts {
        let Some(video) = a.video_id.clone() else {
            continue;
        };
        let error = a.error.clone().unwrap_or_default();
        let attempt = CandidateAttempt {
            downloaded: a.download.is_some(),
            verified: a.verify.as_ref().map(|v| v.verified),
            similarity: a.verify.as_ref().and_then(|v| v.similarity),
            offset_ms: a
                .verify
                .as_ref()
                .and_then(|v| v.offset_s)
                .filter(|s| s.is_finite())
                .map(|s| (s * 1000.0).round() as i64),
            error_code: error.code,
            error: error.message,
        };
        match out.iter_mut().find(|c| c.video_id == video) {
            Some(c) => c.attempt = Some(attempt),
            // Выбранное вручную в поиске не было — показываем его по данным скачивания
            None => out.push(AcquisitionCandidate {
                video_id: video,
                url: None,
                title: a
                    .download
                    .as_ref()
                    .and_then(|d| d.title.clone())
                    .unwrap_or_default(),
                artists: Vec::new(),
                album: None,
                duration_ms: a.download.as_ref().and_then(|d| d.duration_ms),
                kind: None,
                source: a.source.clone(),
                score: a.score,
                rejected: None,
                breakdown: json!({}),
                attempt: Some(attempt),
            }),
        }
    }
    out
}

/// Разбирает результат и решает, что с ним делать. Файл проверяется в хранилище: он должен лежать
/// по ключу, выданному этой попытке, и совпадать по размеру с тем, что отправил исполнитель.
pub async fn examine(
    storage: &Storage,
    payload: &AcquirePayload,
    expected: &Expected,
    raw: &Value,
) -> Examined {
    let result: FetcherResult = match serde_json::from_value(raw.clone()) {
        Ok(r) => r,
        Err(e) => {
            return Examined::failed(
                "invalid",
                Decision::Retry(format!("исполнитель прислал непонятный результат: {e}")),
            );
        }
    };
    let uploaded = result.upload.as_ref().is_some_and(|u| u.ok);
    let mut ex = Examined {
        decision: Decision::Retry(String::new()),
        status: result.status.clone(),
        error_code: result.error.as_ref().and_then(|e| e.code.clone()),
        video_id: result.chosen.as_ref().and_then(|c| {
            c.candidate
                .video_id
                .clone()
                .or_else(|| c.download.as_ref()?.video_id.clone())
        }),
        yt_dlp: result.versions.as_ref().and_then(|v| v.yt_dlp.clone()),
        total_ms: result
            .timings_ms
            .as_ref()
            .and_then(|t| t.total)
            .and_then(|t| i32::try_from(t).ok()),
        candidates: candidates(&result),
        chosen: raw.get("chosen").filter(|c| !c.is_null()).cloned(),
        discard_upload: false,
    };
    if let Some(id) = result.deezer_track_id
        && id != payload.deezer_track_id
    {
        ex.decision = Decision::Fail(format!("результат о другом треке Deezer ({id})"));
        ex.discard_upload = uploaded;
        return ex;
    }
    ex.decision = match plan(&result, payload) {
        Ok(verified) => match accept(storage, payload, expected, &result, verified).await {
            Ok(file) => Decision::Accept(Box::new(file)),
            Err(d) => d,
        },
        Err(d) => d,
    };
    ex.discard_upload = uploaded && !matches!(ex.decision, Decision::Accept(_));
    ex
}

// ───────────── Запись решения ─────────────

/// Порядок качества файлов: lossless лучше любого сжатия, проверенный отпечатком — непроверенного,
/// дальше — битрейт с поправкой на кодек (Opus 130 кбит/с не хуже AAC 170).
pub(crate) fn quality(codec: AudioCodec, verified: bool, kbps: Option<i32>) -> (bool, bool, i64) {
    let lossless = matches!(codec, AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Pcm);
    let factor = match codec {
        AudioCodec::Opus => 1.0,
        AudioCodec::Vorbis => 0.85,
        AudioCodec::Aac => 0.75,
        AudioCodec::Mp3 => 0.6,
        AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Pcm => 1.0,
    };
    let effective = f64::from(kbps.unwrap_or(0)) * factor;
    (lossless, verified, effective.round() as i64)
}

async fn better_than_primary(
    conn: &mut PgConnection,
    recording_id: Uuid,
    file: &NewFile,
) -> Result<bool, sqlx::Error> {
    let current = sqlx::query!(
        "SELECT f.codec, f.verified, f.bitrate_kbps FROM recordings r JOIN audio_files f ON f.id = r.primary_file_id
         WHERE r.id = $1",
        recording_id,
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(current) = current else {
        return Ok(true);
    };
    let Some(codec) = parse_codec(&current.codec) else {
        return Ok(true);
    };
    Ok(quality(file.codec, file.verified, file.bitrate_kbps)
        > quality(codec, current.verified, current.bitrate_kbps))
}

/// Записывает окончательное решение: файл, основной файл записи, состояния скачивания и записи.
async fn record_final(
    conn: &mut PgConnection,
    payload: &AcquirePayload,
    executor: &str,
    ex: &Examined,
) -> Result<(), sqlx::Error> {
    let candidates = serde_json::to_value(&ex.candidates).unwrap_or_else(|_| json!([]));
    let (status, error) = match &ex.decision {
        Decision::Accept(file) => {
            sqlx::query!(
                "INSERT INTO audio_files (id, recording_id, storage_key, codec, container, lossless, bitrate_kbps,
                                          sample_rate, channels, duration_ms, size_bytes, sha256, source, source_ref,
                                          source_title, loudness_lufs, true_peak_db, waveform, match_score, verified,
                                          verify_similarity, verify_offset_ms, acquired_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                         $21, $22, $23)
                 ON CONFLICT (storage_key) DO NOTHING",
                file.id,
                payload.recording_id,
                file.storage_key,
                file.codec.as_str(),
                file.container.as_str(),
                matches!(file.codec, AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Pcm),
                file.bitrate_kbps,
                file.sample_rate,
                file.channels,
                file.duration_ms,
                file.size_bytes,
                file.sha256,
                file.source,
                file.source_ref,
                file.source_title,
                file.loudness_lufs,
                file.true_peak_db,
                file.waveform.as_deref(),
                file.match_score,
                file.verified,
                file.verify_similarity,
                file.verify_offset_ms,
                executor,
            )
            .execute(&mut *conn)
            .await?;
            // Выбор человека и жалоба «Неверная запись» заменяют файл, даже если прежний «лучше»:
            // прежний — не та запись
            if payload.is_manual()
                || payload.replace
                || better_than_primary(conn, payload.recording_id, file).await?
            {
                sqlx::query!(
                    "UPDATE recordings SET primary_file_id = $2 WHERE id = $1",
                    payload.recording_id,
                    file.id,
                )
                .execute(&mut *conn)
                .await?;
            }
            set_audio_status(conn, payload.recording_id, AudioStatus::Ready).await?;
            // Файл на сервере — полный анализ: темп, тональность, сетка долей для переходов, векторы
            crate::reco::analysis::request_full(conn, payload.recording_id).await?;
            (AcquisitionStatus::Ready, None)
        }
        Decision::Dispute(reason) => {
            set_audio_status_unless_ready(conn, payload.recording_id, AudioStatus::Disputed)
                .await?;
            (AcquisitionStatus::Disputed, Some(reason.as_str()))
        }
        Decision::Unavailable(reason) => {
            set_audio_status_unless_ready(conn, payload.recording_id, AudioStatus::Unavailable)
                .await?;
            (AcquisitionStatus::Unavailable, Some(reason.as_str()))
        }
        Decision::Retry(_) | Decision::Postpone(_) | Decision::Fail(_) => return Ok(()),
    };
    sqlx::query!(
        "UPDATE acquisitions SET status = $2, error = $3, chosen = $4, candidates = $5, worker = $6,
                updated_at = now(), finished_at = now()
         WHERE id = $1",
        payload.acquisition_id,
        status.as_str(),
        error,
        ex.chosen,
        candidates,
        executor,
    )
    .execute(&mut *conn)
    .await?;
    publish(conn, payload.acquisition_id).await?;
    recording_changed(conn, payload.recording_id).await
}

/// Скачивание вслед за задачей, которая не закончилась успехом: снова в очереди — и запись «в очереди»,
/// попытки кончились — «не удалось». Закрытое (отменённое, решённое в админке) не трогается.
pub async fn sync_with_job(
    conn: &mut PgConnection,
    job: &JobRow,
    payload: &AcquirePayload,
    candidates: Option<&[AcquisitionCandidate]>,
) -> Result<(), sqlx::Error> {
    let (status, audio) = match job.status() {
        JobStatus::Queued => (AcquisitionStatus::Queued, AudioStatus::Queued),
        JobStatus::Failed => (AcquisitionStatus::Failed, AudioStatus::Failed),
        JobStatus::Running | JobStatus::Done | JobStatus::Cancelled => return Ok(()),
    };
    let candidates = candidates
        .filter(|c| !c.is_empty())
        .and_then(|c| serde_json::to_value(c).ok());
    let updated = sqlx::query!(
        "UPDATE acquisitions SET status = $2, error = COALESCE($3, error), candidates = COALESCE($4, candidates),
                updated_at = now(), finished_at = CASE WHEN $5 THEN now() ELSE NULL END
         WHERE id = $1 AND status IN ('queued', 'matching', 'downloading', 'verifying', 'processing')",
        payload.acquisition_id,
        status.as_str(),
        job.last_error,
        candidates,
        !status.is_active(),
    )
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Ok(());
    }
    if set_audio_status_unless_ready(conn, payload.recording_id, audio).await?
        && audio == AudioStatus::Failed
    {
        recording_changed(conn, payload.recording_id).await?;
    }
    publish(conn, payload.acquisition_id).await
}

async fn log_attempt(
    conn: &mut PgConnection,
    payload: &AcquirePayload,
    job_id: Uuid,
    executor: &str,
    ex: &Examined,
    outcome: AttemptOutcome,
) -> Result<(), sqlx::Error> {
    let error: Option<String> = ex
        .decision
        .message()
        .map(|m| m.chars().take(2000).collect());
    sqlx::query!(
        "INSERT INTO acquisition_attempts (acquisition_id, job_id, executor, status, outcome, error_code, error,
                                           video_id, yt_dlp, total_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        payload.acquisition_id,
        job_id,
        executor,
        ex.status.chars().take(32).collect::<String>(),
        outcome.as_str(),
        ex.error_code,
        error,
        ex.video_id,
        ex.yt_dlp,
        ex.total_ms,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Чем кончилась попытка для очереди и для журнала.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    pub outcome: AttemptOutcome,
    /// Причина неудачи или решения «Спорные».
    pub message: Option<String>,
}

/// Закрывает задачу по результату fetcher — одной транзакцией с файлом, скачиванием и записью.
/// `lock` — кто держит задачу в очереди (`jobs.locked_by`), `executor` — кто качал: `device:<id>`
/// загрузчика или `server`. `Ok(None)` — задача уже не его (аренда кончилась, задачу отменили).
pub async fn settle(
    db: &PgPool,
    storage: &Storage,
    job: &JobRow,
    lock: &str,
    executor: &str,
    raw: &Value,
    postpone: Duration,
) -> Result<Option<Settled>, sqlx::Error> {
    let payload: AcquirePayload = match job.payload() {
        Ok(p) => p,
        Err(e) => {
            let message = format!("неверные данные задачи: {e}");
            let closed = jobs::fail(db, job.id, lock, &message, false).await?;
            return Ok(closed.map(|_| Settled {
                outcome: AttemptOutcome::Failed,
                message: Some(message),
            }));
        }
    };
    let mut conn = db.acquire().await?;
    let expected = expected(&mut conn, payload.acquisition_id).await?;
    drop(conn);
    let ex = examine(storage, &payload, &expected, raw).await;
    let summary = json!({
        "status": ex.status,
        "videoId": ex.video_id,
        "decision": match &ex.decision {
            Decision::Accept(f) => json!({ "accepted": f.id, "verified": f.verified }),
            other => json!({ "message": other.message() }),
        },
        "totalMs": ex.total_ms,
    });
    let mut tx = db.begin().await?;
    let closed = match &ex.decision {
        Decision::Accept(_) | Decision::Dispute(_) | Decision::Unavailable(_) => {
            jobs::complete_in(&mut tx, job.id, lock, summary).await?
        }
        Decision::Retry(m) => jobs::fail_in(&mut tx, job.id, lock, m, true).await?,
        Decision::Fail(m) => jobs::fail_in(&mut tx, job.id, lock, m, false).await?,
        Decision::Postpone(m) => jobs::postpone_in(&mut tx, job.id, lock, m, postpone).await?,
    };
    // Задача уже не наша. Файл не удаляем: ключ в скачивании мог смениться на ключ следующей попытки
    let Some(closed) = closed else {
        return Ok(None);
    };
    let outcome = match &ex.decision {
        Decision::Accept(_) => AttemptOutcome::Ready,
        Decision::Dispute(_) => AttemptOutcome::Disputed,
        Decision::Unavailable(_) => AttemptOutcome::Unavailable,
        Decision::Postpone(_) => AttemptOutcome::Postponed,
        Decision::Retry(_) | Decision::Fail(_) if closed.status() == JobStatus::Queued => {
            AttemptOutcome::Retry
        }
        Decision::Retry(_) | Decision::Fail(_) => AttemptOutcome::Failed,
    };
    if ex.decision.is_final() {
        record_final(&mut tx, &payload, executor, &ex).await?;
    } else {
        sync_with_job(&mut tx, &closed, &payload, Some(&ex.candidates)).await?;
    }
    log_attempt(&mut tx, &payload, job.id, executor, &ex, outcome).await?;
    tx.commit().await?;
    // Администраторам: спорное ждёт решения, YouTube перестал отдавать звук
    let notified = match (outcome, ex.error_code.as_deref()) {
        (AttemptOutcome::Disputed, _) => crate::notify::events::disputed(db).await,
        (AttemptOutcome::Postponed, Some(code)) => {
            crate::notify::events::source_blocked(db, code).await
        }
        _ => Ok(()),
    };
    if let Err(e) = notified {
        tracing::warn!(error = %e, "could not notify admins");
    }
    if ex.discard_upload {
        discard(storage, expected.upload_key.as_deref()).await;
    }
    tracing::info!(job = %job.id, %executor, outcome = %outcome, status = %ex.status, "acquire result");
    Ok(Some(Settled {
        outcome,
        message: ex.decision.message().map(str::to_string),
    }))
}

/// Освобождает место непринятого файла. Не вышло — не беда: ключ уникален и больше не используется.
async fn discard(storage: &Storage, key: Option<&str>) {
    if let Some(key) = key
        && let Err(e) = storage.delete(key).await
    {
        tracing::warn!(%key, error = %e, "could not delete a rejected upload");
    }
}

/// Возвращает взятую задачу в очередь без траты попытки: исполнитель не смог начать (хранилище,
/// сервис недоступен).
pub async fn give_back(
    db: &PgPool,
    job: &JobRow,
    payload: &AcquirePayload,
    lock: &str,
    message: &str,
    delay: Duration,
) -> Result<bool, sqlx::Error> {
    let mut tx = db.begin().await?;
    let Some(closed) = jobs::postpone_in(&mut tx, job.id, lock, message, delay).await? else {
        return Ok(false);
    };
    sync_with_job(&mut tx, &closed, payload, None).await?;
    tx.commit().await?;
    Ok(true)
}

/// Скачивание вслед за задачей, закрытой не через [`settle`]: её вернул сборщик зависших или освободил
/// отзыв загрузчика.
pub async fn after_close(db: &PgPool, job: &JobRow) -> Result<(), sqlx::Error> {
    let Ok(payload) = job.payload::<AcquirePayload>() else {
        return Ok(());
    };
    let mut tx = db.begin().await?;
    sync_with_job(&mut tx, job, &payload, None).await?;
    tx.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> AcquirePayload {
        AcquirePayload {
            acquisition_id: Uuid::nil(),
            recording_id: Uuid::nil(),
            track_id: Uuid::nil(),
            deezer_track_id: 3040222091,
            prefer_video_ids: vec![],
            exclude_video_ids: vec![],
            direct_url: None,
            replace: false,
        }
    }

    fn result(status: &str, score: f64, verify: Option<(bool, f64)>) -> FetcherResult {
        serde_json::from_value(json!({
            "status": status,
            "deezerTrackId": 3040222091i64,
            "chosen": {
                "candidate": {"videoId": "afkL3uLNeXA", "title": "ящики", "source": "album", "kind": "atv", "score": score},
                "download": {"codec": "opus", "container": "mp4", "abrKbps": 136.1},
                "verify": verify.map(|(v, s)| json!({"verified": v, "similarity": s, "offsetS": 12.5})),
            },
        }))
        .unwrap()
    }

    #[test]
    fn fingerprint_decides_and_low_scores_are_disputed() {
        let p = payload();
        assert_eq!(
            judge(&result("verified", 1.0, Some((true, 0.97))), &p),
            Ok(true)
        );
        assert!(judge(&result("verified", 0.5, Some((true, 0.97))), &p).is_err());
        // Без превью — только очень хороший кандидат
        assert_eq!(judge(&result("unverified", 0.95, None), &p), Ok(false));
        assert!(judge(&result("unverified", 0.85, None), &p).is_err());
        // Отпечаток не совпал, а выбора человека не было
        assert!(judge(&result("unverified", 0.99, Some((false, 0.55))), &p).is_err());
    }

    #[test]
    fn manual_choice_is_accepted_without_fingerprint() {
        let mut p = payload();
        p.prefer_video_ids = vec!["afkL3uLNeXA".into()];
        assert_eq!(
            judge(&result("unverified", 0.3, Some((false, 0.55))), &p),
            Ok(false)
        );
    }

    #[test]
    fn errors_are_sorted_by_whose_fault() {
        let p = payload();
        let err = |code: &str| -> FetcherResult {
            serde_json::from_value(
                json!({"status": "error", "error": {"code": code, "message": "m"}}),
            )
            .unwrap()
        };
        assert!(matches!(
            plan(&err("bot_check"), &p),
            Err(Decision::Postpone(_))
        ));
        assert!(matches!(
            plan(&err("geo_blocked"), &p),
            Err(Decision::Retry(_))
        ));
        assert!(matches!(
            plan(&err("deezer_not_found"), &p),
            Err(Decision::Unavailable(_))
        ));
        let found: FetcherResult = serde_json::from_value(json!({"status": "not_found"})).unwrap();
        assert!(matches!(plan(&found, &p), Err(Decision::Unavailable(_))));
        let disputed: FetcherResult =
            serde_json::from_value(json!({"status": "disputed"})).unwrap();
        assert!(matches!(plan(&disputed, &p), Err(Decision::Dispute(_))));
    }

    #[test]
    fn sources_and_formats() {
        assert_eq!(source_of_url("https://soundcloud.com/a/b"), "soundcloud");
        assert_eq!(
            source_of_url("https://artist.bandcamp.com/track/x"),
            "bandcamp"
        );
        assert_eq!(
            source_of_url("https://music.youtube.com/watch?v=x"),
            "youtube_music"
        );
        assert_eq!(
            source_of_url("https://www.youtube.com/watch?v=x"),
            "youtube"
        );
        assert_eq!(source_of_url("https://example.com/a.mp3"), "other");
        assert_eq!(source_of_url("not a url"), "other");
        assert_eq!(parse_container("m4a"), Some(AudioContainer::Mp4));
        assert_eq!(parse_container("mp4"), Some(AudioContainer::Mp4));
        assert_eq!(parse_codec("mp4a"), Some(AudioCodec::Aac));
        assert_eq!(parse_codec("opus"), Some(AudioCodec::Opus));
        let id = Uuid::now_v7();
        assert_eq!(
            file_id(&format!("audio/{}/{id}.m4a", Uuid::nil())),
            Some(id)
        );
        assert_eq!(
            waveform(Some(&[-5.0, 400.4, 1200.0, f64::NAN])),
            Some(vec![0, 400, 1000, 0])
        );
        assert_eq!(waveform(Some(&[])), None);
    }

    #[test]
    fn better_files_win() {
        let opus = quality(AudioCodec::Opus, true, Some(130));
        assert!(
            quality(AudioCodec::Flac, false, Some(900)) > opus,
            "lossless"
        );
        assert!(
            opus > quality(AudioCodec::Opus, false, Some(160)),
            "проверенный"
        );
        assert!(opus > quality(AudioCodec::Aac, true, Some(128)));
        assert!(quality(AudioCodec::Opus, true, Some(256)) > opus);
    }
}
