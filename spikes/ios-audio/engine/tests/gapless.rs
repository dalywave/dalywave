//! Склейка без пауз: выход движка в тестовом режиме сравнивается с непрерывным эталоном.

mod common;

use std::time::Duration;

use common::*;
use dw_audio::testkit::{
    GaplessReport, analyze_joins, compare_with_concat, decode_all, render_queue, resample_all,
    resample_per_track,
};
use dw_audio::{SourceSpec, TrackSpec, Transition};

fn tracks_for(name: &str, ext: &str, http: bool) -> Vec<TrackSpec> {
    (1..=4)
        .map(|n| {
            let id = format!("{name}-{n}");
            if http {
                TrackSpec::url(
                    id,
                    format!("{}/gapless/{name}/part{n}.{ext}", http_server()),
                )
            } else {
                TrackSpec::file(id, part_path(name, ext, n))
            }
        })
        .collect()
}

fn run(name: &str, rate: u32, ext: &str, http: bool) -> (GaplessReport, (usize, f32, i64)) {
    let res = render_queue(
        tracks_for(name, ext, http),
        rate,
        Transition::Gapless,
        Duration::from_secs(60),
    )
    .unwrap();
    let r = analyze_joins(&res.samples, &reference(rate), &bounds(rate));
    let parts: Vec<SourceSpec> = (1..=4)
        .map(|n| SourceSpec::File(part_path(name, ext, n)))
        .collect();
    let exact = compare_with_concat(&res.samples, &parts).unwrap();
    println!(
        "{name:10} {} [{:.0} ms] vs concat: {} diff samples, len {:+} | vs reference: {}",
        if http { "http" } else { "file" },
        res.elapsed.as_secs_f64() * 1000.0,
        exact.0,
        exact.2,
        r.summary()
    );
    (r, exact)
}

#[test]
fn gapless_joins_are_sample_accurate() {
    let mut failures = Vec::new();
    for (name, rate, ext) in FORMATS {
        let (r, (diff, _, len)) = run(name, rate, ext, false);
        // 1) Движок склеивает ровно то, что отдаёт декодер: ни сэмпла лишнего, ни потерянного.
        if diff != 0 || len != 0 {
            failures.push(format!("{name}: engine output differs from concatenated decodes ({diff} samples, len {len:+})"));
        }
        if name == "opus_webm" || name == "vorbis_q6" {
            continue; // см. decode.rs: хвост WebM и неточная гранула кодера Vorbis
        }
        // 2) Сигнал на стыках непрерывен относительно эталона: сдвиг меньше сэмпла, без тишины,
        //    без перепутанных каналов. Точность оценки сдвига на низких частотах свипа ±0,3.
        let lossless = name.starts_with("flac");
        let shift_limit = if lossless { 0.5 } else { 1.0 };
        if r.worst_shift() > shift_limit
            || r.max_zero_run > 4
            || r.offset.abs() > 2.0
            || !r.channels_ok
        {
            failures.push(format!("{name}: {}", r.summary()));
        }
        if lossless && r.worst_click_db() > 3.0 {
            failures.push(format!("{name}: error at join above body: {}", r.summary()));
        }
        if r.got_frames.abs_diff(r.expected_frames) > 1 {
            failures.push(format!(
                "{name}: length {} vs {}",
                r.got_frames, r.expected_frames
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn gapless_over_http_with_range() {
    for (name, rate, ext) in [
        ("opus_ogg", 48000, "opus"),
        ("opus_mp4", 48000, "mp4"),
        ("flac16_44", 44100, "flac"),
    ] {
        let (r, (diff, _, len)) = run(name, rate, ext, true);
        assert!(
            diff == 0 && len == 0,
            "{name}: http output differs from concatenated decodes"
        );
        assert!(r.max_zero_run <= 4, "{name}: {}", r.summary());
    }
}

#[test]
fn known_container_limits_are_reported() {
    // Для отчёта: что даёт WebM без DiscardPadding и Vorbis от экспериментального кодера.
    for (name, rate, ext) in [("opus_webm", 48000, "webm"), ("vorbis_q6", 44100, "ogg")] {
        let (r, _) = run(name, rate, ext, false);
        assert!(
            r.worst_shift() > 0.5,
            "{name}: ожидали заметный сдвиг, {}",
            r.summary()
        );
    }
}

/// FLAC 44.1 кГц на устройстве 48 кГц: цепочка не сбрасывает ресемплер между треками, поэтому
/// выход совпадает с ресемплингом всего альбома одним куском. Для сравнения — ошибка на стыках,
/// если каждый трек ресемплировать отдельно (так делают плееры с ресемплером на трек).
#[test]
fn gapless_with_resampling_is_continuous() {
    let (from, to) = (44100u32, 48000u32);
    let b = bounds(from);
    let mut album = Vec::new();
    for n in 1..=4 {
        album.extend(
            decode_all(&SourceSpec::File(part_path("flac16_44", "flac", n)))
                .unwrap()
                .samples,
        );
    }
    let continuous = resample_all(&album, from, to).unwrap();
    let per_track = resample_per_track(&album, &b, from, to).unwrap();
    let res = render_queue(
        tracks_for("flac16_44", "flac", false),
        to,
        Transition::Gapless,
        Duration::from_secs(60),
    )
    .unwrap();
    let b48: Vec<usize> = b
        .iter()
        .map(|&x| (x as u64 * to as u64).div_ceil(from as u64) as usize)
        .collect();

    let engine = analyze_joins(&res.samples, &continuous, &b48);
    let naive = analyze_joins(&per_track, &continuous, &b48);
    println!("engine   : {}", engine.summary());
    println!("per-track: {}", naive.summary());
    let worst_engine = engine
        .joins
        .iter()
        .map(|j| j.peak_near_db)
        .fold(f64::MIN, f64::max);
    let worst_naive = naive
        .joins
        .iter()
        .map(|j| j.peak_near_db)
        .fold(f64::MIN, f64::max);
    println!(
        "peak error at joins: engine {worst_engine:.1} dBFS, per-track resampling {worst_naive:.1} dBFS"
    );
    assert!(engine.offset.abs() < 0.5, "offset {}", engine.offset);
    assert!(
        worst_engine < -90.0,
        "engine join error {worst_engine:.1} dBFS"
    );
    assert!(engine.err_body_db < -100.0);
}
