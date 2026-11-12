//! Эквалайзер и таймер сна через весь движок (вывод в память, без звука).

mod common;

use std::time::{Duration, Instant};

use common::*;
use dalywave_audio::{
    Engine, EngineConfig, EqPreset, EqSettings, Event, OutputConfig, PlaybackState, SleepTimer,
    TrackSpec, Transition,
};

const RATE: u32 = 48_000;

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("dw-audio-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn dc(frames: usize, level: f32) -> Vec<f32> {
    vec![level; frames * 2]
}

fn config(fade_ms: u32) -> EngineConfig {
    EngineConfig {
        output: OutputConfig::Capture { sample_rate: RATE },
        transition: Transition::Gapless,
        normalize: false,
        preload_secs: 3600,
        sleep_fade_ms: fade_ms,
        ..EngineConfig::default()
    }
}

/// Ждать событие; по пути — колбэк на каждое.
fn wait_for(
    engine: &Engine,
    events: &crossbeam_channel::Receiver<Event>,
    timeout: Duration,
    mut on: impl FnMut(&Engine, &Event) -> bool,
) {
    let t0 = Instant::now();
    loop {
        let left = timeout.checked_sub(t0.elapsed()).expect("timeout");
        let e = events
            .recv_timeout(left)
            .expect("timeout waiting for event");
        if on(engine, &e) {
            return;
        }
    }
}

fn left(samples: &[f32]) -> Vec<f32> {
    samples.as_chunks::<2>().0.iter().map(|p| p[0]).collect()
}

#[test]
fn sleep_timer_fades_out_then_pauses() {
    let a = tmp("sleep_a.wav");
    write_wav(&a, RATE, &dc(RATE as usize * 60, 0.5));
    let fade_ms = 2000u32;
    let (engine, events) = Engine::new(config(fade_ms)).unwrap();
    engine.load_queue(vec![TrackSpec::file("a", &a)], 0, true);
    let mut armed = false;
    let mut fading_seen = false;
    wait_for(
        &engine,
        &events,
        Duration::from_secs(30),
        |e, ev| match ev {
            Event::State(PlaybackState::Playing) if !armed => {
                armed = true;
                // Таймер короче затухания: затухание сразу, длиной до срока
                e.set_sleep(Some(SleepTimer::After { ms: 2000 }));
                false
            }
            Event::Sleep(s) => {
                fading_seen |= s.fading;
                s.finished
            }
            _ => false,
        },
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(fading_seen, "status reports the fade");
    let snap = engine.snapshot();
    assert_eq!(snap.state, PlaybackState::Paused);
    assert!(snap.sleep.is_none());
    let out = left(&engine.take_capture());
    let start = out.iter().position(|&x| x < 0.5).expect("fade starts");
    assert!(out[..start].iter().all(|&x| x == 0.5));
    let silent = start
        + out[start..]
            .iter()
            .position(|&x| x == 0.0)
            .expect("reaches silence");
    let len = silent - start;
    let want = (RATE as u64 * u64::from(fade_ms) / 1000) as usize;
    println!(
        "fade from frame {start}, {len} frames (want ≤ {want}), tail {} frames",
        out.len() - silent
    );
    assert!(len > want * 9 / 10 && len <= want + 1, "fade length {len}");
    // Плавно и монотонно: равномощная кривая, в середине −3 дБ
    for w in out[start..silent].windows(2) {
        assert!(w[1] <= w[0] + 1e-7, "fade must not rise");
        assert!(w[0] - w[1] < 0.5 * 4.0 / len as f32, "no steps");
    }
    let mid = out[start + len / 2] / 0.5;
    assert!(
        (mid - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02,
        "mid gain {mid}"
    );
    assert!(
        out[silent..].iter().all(|&x| x == 0.0),
        "silence until the pause"
    );

    // Снова «играть» — полная громкость
    engine.play();
    wait_for(&engine, &events, Duration::from_secs(10), |_, ev| {
        matches!(ev, Event::State(PlaybackState::Playing))
    });
    std::thread::sleep(Duration::from_millis(200));
    engine.pause();
    std::thread::sleep(Duration::from_millis(100));
    let again = left(&engine.take_capture());
    assert!(
        again.iter().filter(|&&x| x == 0.5).count() > 1000,
        "volume is back after the timer"
    );
}

#[test]
fn sleep_timer_at_end_of_track_stops_on_the_boundary() {
    // Первый трек длинный: вывод в память обгоняет реальное время, таймер должен успеть встать
    let (na, nb) = (RATE as usize * 20, RATE as usize * 5);
    let a = tmp("sleep_eot_a.wav");
    let b = tmp("sleep_eot_b.wav");
    write_wav(&a, RATE, &dc(na, 0.5));
    write_wav(&b, RATE, &dc(nb, -0.25));
    let fade_ms = 1000u32;
    let fade = RATE as usize * fade_ms as usize / 1000;
    let (engine, events) = Engine::new(config(fade_ms)).unwrap();
    engine.load_queue(
        vec![TrackSpec::file("a", &a), TrackSpec::file("b", &b)],
        0,
        true,
    );
    let mut armed = false;
    let mut changed = false;
    wait_for(
        &engine,
        &events,
        Duration::from_secs(30),
        |e, ev| match ev {
            Event::State(PlaybackState::Playing) if !armed => {
                armed = true;
                e.set_sleep(Some(SleepTimer::EndOfTrack));
                false
            }
            Event::TrackChanged { index: 1, .. } => {
                changed = true;
                false
            }
            Event::Sleep(s) => s.finished,
            _ => false,
        },
    );
    std::thread::sleep(Duration::from_millis(150));
    assert!(changed, "the next track became current");
    let snap = engine.snapshot();
    assert_eq!(snap.state, PlaybackState::Paused);
    assert_eq!(snap.index, Some(1));
    assert_eq!(snap.position_ms, 0, "the next track waits from its start");
    let out = left(&engine.take_capture());
    // До затухания — как есть; затухание кончается ровно на стыке; дальше — тишина
    assert!(
        out[..na - fade].iter().all(|&x| x == 0.5),
        "untouched before the fade"
    );
    // Равномощная кривая у начала — 1 − x²/2: первые сэмплы в f32 ещё ровно 0,5
    let start = out.iter().position(|&x| x < 0.5).unwrap();
    assert!(
        (na - fade..na - fade + 16).contains(&start),
        "fade starts at {start}"
    );
    for w in out[na - fade..na].windows(2) {
        assert!(w[1] <= w[0] + 1e-7);
    }
    assert!(
        out[na - 1].abs() < 1e-3,
        "silent at the end of the track: {}",
        out[na - 1]
    );
    assert!(
        out[na..].iter().all(|&x| x == 0.0),
        "the next track is not heard"
    );
}

#[test]
fn sleep_timer_can_be_cancelled() {
    let a = tmp("sleep_cancel.wav");
    write_wav(&a, RATE, &dc(RATE as usize * 60, 0.5));
    let (engine, events) = Engine::new(config(60_000)).unwrap();
    engine.load_queue(vec![TrackSpec::file("a", &a)], 0, true);
    let mut armed = false;
    wait_for(
        &engine,
        &events,
        Duration::from_secs(30),
        |e, ev| match ev {
            Event::State(PlaybackState::Playing) if !armed => {
                armed = true;
                e.set_sleep(Some(SleepTimer::After { ms: 60_000 }));
                false
            }
            Event::Sleep(s) if s.fading => {
                e.set_sleep(None);
                false
            }
            Event::Sleep(s) => s.timer.is_none(),
            _ => false,
        },
    );
    std::thread::sleep(Duration::from_millis(300));
    let snap = engine.snapshot();
    assert_eq!(snap.state, PlaybackState::Playing);
    assert!(snap.sleep.is_none());
    engine.pause();
    std::thread::sleep(Duration::from_millis(100));
    let out = left(&engine.take_capture());
    let tail = &out[out.len().saturating_sub(4800)..out.len().saturating_sub(1200)];
    assert!(
        tail.iter().all(|&x| (x - 0.5).abs() < 1e-6),
        "volume restored"
    );
}

/// Логарифмический свип 20 Гц – 20 кГц, полная шкала минус 1 дБ.
fn sweep(secs: f64) -> Vec<f32> {
    let (f0, f1) = (20.0f64, 20_000.0f64);
    let n = (secs * f64::from(RATE)) as usize;
    let k = (f1 / f0).ln();
    let amp = 10f64.powf(-1.0 / 20.0);
    (0..n)
        .flat_map(|i| {
            let t = i as f64 / f64::from(RATE);
            let v = (amp
                * (std::f64::consts::TAU * f0 * secs / k * ((k * t / secs).exp() - 1.0)).sin())
                as f32;
            [v, v]
        })
        .collect()
}

fn band_gain_db(out: &[f32], input: &[f32], secs: f64, hz: f64) -> f32 {
    let n = (secs * f64::from(RATE)) as usize;
    let k = (20_000.0f64 / 20.0).ln();
    let at = |f: f64| ((f / 20.0).ln() / k * n as f64) as usize;
    let (a, b) = (
        at(hz * 2f64.powf(-1.0 / 12.0)),
        at(hz * 2f64.powf(1.0 / 12.0)),
    );
    let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    20.0 * (rms(&out[a..b]) / rms(&input[a..b])).log10()
}

#[test]
fn equalizer_presets_through_the_engine() {
    let secs = 8.0;
    let input = sweep(secs);
    let path = tmp("eq_sweep.wav");
    write_wav(&path, RATE, &input);
    let mono_in = left(&input);
    let probes = [
        40.0, 62.0, 125.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 14000.0,
    ];
    for preset in EqPreset::ALL {
        let settings = EqSettings::preset(preset);
        let cfg = EngineConfig {
            eq: Some(settings),
            ..config(15_000)
        };
        let res = dalywave_audio::testkit::render_with(
            vec![TrackSpec::file("sweep", &path)],
            cfg,
            Duration::from_secs(60),
            |_, _| {},
        )
        .unwrap();
        let out = left(&res.samples);
        assert!(out.len() >= mono_in.len());
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        let pre = settings.preamp_db(RATE);
        let mut line = Vec::new();
        for hz in probes {
            let got = band_gain_db(&out, &mono_in, secs, hz);
            let want = settings.response_db(hz as f32, RATE) + pre;
            line.push(format!("{hz:.0}:{got:+.1}"));
            assert!(
                (got - want).abs() < 0.75,
                "{}: {hz} Hz got {got:+.2}, want {want:+.2}",
                preset.id()
            );
        }
        println!("{:11} peak {peak:.3} | {}", preset.id(), line.join(" "));
        // Предусиление и ограничитель: в шкале, без перегрузки
        assert!(peak <= 1.0, "{} clips: {peak}", preset.id());
        if preset == EqPreset::Off {
            assert_eq!(&out[..mono_in.len()], &mono_in[..], "off is bit-exact");
        }
    }
}

#[test]
fn equalizer_changes_on_the_fly_without_dropouts() {
    let tone: Vec<f32> = (0..RATE as usize * 6)
        .flat_map(|i| {
            let v = (std::f32::consts::TAU * 100.0 * i as f32 / RATE as f32).sin() * 0.3;
            [v, v]
        })
        .collect();
    let path = tmp("eq_tone.wav");
    write_wav(&path, RATE, &tone);
    let mut switched = false;
    let res = dalywave_audio::testkit::render_with(
        vec![TrackSpec::file("tone", &path)],
        config(15_000),
        Duration::from_secs(60),
        |e, ev| {
            if !switched && matches!(ev, Event::State(PlaybackState::Playing)) {
                switched = true;
                e.set_eq(Some(EqSettings::preset(EqPreset::BassBoost)));
            }
        },
    )
    .unwrap();
    let out = left(&res.samples);
    // Тон 100 Гц стал громче (подъём баса минус предусиление), без скачков между соседними сэмплами
    let s = EqSettings::preset(EqPreset::BassBoost);
    let want = s.response_db(100.0, RATE) + s.preamp_db(RATE);
    let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    let tail = &out[out.len() - RATE as usize..];
    let got = 20.0 * (rms(tail) / (0.3 / std::f32::consts::SQRT_2)).log10();
    assert!((got - want).abs() < 0.3, "got {got:+.2}, want {want:+.2}");
    let max_step =
        0.3 * 10f32.powf(want.max(0.0) / 20.0) * std::f32::consts::TAU * 100.0 / RATE as f32 * 1.5;
    for (i, w) in out.windows(2).enumerate() {
        assert!(
            (w[1] - w[0]).abs() < max_step,
            "jump {} at {i} of {}: {:?}",
            (w[1] - w[0]).abs(),
            out.len(),
            &out[i.saturating_sub(3)..(i + 4).min(out.len())]
        );
    }
}
