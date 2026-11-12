//! Виджеты iPhone и iPad (WidgetKit, расширение `dalywave-widgets` в `gen/apple/Widgets`):
//! «Сейчас играет», «Моя волна» с сохранёнными сочетаниями, «Недавнее», значки экрана блокировки.
//!
//! Виджеты в сеть не ходят и движка не видят: им нужен снимок в общем контейнере App Group
//! `group.rest.daly.wave`. Пишет его Swift приложения (`Sources/dalywave-shell/DalyWaveWidgets.swift`:
//! скачивает и уменьшает обложки, считает цвет фона, просит WidgetKit перерисовать), а сюда — откуда
//! что брать:
//! - что играет — события движка (media.rs, там же, где системная плашка): смена трека, играет
//!   или пауза; после остановки последний трек остаётся на паузе;
//! - недавнее (полка «Продолжить» главной) и сочетания волны — с сервера, с токеном устройства: при
//!   запуске, после входа и при каждом уходе в фон (не чаще раза в 5 минут). Выход — снимок стирается.
//!
//! Кнопки виджетов (пауза, дальше, волна) — App Intents, их выполняет приложение тем же JSON, что
//! у Siri (siri.rs). На iOS 15–16 вместо них ссылки `dalywave://remote/…` и `dalywave://wave?…` —
//! [`open_link`].
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::{
    error::ShellResult,
    media::{Command, NowPlaying, Source},
    siri::{Api, SiriCommand, str_of},
};

/// Недавнее и сочетания обновляются не чаще (кроме входа)
const LIBRARY_EVERY: Duration = Duration::from_secs(5 * 60);
/// Через столько после запуска — первое обновление: сначала интерфейс и звук
const FIRST_REFRESH: Duration = Duration::from_secs(8);
/// Элементов «Недавнего»: большой виджет показывает 6
const RECENT_MAX: usize = 8;

#[derive(Default)]
pub struct Widgets {
    /// Последнее, что играло (для паузы после остановки), и последний отправленный JSON
    now: Mutex<(Option<NowPayload>, String)>,
    library_at: Mutex<Option<Instant>>,
}

/// Что играет — JSON для Swift (`DalyWaveWidgets.nowPlaying:`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct NowPayload {
    title: String,
    artist: String,
    playing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    artwork_url: Option<String>,
}

pub fn init(app: &AppHandle) {
    app.manage(Widgets::default());
    #[cfg(target_os = "ios")]
    {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(FIRST_REFRESH);
            refresh_library(&app, true);
        });
    }
}

/// Событие движка изменило «что играет» (media.rs): смена трека, играет или пауза, остановка.
pub fn now_playing(app: &AppHandle, now: Option<&NowPlaying>) {
    let Some(widgets) = app.try_state::<Widgets>() else {
        return;
    };
    let json = {
        let mut state = widgets.now.lock().unwrap_or_else(|e| e.into_inner());
        let payload = match now {
            Some(n) => NowPayload {
                title: n.title.clone(),
                artist: n.artist.clone(),
                playing: n.playing,
                artwork_url: n.artwork_url.clone(),
            },
            // Остановка: виджет показывает последний трек на паузе
            None => match state.0.clone() {
                Some(last) => NowPayload {
                    playing: false,
                    ..last
                },
                None => return,
            },
        };
        let json = serde_json::to_string(&payload).unwrap_or_default();
        state.0 = Some(payload);
        if state.1 == json {
            return;
        }
        state.1.clone_from(&json);
        json
    };
    swift::now_playing(&json);
}

/// iOS: приложение ушло в фон — виджеты увидят свежее недавнее.
pub fn on_background(app: &AppHandle, background: bool) {
    if background {
        refresh_library(app, false);
    }
}

/// Вход выполнен — недавнее и сочетания нового аккаунта.
pub fn signed_in(app: &AppHandle) {
    refresh_library(app, true);
}

/// Выход: виджеты — к заглушке с кнопкой волны.
pub fn signed_out(app: &AppHandle) {
    if let Some(widgets) = app.try_state::<Widgets>() {
        *widgets.now.lock().unwrap_or_else(|e| e.into_inner()) = (None, String::new());
        *widgets.library_at.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    swift::clear();
}

/// Недавнее и сочетания с сервера → Swift. `force` — без оглядки на последнее обновление.
fn refresh_library(app: &AppHandle, force: bool) {
    if !cfg!(target_os = "ios") {
        return;
    }
    let Some(widgets) = app.try_state::<Widgets>() else {
        return;
    };
    {
        let mut at = widgets.library_at.lock().unwrap_or_else(|e| e.into_inner());
        if !force && at.is_some_and(|t| t.elapsed() < LIBRARY_EVERY) {
            return;
        }
        *at = Some(Instant::now());
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match library(&app).await {
            Ok(body) => swift::library(&body.to_string()),
            Err(e) => {
                log::info!("widgets: library not refreshed: {e}");
                // Повторить при следующем уходе в фон, а не через 5 минут
                if let Some(widgets) = app.try_state::<Widgets>() {
                    *widgets.library_at.lock().unwrap_or_else(|e| e.into_inner()) = None;
                }
            }
        }
    });
}

async fn library(app: &AppHandle) -> ShellResult<Value> {
    let api = Api::new(app)?;
    let home = api.get("/api/home").await?;
    let presets = api.get("/api/wave/presets").await?;
    let recent: Vec<Value> = home["shelves"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| str_of(s, "kind") == "continue")
        .flat_map(|s| s["items"].as_array().cloned().unwrap_or_default())
        .filter_map(|item| recent_item(&api, &item))
        .take(RECENT_MAX)
        .collect();
    let presets: Vec<Value> = presets["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| !str_of(p, "id").is_empty())
        .map(|p| {
            let s = &p["settings"];
            json!({
                "id": str_of(p, "id"),
                "name": str_of(p, "name"),
                "icon": str_of(p, "icon"),
                "mood": s.get("mood"),
                "activity": s.get("activity"),
                "character": s.get("character"),
                "language": s.get("language"),
            })
        })
        .collect();
    log::info!(
        "widgets: {} recent, {} wave presets",
        recent.len(),
        presets.len()
    );
    Ok(json!({ "recent": recent, "presets": presets }))
}

/// Элемент полки «Продолжить» (`ShelfItem`) → «Недавнее»: альбом, плейлист или артист.
fn recent_item(api: &Api, item: &Value) -> Option<Value> {
    let kind = str_of(item, "type");
    let id = str_of(item, "id");
    if id.is_empty() {
        return None;
    }
    let image = |set: &Value, size: &str| set[size].as_str().map(|u| api.absolute(u));
    let (title, subtitle, images, mix, color) = match kind {
        "album" => (
            str_of(item, "title"),
            str_of(&item["artist"], "name"),
            image(&item["image"], "medium")
                .into_iter()
                .collect::<Vec<_>>(),
            false,
            item["palette"][0].as_str(),
        ),
        "artist" => (
            str_of(item, "name"),
            "",
            image(&item["image"], "medium").into_iter().collect(),
            false,
            None,
        ),
        "playlist" => {
            // Как обложка плейлиста в интерфейсе: своя картинка, мозаика из четырёх, одна обложка;
            // микс без обложек — собранная обложка бренда
            let covers = item["covers"].as_array().cloned().unwrap_or_default();
            let own = image(&item["image"], "medium");
            let images: Vec<String> = match own {
                Some(u) => vec![u],
                None if covers.len() >= 4 => covers
                    .iter()
                    .take(4)
                    .filter_map(|c| image(c, "small"))
                    .collect(),
                None => covers
                    .first()
                    .and_then(|c| image(c, "medium"))
                    .into_iter()
                    .collect(),
            };
            let mix = images.is_empty() && str_of(item, "kind") == "mix";
            (str_of(item, "title"), "", images, mix, None)
        }
        _ => return None,
    };
    Some(json!({
        "kind": kind,
        "id": id,
        "title": title,
        "subtitle": subtitle,
        "images": images,
        "mix": mix,
        "color": color,
    }))
}

/// Ссылки виджетов для iOS 15–16 (без кнопок App Intents): `dalywave://wave?mood=…` — волна,
/// `dalywave://remote/toggle` и `…/next` — пульт. true — ссылка разобрана здесь.
pub fn open_link(app: &AppHandle, url: &Url) -> bool {
    match url.host_str() {
        Some("wave") => {
            let get = |k: &str| {
                url.query_pairs()
                    .find(|(key, _)| key == k)
                    .map(|(_, v)| v.into_owned())
            };
            crate::siri::perform(
                app,
                SiriCommand::Wave {
                    mood: get("mood"),
                    activity: get("activity"),
                    character: get("character"),
                    language: get("language"),
                },
            );
            true
        }
        Some("remote") => {
            let command = match url.path() {
                "/toggle" => Command::Toggle,
                "/next" => Command::Next,
                _ => return false,
            };
            crate::player::remote(app, command, Source::System);
            true
        }
        _ => false,
    }
}

/// Swift приложения: класс `DalyWaveWidgets` (DalyWaveWidgets.swift) через рантайм Objective-C.
#[cfg(target_os = "ios")]
mod swift {
    use objc2::msg_send;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyClass;
    use objc2_foundation::NSString;

    fn class() -> Option<&'static AnyClass> {
        AnyClass::get(c"DalyWaveWidgets")
    }

    pub fn now_playing(json: &str) {
        let Some(class) = class() else { return };
        autoreleasepool(|_| {
            let s = NSString::from_str(json);
            // SAFETY: `+[DalyWaveWidgets nowPlaying:]` принимает NSString; Swift сам уходит в свою очередь
            let _: () = unsafe { msg_send![class, nowPlaying: &*s] };
        });
    }

    pub fn library(json: &str) {
        let Some(class) = class() else { return };
        autoreleasepool(|_| {
            let s = NSString::from_str(json);
            // SAFETY: `+[DalyWaveWidgets library:]` принимает NSString
            let _: () = unsafe { msg_send![class, library: &*s] };
        });
    }

    pub fn clear() {
        let Some(class) = class() else { return };
        // SAFETY: `+[DalyWaveWidgets clear]` без аргументов
        autoreleasepool(|_| {
            let _: () = unsafe { msg_send![class, clear] };
        });
    }
}

#[cfg(not(target_os = "ios"))]
mod swift {
    pub fn now_playing(_json: &str) {}
    pub fn library(_json: &str) {}
    pub fn clear() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_playing_json_is_shared_with_swift() {
        let p = NowPayload {
            title: "LOVESONGS".into(),
            artist: "дора".into(),
            playing: true,
            artwork_url: Some("https://wave.daly.rest/img/1.jpg".into()),
        };
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({
                "title": "LOVESONGS",
                "artist": "дора",
                "playing": true,
                "artworkUrl": "https://wave.daly.rest/img/1.jpg"
            })
        );
    }

    fn api() -> Api {
        Api::for_tests("https://wave.daly.rest")
    }

    #[test]
    fn continue_shelf_becomes_recent() {
        let album = json!({
            "type": "album", "id": "a1", "title": "Miss", "artist": { "name": "дора" },
            "image": { "small": "/img/s.jpg", "medium": "/img/m.jpg", "large": "/img/l.jpg" },
            "palette": ["#aabbcc", "#000000"]
        });
        assert_eq!(
            recent_item(&api(), &album).unwrap(),
            json!({
                "kind": "album", "id": "a1", "title": "Miss", "subtitle": "дора",
                "images": ["https://wave.daly.rest/img/m.jpg"], "mix": false, "color": "#aabbcc"
            })
        );
        let cover =
            |n: u8| json!({ "small": format!("/s{n}.jpg"), "medium": format!("/m{n}.jpg") });
        let mosaic = json!({
            "type": "playlist", "id": "p1", "title": "В дорогу", "kind": "user",
            "covers": [cover(1), cover(2), cover(3), cover(4), cover(5)]
        });
        let item = recent_item(&api(), &mosaic).unwrap();
        assert_eq!(item["images"].as_array().unwrap().len(), 4);
        assert_eq!(item["images"][0], "https://wave.daly.rest/s1.jpg");
        let mix = json!({ "type": "playlist", "id": "p2", "title": "Микс дня 1", "kind": "mix", "covers": [] });
        assert_eq!(recent_item(&api(), &mix).unwrap()["mix"], true);
        let track = json!({ "type": "track", "id": "t1", "title": "Трек" });
        assert!(recent_item(&api(), &track).is_none());
    }
}
