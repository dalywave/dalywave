//! Декодирование одного трека: symphonia + libopus, точная обрезка начала и конца,
//! перемотка с точностью до сэмпла. На выходе — стерео f32 с частотой источника.
//!
//! Позиция ведётся по меткам времени пакетов: начало буфера после декодера = `pts + trim_start`
//! (обрезку из контейнера применяет сам декодер). Кадр «полезной» шкалы f соответствует метке
//! `f + ts_offset`, где ts_offset — задержка кодека, которую контейнер не вычитает сам:
//! pre-skip у Opus в Ogg, `elst.media_time` в MP4. Всё, что левее нуля или правее длины трека,
//! отбрасывается здесь, поэтому начало и конец точны независимо от того, как их отдал демуксер.

use std::sync::LazyLock;

use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{TimeBase, Timestamp};

use crate::{AudioError, mp4};

static CODECS: LazyLock<CodecRegistry> = LazyLock::new(|| {
    let mut reg = CodecRegistry::new();
    symphonia::default::register_enabled_codecs(&mut reg);
    reg.register_audio_decoder::<symphonia_adapter_libopus::OpusDecoder>();
    reg
});

/// Сколько кадров декодировать до точки перемотки, чтобы декодер «прогрелся» (MDCT, Opus).
fn preroll_frames(codec: &str, rate: u32) -> u64 {
    match codec {
        // 80 мс по рекомендации libopus мало: −28 дБ ошибки сразу после точки; 200 мс дают −64 дБ.
        "opus" => 9600,
        "flac" | "pcm" => 0,
        _ => (rate as u64) / 20,
    }
}

pub(crate) struct TrackDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    /// Метка первого полезного кадра (в кадрах источника).
    ts_offset: i64,
    pub source_rate: u32,
    pub source_channels: usize,
    pub codec: String,
    pub container: &'static str,
    /// Полезная длина трека в кадрах источника, если известна заранее.
    pub total_frames: Option<u64>,
    /// Обрезать конец сами (MP4: декодер отдаёт последний кадр целиком).
    hard_end: Option<u64>,
    /// Кадры с индексом меньше этого отбрасываются (начало трека или точка перемотки).
    skip_until: i64,
    /// Позиция следующего выдаваемого кадра на полезной шкале.
    pub position: u64,
    /// Шкала по меткам ненадёжна (WebM: метки в мс) — считаем кадры подряд.
    sequential: bool,
    seq_pos: i64,
    scratch: Vec<f32>,
    pub decode_errors: u32,
}

fn hint_for(ext: Option<&str>) -> Hint {
    let mut h = Hint::new();
    if let Some(e) = ext {
        h.with_extension(e);
    }
    h
}

fn is_mp4_ext(ext: Option<&str>) -> bool {
    matches!(ext, Some("mp4" | "m4a" | "m4b" | "mov" | "3gp"))
}

impl TrackDecoder {
    pub fn open(mut source: Box<dyn MediaSource>, ext: Option<&str>) -> Result<Self, AudioError> {
        // MP4: правки читаем до symphonia (она их не применяет).
        let mut mp4_info = None;
        if is_mp4_ext(ext) {
            if let Some(len) = source.byte_len() {
                mp4_info =
                    mp4::parse(&mut source, len).map_err(|e| AudioError::Io(e.to_string()))?;
            }
            source
                .seek(std::io::SeekFrom::Start(0))
                .map_err(|e| AudioError::Io(e.to_string()))?;
        }

        let mss = MediaSourceStream::new(source, MediaSourceStreamOptions::default());
        let format = symphonia::default::get_probe()
            .probe(
                &hint_for(ext),
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| AudioError::Format(e.to_string()))?;
        let container = format.format_info().short_name;
        let track = format
            .default_track(TrackType::Audio)
            .or_else(|| format.first_track_known_codec(TrackType::Audio))
            .ok_or_else(|| AudioError::Format("нет аудиодорожки".into()))?
            .clone();
        let Some(CodecParameters::Audio(params)) = &track.codec_params else {
            return Err(AudioError::Format("нет параметров кодека".into()));
        };
        let mut params: AudioCodecParameters = params.clone();
        let source_rate = params
            .sample_rate
            .ok_or_else(|| AudioError::Format("неизвестна частота".into()))?;
        let source_channels = params
            .channels
            .as_ref()
            .map(|c| c.count())
            .unwrap_or(2)
            .max(1);

        let mut ts_offset: i64 = 0;
        let mut total_frames = track.num_frames;
        let mut hard_end = None;
        let mut sequential = false;
        let is_opus = params.codec == CODEC_ID_OPUS;

        if let Some(info) = &mp4_info {
            if is_opus && let Some(head) = info.dops.as_deref().and_then(mp4::opus_head_from_dops) {
                params.extra_data = Some(head);
            }
            let (skip, valid) = info.trim(source_rate);
            ts_offset = skip as i64;
            total_frames = valid;
            hard_end = valid;
        } else if is_opus {
            // Ogg/WebM: pre-skip вычитаем сами по меткам, адаптеру отдаём нулевой.
            if let Some((head, pre_skip)) = params
                .extra_data
                .as_deref()
                .and_then(mp4::opus_head_without_preskip)
            {
                params.extra_data = Some(head);
                if container == "ogg" {
                    ts_offset = pre_skip as i64;
                    // В Ogg num_frames включает pre-skip, но не хвост.
                    total_frames = track.num_frames.map(|n| n.saturating_sub(pre_skip as u64));
                } else {
                    // WebM: метки в миллисекундах; pre-skip отрежем от потока подряд.
                    sequential = true;
                    ts_offset = pre_skip as i64;
                    total_frames = None;
                }
            }
        }
        // Метки должны быть в кадрах источника, иначе ведём позицию подряд.
        let time_base = track.time_base;
        if let Some(tb) = time_base
            && !(tb.numer.get() == 1 && tb.denom.get() == source_rate)
        {
            sequential = true;
        }

        let factory = CODECS
            .get_audio_decoder(params.codec)
            .ok_or_else(|| AudioError::Codec(format!("{:?}", params.codec)))?
            .factory;
        let decoder = factory(&params, &AudioDecoderOptions::default())
            .map_err(|e| AudioError::Codec(e.to_string()))?;
        let codec = decoder.codec_info().short_name.to_string();

        Ok(Self {
            format,
            decoder,
            track_id: track.id,
            time_base,
            ts_offset,
            source_rate,
            source_channels,
            codec,
            container,
            total_frames,
            hard_end,
            skip_until: 0,
            position: 0,
            sequential,
            seq_pos: -ts_offset,
            scratch: Vec::with_capacity(8192),
            decode_errors: 0,
        })
    }

    pub fn describe(&self) -> String {
        format!(
            "{}/{} {} Гц {} кан., длина {}",
            self.container,
            self.codec,
            self.source_rate,
            self.source_channels,
            self.total_frames
                .map(|f| format!("{:.3} с", f as f64 / self.source_rate as f64))
                .unwrap_or_else(|| "?".into())
        )
    }

    /// Перемотка к кадру полезной шкалы: грубый seek на `preroll` раньше, дальше лишнее
    /// отбрасывается по меткам.
    pub fn seek_frame(&mut self, frame: u64) -> Result<(), AudioError> {
        let preroll = preroll_frames(&self.codec, self.source_rate);
        let target_ts = (frame as i64 + self.ts_offset - preroll as i64).max(0);
        let to = if self.sequential {
            let tb = self
                .time_base
                .unwrap_or_else(|| TimeBase::try_from_recip(self.source_rate).expect("rate"));
            let secs = target_ts as f64 / self.source_rate as f64;
            SeekTo::Timestamp {
                ts: Timestamp::new((secs * tb.denom.get() as f64 / tb.numer.get() as f64) as i64),
                track_id: self.track_id,
            }
        } else {
            SeekTo::Timestamp {
                ts: Timestamp::new(target_ts),
                track_id: self.track_id,
            }
        };
        match self.format.seek(SeekMode::Accurate, to) {
            Ok(seeked) => {
                self.decoder.reset();
                if self.sequential {
                    // В WebM точнее не получится: позиция = фактическая метка.
                    let tb = self.time_base.unwrap_or_else(|| {
                        TimeBase::try_from_recip(self.source_rate).expect("rate")
                    });
                    let secs = seeked.actual_ts.get() as f64 * tb.numer.get() as f64
                        / tb.denom.get() as f64;
                    self.seq_pos = (secs * self.source_rate as f64) as i64 - self.ts_offset;
                }
                self.skip_until = frame as i64;
                self.position = frame;
                Ok(())
            }
            Err(e) => Err(AudioError::Decode(format!("seek: {e}"))),
        }
    }

    /// Декодирует следующий пакет и дописывает стерео-кадры в `out`.
    /// Возвращает число кадров (0 — пакет целиком отброшен) или None в конце трека.
    pub fn read(&mut self, out: &mut Vec<f32>) -> Result<Option<usize>, AudioError> {
        loop {
            if let Some(end) = self.hard_end
                && self.position >= end
            {
                return Ok(None);
            }
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return Ok(None),
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(SymError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let pts = packet.pts.get() + packet.trim_start.get() as i64;
            let buf = match self.decoder.decode(&packet) {
                Ok(b) => b,
                Err(SymError::DecodeError(e)) => {
                    // Битый пакет: пропускаем, как делают все плееры.
                    self.decode_errors += 1;
                    log::warn!("decode error: {e}");
                    continue;
                }
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            };
            let frames = buf.frames();
            if frames == 0 {
                continue;
            }
            // Индекс первого кадра буфера на полезной шкале.
            let first = if self.sequential {
                let f = self.seq_pos;
                self.seq_pos += frames as i64;
                f
            } else {
                pts - self.ts_offset
            };
            let ch = buf.spec().channels().count().max(1);
            self.scratch.clear();
            buf.copy_to_vec_interleaved::<f32>(&mut self.scratch);

            let mut lo = 0usize;
            if first < self.skip_until {
                lo = ((self.skip_until - first) as usize).min(frames);
            }
            let mut hi = frames;
            if let Some(end) = self.hard_end {
                let end_rel = end as i64 - first;
                if end_rel < hi as i64 {
                    hi = end_rel.max(lo as i64) as usize;
                }
            }
            if lo >= hi {
                continue;
            }
            let n = hi - lo;
            out.reserve(n * 2);
            match ch {
                1 => {
                    for &s in &self.scratch[lo..hi] {
                        out.push(s);
                        out.push(s);
                    }
                }
                2 => out.extend_from_slice(&self.scratch[lo * 2..hi * 2]),
                _ => {
                    // Многоканальное: берём фронтальную пару (для спайка достаточно).
                    for f in lo..hi {
                        out.push(self.scratch[f * ch]);
                        out.push(self.scratch[f * ch + 1]);
                    }
                }
            }
            self.position = (first + hi as i64) as u64;
            self.skip_until = self.skip_until.max(first + hi as i64);
            return Ok(Some(n));
        }
    }
}
