//! Буферизованный вывод (AirPlay 2): события — по тому, что слышно, а не по тому, что отдал микшер;
//! смена маршрута переключает вывод без скачка позиции. Маршрут — подставной, вывод — захват,
//! который ведёт себя как буферизованный (слышно на секунду позже). На macOS — ещё и настоящий
//! AVSampleBufferAudioRenderer на громкости 0.

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use dalywave_audio::devices::OutputKind;
use dalywave_audio::{
    BufferedOutput, ChangeReason, Engine, EngineConfig, Event, OutputConfig, PlaybackState,
    RouteProbe, TrackSpec, Transition,
};

const RATE: u32 = 48_000;

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("dw-audio-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn dc_file(name: &str, secs: f64, level: f32) -> std::path::PathBuf {
    let path = tmp(name);
    write_wav(&path, RATE, &vec![level; (secs * RATE as f64) as usize * 2]);
    path
}

fn capture(buffered: BufferedOutput, route: Option<RouteProbe>) -> EngineConfig {
    EngineConfig {
        output: OutputConfig::Capture { sample_rate: RATE },
        transition: Transition::Gapless,
        normalize: false,
        buffered,
        route,
        capture_latency_ms: 1000,
        // Между событиями позиции (раз в 250 мс) — около секунды звука
        capture_speed: 4.0,
        ..EngineConfig::default()
    }
}

/// События до конца очереди (или до `limit`).
fn until_ended(events: &crossbeam_channel::Receiver<Event>, limit: Duration) -> Vec<Event> {
    let t0 = Instant::now();
    let mut out = Vec::new();
    loop {
        let left = limit
            .checked_sub(t0.elapsed())
            .expect("очередь не доиграла");
        let e = events.recv_timeout(left).expect("очередь не доиграла");
        let end = matches!(e, Event::QueueEnded);
        out.push(e);
        if end {
            return out;
        }
    }
}

#[test]
fn track_change_waits_until_it_is_heard() {
    let a = dc_file("buf_a.wav", 3.0, 0.25);
    let b = dc_file("buf_b.wav", 3.0, 0.5);
    let (engine, events) = Engine::new(capture(BufferedOutput::Always, None)).unwrap();
    engine.load_queue(
        vec![TrackSpec::file("a", &a), TrackSpec::file("b", &b)],
        0,
        true,
    );
    let all = until_ended(&events, Duration::from_secs(30));
    let changed = all
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::TrackChanged {
                    index: 1,
                    reason: ChangeReason::Auto,
                    ..
                }
            )
        })
        .expect("переход к b");
    // До перехода — позиции a вплоть до самого конца (слышимое), после — ни одной позиции a:
    // без ожидания интерфейс получил бы «играет b», а следом ещё секунду позиций a
    let last_a = all[..changed]
        .iter()
        .filter_map(|e| match e {
            Event::Position {
                index: 0,
                position_ms,
                ..
            } => Some(*position_ms),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert!(last_a >= 2000, "позиции a обрываются на {last_a} мс");
    for e in &all[changed..] {
        if let Event::Position {
            index, position_ms, ..
        } = e
        {
            assert_eq!(*index, 1, "позиция a после перехода: {position_ms} мс");
        }
    }
    let first_b = all[changed..].iter().find_map(|e| match e {
        Event::Position { position_ms, .. } => Some(*position_ms),
        _ => None,
    });
    assert!(
        first_b.is_none_or(|p| p < 1000),
        "b начинается с {first_b:?} мс"
    );
    // Конец очереди — тоже по слышимому: после последнего трека
    assert!(matches!(all.last(), Some(Event::QueueEnded)));
}

#[test]
fn seek_flushes_what_was_sent_ahead() {
    let a = dc_file("buf_seek.wav", 60.0, 0.25);
    let (engine, events) = Engine::new(capture(BufferedOutput::Always, None)).unwrap();
    engine.load_queue(vec![TrackSpec::file("a", &a)], 0, true);
    let t0 = Instant::now();
    let mut seeked = false;
    loop {
        assert!(t0.elapsed() < Duration::from_secs(20), "не дождались");
        match events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::Position { position_ms, .. } if !seeked && position_ms > 1000 => {
                assert!(engine.snapshot().output_buffered);
                engine.seek(30_000);
                seeked = true;
            }
            // После перемотки слышно сразу новое место: не хвост прежнего, отданный наперёд
            Event::Position { position_ms, .. } if seeked => {
                assert!(
                    (30_000..40_000).contains(&position_ms),
                    "после перемотки {position_ms} мс"
                );
                if position_ms > 31_000 {
                    break;
                }
            }
            _ => {}
        }
    }
}

#[test]
fn route_switch_keeps_the_position() {
    let a = dc_file("buf_route.wav", 40.0, 0.25);
    let route = Arc::new(Mutex::new(vec![OutputKind::BuiltIn]));
    let probe = {
        let route = route.clone();
        RouteProbe::new(move || route.lock().unwrap().clone())
    };
    let (engine, events) = Engine::new(capture(BufferedOutput::Auto, Some(probe))).unwrap();
    assert!(
        !engine.snapshot().output_buffered,
        "динамик — обычный вывод"
    );
    engine.load_queue(vec![TrackSpec::file("a", &a)], 0, true);
    let set = |kinds: Vec<OutputKind>| {
        *route.lock().unwrap() = kinds;
        engine.route_changed();
    };
    let t0 = Instant::now();
    let mut step = 0;
    let mut last = 0u64;
    let mut outputs: Vec<String> = Vec::new();
    loop {
        assert!(
            t0.elapsed() < Duration::from_secs(30),
            "не дождались: {outputs:?}"
        );
        match events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::Position { position_ms, .. } => {
                // Ни шага назад: обычный вывод открывается со слышимого места буферизованного
                assert!(
                    position_ms + 100 >= last,
                    "позиция назад: {last} → {position_ms} мс ({outputs:?})"
                );
                last = position_ms;
                match step {
                    0 if position_ms > 2000 => {
                        // Выбрали две колонки HomePod в окне AirPlay
                        set(vec![OutputKind::AirPlay, OutputKind::AirPlay]);
                        step = 1;
                    }
                    1 if engine.snapshot().output_buffered && position_ms > 8000 => {
                        // Промежуточный пустой маршрут ничего не переключает
                        let reopens = engine.snapshot().output_reopens;
                        set(vec![]);
                        std::thread::sleep(Duration::from_millis(100));
                        assert_eq!(engine.snapshot().output_reopens, reopens);
                        set(vec![OutputKind::Headphones]);
                        step = 2;
                    }
                    2 if !engine.snapshot().output_buffered && position_ms > 14_000 => break,
                    _ => {}
                }
            }
            Event::Output { reason, .. } => outputs.push(reason),
            Event::State(PlaybackState::Ended) => panic!("доиграло до переключений: {outputs:?}"),
            _ => {}
        }
    }
    assert_eq!(
        outputs,
        vec![
            "route airplay+airplay: buffered output".to_string(),
            "route headphones: realtime output".to_string(),
        ]
    );
}

/// Настоящий AVSampleBufferAudioRenderer на устройстве по умолчанию, громкость 0 (без звука):
/// позиция идёт по часам, пауза её останавливает, переход к следующему треку — когда он слышен.
#[cfg(target_os = "macos")]
#[test]
fn renderer_on_this_mac_follows_the_wall_clock() {
    let a = dc_file("buf_mac_a.wav", 3.0, 0.25);
    let b = dc_file("buf_mac_b.wav", 2.0, 0.25);
    let cfg = EngineConfig {
        buffered: BufferedOutput::Always,
        normalize: false,
        volume: 0.0,
        ..EngineConfig::default()
    };
    let (engine, events) = Engine::new(cfg).unwrap();
    assert!(engine.snapshot().output_buffered);
    engine.load_queue(
        vec![TrackSpec::file("a", &a), TrackSpec::file("b", &b)],
        0,
        true,
    );
    let mut started: Option<Instant> = None;
    let mut paused = false;
    let mut changed_after: Option<Duration> = None;
    // Насколько звук уходит наперёд: с такой задержкой слышны эквалайзер и затухание таймера сна
    let mut ahead_max = 0u32;
    let t0 = Instant::now();
    loop {
        assert!(t0.elapsed() < Duration::from_secs(30), "не доиграло");
        let e = events.recv_timeout(Duration::from_secs(12)).unwrap();
        match e {
            Event::State(PlaybackState::Playing) if started.is_none() => {
                started = Some(Instant::now());
            }
            Event::Position {
                index: 0,
                position_ms,
                ..
            } => {
                let Some(s) = started else { continue };
                ahead_max = ahead_max.max(engine.snapshot().output_ahead_ms);
                // Слышимое не обгоняет часы: задержка вывода учтена
                let wall = s.elapsed().as_millis() as u64;
                assert!(
                    position_ms <= wall + 150,
                    "позиция {position_ms} мс за {wall} мс"
                );
                if !paused && position_ms > 1000 {
                    paused = true;
                    engine.pause();
                    std::thread::sleep(Duration::from_millis(300));
                    let before = engine.snapshot().position_ms;
                    std::thread::sleep(Duration::from_millis(600));
                    let after = engine.snapshot().position_ms;
                    // На паузе слышимое стоит, хотя наперёд отдано больше секунды
                    assert!(after.abs_diff(before) < 50, "{before} → {after} мс");
                    assert!(before.abs_diff(position_ms) < 400, "пауза на {before} мс");
                    engine.play();
                }
            }
            Event::TrackChanged {
                index: 1,
                reason: ChangeReason::Auto,
                ..
            } => {
                changed_after = started.map(|s| s.elapsed());
            }
            Event::QueueEnded => break,
            // Системный звук Mac завис (coreaudiod не ведёт ни одно устройство) — проверять нечего
            Event::Output { reason, .. } if reason.contains("buffered output failed") => {
                eprintln!("пропуск: системный звук не работает ({reason})");
                return;
            }
            Event::Error { message, .. } => panic!("{message}"),
            _ => {}
        }
    }
    // 3 с трека a и ~0,9 с паузы: переход не раньше, чем a дозвучало
    eprintln!("рендерер: наперёд до {ahead_max} мс");
    let changed = changed_after.expect("переход к b");
    assert!(
        changed >= Duration::from_millis(3700),
        "переход через {changed:?}"
    );
    assert!(paused);
}
