//! То, что нужно настоящему клиенту поверх спайка: ссылка истекла посреди трека, ссылка по
//! требованию (очередь волны), замена очереди на ходу, уровень для волны, повторное открытие
//! вывода (смена устройства) с продолжением с той же позиции.

mod common;

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::*;
use dalywave_audio::testkit::{compare_with_concat, render_with};
use dalywave_audio::{
    ChangeReason, Engine, EngineConfig, Event, OutputConfig, Resolved, Resolver, SourceSpec,
    TrackSpec, Transition,
};

const RATE: u32 = 48000;

fn tmp(name: &str) -> PathBuf {
    let dir = testdata().join("tmp");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn capture_cfg(rate: u32) -> EngineConfig {
    EngineConfig {
        output: OutputConfig::Capture { sample_rate: rate },
        normalize: false,
        preload_secs: 3600,
        ..EngineConfig::default()
    }
}

/// Запрос к «хранилищу»: подпись ссылки, начало диапазона, ответ.
type Hit = (String, u64, u16);

/// «Хранилище» с подписанными ссылками, как R2. `?sig=old` истекает посреди трека: первый ответ
/// обрывается на трети файла, дальше 403. Остальные подписи действуют.
struct Storage {
    addr: String,
    hits: Arc<Mutex<Vec<Hit>>>,
}

fn storage() -> Storage {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    let hits: Arc<Mutex<Vec<Hit>>> = Arc::default();
    let dir = testdata();
    let log = hits.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (dir, log) = (dir.clone(), log.clone());
            std::thread::spawn(move || {
                let _ = serve(stream, &dir, &log);
            });
        }
    });
    Storage { addr, hits }
}

fn serve(stream: TcpStream, dir: &Path, log: &Mutex<Vec<Hit>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut out = stream;
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(());
    }
    let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let sig = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("sig="))
        .unwrap_or("")
        .to_string();
    let mut start = 0u64;
    loop {
        let mut h = String::new();
        reader.read_line(&mut h)?;
        let h = h.trim_end().to_ascii_lowercase();
        if h.is_empty() {
            break;
        }
        if let Some(v) = h.strip_prefix("range: bytes=") {
            start = v.split('-').next().unwrap_or("0").parse().unwrap_or(0);
        }
    }
    let mut f = std::fs::File::open(dir.join(path.trim_start_matches('/')))?;
    let size = f.metadata()?.len();
    let expired = sig == "old" && start > 0;
    log.lock()
        .unwrap()
        .push((sig.clone(), start, if expired { 403 } else { 206 }));
    if expired {
        let body = "<Error><Code>AccessDenied</Code><Message>Request has expired</Message></Error>";
        write!(
            out,
            "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
        return Ok(());
    }
    let len = size - start;
    write!(
        out,
        "HTTP/1.1 206 Partial Content\r\nContent-Length: {len}\r\nContent-Range: bytes {start}-{}/{size}\r\nConnection: close\r\n\r\n",
        size - 1
    )?;
    f.seek(SeekFrom::Start(start))?;
    // Старая подпись: соединение рвётся на трети файла, как при обрыве связи
    let send = if sig == "old" { size / 3 } else { len };
    let mut left = send;
    let mut buf = vec![0u8; 16 * 1024];
    while left > 0 {
        let take = (left as usize).min(buf.len());
        let n = f.read(&mut buf[..take])?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        left -= n as u64;
    }
    Ok(())
}

#[test]
fn expired_link_is_refreshed_mid_track() {
    let st = storage();
    let calls = Arc::new(AtomicUsize::new(0));
    let (addr, counter) = (st.addr.clone(), calls.clone());
    let resolver = Resolver::new(move |t: &TrackSpec| {
        counter.fetch_add(1, Ordering::SeqCst);
        let part = t.stream_id.as_deref().ok_or("no stream id")?;
        Ok(Resolved {
            url: format!("{addr}/gapless/flac16_44/{part}.flac?sig=new"),
            ..Resolved::default()
        })
    });
    let tracks: Vec<TrackSpec> = [("part1", "old"), ("part2", "new")]
        .into_iter()
        .map(|(part, sig)| {
            let mut t = TrackSpec::url(
                part,
                format!("{}/gapless/flac16_44/{part}.flac?sig={sig}", st.addr),
            );
            t.stream_id = Some(part.into());
            t
        })
        .collect();
    let cfg = EngineConfig {
        resolver: Some(resolver),
        ..capture_cfg(44100)
    };
    let res = render_with(tracks, cfg, Duration::from_secs(60), |_, _| {}).unwrap();
    let parts: Vec<SourceSpec> = (1..=2)
        .map(|n| SourceSpec::File(part_path("flac16_44", "flac", n)))
        .collect();
    let (diff, _, len) = compare_with_concat(&res.samples, &parts).unwrap();
    let hits = st.hits.lock().unwrap().clone();
    println!("requests: {hits:?}");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "одна новая ссылка");
    assert!(hits.iter().any(|h| h.0 == "old" && h.2 == 403));
    assert!(
        hits.iter().any(|h| h.0 == "new" && h.1 > 0),
        "продолжение с места обрыва"
    );
    assert!(
        diff == 0 && len == 0,
        "выход совпадает с декодированием файлов: {diff} отличий, длина {len:+}"
    );
}

#[test]
fn deferred_link_is_resolved_with_loudness() {
    let st = storage();
    let path = tmp("deferred_dc.wav");
    write_wav(&path, RATE, &vec![0.1; RATE as usize * 2]);
    let addr = st.addr.clone();
    let resolver = Resolver::new(move |t: &TrackSpec| {
        assert_eq!(t.stream_id.as_deref(), Some("track-1"));
        Ok(Resolved {
            url: format!("{addr}/tmp/deferred_dc.wav?sig=new"),
            loudness_lufs: Some(-20.0),
            peak_db: Some(-20.0),
            duration_ms: Some(1000),
        })
    });
    let mut t = TrackSpec::url("entry-1", "");
    t.source = SourceSpec::Deferred;
    t.stream_id = Some("track-1".into());
    let cfg = EngineConfig {
        resolver: Some(resolver),
        normalize: true,
        ..capture_cfg(RATE)
    };
    let mut changed = Vec::new();
    let res = render_with(vec![t], cfg, Duration::from_secs(30), |_, e| {
        if let Event::TrackChanged { track, .. } = e {
            changed.push(track.id.clone());
        }
    })
    .unwrap();
    // −20 LUFS → цель −14: +6 дБ
    let want = 0.1 * 10f32.powf(6.0 / 20.0);
    assert!(
        (res.samples[2000] - want).abs() < 1e-5,
        "{}",
        res.samples[2000]
    );
    assert_eq!(sounding(&left(&res.samples)), RATE as usize);
    assert_eq!(changed, vec!["entry-1".to_string()]);
}

/// Прогон с ручным управлением: `setup` — сразу после загрузки очереди, до первого события.
fn run(
    cfg: EngineConfig,
    tracks: Vec<TrackSpec>,
    setup: impl FnOnce(&Engine),
    mut on_event: impl FnMut(&Engine, &Event),
) -> (Vec<f32>, Vec<Event>) {
    let (engine, events) = Engine::new(cfg).unwrap();
    engine.load_queue(tracks, 0, true);
    setup(&engine);
    let t0 = Instant::now();
    let mut log = Vec::new();
    while t0.elapsed() < Duration::from_secs(60) {
        let Ok(e) = events.recv_timeout(Duration::from_secs(30)) else {
            break;
        };
        on_event(&engine, &e);
        let end = matches!(e, Event::QueueEnded);
        log.push(e);
        if end {
            break;
        }
    }
    std::thread::sleep(Duration::from_millis(50));
    (engine.take_capture(), log)
}

fn left(samples: &[f32]) -> Vec<f32> {
    samples.as_chunks::<2>().0.iter().map(|p| p[0]).collect()
}

/// Длина звучащей части: захват пишет блоками по 1024 кадра, хвост последнего блока — нули.
fn sounding(out: &[f32]) -> usize {
    out.iter().rposition(|&x| x != 0.0).map_or(0, |i| i + 1)
}

#[test]
fn upcoming_tracks_can_be_replaced_while_playing() {
    let dc = |name: &str, frames: usize, level: f32| {
        let p = tmp(name);
        write_wav(&p, RATE, &vec![level; frames * 2]);
        TrackSpec::file(name, p)
    };
    let a = dc("up_a.wav", 48_000, 0.5);
    let b = dc("up_b.wav", 24_000, -0.25);
    let c = dc("up_c.wav", 24_000, 0.75);
    let (out, events) = run(
        capture_cfg(RATE),
        vec![a, b],
        |e| e.set_upcoming(vec![c]),
        |_, _| {},
    );
    let out = left(&out);
    assert!(out[..48_000].iter().all(|&x| x == 0.5));
    assert!(
        out[48_000..72_000].iter().all(|&x| x == 0.75),
        "следом — новый трек, без пауз"
    );
    assert!(out[72_000..].iter().all(|&x| x == 0.0));
    let changes: Vec<(String, ChangeReason)> = events
        .iter()
        .filter_map(|e| match e {
            Event::TrackChanged { track, reason, .. } => Some((track.id.clone(), *reason)),
            _ => None,
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            ("up_a.wav".to_string(), ChangeReason::User),
            ("up_c.wav".to_string(), ChangeReason::Auto)
        ]
    );
}

#[test]
fn levels_are_reported_before_volume() {
    let p = tmp("levels_sine.wav");
    let frames = RATE as usize * 30;
    let mut s = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let v = 0.5 * (std::f32::consts::TAU * 600.0 * i as f32 / RATE as f32).sin();
        s.push(v);
        s.push(v);
    }
    write_wav(&p, RATE, &s);
    let cfg = EngineConfig {
        volume: 0.0,
        ..capture_cfg(RATE)
    };
    let mut levels = Vec::new();
    let (out, _) = run(
        cfg,
        vec![TrackSpec::file("sine", &p)],
        |e| e.set_levels(true),
        |_, e| {
            if let Event::Level(l) = e {
                levels.push(*l);
            }
        },
    );
    println!("{} level events, first {:?}", levels.len(), levels.first());
    assert!(
        out.iter().all(|&x| x == 0.0),
        "громкость 0 — тишина на выходе"
    );
    assert!(
        !levels.is_empty(),
        "уровень приходит и при нулевой громкости"
    );
    for l in &levels {
        assert!((l.rms - 0.5 / 2f32.sqrt()).abs() < 0.02, "{l:?}");
        assert!(l.bands[1] > l.bands[0] && l.bands[1] > l.bands[2], "{l:?}");
    }
}

/// Пила 0…0.9 по номеру кадра: по значению видно, какой кадр трека звучит.
fn ramp_track(name: &str, frames: usize) -> (TrackSpec, impl Fn(f64) -> f64) {
    let p = tmp(name);
    let mut s = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let v = (0.9 * i as f64 / frames as f64) as f32;
        s.push(v);
        s.push(v);
    }
    write_wav(&p, RATE, &s);
    (TrackSpec::file(name, p), move |frame: f64| {
        0.9 * frame / frames as f64
    })
}

/// Где сменился вывод: новая дека входит с подъёма громкости от нуля, и пила проседает.
fn switch_point(out: &[f32]) -> usize {
    (1..out.len())
        .find(|&i| out[i] < out[i - 1] - 1e-3)
        .expect("в выходе нет точки смены вывода")
}

#[test]
fn reopened_output_continues_from_the_same_frame() {
    let frames = RATE as usize * 20;
    let (track, ramp) = ramp_track("reopen_same.wav", frames);
    let mut reopened = false;
    let (out, events) = run(
        capture_cfg(RATE),
        vec![track],
        |_| {},
        |e, ev| {
            if !reopened && matches!(ev, Event::FirstAudio { .. }) {
                reopened = true;
                e.reopen_capture(RATE);
            }
        },
    );
    let out = left(&out);
    let i = switch_point(&out);
    // Позиция ведётся в миллисекундах: новая дека начинает с целой миллисекунды до точки смены
    let resumed = (i as u64 * 1000 / RATE as u64) * RATE as u64 / 1000;
    println!(
        "switch at frame {i}, resumed from {resumed}, total {} of {frames}",
        out.len()
    );
    assert!(i > 0 && (i as u64) - resumed < 48);
    let declick = (RATE / 200) as usize;
    for k in declick + 1..declick + 2000 {
        let want = ramp((resumed as usize + k) as f64) as f32;
        assert_eq!(out[i + k], want, "кадр {k} после смены");
    }
    assert_eq!(sounding(&out), i + frames - resumed as usize);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Output { sample_rate, .. } if *sample_rate == RATE))
    );
}

#[test]
fn reopened_output_with_another_rate_seeks_the_deck() {
    let frames = RATE as usize * 20;
    let (track, ramp) = ramp_track("reopen_rate.wav", frames);
    let new_rate = 44_100u32;
    let mut reopened = false;
    let (out, _) = run(
        capture_cfg(RATE),
        vec![track],
        |_| {},
        |e, ev| {
            if !reopened && matches!(ev, Event::FirstAudio { .. }) {
                reopened = true;
                e.reopen_capture(new_rate);
            }
        },
    );
    let out = left(&out);
    let i = switch_point(&out);
    let resumed = (i as u64 * 1000 / RATE as u64) * RATE as u64 / 1000;
    let ratio = RATE as f64 / new_rate as f64;
    let mut worst = 0f64;
    for k in 2000..4000 {
        let want = ramp(resumed as f64 + k as f64 * ratio);
        worst = worst.max((out[i + k] as f64 - want).abs());
    }
    println!("switch at frame {i}, resumed from {resumed}, worst error after it {worst:.2e}");
    assert!(
        worst < 1e-3,
        "после смены частоты пила продолжается: {worst}"
    );
    // Оставшаяся часть трека — на новой частоте
    let rest = ((frames as u64 - resumed) as f64 / ratio).ceil() as usize;
    let len = sounding(&out);
    assert!(len.abs_diff(i + rest) <= 2, "{len} vs {}", i + rest);
}

#[test]
fn gapless_queue_keeps_working_with_transition_switch() {
    // Смена режима перехода посреди очереди не ломает склейку: кроссфейд → без пауз.
    let dc = |name: &str, frames: usize, level: f32| {
        let p = tmp(name);
        write_wav(&p, RATE, &vec![level; frames * 2]);
        TrackSpec::file(name, p)
    };
    let a = dc("tr_a.wav", 48_000, 0.5);
    let b = dc("tr_b.wav", 24_000, -0.25);
    let cfg = EngineConfig {
        transition: Transition::Crossfade { ms: 500 },
        ..capture_cfg(RATE)
    };
    let (out, _) = run(
        cfg,
        vec![a, b],
        |e| e.set_transition(Transition::Gapless),
        |_, _| {},
    );
    let out = left(&out);
    assert!(out[..48_000].iter().all(|&x| x == 0.5));
    assert!(out[48_000..72_000].iter().all(|&x| x == -0.25));
}
