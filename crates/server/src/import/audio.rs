//! Свои аудиофайлы: теги и формат (`ffprobe`), перепаковка (WAV, AIFF и ALAC → FLAC без потерь, Opus из
//! Ogg → MP4 — их играют все браузеры и движок приложений), громкость EBU R128, истинный пик и огибающая
//! (`ffmpeg`, как в fetcher), акустический отпечаток (`fpcalc`) для сверки с превью Deezer.
//! Инструменты ставятся в образ воркера; без них файл ждёт (задача откладывается).

use std::{path::Path, process::Stdio, time::Duration};

use dalywave_protocol::stream::{AudioCodec, AudioContainer};
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;

/// Точек огибающей для полосы прокрутки — столько же, сколько считает fetcher.
pub const WAVEFORM_POINTS: usize = 512;
const WAVEFORM_RATE: u32 = 8000;
/// Порог отпечатка — тот же, что у fetcher (bench/REPORT.md): свои записи от 0,92, чужие до 0,62.
pub const VERIFY_THRESHOLD: f32 = 0.80;
const TRIM_ITEMS: usize = 8;
const MIN_OVERLAP: f32 = 0.9;
const TOOL_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// Инструмента нет на этой машине — повторить там, где он есть.
    #[error("на сервере нет {0}")]
    Missing(&'static str),
    #[error("это не аудиофайл или он повреждён: {0}")]
    Unreadable(String),
    #[error("{0}")]
    Failed(String),
}

fn exe(name: &'static str) -> String {
    // Свой путь — для разработки на Mac (Homebrew) и нестандартных образов
    std::env::var(format!("DALYWAVE_{}", name.to_ascii_uppercase()))
        .unwrap_or_else(|_| name.to_string())
}

pub(crate) async fn run(
    name: &'static str,
    args: &[&std::ffi::OsStr],
) -> Result<std::process::Output, AudioError> {
    let mut cmd = Command::new(exe(name));
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AudioError::Missing(name),
        _ => AudioError::Failed(format!("{name} не запустился: {e}")),
    })?;
    match tokio::time::timeout(TOOL_TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => Err(AudioError::Failed(format!("{name}: {e}"))),
        Err(_) => Err(AudioError::Failed(format!("{name} не уложился в 10 минут"))),
    }
}

pub(crate) fn tail(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let s = s.trim();
    s.chars()
        .rev()
        .take(300)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// Теги файла.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub isrc: Option<String>,
}

/// Что внутри файла.
#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub codec: AudioCodec,
    /// Контейнер, как его назвал ffprobe (`flac`, `mov,mp4,m4a,…`, `ogg`, `wav`, `aiff`…).
    pub format: String,
    pub duration_ms: i32,
    pub bitrate_kbps: Option<i32>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i16>,
    pub bit_depth: Option<i16>,
    pub tags: Tags,
}

impl Probe {
    pub fn lossless(&self) -> bool {
        matches!(
            self.codec,
            AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Pcm
        )
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FfProbe {
    streams: Vec<FfStream>,
    format: FfFormat,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FfStream {
    codec_name: String,
    codec_type: String,
    sample_rate: Option<String>,
    channels: Option<i64>,
    bits_per_raw_sample: Option<String>,
    bits_per_sample: Option<i64>,
    bit_rate: Option<String>,
    duration: Option<String>,
    tags: serde_json::Map<String, Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FfFormat {
    format_name: String,
    duration: Option<String>,
    bit_rate: Option<String>,
    tags: serde_json::Map<String, Value>,
}

fn codec_of(name: &str) -> Option<AudioCodec> {
    Some(match name {
        "flac" => AudioCodec::Flac,
        "alac" => AudioCodec::Alac,
        "mp3" | "mp3float" => AudioCodec::Mp3,
        "opus" => AudioCodec::Opus,
        "vorbis" => AudioCodec::Vorbis,
        "aac" => AudioCodec::Aac,
        n if n.starts_with("pcm_") => AudioCodec::Pcm,
        _ => return None,
    })
}

fn tag(maps: &[&serde_json::Map<String, Value>], keys: &[&str]) -> Option<String> {
    for map in maps {
        for (k, v) in map.iter() {
            if keys.iter().any(|want| k.eq_ignore_ascii_case(want))
                && let Some(s) = v.as_str()
            {
                let s = s.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

fn num<T: std::str::FromStr>(s: &Option<String>) -> Option<T> {
    s.as_deref()?.trim().parse().ok()
}

/// Разбор вывода `ffprobe -print_format json -show_format -show_streams`.
pub fn parse_probe(json: &[u8]) -> Result<Probe, AudioError> {
    let p: FfProbe =
        serde_json::from_slice(json).map_err(|e| AudioError::Unreadable(e.to_string()))?;
    let stream = p
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .ok_or_else(|| AudioError::Unreadable("в файле нет звука".into()))?;
    let codec = codec_of(&stream.codec_name).ok_or_else(|| {
        AudioError::Unreadable(format!("кодек {} не поддерживается", stream.codec_name))
    })?;
    let seconds: f64 = num(&stream.duration)
        .or_else(|| num(&p.format.duration))
        .unwrap_or(0.0);
    if seconds < 1.0 {
        return Err(AudioError::Unreadable("слишком короткий файл".into()));
    }
    let bps: Option<f64> = num(&stream.bit_rate).or_else(|| num(&p.format.bit_rate));
    let maps = [&p.format.tags, &stream.tags];
    let bit_depth = num::<i16>(&stream.bits_per_raw_sample)
        .filter(|b| *b > 0)
        .or(stream.bits_per_sample.filter(|b| *b > 0).map(|b| b as i16));
    Ok(Probe {
        codec,
        format: p.format.format_name.clone(),
        duration_ms: (seconds * 1000.0).round() as i32,
        bitrate_kbps: bps.map(|b| (b / 1000.0).round() as i32).filter(|b| *b > 0),
        sample_rate: num(&stream.sample_rate),
        channels: stream.channels.map(|c| c as i16),
        bit_depth: if matches!(codec, AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Pcm) {
            bit_depth
        } else {
            None
        },
        tags: Tags {
            title: tag(&maps, &["title"]),
            artist: tag(&maps, &["artist", "artists"]),
            album: tag(&maps, &["album"]),
            album_artist: tag(&maps, &["album_artist", "albumartist", "album artist"]),
            isrc: tag(&maps, &["isrc", "tsrc"]),
        },
    })
}

pub async fn probe(path: &Path) -> Result<Probe, AudioError> {
    let out = run(
        "ffprobe",
        &[
            "-v".as_ref(),
            "error".as_ref(),
            "-print_format".as_ref(),
            "json".as_ref(),
            "-show_format".as_ref(),
            "-show_streams".as_ref(),
            path.as_os_str(),
        ],
    )
    .await?;
    if !out.status.success() {
        return Err(AudioError::Unreadable(tail(&out.stderr)));
    }
    parse_probe(&out.stdout)
}

/// Как хранить файл: кодек, контейнер, расширение и MIME; `None` у перепаковки — хранится как есть.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub codec: AudioCodec,
    pub container: AudioContainer,
    pub ext: &'static str,
    pub content_type: &'static str,
    pub repack: Option<Repack>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repack {
    /// Без потерь в FLAC: WAV, AIFF, ALAC.
    Flac,
    /// Тот же поток Opus в MP4.
    Mp4,
}

pub fn target(p: &Probe) -> Result<Target, AudioError> {
    let mp4 = p.format.contains("mp4") || p.format.contains("mov");
    Ok(match p.codec {
        AudioCodec::Flac if p.format == "flac" => Target {
            codec: AudioCodec::Flac,
            container: AudioContainer::Flac,
            ext: "flac",
            content_type: "audio/flac",
            repack: None,
        },
        AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Pcm => Target {
            codec: AudioCodec::Flac,
            container: AudioContainer::Flac,
            ext: "flac",
            content_type: "audio/flac",
            repack: Some(Repack::Flac),
        },
        AudioCodec::Mp3 => Target {
            codec: AudioCodec::Mp3,
            container: AudioContainer::Mp3,
            ext: "mp3",
            content_type: "audio/mpeg",
            repack: None,
        },
        AudioCodec::Aac if mp4 => Target {
            codec: AudioCodec::Aac,
            container: AudioContainer::Mp4,
            ext: "m4a",
            content_type: "audio/mp4",
            repack: None,
        },
        AudioCodec::Opus => Target {
            codec: AudioCodec::Opus,
            container: AudioContainer::Mp4,
            ext: "m4a",
            content_type: "audio/mp4",
            repack: (!mp4).then_some(Repack::Mp4),
        },
        AudioCodec::Vorbis if p.format.contains("ogg") => Target {
            codec: AudioCodec::Vorbis,
            container: AudioContainer::Ogg,
            ext: "ogg",
            content_type: "audio/ogg",
            repack: None,
        },
        AudioCodec::Aac | AudioCodec::Vorbis => {
            return Err(AudioError::Unreadable(format!(
                "{} в контейнере {} не поддерживается",
                p.codec, p.format
            )));
        }
    })
}

/// Перепаковка без потерь качества. Теги сохраняются.
pub async fn repack(input: &Path, output: &Path, how: Repack) -> Result<(), AudioError> {
    let codec: &[&str] = match how {
        Repack::Flac => &["-c:a", "flac", "-compression_level", "5"],
        Repack::Mp4 => &["-c:a", "copy", "-movflags", "+faststart"],
    };
    let mut args: Vec<&std::ffi::OsStr> = vec![
        "-nostdin".as_ref(),
        "-hide_banner".as_ref(),
        "-v".as_ref(),
        "error".as_ref(),
        "-y".as_ref(),
        "-i".as_ref(),
        input.as_os_str(),
        "-map".as_ref(),
        "0:a:0".as_ref(),
        "-map_metadata".as_ref(),
        "0".as_ref(),
    ];
    args.extend(codec.iter().map(|s| std::ffi::OsStr::new(*s)));
    args.push(output.as_os_str());
    let out = run("ffmpeg", &args).await?;
    if !out.status.success() {
        return Err(AudioError::Failed(format!("ffmpeg: {}", tail(&out.stderr))));
    }
    Ok(())
}

/// Облегчённая копия для сотовой сети: Opus с заданным битрейтом в MP4 (`+faststart` — играть можно,
/// не скачав файл целиком). Громкость не меняется: выравнивание берёт её у файла-источника.
pub async fn transcode_opus(input: &Path, output: &Path, kbps: u32) -> Result<(), AudioError> {
    let bitrate = format!("{kbps}k");
    let out = run(
        "ffmpeg",
        &[
            "-nostdin".as_ref(),
            "-hide_banner".as_ref(),
            "-v".as_ref(),
            "error".as_ref(),
            "-y".as_ref(),
            "-i".as_ref(),
            input.as_os_str(),
            "-map".as_ref(),
            "0:a:0".as_ref(),
            "-map_metadata".as_ref(),
            "-1".as_ref(),
            "-c:a".as_ref(),
            "libopus".as_ref(),
            "-b:a".as_ref(),
            bitrate.as_ref(),
            "-vbr".as_ref(),
            "on".as_ref(),
            "-ar".as_ref(),
            "48000".as_ref(),
            "-movflags".as_ref(),
            "+faststart".as_ref(),
            "-f".as_ref(),
            "mp4".as_ref(),
            output.as_os_str(),
        ],
    )
    .await?;
    if !out.status.success() {
        let err = tail(&out.stderr);
        if err.contains("Invalid data") {
            return Err(AudioError::Unreadable(err));
        }
        return Err(AudioError::Failed(format!("ffmpeg: {err}")));
    }
    Ok(())
}

/// Копия для AVPlayer (Apple TV не играет Opus): AAC-LC с заданным битрейтом в MP4, `+faststart`.
/// Частота — 48 кГц (24/96 в AAC не уложить), громкость не меняется: выравнивание берёт её у источника.
pub async fn transcode_aac(input: &Path, output: &Path, kbps: u32) -> Result<(), AudioError> {
    let bitrate = format!("{kbps}k");
    let out = run(
        "ffmpeg",
        &[
            "-nostdin".as_ref(),
            "-hide_banner".as_ref(),
            "-v".as_ref(),
            "error".as_ref(),
            "-y".as_ref(),
            "-i".as_ref(),
            input.as_os_str(),
            "-map".as_ref(),
            "0:a:0".as_ref(),
            "-map_metadata".as_ref(),
            "-1".as_ref(),
            "-c:a".as_ref(),
            "aac".as_ref(),
            "-b:a".as_ref(),
            bitrate.as_ref(),
            "-ar".as_ref(),
            "48000".as_ref(),
            "-movflags".as_ref(),
            "+faststart".as_ref(),
            "-f".as_ref(),
            "mp4".as_ref(),
            output.as_os_str(),
        ],
    )
    .await?;
    if !out.status.success() {
        let err = tail(&out.stderr);
        if err.contains("Invalid data") {
            return Err(AudioError::Unreadable(err));
        }
        return Err(AudioError::Failed(format!("ffmpeg: {err}")));
    }
    Ok(())
}

/// Громкость, истинный пик и огибающая.
#[derive(Debug, Clone, PartialEq)]
pub struct Loudness {
    pub lufs: Option<f32>,
    pub true_peak_db: Option<f32>,
    pub waveform: Vec<i16>,
}

fn after(log: &str, label: &str, unit: &str) -> Option<f32> {
    let i = log.find(label)?;
    let rest = log[i + label.len()..].trim_start();
    let end = rest.find(unit)?;
    let v = rest[..end].trim();
    if v == "-inf" {
        return None;
    }
    v.parse().ok()
}

/// RMS по равным кускам, нормированный на самый громкий: 0..1000 (как `envelope` в fetcher).
pub fn envelope(samples: &[i16], points: usize) -> Vec<i16> {
    if samples.is_empty() || points == 0 {
        return vec![0; points];
    }
    let mut rms = Vec::with_capacity(points);
    for p in 0..points {
        let from = samples.len() * p / points;
        let to = (samples.len() * (p + 1) / points).max(from);
        let chunk = &samples[from..to];
        if chunk.is_empty() {
            rms.push(0.0f64);
            continue;
        }
        let sum: f64 = chunk
            .iter()
            .map(|s| {
                let x = f64::from(*s) / 32768.0;
                x * x
            })
            .sum();
        rms.push((sum / chunk.len() as f64).sqrt());
    }
    let loudest = rms.iter().copied().fold(0.0, f64::max);
    if loudest <= 0.0 {
        return vec![0; points];
    }
    rms.iter()
        .map(|v| (v / loudest * 1000.0).round().clamp(0.0, 1000.0) as i16)
        .collect()
}

pub async fn loudness(path: &Path) -> Result<Loudness, AudioError> {
    let graph = format!(
        "[0:a:0]asplit=2[m][w];[m]ebur128=peak=true:framelog=quiet[mo];\
         [w]aresample={WAVEFORM_RATE},aformat=sample_fmts=s16:channel_layouts=mono[wo]"
    );
    let out = run(
        "ffmpeg",
        &[
            "-nostdin".as_ref(),
            "-hide_banner".as_ref(),
            "-nostats".as_ref(),
            "-i".as_ref(),
            path.as_os_str(),
            "-filter_complex".as_ref(),
            graph.as_ref(),
            "-map".as_ref(),
            "[mo]".as_ref(),
            "-f".as_ref(),
            "null".as_ref(),
            "-".as_ref(),
            "-map".as_ref(),
            "[wo]".as_ref(),
            "-f".as_ref(),
            "s16le".as_ref(),
            "pipe:1".as_ref(),
        ],
    )
    .await?;
    let log = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(AudioError::Failed(format!("ffmpeg: {}", tail(&out.stderr))));
    }
    let summary = log.rfind("Summary:").map_or(&log[..], |i| &log[i..]);
    let samples: Vec<i16> = out
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect();
    Ok(Loudness {
        lufs: after(summary, "I:", "LUFS"),
        true_peak_db: after(summary, "Peak:", "dBFS"),
        waveform: envelope(&samples, WAVEFORM_POINTS),
    })
}

/// Отпечаток chromaprint: по 32 бита на ~0,124 с.
pub async fn fingerprint(path: &Path) -> Result<Vec<u32>, AudioError> {
    let out = run(
        "fpcalc",
        &[
            "-raw".as_ref(),
            "-json".as_ref(),
            "-length".as_ref(),
            "0".as_ref(),
            path.as_os_str(),
        ],
    )
    .await?;
    if !out.status.success() {
        return Err(AudioError::Failed(format!("fpcalc: {}", tail(&out.stderr))));
    }
    #[derive(Deserialize)]
    struct Fp {
        fingerprint: Vec<i64>,
    }
    let fp: Fp =
        serde_json::from_slice(&out.stdout).map_err(|e| AudioError::Failed(e.to_string()))?;
    Ok(fp
        .fingerprint
        .into_iter()
        .map(|v| (v & 0xFFFF_FFFF) as u32)
        .collect())
}

/// Сходство превью с треком на лучшем сдвиге: 1 − доля несовпавших бит. Порт `align` из fetcher.
pub fn similarity(preview: &[u32], track: &[u32]) -> Option<f32> {
    let p = if preview.len() > 4 * TRIM_ITEMS {
        &preview[TRIM_ITEMS..preview.len() - TRIM_ITEMS]
    } else {
        preview
    };
    let (m, n) = (p.len(), track.len());
    if m == 0 || n == 0 {
        return None;
    }
    let need = ((m as f32 * MIN_OVERLAP) as usize).max(1);
    if n < need {
        return None;
    }
    // Превью может чуть выступать за края трека: считаем только перекрытые элементы
    let pad = m - need;
    let mut best = f32::INFINITY;
    for start in 0..(n + 2 * pad).saturating_sub(m) + 1 {
        let mut errors = 0u64;
        let mut count = 0usize;
        for (k, pv) in p.iter().enumerate() {
            let pos = start + k;
            if pos < pad || pos >= pad + n {
                continue;
            }
            errors += u64::from((track[pos - pad] ^ pv).count_ones());
            count += 1;
        }
        if count >= need {
            best = best.min(errors as f32 / (32.0 * count as f32));
        }
    }
    best.is_finite().then_some(1.0 - best)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAC: &str = r#"{"streams": [{"index": 0, "codec_name": "flac", "codec_type": "audio",
        "sample_rate": "96000", "channels": 2, "bits_per_raw_sample": "24", "duration": "162.5"}],
        "format": {"format_name": "flac", "duration": "162.500000", "bit_rate": "3100000",
        "tags": {"TITLE": "Devil", "ARTIST": "Lowborn", "ALBUM": "Devil", "ISRC": "QZ5FN2194847"}}}"#;

    #[test]
    fn probe_output() {
        let p = parse_probe(FLAC.as_bytes()).unwrap();
        assert_eq!(p.codec, AudioCodec::Flac);
        assert!(p.lossless());
        assert_eq!(p.duration_ms, 162_500);
        assert_eq!(p.bitrate_kbps, Some(3100));
        assert_eq!(p.sample_rate, Some(96_000));
        assert_eq!(p.bit_depth, Some(24));
        assert_eq!(p.tags.title.as_deref(), Some("Devil"));
        assert_eq!(p.tags.isrc.as_deref(), Some("QZ5FN2194847"));
        assert_eq!(target(&p).unwrap().repack, None);

        let wav = FLAC
            .replace("\"flac\", \"codec_type\"", "\"pcm_s16le\", \"codec_type\"")
            .replace("\"format_name\": \"flac\"", "\"format_name\": \"wav\"");
        let p = parse_probe(wav.as_bytes()).unwrap();
        assert_eq!(p.codec, AudioCodec::Pcm);
        let t = target(&p).unwrap();
        assert_eq!(
            (t.codec, t.ext, t.repack),
            (AudioCodec::Flac, "flac", Some(Repack::Flac))
        );

        let opus = r#"{"streams": [{"codec_name": "opus", "codec_type": "audio", "sample_rate": "48000",
            "channels": 2, "tags": {"title": "x"}}], "format": {"format_name": "ogg", "duration": "100.0", "bit_rate": "131000"}}"#;
        let p = parse_probe(opus.as_bytes()).unwrap();
        assert_eq!(p.bit_depth, None);
        assert_eq!(p.tags.title.as_deref(), Some("x"));
        let t = target(&p).unwrap();
        assert_eq!(
            (t.container, t.repack),
            (AudioContainer::Mp4, Some(Repack::Mp4))
        );

        assert!(
            parse_probe(
                br#"{"streams": [{"codec_type": "video", "codec_name": "h264"}], "format": {}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn loudness_log() {
        let log = "  Integrated loudness:\n    I:         -9.4 LUFS\n  True peak:\n    Peak:        0.6 dBFS";
        assert_eq!(after(log, "I:", "LUFS"), Some(-9.4));
        assert_eq!(after(log, "Peak:", "dBFS"), Some(0.6));
        assert_eq!(after("I: -inf LUFS", "I:", "LUFS"), None);
        let samples: Vec<i16> = (0..8000)
            .map(|i| if i < 4000 { 1000 } else { 4000 })
            .collect();
        let env = envelope(&samples, 4);
        assert_eq!(env, vec![250, 250, 1000, 1000]);
    }

    #[test]
    fn fingerprint_alignment() {
        // Псевдослучайный «трек» и превью — его кусок с 300-го элемента
        let mut x: u32 = 12345;
        let track: Vec<u32> = (0..1500)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x
            })
            .collect();
        let preview = track[300..540].to_vec();
        let same = similarity(&preview, &track).unwrap();
        assert!(same > 0.99, "{same}");
        let other: Vec<u32> = track
            .iter()
            .map(|v| v.rotate_left(7) ^ 0x5555_5555)
            .collect();
        let diff = similarity(&preview, &other).unwrap();
        assert!(diff < 0.7, "{diff}");
        assert_eq!(similarity(&[], &track), None);
    }
}
