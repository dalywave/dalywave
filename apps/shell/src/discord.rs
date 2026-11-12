//! Статус в Discord (Rich Presence через локальный IPC Discord): «Слушает DalyWave» — название,
//! исполнители, обложка и прогресс. Только компьютеры; на сайте и телефоне IPC нет.
//!
//! Данные берутся из событий движка, а не из интерфейса: статус верен, даже когда окно закрыто в
//! трей. Обложка — публичный адрес Deezer из `GET /api/share/track/{id}`: картинки нашего
//! сервера требуют входа, Discord их не загрузит. Discord не запущен — ничего не происходит;
//! запустили позже — статус появится сам (повторная попытка раз в 15 с, пока есть что показать).

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use dalywave_audio::{Event, PlaybackState};
use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    error::{ShellError, ShellResult},
    settings::{PRODUCTION_SERVER, Settings},
};

/// Пауза дольше этого — статус убирается: «слушает» на паузе вводит в заблуждение
const PAUSE_CLEAR: Duration = Duration::from_secs(30);
/// Discord принимает не больше 5 обновлений за 20 с — чаще одного раза в 4 с не шлём
const MIN_INTERVAL: Duration = Duration::from_secs(4);
const RETRY: Duration = Duration::from_secs(15);
/// Расхождение позиции с ожидаемой, после которого это перемотка и прогресс надо поправить
const SEEK_DRIFT_MS: u64 = 3000;
/// Значок для маленькой картинки — публичный файл сайта, загружать картинки в Discord не нужно
const LOGO: &str = "https://wave.daly.rest/icon-512.png";

/// Что показать. Обложку и ссылку на трек дозапрашиваем у сервера после смены трека.
#[derive(Clone, Debug, Default)]
struct Track {
    id: Option<String>,
    title: String,
    artist: String,
    album: String,
    duration_ms: Option<u64>,
}

enum Msg {
    Track(Track),
    Playing(bool),
    Position(u64, Option<u64>),
    Idle,
    Card { id: String, card: Card },
    Enabled(bool),
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Card {
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
}

pub struct Discord {
    client_id: Option<String>,
    enabled: AtomicBool,
    connected: std::sync::Arc<AtomicBool>,
    tx: Option<Sender<Msg>>,
    path: Option<PathBuf>,
    /// Обложки и ссылки уже спрошенных треков: при повторе не ходить на сервер
    cards: Mutex<HashMap<String, Card>>,
    /// Трек на сервере, который играет: включили статус посреди трека — обложку спросить сразу
    current: Mutex<Option<String>>,
    token: Mutex<Option<(String, Option<String>)>>,
    http: reqwest::Client,
}

/// Состояние для настроек: `available` — оболочка знает id приложения Discord.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordState {
    available: bool,
    enabled: bool,
    connected: bool,
}

/// id приложения Discord: переменная окружения при запуске, затем при сборке, затем
/// `plugins.discord.clientId` в tauri.conf.json. Это не секрет — его видно в любом клиенте Discord.
fn client_id(app: &AppHandle) -> Option<String> {
    std::env::var("DALYWAVE_DISCORD_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("DALYWAVE_DISCORD_CLIENT_ID").map(str::to_string))
        .or_else(|| {
            app.config()
                .plugins
                .0
                .get("discord")
                .and_then(|v| v.get("clientId"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
}

pub fn init(app: &AppHandle) {
    let client_id = client_id(app);
    let path = app
        .path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("discord.json"));
    let stored = path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Stored>(&b).ok())
        .unwrap_or_default();
    let enabled = stored.enabled.unwrap_or(true);
    let connected = std::sync::Arc::new(AtomicBool::new(false));
    let tx = client_id.clone().map(|id| {
        let (tx, rx) = crossbeam_channel::unbounded();
        let connected = connected.clone();
        let spawned = std::thread::Builder::new()
            .name("discord".into())
            .spawn(move || Worker::new(id, enabled, connected).run(rx));
        if let Err(e) = spawned {
            log::warn!("discord: no worker thread: {e}");
        }
        tx
    });
    if client_id.is_none() {
        log::info!("discord: no application id, status is off");
    }
    let http = reqwest::Client::builder()
        .user_agent(format!(
            "DalyWave/{} ({})",
            app.package_info().version,
            crate::platform::OS
        ))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();
    app.manage(Discord {
        client_id,
        enabled: AtomicBool::new(enabled),
        connected,
        tx,
        path,
        cards: Mutex::new(HashMap::new()),
        current: Mutex::new(None),
        token: Mutex::new(None),
        http,
    });
}

impl Discord {
    fn send(&self, msg: Msg) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(msg);
        }
    }

    fn state(&self) -> DiscordState {
        DiscordState {
            available: self.client_id.is_some(),
            enabled: self.enabled.load(Ordering::Relaxed),
            connected: self.connected.load(Ordering::Relaxed),
        }
    }

    fn token_for(&self, server: &str) -> Option<String> {
        if let Some((s, t)) = self.token.lock().unwrap_or_else(|e| e.into_inner()).clone()
            && s == server
        {
            return t;
        }
        let token = crate::auth::load_token(server).ok().flatten();
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((server.to_string(), token.clone()));
        token
    }
}

/// Событие движка → статус. Вызывается из потока событий плеера, поэтому только отправка в канал.
pub fn on_player_event(app: &AppHandle, ev: &Event) {
    let Some(discord) = app.try_state::<Discord>() else {
        return;
    };
    if discord.tx.is_none() {
        return;
    }
    match ev {
        Event::TrackChanged { track, .. } => {
            let t = Track {
                id: track.stream_id.clone(),
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration_ms: track.duration_ms,
            };
            *discord.current.lock().unwrap_or_else(|e| e.into_inner()) = t.id.clone();
            if let Some(id) = t.id.clone() {
                fetch_card(app, id);
            }
            discord.send(Msg::Track(t));
        }
        Event::State(PlaybackState::Playing) => discord.send(Msg::Playing(true)),
        Event::State(PlaybackState::Paused | PlaybackState::Ended) | Event::QueueEnded => {
            discord.send(Msg::Playing(false))
        }
        Event::State(PlaybackState::Idle) => discord.send(Msg::Idle),
        Event::Position {
            position_ms,
            duration_ms,
            ..
        } => discord.send(Msg::Position(*position_ms, *duration_ms)),
        _ => {}
    }
}

/// Публичная обложка и ссылка на трек — с сервера, один раз на трек.
fn fetch_card(app: &AppHandle, id: String) {
    let discord = app.state::<Discord>();
    if !discord.enabled.load(Ordering::Relaxed) {
        return;
    }
    let cached = discord
        .cards
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&id)
        .cloned();
    if let Some(card) = cached {
        discord.send(Msg::Card { id, card });
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let server = app.state::<Settings>().server_url();
        let token = {
            let app = app.clone();
            let server = server.clone();
            tauri::async_runtime::spawn_blocking(move || app.state::<Discord>().token_for(&server))
                .await
                .ok()
                .flatten()
        };
        let discord = app.state::<Discord>();
        let mut req = discord.http.get(format!("{server}/api/share/track/{id}"));
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        let card = match req.send().await {
            Ok(res) if res.status().is_success() => res.json::<Card>().await.ok(),
            Ok(res) => {
                log::debug!("discord: no card for {id}: {}", res.status());
                None
            }
            Err(e) => {
                log::debug!("discord: no card for {id}: {e}");
                None
            }
        };
        let Some(card) = card else { return };
        discord
            .cards
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.clone(), card.clone());
        discord.send(Msg::Card { id, card });
    });
}

/// Поток Discord: IPC синхронный, а Discord может отвечать медленно — интерфейс и звук не ждут.
struct Worker {
    client_id: String,
    enabled: bool,
    connected: std::sync::Arc<AtomicBool>,
    client: Option<DiscordIpcClient>,
    track: Option<Track>,
    card: Card,
    playing: bool,
    /// Позиция и когда она была такой — по ним считаются отметки начала и конца для Discord
    position: (u64, Instant),
    paused_at: Option<Instant>,
    /// Что-то изменилось и ещё не отправлено (ждём интервала или подключения)
    dirty: bool,
    /// Статус в Discord сейчас показан
    shown: bool,
    last_send: Option<Instant>,
    next_retry: Option<Instant>,
}

impl Worker {
    fn new(client_id: String, enabled: bool, connected: std::sync::Arc<AtomicBool>) -> Self {
        Self {
            client_id,
            enabled,
            connected,
            client: None,
            track: None,
            card: Card::default(),
            playing: false,
            position: (0, Instant::now()),
            paused_at: None,
            dirty: false,
            shown: false,
            last_send: None,
            next_retry: None,
        }
    }

    fn run(mut self, rx: Receiver<Msg>) {
        loop {
            let timeout = self.deadline().map_or(Duration::from_secs(3600), |d| {
                d.saturating_duration_since(Instant::now())
            });
            match rx.recv_timeout(timeout) {
                Ok(msg) => self.handle(msg),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            self.tick();
        }
        self.disconnect();
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Track(t) => {
                if self.track.as_ref().map(|c| &c.id) != Some(&t.id) || t.id.is_none() {
                    self.card = Card::default();
                }
                self.track = Some(t);
                self.position = (0, Instant::now());
                self.dirty = true;
            }
            Msg::Card { id, card } => {
                if self.track.as_ref().and_then(|t| t.id.as_deref()) == Some(id.as_str()) {
                    self.card = card;
                    self.dirty = true;
                }
            }
            Msg::Playing(playing) => {
                if playing != self.playing {
                    self.playing = playing;
                    self.paused_at = (!playing).then(Instant::now);
                    self.dirty = true;
                }
            }
            Msg::Position(position_ms, duration_ms) => {
                if let Some(t) = self.track.as_mut()
                    && duration_ms.is_some()
                    && t.duration_ms != duration_ms
                {
                    t.duration_ms = duration_ms;
                    self.dirty = true;
                }
                if self.expected_position().abs_diff(position_ms) > SEEK_DRIFT_MS {
                    self.dirty = true;
                }
                self.position = (position_ms, Instant::now());
            }
            Msg::Idle => {
                self.track = None;
                self.playing = false;
                self.paused_at = None;
                self.dirty = true;
            }
            Msg::Enabled(enabled) => {
                self.enabled = enabled;
                self.dirty = true;
                if !enabled {
                    self.clear();
                    self.disconnect();
                }
            }
        }
    }

    fn expected_position(&self) -> u64 {
        let (pos, at) = self.position;
        if self.playing {
            pos + at.elapsed().as_millis() as u64
        } else {
            pos
        }
    }

    /// Показывать ли сейчас что-нибудь: включено, есть трек, и он не на паузе дольше 30 с.
    fn wanted(&self) -> bool {
        self.enabled
            && self.track.is_some()
            && self.paused_at.is_none_or(|p| p.elapsed() < PAUSE_CLEAR)
    }

    fn deadline(&self) -> Option<Instant> {
        let mut next: Option<Instant> = None;
        let mut take = |d: Instant| next = Some(next.map_or(d, |n| n.min(d)));
        if self.shown
            && let Some(p) = self.paused_at
        {
            take(p + PAUSE_CLEAR);
        }
        if self.dirty && self.wanted() {
            match (self.client.is_some(), self.last_send, self.next_retry) {
                (true, Some(last), _) => take(last + MIN_INTERVAL),
                (true, None, _) => take(Instant::now()),
                (false, _, Some(retry)) => take(retry),
                (false, _, None) => take(Instant::now()),
            }
        }
        next
    }

    fn tick(&mut self) {
        if !self.enabled {
            return;
        }
        if !self.wanted() {
            if self.shown {
                self.clear();
            }
            self.dirty = false;
            return;
        }
        if !self.dirty || self.last_send.is_some_and(|l| l.elapsed() < MIN_INTERVAL) {
            return;
        }
        if self.client.is_none() && !self.connect() {
            return;
        }
        let ok = self.push();
        self.last_send = Some(Instant::now());
        if ok {
            self.dirty = false;
            self.shown = true;
        } else {
            // Discord закрыли: переподключимся при следующей попытке
            self.disconnect();
            self.next_retry = Some(Instant::now() + RETRY);
        }
    }

    fn connect(&mut self) -> bool {
        if self.next_retry.is_some_and(|r| Instant::now() < r) {
            return false;
        }
        let mut client = DiscordIpcClient::new(&self.client_id);
        match client.connect() {
            Ok(()) => {
                log::info!("discord: connected");
                self.client = Some(client);
                self.connected.store(true, Ordering::Relaxed);
                self.next_retry = None;
                true
            }
            Err(e) => {
                // Discord не запущен — обычное дело, не предупреждение
                log::debug!("discord: not available: {e}");
                self.next_retry = Some(Instant::now() + RETRY);
                false
            }
        }
    }

    fn disconnect(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.close();
            log::info!("discord: disconnected");
        }
        self.connected.store(false, Ordering::Relaxed);
        self.shown = false;
    }

    fn clear(&mut self) {
        if let Some(client) = self.client.as_mut()
            && let Err(e) = client.clear_activity()
        {
            log::debug!("discord: clear failed: {e}");
            self.disconnect();
        }
        self.shown = false;
    }

    fn push(&mut self) -> bool {
        let Some(track) = self.track.clone() else {
            return true;
        };
        let activity = activity(&track, &self.card, self.playing, self.expected_position());
        let Some(client) = self.client.as_mut() else {
            return false;
        };
        match client.set_activity(activity) {
            Ok(()) => true,
            Err(e) => {
                log::debug!("discord: update failed: {e}");
                false
            }
        }
    }
}

/// Discord отвергает строки короче 2 и длиннее 128 символов.
fn field(s: &str) -> String {
    let s: String = s.trim().chars().take(128).collect();
    match s.chars().count() {
        0 => String::new(),
        1 => format!("{s} "),
        _ => s,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn activity<'a>(track: &Track, card: &Card, playing: bool, position_ms: u64) -> Activity<'a> {
    let details = field(&track.title);
    let state = field(&track.artist);
    let album = field(&track.album);
    let cover = card.image.clone().filter(|u| u.starts_with("https://"));
    let mut assets = Assets::new()
        .large_image(cover.clone().unwrap_or_else(|| LOGO.to_string()))
        .large_text(if album.is_empty() {
            details.clone()
        } else {
            album
        });
    if cover.is_some() {
        assets = assets.small_image(LOGO).small_text("DalyWave");
    }
    let mut a = Activity::new()
        .activity_type(ActivityType::Listening)
        .status_display_type(StatusDisplayType::Name)
        .assets(assets);
    if !details.is_empty() {
        a = a.details(details);
    }
    if !state.is_empty() {
        a = a.state(state);
    }
    // На паузе прогресс не идёт: без отметок времени Discord не рисует полосу
    if playing {
        let start = now_ms() - position_ms as i64;
        let mut ts = Timestamps::new().start(start);
        if let Some(d) = track.duration_ms.filter(|d| *d > position_ms) {
            ts = ts.end(start + d as i64);
        }
        a = a.timestamps(ts);
    }
    let url = card
        .url
        .clone()
        .filter(|u| u.starts_with("https://"))
        .or_else(|| {
            track
                .id
                .as_ref()
                .map(|id| format!("{PRODUCTION_SERVER}/track/{id}"))
        });
    if let Some(url) = url {
        a = a.buttons(vec![Button::new("Слушать в DalyWave", url)]);
    }
    a
}

#[tauri::command]
pub fn discord_state(app: AppHandle) -> DiscordState {
    app.state::<Discord>().state()
}

/// Переключатель «Статус в Discord» в настройках; запоминается в `discord.json`.
#[tauri::command]
pub fn discord_set_enabled(app: AppHandle, enabled: bool) -> ShellResult<DiscordState> {
    let discord = app.state::<Discord>();
    discord.enabled.store(enabled, Ordering::Relaxed);
    if let Some(path) = &discord.path {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(ShellError::internal)?;
        }
        let json = serde_json::to_vec_pretty(&Stored {
            enabled: Some(enabled),
        })
        .map_err(ShellError::internal)?;
        std::fs::write(path, json).map_err(ShellError::internal)?;
    }
    discord.send(Msg::Enabled(enabled));
    // Включили посреди трека: обложку спросить сейчас
    if enabled
        && let Some(id) = discord
            .current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    {
        fetch_card(&app, id);
    }
    Ok(discord.state())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_fit_discord_limits() {
        assert_eq!(field(""), "");
        assert_eq!(field("A"), "A ");
        assert_eq!(field("  Float "), "Float");
        assert_eq!(field(&"x".repeat(300)).chars().count(), 128);
    }

    #[test]
    fn activity_shows_progress_only_while_playing() {
        let track = Track {
            id: Some("0193f0c2-7a1e-7c3a-9b1d-2f4e5a6b7c8d".into()),
            title: "Float".into(),
            artist: "Call Me Karizma".into(),
            album: "Float".into(),
            duration_ms: Some(169_000),
        };
        let card = Card {
            image: Some("https://cdn-images.dzcdn.net/images/cover/x/1000x1000.jpg".into()),
            url: Some("https://wave.daly.rest/track/0193f0c2-7a1e-7c3a-9b1d-2f4e5a6b7c8d".into()),
        };
        let playing = serde_json::to_value(activity(&track, &card, true, 62_000)).unwrap();
        assert_eq!(playing["type"], 2);
        assert_eq!(playing["details"], "Float");
        assert_eq!(playing["state"], "Call Me Karizma");
        assert_eq!(
            playing["assets"]["large_image"],
            card.image.clone().unwrap()
        );
        assert_eq!(playing["assets"]["small_image"], LOGO);
        let ts = &playing["timestamps"];
        assert_eq!(
            ts["end"].as_i64().unwrap() - ts["start"].as_i64().unwrap(),
            169_000
        );
        assert_eq!(playing["buttons"][0]["url"], card.url.clone().unwrap());

        let paused =
            serde_json::to_value(activity(&track, &Card::default(), false, 62_000)).unwrap();
        assert!(paused.get("timestamps").is_none());
        assert_eq!(paused["assets"]["large_image"], LOGO);
        assert!(paused["assets"].get("small_image").is_none());
        assert_eq!(
            paused["buttons"][0]["url"],
            "https://wave.daly.rest/track/0193f0c2-7a1e-7c3a-9b1d-2f4e5a6b7c8d"
        );
    }
}
