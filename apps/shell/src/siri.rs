//! Siri и «Команды» (iPhone, iPad): «Включи мою волну» (с настроением или занятием), «Включи
//! <плейлист, альбом, артист> в DalyWave», «Пауза», «Дальше», «Назад», «Нравится».
//!
//! Входы — Swift в Xcode-проекте: App Intents и SiriKit доступны только из Swift и Objective-C.
//! - App Intents (iOS 17+, `gen/apple/Sources/dalywave-shell/DalyWaveIntents.swift`) — в процессе
//!   приложения, фразы App Shortcuts на русском и английском, приложение «Команды». Зовут сюда
//!   через C ABI: `dalywave_siri_perform` (JSON команды) и `dalywave_siri_search` (поиск для
//!   параметра «что включить»).
//! - SiriKit `INPlayMediaIntent` — путь HomePod («Привет, Siri, включи … в DalyWave» на колонке):
//!   расширение `dalywave-siri` (`gen/apple/SiriExtension`) превращает запрос в JSON команды и
//!   отвечает `.handleInApp`; система будит приложение в фоне и зовёт
//!   `application:handleIntent:completionHandler:` — метод добавляется делегату tao здесь, как
//!   быстрое действие в recognize.rs. Звук уходит на колонку по AirPlay 2 (буферизованный вывод).
//!
//! Выполнение. Пауза, дальше, назад, продолжить — сразу движок (как пульт: WebView может спать).
//! Волна и подборки — интерфейс (`siri:command`, как его кнопки: история, докачка волны), а если он
//! не забрал команду за 3 с или приложение в фоне — сама оболочка: очередь с сервера прямо в движок.
//! «Нравится» — запрос к серверу отсюда, интерфейсу — событие для сердечка.
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use dalywave_audio::{SourceSpec, TrackSpec};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    MAIN,
    error::{ShellError, ShellResult},
    media::{Command, Source},
    player::Player,
    settings::Settings,
};

pub const EVENT_COMMAND: &str = "siri:command";
/// Столько ждём, что интерфейс заберёт команду, — дальше включает оболочка
const UI_WAIT: Duration = Duration::from_secs(3);
/// Треков подборки в очередь движка, когда включает оболочка
const NATIVE_QUEUE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Album,
    Artist,
    Playlist,
    Track,
    /// «Мне нравится»
    Liked,
}

/// Команда голосом. JSON — общий со Swift (App Intents, расширение SiriKit) и с интерфейсом.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SiriCommand {
    /// Моя волна; настроение и занятие — как в настройках волны интерфейса
    Wave {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mood: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        activity: Option<String>,
    },
    /// Включить найденное (id на сервере)
    Play {
        kind: MediaKind,
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default)]
        shuffle: bool,
    },
    /// Найти и включить: запрос Siri с HomePod, текст из «Команд»
    Search {
        query: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<MediaKind>,
        #[serde(default)]
        shuffle: bool,
    },
    Pause,
    Resume,
    Next,
    Previous,
    /// «Нравится» — то, что играет
    Like,
    /// Интерфейсу: трек отмечен «Нравится» (сердечко)
    Liked {
        track_id: String,
    },
}

/// Что вернуть Swift: 0 — принято, 2 — не тот JSON, 3 — приложение ещё не запустилось.
const OK: i32 = 0;
const BAD_REQUEST: i32 = 2;
const NOT_READY: i32 = 3;

#[derive(Default)]
pub struct Siri {
    pending: Mutex<Vec<(u64, SiriCommand)>>,
    seq: AtomicU64,
    /// Разрешение Siri и подсказки «Команд» — один раз после входа
    prepared: AtomicBool,
}

static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// При запуске, до `App::run`: состояние и (iOS) приём `INPlayMediaIntent` от SiriKit.
pub fn install(app: &AppHandle) {
    app.manage(Siri::default());
    let _ = APP.set(app.clone());
    #[cfg(target_os = "ios")]
    ios::install();
}

/// Приложение, когда Swift зовёт нас: при запуске самим Siri — подождать, пока поднимется.
fn app_handle() -> Option<AppHandle> {
    for _ in 0..100 {
        if let Some(app) = APP.get() {
            return Some(app.clone());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

/// Выполнить команду голосом.
pub fn perform(app: &AppHandle, cmd: SiriCommand) {
    log::info!("siri: {cmd:?}");
    let remote = |c| crate::player::remote(app, c, Source::System);
    match cmd {
        SiriCommand::Pause => remote(Command::Pause),
        // Нечего продолжать — интерфейс включит волну, как кнопка «играть»
        SiriCommand::Resume => remote(Command::Play),
        SiriCommand::Next => remote(Command::Next),
        SiriCommand::Previous => remote(Command::Previous),
        SiriCommand::Like => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = like_current(&app).await {
                    log::warn!("siri: like: {e}");
                }
            });
        }
        SiriCommand::Search {
            query,
            kind,
            shuffle,
        } => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match resolve(&app, &query, kind).await {
                    Ok(Some(mut found)) => {
                        if let SiriCommand::Play { shuffle: s, .. } = &mut found {
                            *s = shuffle;
                        }
                        deliver(&app, found);
                    }
                    Ok(None) => log::info!("siri: nothing found for «{query}»"),
                    Err(e) => log::warn!("siri: search «{query}»: {e}"),
                }
            });
        }
        cmd @ (SiriCommand::Wave { .. } | SiriCommand::Play { .. }) => deliver(app, cmd),
        SiriCommand::Liked { .. } => {}
    }
}

/// Волну и подборки включает интерфейс; не забрал за 3 с (WebView спит) или приложение в фоне —
/// оболочка сама.
fn deliver(app: &AppHandle, cmd: SiriCommand) {
    let siri = app.state::<Siri>();
    let seq = siri.seq.fetch_add(1, Ordering::Relaxed);
    if app.state::<Player>().in_background() {
        spawn_native(app, cmd);
        return;
    }
    siri.pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((seq, cmd.clone()));
    let _ = app.emit_to(MAIN, EVENT_COMMAND, cmd);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(UI_WAIT);
        let left = {
            let siri = app.state::<Siri>();
            let mut pending = siri.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending
                .iter()
                .position(|(s, _)| *s == seq)
                .map(|i| pending.remove(i).1)
        };
        if let Some(cmd) = left {
            log::info!("siri: the interface did not take the command, playing it here");
            spawn_native(&app, cmd);
        }
    });
}

fn spawn_native(app: &AppHandle, cmd: SiriCommand) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = native(&app, &cmd).await {
            log::warn!("siri: {cmd:?}: {e}");
        }
    });
}

/// Команды Siri, которых интерфейс ещё не видел (и забыть их): при запуске после входа и на каждое
/// `siri:command`. Первый вызов после входа ещё и спрашивает разрешение Siri (iOS один раз
/// покажет окно) и обновляет подсказки фраз «Команд» (плейлисты медиатеки).
#[tauri::command]
pub fn siri_take(app: AppHandle) -> Vec<SiriCommand> {
    let siri = app.state::<Siri>();
    if !siri.prepared.swap(true, Ordering::SeqCst) {
        #[cfg(target_os = "ios")]
        {
            let _ = app.run_on_main_thread(ios::prepare);
        }
    }
    std::mem::take(&mut *siri.pending.lock().unwrap_or_else(|e| e.into_inner()))
        .into_iter()
        .map(|(_, c)| c)
        .collect()
}

// ---- сервер ----

/// Запросы к серверу от имени устройства (токен из связки ключей).
struct Api {
    http: reqwest::Client,
    server: String,
    token: Option<String>,
}

impl Api {
    fn new(app: &AppHandle) -> ShellResult<Api> {
        if crate::offline::offline_now(app) {
            return Err(ShellError::Network("нет сети".into()));
        }
        let server = app.state::<Settings>().server_url();
        let token = crate::auth::load_token(&server)?;
        if token.is_none() {
            return Err(ShellError::Invalid("нужен вход".into()));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(ShellError::internal)?;
        Ok(Api {
            http,
            server,
            token,
        })
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> ShellResult<Value> {
        let req = match &self.token {
            Some(t) => req.bearer_auth(t),
            None => req,
        };
        let res = req
            .send()
            .await
            .map_err(|e| ShellError::Network(e.to_string()))?;
        let status = res.status();
        if !status.is_success() {
            return Err(ShellError::Server {
                status: status.as_u16(),
                message: res.text().await.unwrap_or_default(),
            });
        }
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        res.json()
            .await
            .map_err(|e| ShellError::Network(e.to_string()))
    }

    async fn get(&self, path: &str) -> ShellResult<Value> {
        self.send(self.http.get(format!("{}{path}", self.server)))
            .await
    }

    async fn post(&self, path: &str, body: Value) -> ShellResult<Value> {
        self.send(self.http.post(format!("{}{path}", self.server)).json(&body))
            .await
    }

    async fn put(&self, path: &str) -> ShellResult<Value> {
        self.send(self.http.put(format!("{}{path}", self.server)))
            .await
    }

    fn absolute(&self, url: &str) -> String {
        if url.starts_with('/') {
            format!("{}{url}", self.server)
        } else {
            url.to_string()
        }
    }
}

async fn like_current(app: &AppHandle) -> ShellResult<()> {
    let engine = crate::player::engine(app).await?;
    let track_id = engine
        .snapshot()
        .stream_id
        .ok_or_else(|| ShellError::Invalid("ничего не играет".into()))?;
    Api::new(app)?
        .put(&format!("/api/library/tracks/{track_id}"))
        .await?;
    log::info!("siri: liked {track_id}");
    let _ = app.emit_to(MAIN, EVENT_COMMAND, SiriCommand::Liked { track_id });
    Ok(())
}

/// Строка для сравнения названий: регистр, «ё», знаки препинания и лишние пробелы не важны.
pub fn normalize(s: &str) -> String {
    let lower = s.to_lowercase().replace('ё', "е");
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Насколько название подходит к запросу: 3 — совпало, 2 — начинается с него, 1 — содержит.
fn closeness(title: &str, query: &str) -> u8 {
    let (t, q) = (normalize(title), normalize(query));
    if q.is_empty() {
        0
    } else if t == q {
        3
    } else if t.starts_with(&q) || q.starts_with(&t) && !t.is_empty() {
        2
    } else if t.contains(&q) {
        1
    } else {
        0
    }
}

/// Найденное для поиска «Команд» и для `Search`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub kind: MediaKind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    score: u8,
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// Плейлисты медиатеки, затем каталог: артисты, альбомы, треки. Лучшие совпадения — первыми.
async fn search(app: &AppHandle, query: &str, kind: Option<MediaKind>) -> ShellResult<Vec<Found>> {
    let api = Api::new(app)?;
    let mut out = Vec::new();
    let want = |k: MediaKind| kind.is_none_or(|w| w == k);
    if want(MediaKind::Playlist) {
        let list = api.get("/api/playlists").await?;
        for p in list["items"].as_array().into_iter().flatten() {
            let title = str_of(p, "title");
            let score = closeness(title, query);
            if score > 0 {
                out.push(Found {
                    kind: MediaKind::Playlist,
                    id: str_of(p, "id").to_string(),
                    title: title.to_string(),
                    subtitle: String::new(),
                    score: score + 1,
                });
            }
        }
    }
    let types: Vec<&str> = [
        (MediaKind::Artist, "artist"),
        (MediaKind::Album, "album"),
        (MediaKind::Track, "track"),
    ]
    .iter()
    .filter(|(k, _)| want(*k))
    .map(|(_, t)| *t)
    .collect();
    if !types.is_empty() {
        let res = api
            .send(api.http.get(format!("{}/api/search", api.server)).query(&[
                ("q", query),
                ("types", &types.join(",")),
                ("limit", "5"),
            ]))
            .await?;
        for a in res["artists"].as_array().into_iter().flatten() {
            let name = str_of(a, "name");
            out.push(Found {
                kind: MediaKind::Artist,
                id: str_of(a, "id").to_string(),
                title: name.to_string(),
                subtitle: String::new(),
                score: closeness(name, query),
            });
        }
        for a in res["albums"].as_array().into_iter().flatten() {
            let title = str_of(a, "title");
            out.push(Found {
                kind: MediaKind::Album,
                id: str_of(a, "id").to_string(),
                title: title.to_string(),
                subtitle: str_of(&a["artist"], "name").to_string(),
                score: closeness(title, query),
            });
        }
        for t in res["tracks"].as_array().into_iter().flatten() {
            let title = str_of(t, "title");
            out.push(Found {
                kind: MediaKind::Track,
                id: str_of(t, "id").to_string(),
                title: title.to_string(),
                subtitle: artists(t),
                score: closeness(title, query),
            });
        }
    }
    // Порядок сервера — внутри одной оценки
    out.sort_by_key(|f| std::cmp::Reverse(f.score));
    Ok(out)
}

async fn resolve(
    app: &AppHandle,
    query: &str,
    kind: Option<MediaKind>,
) -> ShellResult<Option<SiriCommand>> {
    if kind == Some(MediaKind::Liked) {
        return Ok(Some(SiriCommand::Play {
            kind: MediaKind::Liked,
            id: "liked".into(),
            title: None,
            shuffle: false,
        }));
    }
    Ok(search(app, query, kind)
        .await?
        .into_iter()
        .next()
        .map(|f| SiriCommand::Play {
            kind: f.kind,
            id: f.id,
            title: Some(f.title),
            shuffle: false,
        }))
}

/// Подсказки фраз «Включи <…> в DalyWave»: плейлисты, сохранённые альбомы, подписки.
async fn suggestions(app: &AppHandle) -> ShellResult<Vec<Found>> {
    let api = Api::new(app)?;
    let mut out = Vec::new();
    let playlists = api.get("/api/playlists").await?;
    for p in playlists["items"].as_array().into_iter().flatten() {
        out.push(Found {
            kind: MediaKind::Playlist,
            id: str_of(p, "id").to_string(),
            title: str_of(p, "title").to_string(),
            subtitle: String::new(),
            score: 0,
        });
    }
    let albums = api.get("/api/library/albums?limit=50").await?;
    for a in albums["items"].as_array().into_iter().flatten() {
        let a = a.get("album").unwrap_or(a);
        out.push(Found {
            kind: MediaKind::Album,
            id: str_of(a, "id").to_string(),
            title: str_of(a, "title").to_string(),
            subtitle: str_of(&a["artist"], "name").to_string(),
            score: 0,
        });
    }
    let artists = api.get("/api/library/artists?limit=50").await?;
    for a in artists["items"].as_array().into_iter().flatten() {
        let a = a.get("artist").unwrap_or(a);
        out.push(Found {
            kind: MediaKind::Artist,
            id: str_of(a, "id").to_string(),
            title: str_of(a, "name").to_string(),
            subtitle: String::new(),
            score: 0,
        });
    }
    out.retain(|f| !f.id.is_empty() && !f.title.is_empty());
    Ok(out)
}

fn artists(track: &Value) -> String {
    track["artists"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| str_of(a, "name"))
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Трек сервера (`Track` API) → очередь движка; ссылку движок спросит сам.
fn track_spec(api: &Api, track: &Value, n: usize) -> Option<TrackSpec> {
    let id = track.get("id")?.as_str()?;
    Some(TrackSpec {
        id: format!("siri-{n}-{id}"),
        source: SourceSpec::Deferred,
        stream_id: Some(id.to_string()),
        title: str_of(track, "title").to_string(),
        artist: artists(track),
        album: str_of(&track["album"], "title").to_string(),
        artwork_url: track["album"]["image"]["large"]
            .as_str()
            .map(|u| api.absolute(u)),
        duration_ms: track.get("durationMs").and_then(Value::as_u64),
        loudness_lufs: None,
        peak_db: None,
        analysis: None,
    })
}

/// Включить без интерфейса: очередь с сервера — прямо в движок (скачанное — из файлов).
async fn native(app: &AppHandle, cmd: &SiriCommand) -> ShellResult<()> {
    let api = Api::new(app)?;
    let (mut tracks, shuffle): (Vec<Value>, bool) = match cmd {
        SiriCommand::Wave { mood, activity } => {
            let mut settings = serde_json::Map::new();
            if let Some(m) = mood {
                settings.insert("mood".into(), json!(m));
            }
            if let Some(a) = activity {
                settings.insert("activity".into(), json!(a));
            }
            let start = api
                .post("/api/wave/sessions", json!({ "settings": settings }))
                .await?;
            (
                start["queue"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|i| i.get("track").cloned())
                    .collect(),
                false,
            )
        }
        SiriCommand::Play {
            kind, id, shuffle, ..
        } => {
            let list = match kind {
                MediaKind::Album => api.get(&format!("/api/albums/{id}")).await?["tracks"].take(),
                MediaKind::Artist => {
                    api.get(&format!("/api/artists/{id}")).await?["topTracks"].take()
                }
                MediaKind::Playlist => Value::Array(
                    api.get(&format!("/api/playlists/{id}")).await?["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|i| i.get("track").cloned())
                        .collect(),
                ),
                MediaKind::Track => json!([api.get(&format!("/api/tracks/{id}")).await?]),
                MediaKind::Liked => Value::Array(
                    api.get("/api/library/tracks?limit=200").await?["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|i| i.get("track").cloned())
                        .collect(),
                ),
            };
            (list.as_array().cloned().unwrap_or_default(), *shuffle)
        }
        _ => return Ok(()),
    };
    if shuffle {
        // Фишер — Йетс на xorshift: для порядка треков хватает
        let mut x = getrandom::u64().unwrap_or(0x9e37_79b9_7f4a_7c15) | 1;
        for i in (1..tracks.len()).rev() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            tracks.swap(i, (x % (i as u64 + 1)) as usize);
        }
    }
    let specs: Vec<TrackSpec> = tracks
        .iter()
        .filter(|t| str_of(t, "audioStatus") != "unavailable")
        .take(NATIVE_QUEUE)
        .enumerate()
        .filter_map(|(n, t)| track_spec(&api, t, n))
        .map(|s| crate::offline::prefer_local(app, s))
        .collect();
    if specs.is_empty() {
        return Err(ShellError::Invalid("нечего играть".into()));
    }
    log::info!("siri: playing {} tracks without the interface", specs.len());
    crate::player::engine(app).await?.load_queue(specs, 0, true);
    Ok(())
}

// ---- Swift (App Intents) → Rust ----

/// JSON из C-строки Swift.
///
/// # Safety
/// `ptr` — NUL-строка, живая на время вызова (или NULL).
unsafe fn json_arg<T: serde::de::DeserializeOwned>(ptr: *const std::ffi::c_char) -> Option<T> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: Swift отдаёт строку из withCString — она жива до конца вызова
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str().ok()?;
    serde_json::from_str(s).ok()
}

fn c_string(v: &impl Serialize) -> *mut std::ffi::c_char {
    let s = serde_json::to_string(v).unwrap_or_else(|_| "[]".into());
    std::ffi::CString::new(s).map_or(std::ptr::null_mut(), std::ffi::CString::into_raw)
}

/// Выполнить команду (JSON [`SiriCommand`]). 0 — принято.
///
/// # Safety
/// `request` — NUL-строка UTF-8 или NULL.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_siri_perform(request: *const std::ffi::c_char) -> i32 {
    // SAFETY: договор вызова — см. выше
    let Some(cmd) = (unsafe { json_arg::<SiriCommand>(request) }) else {
        return BAD_REQUEST;
    };
    let Some(app) = app_handle() else {
        return NOT_READY;
    };
    perform(&app, cmd);
    OK
}

#[derive(Deserialize)]
struct SearchRequest {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    kind: Option<MediaKind>,
}

/// Найти для параметра «что включить»: `{"query": "…"}` — поиск, `{}` — подсказки (медиатека).
/// Ответ — JSON-массив [`Found`]; освободить — `dalywave_siri_free`. Блокирует (сеть).
///
/// # Safety
/// `request` — NUL-строка UTF-8 или NULL.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_siri_search(
    request: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    // SAFETY: договор вызова — см. выше
    let req = unsafe { json_arg::<SearchRequest>(request) }.unwrap_or(SearchRequest {
        query: None,
        kind: None,
    });
    let Some(app) = app_handle() else {
        return c_string(&Vec::<Found>::new());
    };
    let found = tauri::async_runtime::block_on(async {
        match req
            .query
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
        {
            Some(q) => search(&app, q, req.kind).await,
            None => suggestions(&app).await,
        }
    });
    match found {
        Ok(f) => c_string(&f),
        Err(e) => {
            log::warn!("siri: search: {e}");
            c_string(&Vec::<Found>::new())
        }
    }
}

/// Освободить строку из `dalywave_siri_search`.
///
/// # Safety
/// `ptr` — строка из `dalywave_siri_search` (или NULL), освобождается один раз.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dalywave_siri_free(ptr: *mut std::ffi::c_char) {
    if !ptr.is_null() {
        // SAFETY: строка создана CString::into_raw
        drop(unsafe { std::ffi::CString::from_raw(ptr) });
    }
}

/// iOS: `INPlayMediaIntent` от расширения SiriKit (HomePod), разрешение Siri, подсказки «Команд».
#[cfg(target_os = "ios")]
mod ios {
    use std::ffi::{CStr, c_char};

    use block2::RcBlock;
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, Sel};
    use objc2::{ffi, msg_send, sel};
    use objc2_foundation::NSString;

    use super::{SiriCommand, app_handle, perform};

    /// `INPlayMediaIntentResponseCode`
    const SUCCESS: isize = 4;
    const FAILURE: isize = 5;
    /// `INSiriAuthorizationStatusNotDetermined`
    const NOT_DETERMINED: isize = 0;

    type Handler = unsafe extern "C-unwind" fn(
        *mut AnyObject,
        Sel,
        *mut AnyObject,
        *mut AnyObject,
        *mut AnyObject,
    );

    /// Команда из `INPlayMediaIntent`: расширение кладёт её JSON в id первого элемента.
    fn command(intent: &AnyObject) -> Option<SiriCommand> {
        let class = AnyClass::get(c"INPlayMediaIntent")?;
        // SAFETY: свойства INPlayMediaIntent и INMediaItem; главный поток
        unsafe {
            let ours: Bool = msg_send![intent, isKindOfClass: class];
            if !ours.as_bool() {
                return None;
            }
            let items: Option<Retained<NSObject>> = msg_send![intent, mediaItems];
            let item: Option<Retained<NSObject>> = msg_send![&*items?, firstObject];
            let id: Option<Retained<NSString>> = msg_send![&*item?, identifier];
            let mut cmd: SiriCommand = serde_json::from_str(&id?.to_string()).ok()?;
            // «Включи вперемешку»
            let shuffled: Option<Retained<NSObject>> = msg_send![intent, playShuffled];
            if let Some(n) = shuffled {
                let on: Bool = msg_send![&*n, boolValue];
                if let SiriCommand::Play { shuffle, .. } | SiriCommand::Search { shuffle, .. } =
                    &mut cmd
                {
                    *shuffle |= on.as_bool();
                }
            }
            Some(cmd)
        }
    }

    /// `application:handleIntent:completionHandler:` — расширение ответило `.handleInApp`.
    unsafe extern "C-unwind" fn handle_intent(
        _this: *mut AnyObject,
        _sel: Sel,
        _app: *mut AnyObject,
        intent: *mut AnyObject,
        handler: *mut AnyObject,
    ) {
        // SAFETY: система передаёт живой INIntent
        let cmd = unsafe { intent.as_ref() }.and_then(command);
        let code = match (cmd, app_handle()) {
            (Some(cmd), Some(app)) => {
                perform(&app, cmd);
                SUCCESS
            }
            (cmd, _) => {
                log::warn!("siri: intent not handled: {cmd:?}");
                FAILURE
            }
        };
        if handler.is_null() {
            return;
        }
        let Some(class) = AnyClass::get(c"INPlayMediaIntentResponse") else {
            return;
        };
        // SAFETY: INPlayMediaIntentResponse initWithCode:userActivity:; completionHandler —
        // блок `void (^)(INIntentResponse *)` от системы
        unsafe {
            let allocated: Allocated<NSObject> = msg_send![class, alloc];
            let response: Retained<NSObject> = msg_send![
                allocated,
                initWithCode: code,
                userActivity: std::ptr::null::<AnyObject>()
            ];
            let block = handler.cast::<block2::DynBlock<dyn Fn(*mut AnyObject)>>();
            (*block).call((Retained::as_ptr(&response).cast_mut().cast(),));
        }
    }

    fn add(class: &AnyClass, sel: Sel, imp: Imp, types: &CStr) {
        // SAFETY: класс зарегистрирован, сигнатура совпадает с types
        let added = unsafe {
            ffi::class_addMethod(
                (class as *const AnyClass).cast_mut(),
                sel,
                imp,
                types.as_ptr() as *const c_char,
            )
        };
        if !added.as_bool() {
            log::warn!("siri: {} already has {sel}", class.name().to_string_lossy());
        }
    }

    pub fn install() {
        if let Some(delegate) = AnyClass::get(c"AppDelegate") {
            // SAFETY: приведение указателя на функцию к Imp; вызывает её только рантайм ObjC
            let imp: Imp = unsafe { std::mem::transmute::<Handler, Imp>(handle_intent) };
            add(
                delegate,
                sel!(application:handleIntent:completionHandler:),
                imp,
                c"v@:@@@?",
            );
        }
    }

    /// После входа, главный поток: разрешение Siri (SiriKit спросит один раз), «я — музыкальный
    /// сервис с подпиской» для Siri (INMediaUserContext) и подсказки фраз «Команд».
    pub fn prepare() {
        // SAFETY: классы Intents.framework (подключён в project.yml); главный поток
        unsafe {
            if let Some(prefs) = AnyClass::get(c"INPreferences") {
                let status: isize = msg_send![prefs, siriAuthorizationStatus];
                if status == NOT_DETERMINED {
                    let block = RcBlock::new(|status: isize| {
                        log::info!("siri: authorization status {status}");
                    });
                    let _: () = msg_send![prefs, requestSiriAuthorization: &*block];
                } else {
                    log::info!("siri: authorization status {status}");
                }
            }
            if let Some(class) = AnyClass::get(c"INMediaUserContext") {
                let allocated: Allocated<NSObject> = msg_send![class, alloc];
                let context: Retained<NSObject> = msg_send![allocated, init];
                // INMediaUserContextSubscriptionStatusSubscribed
                let _: () = msg_send![&*context, setSubscriptionStatus: 2isize];
                let _: () = msg_send![&*context, becomeCurrent];
            }
            // Swift (DalyWaveIntents.swift): подсказки параметра «что включить» — из медиатеки
            if let Some(bridge) = AnyClass::get(c"DalyWaveShortcuts") {
                let _: () = msg_send![bridge, refresh];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_json_is_shared_with_swift() {
        let wave: SiriCommand = serde_json::from_str(r#"{"action":"wave","mood":"calm"}"#).unwrap();
        assert_eq!(
            wave,
            SiriCommand::Wave {
                mood: Some("calm".into()),
                activity: None
            }
        );
        let play: SiriCommand = serde_json::from_str(
            r#"{"action":"play","kind":"playlist","id":"0190","title":"Дорога"}"#,
        )
        .unwrap();
        assert!(matches!(
            play,
            SiriCommand::Play {
                kind: MediaKind::Playlist,
                shuffle: false,
                ..
            }
        ));
        let search: SiriCommand = serde_json::from_str(
            r#"{"action":"search","query":"Lowborn","kind":"artist","shuffle":true}"#,
        )
        .unwrap();
        assert!(matches!(search, SiriCommand::Search { shuffle: true, .. }));
        assert_eq!(
            serde_json::to_value(SiriCommand::Liked {
                track_id: "t1".into()
            })
            .unwrap(),
            json!({ "action": "liked", "trackId": "t1" })
        );
        assert_eq!(
            serde_json::to_value(SiriCommand::Wave {
                mood: None,
                activity: Some("road".into())
            })
            .unwrap(),
            json!({ "action": "wave", "activity": "road" })
        );
        for a in ["pause", "resume", "next", "previous", "like"] {
            let c: SiriCommand = serde_json::from_value(json!({ "action": a })).unwrap();
            assert_eq!(serde_json::to_value(c).unwrap()["action"], a);
        }
    }

    #[test]
    fn titles_match_spoken_queries() {
        assert_eq!(normalize("  Ёлки — Live!  "), "елки live");
        assert_eq!(closeness("Дорога домой", "дорога домой"), 3);
        assert_eq!(closeness("Дорога домой (2024)", "дорога домой"), 2);
        assert_eq!(closeness("Моя дорога домой", "дорога домой"), 1);
        assert_eq!(closeness("Работа", "дорога"), 0);
        assert_eq!(closeness("Что угодно", ""), 0);
    }
}
