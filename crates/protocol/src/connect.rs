//! Управление между устройствами (этап 3), как Spotify Connect. Каждое приложение и вкладка сайта —
//! устройство со своим id (на приложение — постоянный, на вкладку — на время её жизни). Звук играет одно
//! активное устройство: оно владеет очередью и публикует снимки состояния — при каждом изменении и раз
//! в ~5 с с позицией. Остальные — пульты: показывают снимок (позицию досчитывают по `at`) и шлют
//! команды; сервер пересылает команду активному устройству, а его подтверждение — обратно.
//!
//! Всё идёт по WebSocket `/api/ws` сообщениями `connect.*` (см. [`crate::realtime`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{auth::Platform, catalog::Track, events::ContextType, wave::WaveSettings};

/// Самый длинный id устройства, который принимает сервер.
pub const MAX_DEVICE_ID: usize = 64;
/// Без сообщений от устройства столько секунд — оно «не в сети».
pub const ONLINE_WINDOW_SECS: i64 = 15;
/// Как часто устройство напоминает о себе (`connect.heartbeat`) и активное — публикует позицию.
pub const HEARTBEAT_MS: u32 = 5000;

/// Вид устройства — для значка в списке.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectKind {
    /// Вкладка браузера.
    Web,
    Laptop,
    Desktop,
    Phone,
    Tablet,
    /// Часы: пульт на запястье.
    Watch,
}

names!(ConnectKind {
    Web => "web",
    Laptop => "laptop",
    Desktop => "desktop",
    Phone => "phone",
    Tablet => "tablet",
    Watch => "watch",
});

/// Что устройство умеет — пульт прячет то, чего нет.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectCapabilities {
    /// Громкость меняется из приложения (у Safari на iPhone — только кнопками телефона).
    #[serde(default)]
    pub volume: bool,
    /// Режим переходов «Диджей» (движок приложения).
    #[serde(default)]
    pub dj: bool,
    /// Только пульт: звук здесь не играет (часы), перенести сюда воспроизведение нельзя.
    #[serde(default)]
    pub remote: bool,
}

/// Первое сообщение устройства после `hello` сервера: кто оно. Повторяется после каждого переподключения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectHello {
    /// Постоянный id приложения или id вкладки; до 64 символов.
    pub device_id: String,
    /// Имя для списка. У приложений сервер берёт имя устройства из входа («MacBook Pro Захара»).
    pub name: String,
    pub kind: ConnectKind,
    pub platform: Platform,
    #[serde(default)]
    pub app_version: Option<String>,
    #[serde(default)]
    pub capabilities: ConnectCapabilities,
}

/// Устройство в списке «Где играть».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectDevice {
    pub id: String,
    pub name: String,
    pub kind: ConnectKind,
    pub platform: Platform,
    #[schema(required = true)]
    pub app_version: Option<String>,
    pub capabilities: ConnectCapabilities,
    /// Соединение открыто и устройство напоминало о себе за последние 15 с.
    pub online: bool,
    /// Звук играет здесь (даже если устройство пропало из сети).
    pub active: bool,
    pub last_seen_at: DateTime<Utc>,
}

/// Список устройств: активное — первым, затем в сети, затем недавние.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectDevices {
    pub devices: Vec<ConnectDevice>,
    #[schema(required = true)]
    pub active_device_id: Option<String>,
}

/// Откуда элемент очереди.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueueSource {
    /// Добавлено вручную.
    Manual,
    /// Остаток альбома или плейлиста.
    Context,
    Wave,
}

/// Элемент очереди: `uid` — id элемента у активного устройства (команды ссылаются на него).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub uid: String,
    pub track: Track,
    pub source: QueueSource,
    /// «Почему этот трек» у волны.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Что играет: альбом, плейлист, волна…
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayContext {
    #[serde(rename = "type")]
    pub kind: ContextType,
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
}

/// Режим переходов между треками.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransitionMode {
    Gapless,
    Smooth,
    #[default]
    Smart,
    Dj,
}

/// Трек выбран, но файла на сервере ещё нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PendingStatus {
    Acquiring,
    WaitingForDownloader,
}

/// Повтор: выключен, вся очередь по кругу (альбом, плейлист) или один трек.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

fn full_volume() -> f32 {
    1.0
}

/// Снимок состояния активного устройства. Очередь — окно: ближайшие треки и немного истории.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    #[serde(default)]
    pub current: Option<QueueItem>,
    #[serde(default)]
    pub manual: Vec<QueueItem>,
    #[serde(default)]
    pub upcoming: Vec<QueueItem>,
    #[serde(default)]
    pub history: Vec<QueueItem>,
    #[serde(default)]
    pub context: Option<PlayContext>,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub pending: Option<PendingStatus>,
    /// Позиция на момент `at`.
    #[serde(default)]
    pub position_ms: u32,
    #[serde(default)]
    pub duration_ms: u32,
    /// 0…1
    #[serde(default = "full_volume")]
    pub volume: f32,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub transition: TransitionMode,
    /// Перемешивание очереди альбома или плейлиста.
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub repeat: RepeatMode,
    #[serde(default)]
    pub wave_session_id: Option<Uuid>,
    #[serde(default)]
    pub wave: Option<WaveSettings>,
    /// Форма волны текущего трека, 0…1 — для полосы прокрутки на пульте.
    #[serde(default)]
    pub peaks: Vec<f32>,
    /// Когда снят снимок — время сервера (его ставит сервер при приёме).
    #[serde(default)]
    pub at: Option<DateTime<Utc>>,
}

impl PlaybackState {
    /// Позиция на момент `now`: пока играет, время идёт.
    pub fn position_at(&self, now: DateTime<Utc>) -> u32 {
        let Some(at) = self.at else {
            return self.position_ms;
        };
        if !self.playing || self.pending.is_some() {
            return self.position_ms;
        }
        let gone = (now - at).num_milliseconds().max(0);
        let pos = i64::from(self.position_ms).saturating_add(gone);
        let pos = if self.duration_ms > 0 {
            pos.min(i64::from(self.duration_ms))
        } else {
            pos
        };
        u32::try_from(pos).unwrap_or(u32::MAX)
    }

    /// Тот же снимок, но «на сейчас»: позиция досчитана, `at` = `now`.
    pub fn advanced(mut self, now: DateTime<Utc>) -> Self {
        self.position_ms = self.position_at(now);
        self.at = Some(now);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSeek {
    pub position_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteVolume {
    /// 0…1
    pub volume: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMuted {
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTransition {
    pub mode: TransitionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteShuffle {
    pub shuffle: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRepeat {
    pub mode: RepeatMode,
}

/// Играть список на устройстве: треки — по id (устройство загрузит их само).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemotePlayTracks {
    pub track_ids: Vec<Uuid>,
    #[serde(default)]
    pub index: u32,
    pub context: PlayContext,
    #[serde(default)]
    pub shuffle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWave {
    /// Не задано — текущие настройки устройства.
    #[serde(default)]
    pub settings: Option<WaveSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteQueueItem {
    pub uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTrack {
    pub track: Track,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMove {
    pub from: u32,
    pub to: u32,
}

/// Команда пульта. Поле `action`, параметры — рядом с ним.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "action")]
pub enum RemoteCommand {
    #[serde(rename = "play")]
    Play,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "toggle")]
    Toggle,
    #[serde(rename = "next")]
    Next,
    #[serde(rename = "previous")]
    Previous,
    #[serde(rename = "seek")]
    Seek(RemoteSeek),
    #[serde(rename = "setVolume")]
    SetVolume(RemoteVolume),
    #[serde(rename = "setMuted")]
    SetMuted(RemoteMuted),
    #[serde(rename = "setTransition")]
    SetTransition(RemoteTransition),
    #[serde(rename = "setShuffle")]
    SetShuffle(RemoteShuffle),
    #[serde(rename = "setRepeat")]
    SetRepeat(RemoteRepeat),
    /// «Не нравится» текущему треку — и следующий.
    #[serde(rename = "dislike")]
    Dislike,
    #[serde(rename = "notNow")]
    NotNow,
    #[serde(rename = "moreLikeThis")]
    MoreLikeThis,
    #[serde(rename = "playTracks")]
    PlayTracks(RemotePlayTracks),
    #[serde(rename = "startWave")]
    StartWave(RemoteWave),
    /// Новые настройки идущей волны.
    #[serde(rename = "setWave")]
    SetWave(RemoteWave),
    #[serde(rename = "jumpTo")]
    JumpTo(RemoteQueueItem),
    #[serde(rename = "remove")]
    Remove(RemoteQueueItem),
    #[serde(rename = "playNext")]
    PlayNext(RemoteTrack),
    #[serde(rename = "enqueue")]
    Enqueue(RemoteTrack),
    #[serde(rename = "clearManual")]
    ClearManual,
    /// Перестановка в «добавлено вручную».
    #[serde(rename = "moveManual")]
    MoveManual(RemoteMove),
}

/// Сигнал без данных: `connect.heartbeat`, `connect.claim`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectEmpty {
    /// Необязательная метка — для журнала клиента; сервер её не читает.
    #[serde(default)]
    pub id: Option<String>,
}

/// Активное устройство публикует снимок.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectPublish {
    pub state: Box<PlaybackState>,
}

/// Снимок активного устройства — всем устройствам человека.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectState {
    pub device_id: String,
    pub state: Box<PlaybackState>,
}

/// Команда устройству `to`. Ответ — `connect.ack` с тем же `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSend {
    pub id: String,
    pub to: String,
    pub command: Box<RemoteCommand>,
}

/// Команда от устройства `from`: выполнить и ответить `connect.ack`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectCommand {
    pub id: String,
    pub from: String,
    pub command: Box<RemoteCommand>,
}

/// Подтверждение команды: клиент шлёт его с `to` (кому), сервер доставляет с `from` (от кого).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectAck {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub ok: bool,
    /// Машинная причина: `offline` (устройства нет в сети), `failed`, `unsupported`.
    #[serde(default)]
    pub error: Option<String>,
}

/// «Перенести воспроизведение»: устройство `to` продолжит с того же места с той же очередью.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectTransfer {
    /// Если задан — ответ `connect.ack` с этим `id` (например, `offline`, если устройства нет в сети).
    #[serde(default)]
    pub id: Option<String>,
    pub to: String,
    /// Играть ли после переноса; не задано — как было.
    #[serde(default)]
    pub play: Option<bool>,
}

/// Активным стало другое устройство (или то же после переподключения). Прежнее активное
/// останавливает звук и становится пультом; при переносе `to` берёт `state` и продолжает.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectActive {
    #[schema(required = true)]
    pub device_id: Option<String>,
    /// Кто был активным до этого.
    #[schema(required = true)]
    pub from: Option<String>,
    /// Перенос воспроизведения: новое активное устройство продолжает `state`.
    pub transfer: bool,
    /// Последний снимок, позиция досчитана на момент отправки.
    #[schema(required = true)]
    pub state: Option<Box<PlaybackState>>,
}

// ---------- Подхват на другом устройстве ----------

/// Устройство вывода звука, о котором сообщает `connect.signal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectOutput {
    /// Имя, как его показывает система: «AirPods Захара».
    pub name: String,
    /// `builtin`, `headphones`, `bluetooth`, `airplay`, `usb`, `hdmi`, `other`.
    pub kind: String,
}

impl ConnectOutput {
    /// Наушники — личный вывод: за ними музыка и переходит. Колонки и AirPlay — нет.
    pub fn personal(&self) -> bool {
        matches!(self.kind.as_str(), "headphones" | "bluetooth")
    }
}

/// Что случилось с устройством.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectSignalKind {
    /// Пропал вывод, на который шёл звук (наушники ушли на другое устройство, сели, их сняли).
    OutputLost,
    /// Появился и стал текущим вывод (подключились наушники).
    OutputAdded,
    /// Экран компьютера заблокирован (человек отошёл).
    Locked,
    Unlocked,
    /// Приложение открыли; `output` — наушники, если звук сейчас пошёл бы в них.
    Foreground,
}

names!(ConnectSignalKind {
    OutputLost => "output_lost",
    OutputAdded => "output_added",
    Locked => "locked",
    Unlocked => "unlocked",
    Foreground => "foreground",
});

/// `connect.signal`: повод подхватить музыку на другом устройстве. Сервер сводит сигналы всех устройств
/// человека и решает: перенести, предложить или ничего.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSignal {
    pub signal: ConnectSignalKind,
    #[serde(default)]
    pub output: Option<ConnectOutput>,
    /// У `output_lost`: звук здесь играл, когда вывод пропал.
    #[serde(default)]
    pub playing: bool,
}

/// Почему музыка переходит.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandoffReason {
    /// Наушники с тем же именем ушли с одного устройства на другое.
    Headphones,
    /// Компьютер заблокирован, а к телефону подключили наушники.
    Away,
}

names!(HandoffReason { Headphones => "headphones", Away => "away" });

/// `connect.handoff`: музыку подхватили (`transferred`) или предлагают подхватить на устройстве `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectHandoff {
    pub to: String,
    pub from: String,
    pub reason: HandoffReason,
    /// Имя наушников («AirPods Захара»), если причина — они.
    #[schema(required = true)]
    pub output: Option<String>,
    /// Перенос уже сделан (режим «Сразу»); иначе — предложение «Продолжить здесь?».
    pub transferred: bool,
    /// Продолжить со звуком (на прежнем устройстве музыка играла).
    pub play: bool,
}

/// `GET /api/devices`: устройства и последнее состояние.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectOverview {
    pub devices: Vec<ConnectDevice>,
    #[schema(required = true)]
    pub active_device_id: Option<String>,
    #[schema(required = true)]
    pub state: Option<PlaybackState>,
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    #[test]
    fn commands_are_tagged_by_action() {
        let c: RemoteCommand =
            serde_json::from_str(r#"{"action":"seek","positionMs":1200}"#).unwrap();
        assert_eq!(c, RemoteCommand::Seek(RemoteSeek { position_ms: 1200 }));
        let c: RemoteCommand = serde_json::from_str(r#"{"action":"next"}"#).unwrap();
        assert_eq!(c, RemoteCommand::Next);
        assert_eq!(
            serde_json::to_value(RemoteCommand::SetVolume(RemoteVolume { volume: 0.5 })).unwrap(),
            serde_json::json!({ "action": "setVolume", "volume": 0.5 })
        );
        assert!(serde_json::from_str::<RemoteCommand>(r#"{"action":"explode"}"#).is_err());
    }

    #[test]
    fn shuffle_and_repeat_commands() {
        let c: RemoteCommand =
            serde_json::from_str(r#"{"action":"setShuffle","shuffle":true}"#).unwrap();
        assert_eq!(
            c,
            RemoteCommand::SetShuffle(RemoteShuffle { shuffle: true })
        );
        let c: RemoteCommand =
            serde_json::from_str(r#"{"action":"setRepeat","mode":"one"}"#).unwrap();
        assert_eq!(
            c,
            RemoteCommand::SetRepeat(RemoteRepeat {
                mode: RepeatMode::One
            })
        );
        assert_eq!(
            serde_json::to_value(RemoteCommand::SetRepeat(RemoteRepeat {
                mode: RepeatMode::All
            }))
            .unwrap(),
            serde_json::json!({ "action": "setRepeat", "mode": "all" })
        );
        assert!(
            serde_json::from_str::<RemoteCommand>(r#"{"action":"setRepeat","mode":"twice"}"#)
                .is_err()
        );
    }

    #[test]
    fn old_snapshots_have_no_shuffle_or_repeat() {
        let s: PlaybackState = serde_json::from_str("{}").unwrap();
        assert!(!s.shuffle);
        assert_eq!(s.repeat, RepeatMode::Off);
        let s: PlaybackState = serde_json::from_str(r#"{"shuffle":true,"repeat":"all"}"#).unwrap();
        assert!(s.shuffle);
        assert_eq!(s.repeat, RepeatMode::All);
    }

    #[test]
    fn position_runs_only_while_playing() {
        let at = Utc::now();
        let mut s: PlaybackState = serde_json::from_str("{}").unwrap();
        assert_eq!(s.volume, 1.0);
        s.position_ms = 10_000;
        s.duration_ms = 12_000;
        s.at = Some(at);
        assert_eq!(s.position_at(at + Duration::seconds(1)), 10_000, "пауза");
        s.playing = true;
        assert_eq!(s.position_at(at + Duration::seconds(1)), 11_000);
        assert_eq!(
            s.position_at(at + Duration::seconds(9)),
            12_000,
            "не дальше конца"
        );
        assert_eq!(s.position_at(at - Duration::seconds(5)), 10_000);
        let moved = s.clone().advanced(at + Duration::milliseconds(500));
        assert_eq!(moved.position_ms, 10_500);
        assert_eq!(moved.at, Some(at + Duration::milliseconds(500)));
    }
}
