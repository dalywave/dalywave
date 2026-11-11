//! Минимальный разбор MP4 ради точной обрезки (gapless).
//!
//! symphonia-format-isomp4 0.6.1 не применяет списки правок (`elst`): у AAC остаются 1024
//! «разгонных» сэмпла и хвост кадра, у Opus — хвост. Читаем `moov` сами: `elst` даёт сдвиг
//! начала (media_time), а точная длина — `mdhd.duration − media_time` (ffmpeg укорачивает
//! длительность последнего сэмпла в `stts`, а `elst.segment_duration` округлён до тайм-скейла
//! ролика, обычно до миллисекунды).

use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone, Default)]
pub(crate) struct Mp4Audio {
    pub movie_timescale: u32,
    pub media_timescale: u32,
    pub media_duration: u64,
    /// Первая непустая правка: (segment_duration в тайм-скейле ролика, media_time в тайм-скейле дорожки).
    pub edit: Option<(u64, i64)>,
    /// Содержимое `dOps` (Opus в MP4), поля в big-endian.
    pub dops: Option<Vec<u8>>,
}

impl Mp4Audio {
    /// Сколько кадров отбросить в начале и сколько полезных кадров, в кадрах частоты `rate`.
    pub fn trim(&self, rate: u32) -> (u64, Option<u64>) {
        if self.media_timescale == 0 {
            return (0, None);
        }
        let to_frames = |v: u64, ts: u32| -> u64 { (v as u128 * rate as u128 / ts as u128) as u64 };
        let Some((seg, media_time)) = self.edit else {
            return (
                0,
                Some(to_frames(self.media_duration, self.media_timescale)),
            );
        };
        let skip_ts = media_time.max(0) as u64;
        let skip = to_frames(skip_ts, self.media_timescale);
        let by_mdhd = to_frames(
            self.media_duration.saturating_sub(skip_ts),
            self.media_timescale,
        );
        if self.movie_timescale == 0 || seg == 0 {
            return (skip, Some(by_mdhd));
        }
        let by_elst = to_frames(seg, self.movie_timescale);
        // elst округлён до тика ролика: если расхождение в пределах тика — верим mdhd.
        let tick = (rate / self.movie_timescale).max(1) as u64 + 1;
        let valid = if by_mdhd.abs_diff(by_elst) <= tick {
            by_mdhd
        } else {
            by_elst.min(by_mdhd)
        };
        (skip, Some(valid))
    }
}

fn read_u32(r: &mut impl Read) -> std::io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_be_bytes(b))
}

fn read_u64(r: &mut impl Read) -> std::io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_be_bytes(b))
}

struct BoxHeader {
    kind: [u8; 4],
    body_start: u64,
    end: u64,
}

fn read_header<R: Read + Seek>(r: &mut R, limit: u64) -> std::io::Result<Option<BoxHeader>> {
    let start = r.stream_position()?;
    if start + 8 > limit {
        return Ok(None);
    }
    let size32 = read_u32(r)?;
    let mut kind = [0u8; 4];
    r.read_exact(&mut kind)?;
    let (size, hdr) = match size32 {
        1 => (read_u64(r)?, 16),
        0 => (limit - start, 8),
        n => (n as u64, 8),
    };
    if size < hdr {
        return Ok(None);
    }
    Ok(Some(BoxHeader {
        kind,
        body_start: start + hdr,
        end: (start + size).min(limit),
    }))
}

/// Читает аудиодорожку из MP4. Позиция источника после вызова не определена.
pub(crate) fn parse<R: Read + Seek>(r: &mut R, len: u64) -> std::io::Result<Option<Mp4Audio>> {
    r.seek(SeekFrom::Start(0))?;
    let mut movie_timescale = 0;
    while let Some(h) = read_header(r, len)? {
        if &h.kind == b"moov" {
            let mut found = None;
            let mut pos = h.body_start;
            while pos < h.end {
                r.seek(SeekFrom::Start(pos))?;
                let Some(c) = read_header(r, h.end)? else {
                    break;
                };
                match &c.kind {
                    b"mvhd" => {
                        let mut vf = [0u8; 4];
                        r.read_exact(&mut vf)?;
                        if vf[0] == 1 {
                            r.seek(SeekFrom::Current(16))?;
                        } else {
                            r.seek(SeekFrom::Current(8))?;
                        }
                        movie_timescale = read_u32(r)?;
                    }
                    b"trak" if found.is_none() => {
                        if let Some(t) = parse_trak(r, c.body_start, c.end)? {
                            found = Some(t);
                        }
                    }
                    _ => {}
                }
                pos = c.end;
            }
            return Ok(found.map(|mut t: Mp4Audio| {
                t.movie_timescale = movie_timescale;
                t
            }));
        }
        r.seek(SeekFrom::Start(h.end))?;
    }
    Ok(None)
}

fn parse_trak<R: Read + Seek>(
    r: &mut R,
    start: u64,
    end: u64,
) -> std::io::Result<Option<Mp4Audio>> {
    let mut out = Mp4Audio::default();
    let mut is_audio = false;
    walk(r, start, end, &mut |r, kind, body, bend| {
        match kind {
            b"elst" => {
                r.seek(SeekFrom::Start(body))?;
                let mut vf = [0u8; 4];
                r.read_exact(&mut vf)?;
                let n = read_u32(r)?;
                for _ in 0..n {
                    let (seg, mt) = if vf[0] == 1 {
                        (read_u64(r)?, read_u64(r)? as i64)
                    } else {
                        (read_u32(r)? as u64, read_u32(r)? as i32 as i64)
                    };
                    let _rate = read_u32(r)?;
                    // Пустая правка (media_time = −1) — задержка показа, не обрезка.
                    if mt >= 0 && out.edit.is_none() {
                        out.edit = Some((seg, mt));
                    }
                }
            }
            b"mdhd" => {
                r.seek(SeekFrom::Start(body))?;
                let mut vf = [0u8; 4];
                r.read_exact(&mut vf)?;
                if vf[0] == 1 {
                    r.seek(SeekFrom::Current(16))?;
                    out.media_timescale = read_u32(r)?;
                    out.media_duration = read_u64(r)?;
                } else {
                    r.seek(SeekFrom::Current(8))?;
                    out.media_timescale = read_u32(r)?;
                    out.media_duration = read_u32(r)? as u64;
                }
            }
            b"hdlr" => {
                r.seek(SeekFrom::Start(body + 8))?;
                let mut t = [0u8; 4];
                r.read_exact(&mut t)?;
                is_audio = &t == b"soun";
            }
            b"stsd" => {
                // Ищем dOps внутри описания сэмпла: таблица маленькая, читаем целиком.
                let n = (bend - body).min(64 * 1024) as usize;
                let mut buf = vec![0u8; n];
                r.seek(SeekFrom::Start(body))?;
                r.read_exact(&mut buf)?;
                if let Some(i) = buf.windows(4).position(|w| w == b"dOps")
                    && i >= 4
                {
                    let sz = u32::from_be_bytes([buf[i - 4], buf[i - 3], buf[i - 2], buf[i - 1]])
                        as usize;
                    if sz >= 8 && i - 4 + sz <= buf.len() {
                        out.dops = Some(buf[i + 4..i - 4 + sz].to_vec());
                    }
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(is_audio.then_some(out))
}

type Visit<'a, R> = dyn FnMut(&mut R, &[u8; 4], u64, u64) -> std::io::Result<()> + 'a;

fn walk<R: Read + Seek>(
    r: &mut R,
    start: u64,
    end: u64,
    f: &mut Visit<'_, R>,
) -> std::io::Result<()> {
    let mut pos = start;
    while pos + 8 <= end {
        r.seek(SeekFrom::Start(pos))?;
        let Some(h) = read_header(r, end)? else { break };
        match &h.kind {
            b"mdia" | b"minf" | b"stbl" | b"edts" => walk(r, h.body_start, h.end, f)?,
            k => f(r, k, h.body_start, h.end)?,
        }
        pos = h.end;
    }
    Ok(())
}

/// OpusHead (little-endian, как в Ogg) из `dOps` (big-endian) с нулевым pre-skip:
/// адаптер libopus читает pre-skip из extra_data как LE и сам обрезает начало, а обрезку
/// мы делаем по `elst`. Без этого 312 читается как 14337 и теряется кадр звука.
pub(crate) fn opus_head_from_dops(dops: &[u8]) -> Option<Box<[u8]>> {
    if dops.len() < 11 {
        return None;
    }
    let channels = dops[1];
    let rate = u32::from_be_bytes([dops[4], dops[5], dops[6], dops[7]]);
    let gain = i16::from_be_bytes([dops[8], dops[9]]);
    let family = dops[10];
    let mut head = Vec::with_capacity(19 + dops.len().saturating_sub(11));
    head.extend_from_slice(b"OpusHead");
    head.push(1);
    head.push(channels);
    head.extend_from_slice(&0u16.to_le_bytes());
    head.extend_from_slice(&rate.to_le_bytes());
    head.extend_from_slice(&gain.to_le_bytes());
    head.push(family);
    head.extend_from_slice(&dops[11..]);
    Some(head.into_boxed_slice())
}

/// Обнуляет pre-skip в OpusHead из Ogg: обрезку начала делаем сами по меткам времени,
/// иначе после перемотки на свежем декодере адаптер срезал бы лишние 312 сэмплов.
pub(crate) fn opus_head_without_preskip(head: &[u8]) -> Option<(Box<[u8]>, u16)> {
    if head.len() < 19 || &head[..8] != b"OpusHead" {
        return None;
    }
    let pre_skip = u16::from_le_bytes([head[10], head[11]]);
    let mut out = head.to_vec();
    out[10] = 0;
    out[11] = 0;
    Some((out.into_boxed_slice(), pre_skip))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_prefers_exact_mdhd_when_elst_is_rounded() {
        let a = Mp4Audio {
            movie_timescale: 1000,
            media_timescale: 48000,
            media_duration: 287_651,
            edit: Some((5986, 312)),
            dops: None,
        };
        assert_eq!(a.trim(48000), (312, Some(287_339)));
    }

    #[test]
    fn dops_is_converted_to_le_opus_head() {
        let dops = [
            0x00, 0x02, 0x01, 0x38, 0x00, 0x00, 0xbb, 0x80, 0x00, 0x00, 0x00,
        ];
        let head = opus_head_from_dops(&dops).unwrap();
        assert_eq!(&head[..8], b"OpusHead");
        assert_eq!(head[9], 2);
        assert_eq!(u16::from_le_bytes([head[10], head[11]]), 0);
        assert_eq!(
            u32::from_le_bytes([head[12], head[13], head[14], head[15]]),
            48000
        );
    }
}
