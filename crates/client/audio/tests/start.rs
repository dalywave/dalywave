//! Запуск: очередь доходит до «играет» быстро в любом режиме — без «Диджея», с «Диджеем» и
//! медленным (или пустым) анализом, с готовым анализом. Анализ не держит старт: он нужен только к
//! переходу. Вывод — тестовый захват, без колонок.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::testdata;
use dalywave_audio::{
    Analysis, AnalysisSource, Engine, EngineConfig, Event, OutputConfig, PlaybackState, TrackSpec,
    Transition,
};

const LIMIT: Duration = Duration::from_secs(3);

fn queue(analysis: Option<Arc<Analysis>>) -> Vec<TrackSpec> {
    ["a.opus", "b.opus"]
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut t = TrackSpec::file(format!("t{i}"), testdata().join("demo").join(f));
            t.stream_id = Some(format!("s{i}"));
            t.analysis = analysis.clone();
            t
        })
        .collect()
}

/// Сколько прошло от загрузки очереди до «играет».
fn time_to_playing(cfg: EngineConfig, tracks: Vec<TrackSpec>) -> Duration {
    let (engine, events) = Engine::new(cfg).unwrap();
    let t0 = Instant::now();
    engine.load_queue(tracks, 0, true);
    loop {
        let left = LIMIT.saturating_sub(t0.elapsed());
        match events.recv_timeout(left) {
            Ok(Event::State(PlaybackState::Playing)) => return t0.elapsed(),
            Ok(Event::Error { message, .. }) => panic!("ошибка: {message}"),
            Ok(_) => {}
            Err(_) => panic!("за {LIMIT:?} не заиграло: {:?}", engine.snapshot().state),
        }
    }
}

fn capture(transition: Transition) -> EngineConfig {
    EngineConfig {
        output: OutputConfig::Capture {
            sample_rate: 48_000,
        },
        transition,
        ..EngineConfig::default()
    }
}

#[test]
fn plain_play_starts() {
    let t = time_to_playing(capture(Transition::Gapless), queue(None));
    assert!(t < LIMIT, "{t:?}");
    let t = time_to_playing(capture(Transition::Crossfade { ms: 4000 }), queue(None));
    assert!(t < LIMIT, "{t:?}");
}

#[test]
fn dj_starts_without_waiting_for_analysis() {
    // Сервер отвечает 5 с, и анализа нет — старт его не ждёт
    let mut cfg = capture(Transition::Dj { expressiveness: 50 });
    cfg.analysis = Some(AnalysisSource::new(|_| {
        std::thread::sleep(Duration::from_secs(5));
        Ok(None)
    }));
    let t = time_to_playing(cfg, queue(None));
    assert!(t < LIMIT, "{t:?}");
}

#[test]
fn dj_starts_with_analysis() {
    let step = 500.0;
    let beats: Vec<f64> = (0..60).map(|i| i as f64 * step).collect();
    let analysis = Analysis {
        duration_ms: Some(30_000.0),
        bpm: Some(120.0),
        bpm_confidence: Some(0.9),
        downbeats_ms: beats.iter().step_by(4).copied().collect(),
        beats_ms: beats,
        ..Default::default()
    };
    let t = time_to_playing(
        capture(Transition::Dj { expressiveness: 50 }),
        queue(Some(Arc::new(analysis))),
    );
    assert!(t < LIMIT, "{t:?}");
}
