//! Проверка умного офлайна без входа: `DALYWAVE_OFFLINE_SMOKE=<тестовый сервер>`
//! (`crates/client/audio/tools/serve.py`). Своя база («smoke»), сервер не нужен:
//!
//! 1. медиатека — набор треков тестового сервера (как будто пришла синхронизацией) с признаками
//!    звучания и набором волны;
//! 2. файлы качаются тем же путём, что настоящие: компьютер — с докачкой и проверкой размера, iPhone —
//!    фоновой сессией URLSession;
//! 3. включается «Без сети», волна без сети подбирает очередь из скачанного, плеер играет её из файлов
//!    (ссылок нет вовсе — `prefer_local`). В журнал — `offline smoke: …`. Громкость — `DW_VOLUME`.

use std::time::{Duration, Instant};

use chrono::Utc;
use dalywave_audio::{SourceSpec, TrackSpec};
use dalywave_core::wave::Session as WaveSession;
use dalywave_protocol::{
    catalog::{ArtistRole, AudioStatus, RecordType},
    sync::{
        SyncAlbum, SyncArtist, SyncDeviceSetItem, SyncLikedTrack, SyncPage, SyncRecording,
        SyncTrack, SyncTrackArtist, SyncTrackFeatures,
    },
    wave::{WaveMood, WaveSettings},
};
use dalywave_store::{FileState, NewFile};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::{Offline, candidates, lock};

pub fn spawn(app: &AppHandle) {
    let Ok(base) = std::env::var("DALYWAVE_OFFLINE_SMOKE") else {
        return;
    };
    if base.is_empty() {
        return;
    }
    let _ = app.state::<Offline>().smoke.set("smoke".into());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(&app, base.trim_end_matches('/')).await {
            log::error!("offline smoke: {e}");
        }
    });
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0dd0_0000_0000_0000_0000_0000_0000_0000 | n)
}

async fn run(app: &AppHandle, base: &str) -> Result<(), String> {
    tokio::time::sleep(Duration::from_secs(3)).await;
    let off = app.state::<Offline>();
    let ctx = off.ctx(app).map_err(|e| e.to_string())?;
    let manifest: Value = off
        .http
        .get(format!("{base}/manifest.json"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    // Демо и два набора без пауз: разные форматы, у каждого трека — свой артист
    let mut items: Vec<(String, String, u64, f32)> = Vec::new();
    for album in manifest["albums"].as_array().into_iter().flatten() {
        if !matches!(
            album["id"].as_str(),
            Some("demo" | "gapless-opus_mp4" | "gapless-mp3_320")
        ) {
            continue;
        }
        for t in album["tracks"].as_array().into_iter().flatten() {
            items.push((
                t["file"].as_str().unwrap_or_default().to_string(),
                format!(
                    "{} · {}",
                    t["title"].as_str().unwrap_or(""),
                    album["format"].as_str().unwrap_or("demo")
                ),
                t["durationMs"].as_u64().unwrap_or(0),
                t["lufs"].as_f64().unwrap_or(-14.0) as f32,
            ));
        }
    }
    let now = Utc::now();
    let mut page = SyncPage {
        cursor: "smoke".into(),
        albums: vec![SyncAlbum {
            id: id(1),
            rev: 1,
            deezer_id: None,
            title: "Тестовые сигналы".into(),
            artist_id: id(100),
            record_type: RecordType::Album,
            release_date: None,
            label: None,
            genres: Vec::new(),
            explicit: false,
            tracks_count: Some(items.len() as i32),
            duration_ms: None,
            image: None,
            palette: None,
        }],
        ..Default::default()
    };
    for (i, (_, title, dur, _)) in items.iter().enumerate() {
        let n = i as u128;
        let artist = id(100 + n % 4);
        if n < 4 {
            page.artists.push(SyncArtist {
                id: artist,
                rev: 1,
                deezer_id: None,
                name: format!("Сигнал {}", n + 1),
                image: None,
            });
        }
        page.tracks.push(SyncTrack {
            id: id(1000 + n),
            rev: 1,
            deezer_id: None,
            title: title.clone(),
            version: String::new(),
            album_id: Some(id(1)),
            disc_number: 1,
            track_position: Some(i as i16 + 1),
            duration_ms: *dur as i32,
            explicit: false,
            isrc: None,
            recording_id: Some(id(2000 + n)),
            artists: vec![SyncTrackArtist {
                artist_id: artist,
                role: ArtistRole::Main,
            }],
        });
        page.recordings.push(SyncRecording {
            id: id(2000 + n),
            rev: 1,
            isrc: None,
            duration_ms: *dur as i32,
            audio_status: AudioStatus::Ready,
            primary_file_id: None,
        });
        page.track_features.push(SyncTrackFeatures {
            recording_id: id(2000 + n),
            rev: 1,
            energy: Some(0.2 + 0.7 * (i as f32 / items.len().max(1) as f32)),
            valence: Some(0.5),
            danceability: Some(0.5),
            bpm: Some(120.0),
            key_camelot: Some("8A".into()),
            voice: Some(0.5),
            language: None,
            approachability: Some(0.5),
            relaxed: None,
            party: None,
        });
        page.device_sets.push(SyncDeviceSetItem {
            track_id: id(1000 + n),
            rev: 1,
            device_id: Uuid::nil(),
            kind: if i % 2 == 0 { "favorite" } else { "fresh" }.into(),
            score: 1.0 - i as f32 / 20.0,
        });
        if i % 2 == 0 {
            page.liked_tracks.push(SyncLikedTrack {
                track_id: id(1000 + n),
                rev: 1,
                liked_at: now,
            });
        }
    }
    ctx.store().apply(&page).map_err(|e| e.to_string())?;
    log::info!(
        "offline smoke: library of {} tracks in the local base",
        items.len()
    );

    // Файлы — тем же путём, что настоящие загрузки
    let started = Instant::now();
    for (i, (file, _, dur, lufs)) in items.iter().enumerate() {
        let track = id(1000 + i as u128);
        if ctx.store().ready_file(track).ok().flatten().is_some() {
            continue;
        }
        let url = format!("{base}/{file}");
        // Размер — из Content-Range ответа на первый байт (тестовый сервер не отвечает на HEAD)
        let size = off
            .http
            .get(&url)
            .header("Range", "bytes=0-0")
            .send()
            .await
            .ok()
            .and_then(|r| {
                r.headers()
                    .get("content-range")?
                    .to_str()
                    .ok()?
                    .rsplit('/')
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
            .unwrap_or(0);
        let ext = file.rsplit('.').next().unwrap_or("bin");
        let rel = format!("smoke/{track}.{ext}");
        let new = NewFile {
            track_id: track,
            recording_id: id(2000 + i as u128),
            file_id: id(3000 + i as u128),
            path: rel.clone(),
            size_bytes: size,
            sha256: None,
            codec: ext.into(),
            container: ext.into(),
            duration_ms: *dur as i64,
            loudness_lufs: Some(*lufs),
            true_peak_db: None,
            waveform: None,
        };
        ctx.store().queue_file(&new).map_err(|e| e.to_string())?;
        #[cfg(target_os = "ios")]
        {
            super::ios_bg::start(
                track,
                &url,
                &dalywave_downloads::part_path(&ctx.abs(&rel)),
                true,
            );
            let _ = ctx
                .store()
                .set_file_state(track, FileState::Downloading, 0, None);
        }
        #[cfg(not(target_os = "ios"))]
        {
            let dest = ctx.abs(&rel);
            let ua = off.user_agent.clone();
            let res = tauri::async_runtime::spawn_blocking(move || {
                let req = dalywave_downloads::Request {
                    url: &url,
                    dest: &dest,
                    size,
                    sha256: None,
                    user_agent: &ua,
                };
                dalywave_downloads::fetch(
                    &req,
                    &std::sync::atomic::AtomicBool::new(false),
                    |_, _| {},
                )
            })
            .await
            .map_err(|e| e.to_string())?;
            match res {
                Ok(n) => {
                    let _ = ctx.store().set_file_state(track, FileState::Ready, n, None);
                }
                Err(e) => log::warn!("offline smoke: {file}: {e}"),
            }
        }
    }
    // iPhone: ждать фоновую сессию (события обрабатывает цикл загрузок)
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let ready = ctx
            .store()
            .files()
            .map(|f| f.iter().filter(|f| f.state == FileState::Ready).count())
            .unwrap_or(0);
        if ready >= items.len() || Instant::now() > deadline {
            log::info!(
                "offline smoke: {ready} of {} files on the device in {:.1} s (verified)",
                items.len(),
                started.elapsed().as_secs_f64()
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let usage = ctx.store().usage().map_err(|e| e.to_string())?;
    log::info!(
        "offline smoke: usage reserve {} B, pinned {} B, cache {} B",
        usage.reserve,
        usage.pinned,
        usage.cache
    );

    // Без сети: волна из скачанного, «Бодрое» — энергичное первым
    let mut s = ctx.settings();
    s.force_offline = true;
    let _ = ctx.store().set_meta(
        "settings.offline",
        serde_json::to_string(&s).ok().as_deref(),
    );
    log::info!(
        "offline smoke: offline mode on, offline_now = {}",
        off.offline_now()
    );
    let cands = candidates(&ctx).map_err(|e| e.to_string())?;
    let mut wave = WaveSession::new(
        WaveSettings {
            mood: WaveMood::Energetic,
            ..Default::default()
        },
        7,
    );
    let picks = wave.next(&cands, 6, Utc::now());
    let tracks = ctx
        .store()
        .tracks(&picks.iter().map(|p| p.track_id).collect::<Vec<_>>())
        .map_err(|e| e.to_string())?;
    for p in &picks {
        let c = cands.iter().find(|c| c.track_id == p.track_id);
        log::info!(
            "offline smoke: wave pick {} — energy {:.2}, {}",
            tracks
                .iter()
                .find(|t| t.id == p.track_id)
                .map_or("?", |t| t.title.as_str()),
            c.and_then(|c| c.energy).unwrap_or(0.0),
            p.reason
        );
    }
    let specs: Vec<TrackSpec> = tracks
        .iter()
        .map(|t| {
            let mut spec = TrackSpec::url(format!("smoke-{}", t.id), "");
            spec.source = SourceSpec::Deferred;
            spec.stream_id = Some(t.id.to_string());
            spec.title = t.title.clone();
            spec.artist = t
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default();
            super::prefer_local(app, spec)
        })
        .collect();
    let local = specs
        .iter()
        .filter(|s| matches!(s.source, SourceSpec::File(_)))
        .count();
    log::info!(
        "offline smoke: {local} of {} queue items play from files",
        specs.len()
    );
    let engine = crate::player::engine(app)
        .await
        .map_err(|e| e.to_string())?;
    engine.load_queue(specs, 0, true);
    lock(&off.waves).insert("smoke".into(), wave);
    Ok(())
}
