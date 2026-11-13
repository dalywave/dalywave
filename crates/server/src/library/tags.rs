//! Что внутри файла своей музыки: теги (lofty — ID3, Vorbis comments, MP4, APE, RIFF INFO), длительность,
//! формат и встроенная обложка. Файл не меняется; чтение — блокирующее, вызывать из `spawn_blocking`.
//!
//! Играют ли клиенты файл как есть — те же правила, что у своих загруженных файлов (`import::audio::target`):
//! FLAC, MP3, AAC и FLAC в MP4, Vorbis в Ogg и WAV — да; ALAC, AIFF, Opus в Ogg, APE, WavPack, Musepack —
//! нет, им нужна копия.

use std::{fs::File, path::Path};

use chrono::NaiveDate;
use lofty::{
    config::ParseOptions,
    file::{AudioFile, FileType, TaggedFileExt},
    mp4::{Mp4Codec, Mp4File},
    picture::PictureType,
    tag::{ItemKey, Tag},
};
use serde::{Deserialize, Serialize};

/// Расширения, которые сканер считает аудио. Остальные файлы папки (обложки, .cue, .log) не трогаются.
pub const AUDIO_EXTENSIONS: [&str; 18] = [
    "flac", "mp3", "m4a", "m4b", "mp4", "aac", "alac", "opus", "ogg", "oga", "wav", "aif", "aiff",
    "ape", "wv", "mpc", "spx", "wma",
];

/// Названия обложек в папке альбома, по убыванию важности.
pub const COVER_NAMES: [&str; 6] = ["cover", "folder", "front", "album", "albumart", "artwork"];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Теги, как они записаны в файле (пустые — `None`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tags {
    pub title: Option<String>,
    /// Исполнители трека: из многозначного тега или разделённые `;`.
    pub artists: Vec<String>,
    /// Приглашённые (`feat.` в теге исполнителя).
    pub featured: Vec<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track: Option<u32>,
    pub track_total: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<i32>,
    pub date: Option<NaiveDate>,
    pub genres: Vec<String>,
    pub isrc: Option<String>,
    pub compilation: bool,
    pub mb_recording_id: Option<String>,
    pub mb_release_id: Option<String>,
    pub mb_artist_id: Option<String>,
    /// ReplayGain трека, дБ, и пик (доля полной шкалы).
    pub replay_gain_db: Option<f32>,
    pub replay_peak: Option<f32>,
}

/// Файл целиком: формат, свойства, теги, встроенная обложка.
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub tags: Tags,
    pub duration_ms: i32,
    /// Значения `audio_files.codec` и `audio_files.container`.
    pub codec: &'static str,
    pub container: &'static str,
    /// Клиенты играют файл как есть.
    pub direct: bool,
    pub lossless: bool,
    pub bitrate_kbps: Option<i32>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i16>,
    pub bit_depth: Option<i16>,
    /// Для админки: «FLAC 24/96», «MP3 320».
    pub label: String,
    pub picture: Option<Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// Формат, которого сервер не читает (WMA, DSD…).
    #[error("формат не поддерживается: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Broken(String),
}

fn clean(s: &str) -> Option<String> {
    let s = s.trim().trim_matches('\0').trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Первое непустое значение ключа среди тегов файла.
fn first(tags: &[&Tag], key: ItemKey) -> Option<String> {
    tags.iter().find_map(|t| t.get_string(key).and_then(clean))
}

fn all(tags: &[&Tag], key: ItemKey) -> Vec<String> {
    for t in tags {
        let v: Vec<String> = t.get_strings(key).filter_map(clean).collect();
        if !v.is_empty() {
            return v;
        }
    }
    Vec::new()
}

/// «A feat. B» → (A, [B]); «A; B» → (A, B). Запятая и `&` не делят: «Simon & Garfunkel» — один артист.
pub fn split_artists(raw: &[String]) -> (Vec<String>, Vec<String>) {
    let mut main = Vec::new();
    let mut featured = Vec::new();
    let push = |list: &mut Vec<String>, name: &str| {
        if let Some(n) = clean(name)
            && !list.iter().any(|x: &String| x.eq_ignore_ascii_case(&n))
        {
            list.push(n);
        }
    };
    for value in raw {
        for part in value.split([';', '\0']).flat_map(|p| p.split(" / ")) {
            let lower = part.to_lowercase();
            let cut = [" feat. ", " feat ", " ft. ", " featuring ", " при уч. "]
                .iter()
                .filter_map(|m| lower.find(m).map(|i| (i, m.len())))
                .min_by_key(|(i, _)| *i);
            match cut {
                Some((i, len)) if part.is_char_boundary(i) && part.is_char_boundary(i + len) => {
                    push(&mut main, &part[..i]);
                    for f in part[i + len..].split([',', '&']) {
                        push(&mut featured, f);
                    }
                }
                _ => push(&mut main, part),
            }
        }
    }
    featured.retain(|f| !main.iter().any(|m| m.eq_ignore_ascii_case(f)));
    (main, featured)
}

/// «7», «7/12», «07» → (7, Some(12)).
fn number(s: &str) -> (Option<u32>, Option<u32>) {
    let (a, b) = s.split_once('/').unwrap_or((s, ""));
    (a.trim().parse().ok(), b.trim().parse().ok())
}

/// «-6.5 dB» → −6.5.
fn decibels(s: &str) -> Option<f32> {
    s.trim()
        .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c.is_whitespace())
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

fn date_of(s: &str) -> (Option<i32>, Option<NaiveDate>) {
    let s = s.trim();
    let year = s
        .get(..4)
        .and_then(|y| y.parse::<i32>().ok())
        .filter(|y| (1000..3000).contains(y));
    let date = s
        .get(..10)
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    (year, date)
}

pub fn normalize_isrc(s: &str) -> Option<String> {
    let code: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    (code.len() == 12).then_some(code)
}

fn read_tags(tags: &[&Tag]) -> Tags {
    let mut raw_artists = all(tags, ItemKey::TrackArtists);
    if raw_artists.is_empty() {
        raw_artists = all(tags, ItemKey::TrackArtist);
    }
    let (artists, featured) = split_artists(&raw_artists);
    let (track, mut track_total) = first(tags, ItemKey::TrackNumber)
        .map(|s| number(&s))
        .unwrap_or_default();
    if track_total.is_none() {
        track_total = first(tags, ItemKey::TrackTotal).and_then(|s| s.trim().parse().ok());
    }
    let disc = first(tags, ItemKey::DiscNumber).and_then(|s| number(&s).0);
    let (year, date) = first(tags, ItemKey::RecordingDate)
        .or_else(|| first(tags, ItemKey::Year))
        .or_else(|| first(tags, ItemKey::OriginalReleaseDate))
        .map(|s| date_of(&s))
        .unwrap_or_default();
    let mut genres = Vec::new();
    for g in all(tags, ItemKey::Genre) {
        for part in g.split([';', '/', '\0']) {
            if let Some(p) = clean(part)
                && !genres.iter().any(|x: &String| x.eq_ignore_ascii_case(&p))
            {
                genres.push(p);
            }
        }
    }
    let compilation = first(tags, ItemKey::FlagCompilation)
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "True" | "yes"));
    Tags {
        title: first(tags, ItemKey::TrackTitle),
        artists,
        featured,
        album: first(tags, ItemKey::AlbumTitle),
        album_artist: first(tags, ItemKey::AlbumArtist),
        track: track.filter(|n| *n > 0),
        track_total,
        disc: disc.filter(|n| *n > 0),
        year,
        date,
        genres,
        isrc: first(tags, ItemKey::Isrc).and_then(|s| normalize_isrc(&s)),
        compilation,
        mb_recording_id: first(tags, ItemKey::MusicBrainzRecordingId),
        mb_release_id: first(tags, ItemKey::MusicBrainzReleaseId),
        mb_artist_id: first(tags, ItemKey::MusicBrainzArtistId),
        replay_gain_db: first(tags, ItemKey::ReplayGainTrackGain).and_then(|s| decibels(&s)),
        replay_peak: first(tags, ItemKey::ReplayGainTrackPeak)
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|p| p.is_finite() && *p > 0.0),
    }
}

/// Кодек, контейнер, играется ли как есть, без потерь ли.
fn format_of(
    ty: FileType,
    mp4: Option<Mp4Codec>,
) -> Option<(&'static str, &'static str, bool, bool)> {
    Some(match ty {
        FileType::Flac => ("flac", "flac", true, true),
        FileType::Mpeg => ("mp3", "mp3", true, false),
        FileType::Mp4 => match mp4 {
            Some(Mp4Codec::AAC) | None => ("aac", "mp4", true, false),
            Some(Mp4Codec::ALAC) => ("alac", "mp4", false, true),
            Some(Mp4Codec::FLAC) => ("flac", "mp4", true, true),
            Some(Mp4Codec::MP3) => ("mp3", "other", false, false),
            Some(_) => ("other", "mp4", false, false),
        },
        FileType::Vorbis => ("vorbis", "ogg", true, false),
        // Opus в Ogg играют не все (Safari) — как и у своих загрузок, нужна копия
        FileType::Opus => ("opus", "ogg", false, false),
        FileType::Wav => ("pcm", "wav", true, true),
        FileType::Aiff => ("pcm", "other", false, true),
        FileType::Aac => ("aac", "other", false, false),
        FileType::Ape | FileType::WavPack => ("other", "other", false, true),
        FileType::Mpc | FileType::Speex => ("other", "other", false, false),
        _ => return None,
    })
}

fn name_of(codec: &str, ty: FileType) -> String {
    match (codec, ty) {
        ("pcm", FileType::Aiff) => "AIFF".into(),
        ("pcm", _) => "WAV".into(),
        ("other", FileType::Ape) => "APE".into(),
        ("other", FileType::WavPack) => "WavPack".into(),
        ("other", FileType::Mpc) => "Musepack".into(),
        ("other", FileType::Speex) => "Speex".into(),
        (c, _) => c.to_ascii_uppercase(),
    }
}

pub fn read(path: &Path) -> Result<FileInfo, ReadError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tagged = match lofty::read_from_path(path) {
        Ok(t) => t,
        Err(e) if e.is_unknown_format() => {
            return Err(ReadError::Unsupported(ext.to_ascii_uppercase()));
        }
        Err(e) => {
            tracing::debug!(path = %path.display(), error = %e, "music file is unreadable");
            return Err(ReadError::Broken(
                "файл повреждён или это не аудиофайл".into(),
            ));
        }
    };
    let ty = tagged.file_type();
    let mp4 = if ty == FileType::Mp4 {
        File::open(path)
            .ok()
            .and_then(|mut f| Mp4File::read_from(&mut f, ParseOptions::new().read_tags(false)).ok())
            .and_then(|m| m.properties().codec())
    } else {
        None
    };
    let (codec, container, direct, lossless) =
        format_of(ty, mp4).ok_or_else(|| ReadError::Unsupported(ext.to_ascii_uppercase()))?;
    let props = tagged.properties();
    let duration_ms = props.duration().as_millis().min(i32::MAX as u128) as i32;
    if duration_ms < 1000 {
        return Err(ReadError::Broken("слишком короткий файл".into()));
    }
    let mut ordered: Vec<&Tag> = Vec::new();
    if let Some(t) = tagged.primary_tag() {
        ordered.push(t);
    }
    for t in tagged.tags() {
        if !ordered.iter().any(|o| std::ptr::eq(*o, t)) {
            ordered.push(t);
        }
    }
    let tags = read_tags(&ordered);
    let picture = ordered.iter().find_map(|t| {
        let pics = t.pictures();
        pics.iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| pics.first())
            .map(|p| p.data().to_vec())
    });
    let bitrate = props
        .audio_bitrate()
        .or(props.overall_bitrate())
        .map(|b| b as i32);
    let bit_depth = props.bit_depth().filter(|_| lossless).map(i16::from);
    let sample_rate = props.sample_rate().map(|r| r as i32);
    let name = name_of(codec, ty);
    let label = match (lossless, bit_depth, sample_rate, bitrate) {
        (true, Some(b), Some(r), _) => format!("{name} {b}/{}", fmt_rate(r)),
        (false, _, _, Some(k)) => format!("{name} {k}"),
        _ => name,
    };
    Ok(FileInfo {
        tags,
        duration_ms,
        codec,
        container,
        direct,
        lossless,
        bitrate_kbps: bitrate.filter(|b| *b > 0),
        sample_rate,
        channels: props.channels().map(i16::from),
        bit_depth,
        label,
        picture,
    })
}

/// 44100 → «44,1», 96000 → «96».
fn fmt_rate(hz: i32) -> String {
    if hz % 1000 == 0 {
        (hz / 1000).to_string()
    } else {
        format!("{:.1}", f64::from(hz) / 1000.0).replace('.', ",")
    }
}

/// Обложка из папки файла: cover.jpg, folder.jpg и так далее (регистр не важен).
pub fn folder_cover(dir: &Path) -> Option<Vec<u8>> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut found: Vec<(usize, std::path::PathBuf)> = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        let (Some(stem), Some(ext)) = (
            p.file_stem().and_then(|s| s.to_str()),
            p.extension().and_then(|s| s.to_str()),
        ) else {
            continue;
        };
        if !matches!(
            ext.to_ascii_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "webp"
        ) {
            continue;
        }
        let stem = stem.to_ascii_lowercase();
        if let Some(rank) = COVER_NAMES
            .iter()
            .position(|n| stem == *n || stem.starts_with(&format!("{n} ")))
        {
            found.push((rank, p));
        }
    }
    found.sort();
    let (_, path) = found.into_iter().next()?;
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > 20 * 1024 * 1024 {
        return None;
    }
    std::fs::read(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artists_are_split() {
        let (m, f) = split_artists(&["Кино".into()]);
        assert_eq!(m, vec!["Кино"]);
        assert!(f.is_empty());
        let (m, f) = split_artists(&["Oxxxymiron feat. Porchy, ЛСП".into()]);
        assert_eq!(m, vec!["Oxxxymiron"]);
        assert_eq!(f, vec!["Porchy", "ЛСП"]);
        let (m, _) = split_artists(&["Simon & Garfunkel".into()]);
        assert_eq!(m, vec!["Simon & Garfunkel"]);
        let (m, _) = split_artists(&["AC/DC".into()]);
        assert_eq!(m, vec!["AC/DC"]);
        let (m, _) = split_artists(&["Daft Punk; Pharrell Williams".into(), "daft punk".into()]);
        assert_eq!(m, vec!["Daft Punk", "Pharrell Williams"]);
    }

    #[test]
    fn numbers_and_dates() {
        assert_eq!(number("07/12"), (Some(7), Some(12)));
        assert_eq!(number("3"), (Some(3), None));
        assert_eq!(decibels("-6.52 dB"), Some(-6.52));
        assert_eq!(decibels("+1.0 dB"), Some(1.0));
        assert_eq!(date_of("1988"), (Some(1988), None));
        assert_eq!(
            date_of("2019-03-08T00:00:00"),
            (Some(2019), NaiveDate::from_ymd_opt(2019, 3, 8))
        );
        assert_eq!(
            normalize_isrc("ru-a01-88-00001").as_deref(),
            Some("RUA018800001")
        );
        assert_eq!(fmt_rate(44100), "44,1");
        assert_eq!(fmt_rate(96000), "96");
        assert!(is_audio(Path::new("/m/a/01 Песня.FLAC")));
        assert!(!is_audio(Path::new("/m/a/cover.jpg")));
    }
}
