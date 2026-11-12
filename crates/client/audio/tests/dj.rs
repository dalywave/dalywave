//! «Диджей» на синтетике: два «бита» с бочкой на каждой доле в разных темпах (124 и 120 BPM).
//! Движок сам выбирает переход, растягивает входящий и стартует его на сильной доле; дорожки
//! уходящего и входящего рендерятся по отдельности, атаки на сильных долях сверяются — расхождение
//! не больше 10 мс (на слух флэм начинается примерно с 10–20 мс).

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::write_wav;
use dalywave_audio::testkit::{downbeat_alignment, render_dj};
use dalywave_audio::{Analysis, TrackSpec};

const RATE: u32 = 48_000;

/// Бочка на каждой доле (сильная — громче), тихий хэт между долями. `first` — первая доля, с.
fn beat_track(bpm: f64, secs: f64, first: f64) -> (Vec<f32>, Analysis) {
    let n = (secs * RATE as f64) as usize;
    let mut v = vec![0.0f32; n * 2];
    let step = 60.0 / bpm;
    let mut beats = Vec::new();
    let mut k = 0usize;
    loop {
        let t0 = first + k as f64 * step;
        if t0 + 0.2 > secs {
            break;
        }
        beats.push(t0 * 1000.0);
        let accent = if k.is_multiple_of(4) { 0.8 } else { 0.55 };
        let start = (t0 * RATE as f64) as usize;
        for i in 0..(0.18 * RATE as f64) as usize {
            let t = i as f64 / RATE as f64;
            let kick = (std::f64::consts::TAU * (55.0 + 90.0 * (-t / 0.03).exp()) * t).sin()
                * (-t / 0.06).exp()
                * accent;
            v[(start + i) * 2] += kick as f32;
            v[(start + i) * 2 + 1] += kick as f32;
        }
        let hat = ((t0 + step / 2.0) * RATE as f64) as usize;
        for i in 0..(0.03 * RATE as f64) as usize {
            let noise = (((hat + i) as f64 * 12.9898).sin() * 43758.5453).fract() * 2.0 - 1.0;
            let x = (noise * 0.08 * (-(i as f64) / 400.0).exp()) as f32;
            if (hat + i) < n {
                v[(hat + i) * 2] += x;
                v[(hat + i) * 2 + 1] -= x;
            }
        }
        k += 1;
    }
    let analysis = Analysis {
        duration_ms: Some(secs * 1000.0),
        bpm: Some(bpm),
        bpm_confidence: Some(0.95),
        downbeats_ms: beats.iter().step_by(4).copied().collect(),
        beats_ms: beats,
        energy_curve: vec![600.0; secs.ceil() as usize],
        key_camelot: Some("8A".into()),
        ..Default::default()
    };
    (v, analysis)
}

fn track(name: &str, bpm: f64, secs: f64, first: f64) -> TrackSpec {
    let (samples, analysis) = beat_track(bpm, secs, first);
    let dir = std::env::temp_dir().join("dw-audio-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("dj_{name}.wav"));
    write_wav(&path, RATE, &samples);
    let mut t = TrackSpec::file(name, path);
    t.analysis = Some(Arc::new(analysis));
    t
}

#[test]
fn dj_blend_lands_downbeats_together() {
    let a = track("a", 124.0, 110.0, 0.1);
    let b = track("b", 120.0, 100.0, 0.25);
    // Переход начнётся не раньше чем через 12 с от старта: начинаем за 20 с до первой подходящей
    // границы фразы во второй половине трека
    let from_ms = 40_000;
    let r = render_dj(&a, &b, from_ms, RATE, 20, true, Duration::from_secs(120)).unwrap();
    let p = &r.plan;
    assert!(p.technique == "blend" || p.technique == "filter", "{p:?}");
    // Где по анализу и карте темпа должны звучать доли обеих дорожек — время в рендере
    let from = from_ms as f64;
    let out_beats: Vec<f64> = a
        .analysis
        .as_ref()
        .unwrap()
        .beats_ms
        .iter()
        .map(|b| b - from)
        .collect();
    let in_start = p.start_ms + p.in_offset_ms - from;
    let in_beats: Vec<f64> = b
        .analysis
        .as_ref()
        .unwrap()
        .beats_ms
        .iter()
        .map(|s| in_start + p.tempo.out_at(*s))
        .collect();
    let report = downbeat_alignment(
        &r.outgoing,
        &r.incoming,
        RATE,
        &out_beats,
        &in_beats,
        in_start..p.start_ms + p.length_ms - from,
        10.0,
    );
    assert!(report.checked >= 3, "{}", report.summary());
    assert_eq!(
        report.within,
        report.checked,
        "{} {:?}",
        report.summary(),
        report.offsets
    );
    assert!(report.median_ms <= 3.0, "{}", report.summary());
    // После перехода звучит только входящий, без щелчков и перегруза
    let peak = r.mix.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak <= 0.95, "пик {peak}");
}

#[test]
fn dj_far_tempos_go_out_on_a_downbeat() {
    // Темпы расходятся на 23 % — по долям не свести, но и не «Умный» на затухании: эхо-выход или
    // резкая смена на сильной доле уходящего, входящий — со своей сильной доли
    let a = track("c", 124.0, 60.0, 0.1);
    let b = track("d", 96.0, 40.0, 0.1);
    let r = render_dj(&a, &b, 30_000, RATE, 50, false, Duration::from_secs(120)).unwrap();
    let p = &r.plan;
    assert!(p.technique == "echo" || p.technique == "cut", "{p:?}");
    assert!(p.note.contains("расходятся"), "{}", p.note);
    assert!(p.tempo.is_identity());
    let cut = p.start_ms + p.in_offset_ms;
    let bar = 4.0 * 60_000.0 / 124.0;
    let bars = ((cut - 100.0) / bar).round();
    assert!(
        (100.0 + bars * bar - cut).abs() < 1.0,
        "срез {cut} не на сильной доле"
    );
    assert!((p.in_start_ms - 100.0).abs() < 1.0, "{p:?}");
    // До среза — уходящий, после — весь входящий с его первой доли
    let total = r.mix.len() as f64 / 2.0 / RATE as f64;
    let expected = (cut - 30_000.0) / 1000.0 + (40_000.0 - p.in_start_ms) / 1000.0;
    assert!(
        (total - expected).abs() < 1.0,
        "длина рендера {total:.2} с, ждали {expected:.2}"
    );
}
