//! Декодирование каждого формата: точная длина, совпадение с эталоном, точность перемотки.

mod common;

use common::*;
use dw_audio::SourceSpec;
use dw_audio::testkit::{decode_all, decode_from};

fn snr_db(got: &[f32], want: &[f32]) -> f64 {
    let n = got.len().min(want.len());
    let (mut s, mut e) = (0.0f64, 0.0f64);
    for i in 0..n {
        s += (want[i] as f64).powi(2);
        e += (got[i] as f64 - want[i] as f64).powi(2);
    }
    10.0 * (s / e.max(1e-30)).log10()
}

/// (формат, допустимый лишний хвост в кадрах, минимальный SNR, дБ)
const EXPECT: [(&str, i64, f64); 8] = [
    ("opus_ogg", 0, 20.0),
    ("opus_mp4", 0, 20.0),
    // WebM: symphonia не применяет DiscardPadding — остаётся хвост последнего кадра Opus (< 960).
    ("opus_webm", 959, 20.0),
    ("flac16_44", 0, 90.0),
    ("flac24_96", 0, 130.0),
    ("mp3_320", 0, 30.0),
    ("aac_256", 0, 30.0),
    // Экспериментальный кодер Vorbis в ffmpeg округляет конечную гранулу вверх до 64 кадров.
    ("vorbis_q6", 63, 15.0),
];

#[test]
fn decodes_every_format_with_exact_length() {
    let mut failures = Vec::new();
    for (name, rate, ext) in FORMATS {
        let b = bounds(rate);
        let reference = reference(rate);
        let (_, tol, min_snr) = EXPECT.iter().find(|e| e.0 == name).copied().unwrap();
        for part in 1..=4usize {
            let d = decode_all(&SourceSpec::File(part_path(name, ext, part))).unwrap();
            assert_eq!(d.rate, rate, "{name} part{part}: rate");
            let want = b[part] - b[part - 1];
            let got = d.samples.len() / 2;
            let diff = got as i64 - want as i64;
            let reference_part = &reference[b[part - 1] * 2..b[part] * 2];
            let snr = snr_db(&d.samples, reference_part);
            println!(
                "{name:10} part{part}: {}/{} frames {got} (want {want}, diff {diff:+}) SNR {snr:.1} dB",
                d.container, d.codec
            );
            if diff < 0 || diff > tol || snr < min_snr {
                failures.push(format!(
                    "{name} part{part}: diff {diff} (expected {tol}), snr {snr:.1} (min {min_snr})"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn seek_is_sample_accurate() {
    // Перемотка к произвольному кадру даёт те же сэмплы, что и чтение подряд.
    for (name, rate, ext) in FORMATS {
        if name == "opus_webm" {
            continue; // метки в миллисекундах — точнее кадра не будет
        }
        let path = part_path(name, ext, 2);
        let full = decode_all(&SourceSpec::File(path.clone())).unwrap().samples;
        let frame = (rate as u64 * 3) + 4321;
        let tail = decode_from(&SourceSpec::File(path), frame).unwrap();
        let want = &full[frame as usize * 2..];
        assert_eq!(tail.len(), want.len(), "{name}: length after seek");
        // Lossless — бит в бит; lossy — после прогрева декодера отличия на уровне шума.
        let max_err = tail
            .iter()
            .zip(want)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        println!("{name:10} seek to {frame}: max diff {max_err:.2e}");
        // Opus после перемотки сходится не бит в бит (состояние CELT), с прогревом 200 мс — ниже −60 дБ.
        let lim = if name.starts_with("opus") { 2e-3 } else { 1e-6 };
        assert!(max_err <= lim, "{name}: max diff after seek {max_err}");
    }
}
