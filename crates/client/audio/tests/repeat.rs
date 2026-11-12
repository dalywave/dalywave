//! Повтор одного трека через весь движок (вывод в память, без звука).

mod common;

use std::time::{Duration, Instant};

use common::*;
use dalywave_audio::{
    ChangeReason, Engine, EngineConfig, Event, OutputConfig, PlaybackState, TrackSpec, Transition,
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

fn config() -> EngineConfig {
    EngineConfig {
        output: OutputConfig::Capture { sample_rate: RATE },
        transition: Transition::Gapless,
        normalize: false,
        preload_secs: 3600,
        ..EngineConfig::default()
    }
}

fn wait_for(
    events: &crossbeam_channel::Receiver<Event>,
    timeout: Duration,
    mut on: impl FnMut(&Event) -> bool,
) {
    let t0 = Instant::now();
    loop {
        let left = timeout.checked_sub(t0.elapsed()).expect("timeout");
        let e = events
            .recv_timeout(left)
            .expect("timeout waiting for event");
        if on(&e) {
            return;
        }
    }
}

fn left(samples: &[f32]) -> Vec<f32> {
    samples.as_chunks::<2>().0.iter().map(|p| p[0]).collect()
}

/// Id в событиях — только элементы очереди интерфейса, без хвостов копий повтора.
fn id_of(e: &Event) -> Option<&str> {
    match e {
        Event::TrackChanged { track, .. } => Some(&track.id),
        Event::Position { id, .. } => Some(id),
        _ => None,
    }
}

fn files(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, usize) {
    let na = RATE as usize / 4;
    let a = tmp(&format!("repeat_{tag}_a.wav"));
    let b = tmp(&format!("repeat_{tag}_b.wav"));
    write_wav(&a, RATE, &dc(na, 0.5));
    write_wav(&b, RATE, &dc(RATE as usize / 4, -0.25));
    (a, b, na)
}

#[test]
fn repeat_one_replays_the_track_seamlessly_until_turned_off() {
    let (a, b, na) = files("seamless");
    let (engine, events) = Engine::new(config()).unwrap();
    engine.set_repeat_one(true);
    engine.load_queue(
        vec![TrackSpec::file("a", &a), TrackSpec::file("b", &b)],
        0,
        true,
    );
    let mut repeats = 0;
    wait_for(&events, Duration::from_secs(20), |ev| {
        if let Some(id) = id_of(ev) {
            assert!(id == "a" || id == "b", "leaked id {id:?}");
        }
        match ev {
            Event::TrackChanged {
                track,
                reason: ChangeReason::Auto,
                ..
            } => {
                assert_eq!(track.id, "a", "repeat one stays on the track");
                repeats += 1;
                repeats >= 3
            }
            _ => false,
        }
    });
    let snap = engine.snapshot();
    assert!(snap.repeat_one);
    assert_eq!(snap.id.as_deref(), Some("a"));
    assert!(snap.has_next, "b is still ahead");
    // Повтор сняли: дальше — следующий трек
    engine.set_repeat_one(false);
    let mut more = 0;
    wait_for(&events, Duration::from_secs(20), |ev| match ev {
        Event::TrackChanged { track, .. } if track.id == "a" => {
            more += 1;
            false
        }
        Event::TrackChanged { track, .. } => track.id == "b",
        _ => false,
    });
    wait_for(&events, Duration::from_secs(20), |ev| {
        matches!(ev, Event::State(PlaybackState::Ended))
    });
    let out = left(&engine.take_capture());
    let plays_a = 1 + repeats + more;
    // Повторы — встык, без тишины и щелчков: все сэмплы a подряд — ровно 0,5
    let span = plays_a * na;
    assert!(out.len() >= span, "{} < {span}", out.len());
    assert!(
        out[..span].iter().all(|&x| x == 0.5),
        "repeats join without gaps"
    );
    assert!(out[span..].iter().any(|&x| x == -0.25), "then b is heard");
}

#[test]
fn next_and_previous_skip_the_repeat() {
    let (a, b, _) = files("skip");
    let (engine, events) = Engine::new(config()).unwrap();
    engine.set_repeat_one(true);
    engine.load_queue(
        vec![TrackSpec::file("a", &a), TrackSpec::file("b", &b)],
        0,
        false,
    );
    wait_for(
        &events,
        Duration::from_secs(10),
        |ev| matches!(ev, Event::TrackChanged { track, .. } if track.id == "a"),
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(engine.snapshot().has_next);
    assert!(!engine.snapshot().has_previous, "a has no history");
    engine.next();
    wait_for(&events, Duration::from_secs(10), |ev| match ev {
        Event::TrackChanged { track, reason, .. } => {
            assert_eq!(track.id, "b", "next goes past the copy of a");
            assert_eq!(*reason, ChangeReason::User);
            true
        }
        _ => false,
    });
    std::thread::sleep(Duration::from_millis(100));
    let snap = engine.snapshot();
    assert!(!snap.has_next, "only the copy of b is ahead");
    assert!(snap.has_previous);
    engine.previous();
    wait_for(&events, Duration::from_secs(10), |ev| match ev {
        Event::TrackChanged { track, .. } => {
            assert_eq!(track.id, "a");
            true
        }
        _ => false,
    });
    // Новая очередь после текущего не стирает повтор
    engine.set_upcoming(vec![TrackSpec::file("c", &b)]);
    std::thread::sleep(Duration::from_millis(100));
    assert!(engine.snapshot().has_next);
    engine.next();
    wait_for(&events, Duration::from_secs(10), |ev| match ev {
        Event::TrackChanged { track, .. } => {
            assert_eq!(track.id, "c");
            true
        }
        _ => false,
    });
}
