//! Системный вывод не отвечает (аудиосервер завис): движок не падает и не встаёт, а сообщает
//! «звук недоступен», повторяет попытки и, когда сервер оживает, играет дальше с той же позиции.
//! Вывод — проверочный ([`FakeOutput`]): поток как у устройства, реальное время, звук никуда не
//! идёт; «зависание» — запуск не возвращается, колбэки не приходят.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use dalywave_audio::{
    Engine, EngineConfig, Event, FakeOutput, OutputConfig, PlaybackState, TrackSpec, Transition,
};

const RATE: u32 = 48_000;

/// Тон на `secs` секунд — трек для проверки.
fn tone(name: &str, secs: f32) -> PathBuf {
    let dir = std::env::temp_dir().join("dw-audio-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let frames = (secs * RATE as f32) as usize;
    let mut s = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let x = 0.1 * (i as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin();
        s.extend_from_slice(&[x, x]);
    }
    dalywave_audio::testkit::write_wav(&path, RATE, &s).unwrap();
    path
}

fn engine(fake: &FakeOutput) -> (Engine, Receiver<Event>) {
    Engine::new(EngineConfig {
        output: OutputConfig::Fake {
            sample_rate: RATE,
            fake: fake.clone(),
        },
        transition: Transition::Gapless,
        normalize: false,
        ..EngineConfig::default()
    })
    .unwrap()
}

/// Ждать событие, подходящее под `want`; ошибка движка — провал (звук недоступен — не ошибка
/// трека).
fn wait(
    events: &Receiver<Event>,
    timeout: Duration,
    mut want: impl FnMut(&Event) -> bool,
) -> Event {
    let t0 = Instant::now();
    loop {
        let left = timeout
            .checked_sub(t0.elapsed())
            .unwrap_or_else(|| panic!("за {timeout:?} не дождались"));
        let e = events.recv_timeout(left).expect("нет событий");
        if let Event::Error { message, .. } = &e {
            panic!("ошибка движка: {message}");
        }
        if want(&e) {
            return e;
        }
    }
}

/// Ждать, пока позиция по снимку не дойдёт до `ms`.
fn wait_position(engine: &Engine, ms: u64, timeout: Duration) {
    let t0 = Instant::now();
    while engine.snapshot().position_ms < ms {
        assert!(
            t0.elapsed() < timeout,
            "позиция {} мс, ждали {ms}",
            engine.snapshot().position_ms
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Снимок обновляется в том же такте контроллера, что и событие, — чуть позже.
fn stalled(engine: &Engine, want: bool) {
    let t0 = Instant::now();
    while engine.snapshot().output_stalled != want {
        assert!(
            t0.elapsed() < Duration::from_millis(500),
            "снимок: output_stalled != {want}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn hung_audio_server_stalls_then_resumes_from_the_same_place() {
    let fake = FakeOutput::default();
    let (engine, events) = engine(&fake);
    let track = TrackSpec::file("t0", tone("stall-hang.wav", 30.0));
    engine.load_queue(vec![track], 0, true);
    wait(&events, Duration::from_secs(5), |e| {
        matches!(e, Event::State(PlaybackState::Playing))
    });
    wait_position(&engine, 1000, Duration::from_secs(5));

    // Аудиосервер завис: колбэки не приходят, новый запуск не возвращается
    fake.set_hung(true);
    let hung_at = engine.snapshot().position_ms;
    let t0 = Instant::now();
    wait(&events, Duration::from_secs(6), |e| {
        matches!(e, Event::OutputStalled { attempt: 0, .. })
    });
    let noticed = t0.elapsed();
    assert!(noticed < Duration::from_secs(5), "заметили за {noticed:?}");
    stalled(&engine, true);
    let s = engine.snapshot();
    assert!(
        s.position_ms.abs_diff(hung_at) < 300,
        "позиция ушла: {} → {}",
        hung_at,
        s.position_ms
    );
    // Повтор с отступом: вывод открыт заново, запуск висит — движок ждёт его, а не открывает
    // новые (каждый застрял бы так же)
    wait(&events, Duration::from_secs(4), |e| {
        matches!(e, Event::OutputStalled { attempt: 1, .. })
    });
    wait(&events, Duration::from_secs(5), |e| {
        matches!(e, Event::OutputStalled { attempt: 2, .. })
    });

    // Сервер ожил: звук снова идёт, с того же места
    fake.set_hung(false);
    let t1 = Instant::now();
    wait(&events, Duration::from_secs(4), |e| {
        matches!(e, Event::OutputRecovered)
    });
    assert!(
        t1.elapsed() < Duration::from_secs(3),
        "ожил за {:?}",
        t1.elapsed()
    );
    stalled(&engine, false);
    let resumed = match wait(
        &events,
        Duration::from_secs(5),
        |e| matches!(e, Event::Position { position_ms, .. } if *position_ms > hung_at),
    ) {
        Event::Position { position_ms, .. } => position_ms,
        _ => unreachable!(),
    };
    assert!(
        resumed < hung_at + 1500,
        "продолжил не с места: {hung_at} → {resumed}"
    );
    wait_position(&engine, hung_at + 1000, Duration::from_secs(5));
    assert_eq!(engine.snapshot().state, PlaybackState::Playing);
}

#[test]
fn output_missing_at_start_is_retried_on_play() {
    let fake = FakeOutput::default();
    // Открытие при создании движка, при «играть» и при первом повторе не удаются
    fake.fail_opens(3);
    let (engine, events) = engine(&fake);
    let track = TrackSpec::file("t0", tone("stall-open.wav", 10.0));
    engine.load_queue(vec![track], 0, true);
    wait(&events, Duration::from_secs(3), |e| {
        matches!(e, Event::OutputStalled { attempt: 0, .. })
    });
    stalled(&engine, true);
    // Второй повтор (через 1 + 2 с) открывает вывод
    wait(&events, Duration::from_secs(8), |e| {
        matches!(e, Event::OutputRecovered)
    });
    wait(&events, Duration::from_secs(5), |e| {
        matches!(e, Event::State(PlaybackState::Playing))
    });
    wait_position(&engine, 500, Duration::from_secs(5));
}

#[test]
fn pause_ends_the_wait_for_the_output() {
    let fake = FakeOutput::default();
    let (engine, events) = engine(&fake);
    let track = TrackSpec::file("t0", tone("stall-pause.wav", 30.0));
    engine.load_queue(vec![track], 0, true);
    wait(&events, Duration::from_secs(5), |e| {
        matches!(e, Event::State(PlaybackState::Playing))
    });
    fake.set_hung(true);
    wait(&events, Duration::from_secs(6), |e| {
        matches!(e, Event::OutputStalled { .. })
    });
    engine.pause();
    wait(&events, Duration::from_secs(2), |e| {
        matches!(e, Event::OutputRecovered)
    });
    stalled(&engine, false);
    // На паузе повторов нет
    let quiet = Instant::now();
    while quiet.elapsed() < Duration::from_secs(2) {
        if let Ok(e) = events.recv_timeout(Duration::from_millis(100)) {
            assert!(!matches!(e, Event::OutputStalled { .. }), "повтор на паузе");
        }
    }
    // Сервер ожил, «играть» — звук идёт
    fake.set_hung(false);
    let at = engine.snapshot().position_ms;
    engine.play();
    wait_position(&engine, at + 500, Duration::from_secs(5));
}
