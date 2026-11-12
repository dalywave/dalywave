//! Проигрывает очередь на системном устройстве: `cargo run --example play -- <url|file>...`
//! DW_XFADE=мс — кроссфейд; DW_VOLUME=0..1; DW_SEEK=мс — перемотка через 2 с после старта;
//! DALYWAVE_OUTPUT=buffered — буферизованный вывод Apple (как у AirPlay), realtime — только cpal.
use dalywave_audio::{Engine, EngineConfig, Event, TrackSpec, Transition};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tracks: Vec<TrackSpec> = args
        .iter()
        .enumerate()
        .map(|(i, a)| {
            if a.starts_with("http") {
                TrackSpec::url(format!("t{i}"), a)
            } else {
                TrackSpec::file(format!("t{i}"), a)
            }
        })
        .collect();
    let mut cfg = EngineConfig::default();
    if let Ok(ms) = std::env::var("DW_XFADE") {
        cfg.transition = Transition::Crossfade {
            ms: ms.parse().unwrap(),
        };
    }
    if let Ok(v) = std::env::var("DALYWAVE_OUTPUT") {
        cfg.buffered = dalywave_audio::BufferedOutput::from_env_value(&v);
    }
    cfg.volume = std::env::var("DW_VOLUME")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0); // без DW_VOLUME — тишина: машина сборки не должна звучать
    let t0 = Instant::now();
    let (engine, events) = Engine::new(cfg).expect("engine");
    println!(
        "[{:7.1} ms] engine ready: {:?}",
        t0.elapsed().as_secs_f64() * 1000.0,
        engine.snapshot().device_rate
    );
    engine.load_queue(tracks, 0, true);
    let seek: Option<u64> = std::env::var("DW_SEEK").ok().and_then(|v| v.parse().ok());
    let mut seeked = false;
    loop {
        match events.recv_timeout(Duration::from_secs(30)) {
            Ok(Event::Position {
                index, position_ms, ..
            }) => {
                if position_ms % 1000 < 250 {
                    let s = engine.snapshot();
                    println!(
                        "[{:7.1} ms] pos #{index} {position_ms} ms (ahead {} ms{})",
                        t0.elapsed().as_secs_f64() * 1000.0,
                        s.output_ahead_ms,
                        if s.output_buffered { ", buffered" } else { "" }
                    );
                }
                if let (Some(s), false) = (seek, seeked)
                    && position_ms > 2000
                {
                    engine.seek(s);
                    seeked = true;
                }
            }
            Ok(Event::QueueEnded) => {
                println!(
                    "[{:7.1} ms] queue ended",
                    t0.elapsed().as_secs_f64() * 1000.0
                );
                break;
            }
            Ok(e) => println!("[{:7.1} ms] {e:?}", t0.elapsed().as_secs_f64() * 1000.0),
            Err(_) => {
                println!("timeout");
                break;
            }
        }
    }
    let s = engine.snapshot();
    println!(
        "device {} Hz {} ch, buffer {} frames, underruns {}, callback load avg {:.3}% max {:.3}%",
        s.device_rate,
        s.device_channels,
        s.buffer_frames,
        s.underruns,
        s.callback_load_avg * 100.0,
        s.callback_load_max * 100.0
    );
}
