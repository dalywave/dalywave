//! Микшер на синтетике (WAV с постоянным уровнем): точная склейка, равномощный кроссфейд,
//! нормализация громкости.

mod common;

use std::time::Duration;

use common::*;
use dw_audio::testkit::render_queue;
use dw_audio::{Engine, EngineConfig, Event, OutputConfig, TrackSpec, Transition, equal_power};

const RATE: u32 = 48000;

fn dc(frames: usize, level: f32) -> Vec<f32> {
    vec![level; frames * 2]
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("dw-audio-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn gapless_join_of_constant_levels_is_exact() {
    let (na, nb) = (48_001usize, 30_007usize);
    let a = tmp("gl_a.wav");
    let b = tmp("gl_b.wav");
    write_wav(&a, RATE, &dc(na, 0.5));
    write_wav(&b, RATE, &dc(nb, -0.25));
    let res = render_queue(
        vec![TrackSpec::file("a", &a), TrackSpec::file("b", &b)],
        RATE,
        Transition::Gapless,
        Duration::from_secs(20),
    )
    .unwrap();
    let out: Vec<f32> = res
        .samples
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| p[0])
        .collect();
    assert!(out.len() >= na + nb, "len {}", out.len());
    assert!(out[..na].iter().all(|&x| x == 0.5), "A must be untouched");
    assert!(
        out[na..na + nb].iter().all(|&x| x == -0.25),
        "B must start exactly at frame {na}"
    );
    assert!(out[na + nb..].iter().all(|&x| x == 0.0));
}

#[test]
fn crossfade_follows_equal_power_curve() {
    let (na, nb, xf_ms) = (RATE as usize * 3, RATE as usize * 3, 1000u32);
    let xf = (RATE as u64 * xf_ms as u64 / 1000) as usize;
    let a = tmp("xf_a.wav");
    let b = tmp("xf_b.wav");
    write_wav(&a, RATE, &dc(na, 0.5));
    write_wav(&b, RATE, &dc(nb, -0.25));
    let res = render_queue(
        vec![TrackSpec::file("a", &a), TrackSpec::file("b", &b)],
        RATE,
        Transition::Crossfade { ms: xf_ms },
        Duration::from_secs(20),
    )
    .unwrap();
    let out: Vec<f32> = res
        .samples
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| p[0])
        .collect();
    // Кроссфейд начинается ровно за xf кадров до конца A.
    let start = na - xf;
    assert!(out[..start].iter().all(|&x| x == 0.5));
    let mut max_err = 0f32;
    for k in 0..xf {
        let (g_out, g_in) = equal_power((k + 1) as f32 / xf as f32);
        let want = 0.5 * g_out - 0.25 * g_in;
        max_err = max_err.max((out[start + k] - want).abs());
    }
    println!("crossfade max deviation from equal-power: {max_err:.2e}");
    assert!(max_err < 1e-5, "crossfade curve error {max_err}");
    // Дальше только B, общая длина = A + B − кроссфейд.
    let end = na + nb - xf;
    assert!(out[start + xf..end].iter().all(|&x| x == -0.25));
    assert!(
        out[end..].iter().all(|&x| x == 0.0),
        "length must be A + B - crossfade"
    );
}

#[test]
fn loudness_gain_is_applied_per_track() {
    let a = tmp("lg_a.wav");
    write_wav(&a, RATE, &dc(RATE as usize, 0.1));
    let mut t = TrackSpec::file("a", &a);
    // −20 LUFS → цель −14: +6 дБ, пик −20 dBFS не мешает.
    t.loudness_lufs = Some(-20.0);
    t.peak_db = Some(-20.0);
    let cfg = EngineConfig {
        output: OutputConfig::Capture { sample_rate: RATE },
        ..EngineConfig::default()
    };
    let (engine, events) = Engine::new(cfg).unwrap();
    engine.load_queue(vec![t.clone()], 0, true);
    while let Ok(e) = events.recv_timeout(Duration::from_secs(10)) {
        if matches!(e, Event::QueueEnded) {
            break;
        }
    }
    std::thread::sleep(Duration::from_millis(50));
    let out = engine.take_capture();
    let level = out[1000];
    let want = 0.1 * 10f32.powf(6.0 / 20.0);
    println!("normalised level {level} (want {want})");
    assert!((level - want).abs() < 1e-5);
    // Пик ограничивает усиление: пик −3 dBFS → не выше −1 dBFS, т. е. +2 дБ вместо +6.
    t.peak_db = Some(-3.0);
    assert!((t.gain_for(-14.0) - 10f32.powf(2.0 / 20.0)).abs() < 1e-5);
}
