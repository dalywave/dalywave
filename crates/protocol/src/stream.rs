//! Поток трека: подписанная ссылка на аудио или состояние скачивания (`GET /api/tracks/{id}/stream`).
//!
//! Ссылку сервер выдаёт на каждое воспроизведение; байты плеер качает напрямую из хранилища.
//! Трека ещё нет на сервере — скачивание ставится вне очереди, а ответ говорит, чего ждать.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Кодек аудиофайла. Хранится в `audio_files.codec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudioCodec {
    Opus,
    Vorbis,
    Aac,
    Mp3,
    Flac,
    Alac,
    Pcm,
}

names!(AudioCodec {
    Opus => "opus",
    Vorbis => "vorbis",
    Aac => "aac",
    Mp3 => "mp3",
    Flac => "flac",
    Alac => "alac",
    Pcm => "pcm",
});

/// Контейнер аудиофайла. Хранится в `audio_files.container`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudioContainer {
    Ogg,
    Webm,
    /// ISO BMFF (`.m4a`): Opus и AAC с YouTube приходят в нём.
    Mp4,
    Mp3,
    Flac,
    Wav,
}

names!(AudioContainer {
    Ogg => "ogg",
    Webm => "webm",
    Mp4 => "mp4",
    Mp3 => "mp3",
    Flac => "flac",
    Wav => "wav",
});

impl AudioContainer {
    /// MIME-тип для плеера, с кодеком: `audio/mp4; codecs="opus"`.
    pub fn mime(self, codec: AudioCodec) -> String {
        let codecs = match codec {
            AudioCodec::Opus => Some("opus"),
            AudioCodec::Vorbis => Some("vorbis"),
            AudioCodec::Aac => Some("mp4a.40.2"),
            AudioCodec::Flac => Some("flac"),
            AudioCodec::Alac => Some("alac"),
            AudioCodec::Mp3 | AudioCodec::Pcm => None,
        };
        let base = match self {
            Self::Ogg => "audio/ogg",
            Self::Webm => "audio/webm",
            Self::Mp4 => "audio/mp4",
            Self::Mp3 => return "audio/mpeg".into(),
            Self::Flac => return "audio/flac".into(),
            Self::Wav => return "audio/wav".into(),
        };
        match codecs {
            Some(c) => format!("{base}; codecs=\"{c}\""),
            None => base.into(),
        }
    }
}

/// Аудио готово: ссылка на файл и всё, что нужно плееру без лишних запросов.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamReady {
    /// Подписанная ссылка на файл в хранилище.
    pub url: String,
    /// Ссылка перестаёт работать в это время (через 6 часов): потом — запросить поток снова.
    pub expires_at: DateTime<Utc>,
    pub file_id: Uuid,
    pub codec: AudioCodec,
    pub container: AudioContainer,
    /// Для `<audio>` и `MediaSource`: `audio/mp4; codecs="opus"`.
    pub mime_type: String,
    #[schema(required = true)]
    pub bitrate_kbps: Option<i32>,
    pub duration_ms: i32,
    /// Интегральная громкость (EBU R128) — для выравнивания.
    #[schema(required = true)]
    pub loudness_lufs: Option<f32>,
    #[schema(required = true)]
    pub true_peak_db: Option<f32>,
    /// Огибающая для полосы прокрутки: 512 точек, 0..1000.
    #[schema(required = true)]
    pub waveform: Option<Vec<i16>>,
}

/// Аудио ещё нет: скачивание стоит в очереди или идёт.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamPending {
    /// Следить за ним — `GET /api/acquisitions/{id}` или событие `acquisition.updated`.
    pub acquisition_id: Uuid,
    /// Сколько скачиваний впереди; 0 — следующее или уже идёт.
    pub queue_position: i64,
    /// Загрузчики на связи.
    pub downloaders_online: i64,
}

/// Ответ `GET /api/tracks/{id}/stream`; поле `status` говорит, какой.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TrackStream {
    /// 200: можно играть.
    Ready(StreamReady),
    /// 202: скачивается или скоро начнётся.
    Acquiring(StreamPending),
    /// 202: в очереди, но качать некому — загрузчик на Mac не на связи, серверный fetcher выключен.
    WaitingForDownloader(StreamPending),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_types() {
        assert_eq!(
            AudioContainer::Mp4.mime(AudioCodec::Opus),
            "audio/mp4; codecs=\"opus\""
        );
        assert_eq!(
            AudioContainer::Mp4.mime(AudioCodec::Aac),
            "audio/mp4; codecs=\"mp4a.40.2\""
        );
        assert_eq!(
            AudioContainer::Ogg.mime(AudioCodec::Opus),
            "audio/ogg; codecs=\"opus\""
        );
        assert_eq!(AudioContainer::Mp3.mime(AudioCodec::Mp3), "audio/mpeg");
        for c in AudioCodec::ALL {
            assert_eq!(c.as_str().parse::<AudioCodec>().unwrap(), *c);
        }
        for c in AudioContainer::ALL {
            assert_eq!(serde_json::to_value(c).unwrap(), c.as_str());
        }
    }

    #[test]
    fn stream_is_tagged_by_status() {
        let pending = StreamPending {
            acquisition_id: Uuid::nil(),
            queue_position: 2,
            downloaders_online: 0,
        };
        let json = serde_json::to_value(TrackStream::WaitingForDownloader(pending)).unwrap();
        assert_eq!(json["status"], "waiting_for_downloader");
        assert_eq!(json["queuePosition"], 2);
        assert_eq!(json["downloadersOnline"], 0);
    }
}
