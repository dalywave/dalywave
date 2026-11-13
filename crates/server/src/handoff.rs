//! Подхват на другом устройстве: музыка играет на Mac, человек надевает AirPods и уходит с iPhone —
//! музыка переходит на iPhone с того же места. И обратно.
//!
//! Что реально видно устройствам:
//! - компьютер (CoreAudio) — пропал вывод, на который шёл звук: AirPods ушли на телефон, сели или их
//!   выключили; появился и стал текущим новый (наушники подключились к компьютеру); экран заблокирован;
//! - iPhone и iPad (AVAudioSession) — смена маршрута, но только пока приложение живо: на переднем плане
//!   или играет. Спящее приложение iOS ничего не узнаёт и звук само не включит, поэтому оно сообщает
//!   ещё и об открытии — с наушниками, в которые пошёл бы звук.
//!
//! Устройства шлют сигналы `connect.signal`, сервер держит их в памяти полчаса и сводит по правилам
//! ([`decide`], чистая функция):
//! 1. «Наушники ушли»: у активного устройства пропали наушники, когда играла музыка, а другое устройство
//!    того же человека сообщило, что к нему подключились наушники с тем же именем (в пределах
//!    [`PAIR_WINDOW`]; если его открыли позже — [`RETURN_WINDOW`]). Совпадение по имени — сильный
//!    признак: в режиме «Сразу» музыка переходит сама, иначе — «Продолжить здесь?».
//! 2. «Отошёл от компьютера»: активен компьютер, его экран заблокирован дольше [`AWAY_AFTER`], а к
//!    телефону подключили наушники (или его открыли с наушниками). Признак слабее (на компьютере музыку
//!    могут слушать другие), поэтому только предложение — и в режиме «Сразу».
//!
//! Одно и то же решение не повторяется [`REPEAT_AFTER`]. Каждое решение — в журнале админки с причиной.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use dalywave_protocol::{
    connect::{
        ConnectHandoff, ConnectKind, ConnectSignal, ConnectSignalKind, ConnectTransfer,
        HandoffReason,
    },
    realtime::ServerEvent,
    settings::HandoffMode,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit,
    connect::{self, Link},
    error::AppResult,
    realtime::{self, Audience},
    state::AppState,
};

/// Наушники пропали на одном устройстве и появились на другом — в пределах этого (переключение AirPods
/// занимает секунды, выход из зоны Bluetooth — до минуты).
pub const PAIR_WINDOW: Duration = Duration::from_secs(90);
/// Приложение открыли позже (iPhone спал в кармане) — наушники всё ещё «те же», если прошло не больше.
pub const RETURN_WINDOW: Duration = Duration::from_secs(20 * 60);
/// Экран компьютера заблокирован дольше — человек отошёл.
pub const AWAY_AFTER: Duration = Duration::from_secs(2 * 60);
/// То же решение для того же устройства не повторяется раньше.
pub const REPEAT_AFTER: Duration = Duration::from_secs(2 * 60);
/// Сколько помнить сигналы.
const KEEP: Duration = Duration::from_secs(30 * 60);
const MAX_SIGNALS: usize = 64;

/// Сигнал устройства, как его помнит сервер.
#[derive(Debug, Clone)]
pub struct Signal {
    pub device: String,
    pub kind: ConnectKind,
    pub signal: ConnectSignalKind,
    /// Наушники: имя для сравнения ([`same_name`]) и для людей. Колонки и AirPlay не в счёт.
    pub output: Option<String>,
    pub playing: bool,
    pub at: Instant,
}

impl Signal {
    fn output_key(&self) -> Option<String> {
        self.output.as_deref().map(name_key)
    }
}

/// Устройство, куда может перейти музыка.
#[derive(Debug, Clone)]
pub struct Target {
    pub id: String,
    pub kind: ConnectKind,
    pub online: bool,
    /// Только пульт (часы)
    pub remote: bool,
}

/// Всё, что нужно правилам.
#[derive(Debug, Clone)]
pub struct Facts<'a> {
    pub mode: HandoffMode,
    /// Где играет музыка
    pub active: Option<&'a str>,
    /// У активного есть трек (иначе переносить нечего)
    pub has_track: bool,
    pub devices: &'a [Target],
    /// Прежние сигналы, старые первыми (без нового)
    pub history: &'a [Signal],
    pub now: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub to: String,
    pub from: String,
    pub reason: HandoffReason,
    /// Имя наушников для людей
    pub output: Option<String>,
    /// Перенести сразу; иначе — предложить
    pub transfer: bool,
    pub play: bool,
}

/// Имя наушников для сравнения: macOS и iOS пишут одно имя Bluetooth, но кавычки, регистр и пробелы
/// могут отличаться — сравниваются только буквы и цифры.
pub fn name_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn computer(kind: ConnectKind) -> bool {
    matches!(kind, ConnectKind::Laptop | ConnectKind::Desktop)
}

fn handheld(kind: ConnectKind) -> bool {
    matches!(kind, ConnectKind::Phone | ConnectKind::Tablet)
}

/// Устройство может подхватить музыку: в сети, звук играет там (не часы и не вкладка — вкладка
/// не знает своих наушников).
fn can_take(f: &Facts, id: &str) -> bool {
    f.devices.iter().any(|d| {
        d.id == id
            && d.online
            && !d.remote
            && !matches!(d.kind, ConnectKind::Web | ConnectKind::Watch)
    })
}

fn age(f: &Facts, s: &Signal) -> Duration {
    f.now.saturating_duration_since(s.at)
}

/// Последний сигнал устройства об этих наушниках.
fn last_about<'a>(f: &'a Facts, device: &str, key: &str) -> Option<&'a Signal> {
    f.history
        .iter()
        .rev()
        .find(|s| s.device == device && s.output_key().as_deref() == Some(key))
}

/// Решение по новому сигналу: перенести, предложить или ничего.
pub fn decide(f: &Facts, new: &Signal) -> Option<Decision> {
    if f.mode == HandoffMode::Off || !f.has_track {
        return None;
    }
    let active = f.active?;
    match new.signal {
        ConnectSignalKind::OutputAdded | ConnectSignalKind::Foreground if new.device != active => {
            let key = new.output_key()?;
            headphones_arrived(f, active, new, &key).or_else(|| away(f, active, new))
        }
        ConnectSignalKind::OutputLost if new.device == active && new.playing => {
            headphones_left(f, active, new)
        }
        _ => None,
    }
}

/// Правило 1, порядок «пропали у активного → появились здесь».
fn headphones_arrived(f: &Facts, active: &str, new: &Signal, key: &str) -> Option<Decision> {
    let window = match new.signal {
        ConnectSignalKind::OutputAdded => PAIR_WINDOW,
        _ => RETURN_WINDOW,
    };
    let lost = last_about(f, active, key)?;
    let fits =
        lost.signal == ConnectSignalKind::OutputLost && lost.playing && age(f, lost) <= window;
    if !fits || !can_take(f, &new.device) {
        return None;
    }
    Some(Decision {
        to: new.device.clone(),
        from: active.to_string(),
        reason: HandoffReason::Headphones,
        output: new.output.clone(),
        transfer: f.mode == HandoffMode::Auto,
        play: true,
    })
}

/// Правило 1, порядок «появились на другом → пропали у активного» (сеть доставила иначе).
fn headphones_left(f: &Facts, active: &str, new: &Signal) -> Option<Decision> {
    let key = new.output_key()?;
    let arrived = f.history.iter().rev().find(|s| {
        s.device != active
            && matches!(
                s.signal,
                ConnectSignalKind::OutputAdded | ConnectSignalKind::Foreground
            )
            && s.output_key().as_deref() == Some(key.as_str())
            && age(f, s) <= PAIR_WINDOW
    })?;
    // С тех пор наушники с того устройства ушли — оно не в счёт
    let still = last_about(f, &arrived.device, &key)
        .is_some_and(|s| s.signal != ConnectSignalKind::OutputLost);
    if !still || !can_take(f, &arrived.device) {
        return None;
    }
    Some(Decision {
        to: arrived.device.clone(),
        from: active.to_string(),
        reason: HandoffReason::Headphones,
        output: arrived.output.clone(),
        transfer: f.mode == HandoffMode::Auto,
        play: true,
    })
}

/// Правило 2: компьютер заблокирован давно, а телефон — с наушниками.
fn away(f: &Facts, active: &str, new: &Signal) -> Option<Decision> {
    let from = f.devices.iter().find(|d| d.id == active)?;
    if !computer(from.kind) || !handheld(new.kind) || !can_take(f, &new.device) {
        return None;
    }
    let lock = f.history.iter().rev().find(|s| {
        s.device == active
            && matches!(
                s.signal,
                ConnectSignalKind::Locked | ConnectSignalKind::Unlocked
            )
    })?;
    if lock.signal != ConnectSignalKind::Locked || age(f, lock) < AWAY_AFTER {
        return None;
    }
    Some(Decision {
        to: new.device.clone(),
        from: active.to_string(),
        reason: HandoffReason::Away,
        output: new.output.clone(),
        transfer: false,
        play: true,
    })
}

// ---------- Память сигналов ----------

#[derive(Default)]
struct UserSignals {
    signals: VecDeque<Signal>,
    /// Последнее решение: куда и когда
    last: Option<(String, Instant)>,
}

/// Недавние сигналы устройств каждого человека. Все соединения — в процессе API, поэтому память
/// процесса; после перезапуска сигналы просто копятся заново.
#[derive(Default)]
pub struct Memory {
    users: Mutex<HashMap<Uuid, UserSignals>>,
}

impl Memory {
    /// Запомнить сигнал; вернуть прежние (старые первыми, без устаревших).
    pub fn remember(&self, user: Uuid, signal: Signal) -> Vec<Signal> {
        let mut users = self.users.lock().unwrap_or_else(|e| e.into_inner());
        let now = signal.at;
        // Заодно забыть тех, кто давно молчит
        users.retain(|_, u| {
            u.signals
                .back()
                .is_some_and(|s| now.saturating_duration_since(s.at) <= KEEP)
                || u.last
                    .as_ref()
                    .is_some_and(|(_, at)| now.saturating_duration_since(*at) <= KEEP)
        });
        let entry = users.entry(user).or_default();
        entry
            .signals
            .retain(|s| now.saturating_duration_since(s.at) <= KEEP);
        let before: Vec<Signal> = entry.signals.iter().cloned().collect();
        entry.signals.push_back(signal);
        while entry.signals.len() > MAX_SIGNALS {
            entry.signals.pop_front();
        }
        before
    }

    /// Решение новое (то же устройство не получало его недавно)? Да — запомнить.
    pub fn fresh(&self, user: Uuid, decision: &Decision, now: Instant) -> bool {
        let mut users = self.users.lock().unwrap_or_else(|e| e.into_inner());
        let entry = users.entry(user).or_default();
        if let Some((to, at)) = &entry.last
            && *to == decision.to
            && now.saturating_duration_since(*at) < REPEAT_AFTER
        {
            return false;
        }
        entry.last = Some((decision.to.clone(), now));
        true
    }
}

// ---------- Сигнал от устройства ----------

/// `connect.signal`: запомнить и, если правила сошлись, перенести музыку или предложить перенос.
pub async fn signal(
    st: &AppState,
    user: Uuid,
    link: &Link,
    s: ConnectSignal,
) -> AppResult<Option<ServerEvent>> {
    let Some(device) = link.device.clone().filter(|_| !link.remote) else {
        return Ok(None);
    };
    let list = connect::devices(&st.db, user).await?;
    let Some(me) = list.devices.iter().find(|d| d.id == device) else {
        return Ok(None);
    };
    let now = Instant::now();
    let new = Signal {
        device: device.clone(),
        kind: me.kind,
        signal: s.signal,
        output: s
            .output
            .filter(|o| o.personal())
            .map(|o| o.name.trim().chars().take(80).collect::<String>())
            .filter(|n| !name_key(n).is_empty()),
        playing: s.playing,
        at: now,
    };
    tracing::debug!(%user, device = %new.device, signal = %new.signal, "handoff signal");
    let history = st.handoff.remember(user, new.clone());
    let settings = crate::routes::settings::of(&st.db, user).await?;
    if settings.handoff == HandoffMode::Off {
        return Ok(None);
    }
    let (active, state) = connect::playback(&st.db, user).await?;
    let targets: Vec<Target> = list
        .devices
        .iter()
        .map(|d| Target {
            id: d.id.clone(),
            kind: d.kind,
            online: d.online,
            remote: d.capabilities.remote,
        })
        .collect();
    let facts = Facts {
        mode: settings.handoff,
        active: active.as_deref(),
        has_track: state.as_ref().is_some_and(|s| s.current.is_some()),
        devices: &targets,
        history: &history,
        now,
    };
    let Some(d) = decide(&facts, &new) else {
        return Ok(None);
    };
    if !st.handoff.fresh(user, &d, now) {
        return Ok(None);
    }
    let name_of = |id: &str| {
        list.devices
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.name.clone())
            .unwrap_or_else(|| id.to_string())
    };
    tracing::info!(%user, to = %d.to, from = %d.from, reason = %d.reason, transfer = d.transfer, "handoff");
    let journal = audit::record(
        &st.db,
        Some(user),
        audit::HANDOFF,
        Some(name_of(&d.to)),
        json!({
            "decision": if d.transfer { "transfer" } else { "suggest" },
            "reason": d.reason.as_str(),
            "from": name_of(&d.from),
            "to": name_of(&d.to),
            "output": d.output,
            "mode": settings.handoff.as_str(),
        }),
    )
    .await;
    if let Err(e) = journal {
        tracing::warn!(error = %e, "handoff journal failed");
    }
    if d.transfer {
        connect::transfer(
            st,
            user,
            ConnectTransfer {
                id: None,
                to: d.to.clone(),
                play: Some(d.play),
            },
        )
        .await?;
    }
    // Перенос видят оба устройства (тост «музыка перешла»), предложение — только то, куда звать
    let to = if d.transfer {
        Audience::User(user)
    } else {
        Audience::Device {
            user,
            device: d.to.clone(),
        }
    };
    realtime::deliver(
        &st.hub,
        to,
        ServerEvent::ConnectHandoff(ConnectHandoff {
            to: d.to,
            from: d.from,
            reason: d.reason,
            output: d.output,
            transferred: d.transfer,
            play: d.play,
        }),
    );
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: &str = "mac";
    const PHONE: &str = "phone";

    fn devices() -> Vec<Target> {
        vec![
            Target {
                id: MAC.into(),
                kind: ConnectKind::Laptop,
                online: true,
                remote: false,
            },
            Target {
                id: PHONE.into(),
                kind: ConnectKind::Phone,
                online: true,
                remote: false,
            },
            Target {
                id: "watch".into(),
                kind: ConnectKind::Watch,
                online: true,
                remote: true,
            },
            Target {
                id: "tab".into(),
                kind: ConnectKind::Web,
                online: true,
                remote: false,
            },
        ]
    }

    struct World {
        start: Instant,
        devices: Vec<Target>,
        history: Vec<Signal>,
        mode: HandoffMode,
        active: &'static str,
        has_track: bool,
    }

    impl World {
        fn new() -> Self {
            Self {
                start: Instant::now(),
                devices: devices(),
                history: Vec::new(),
                mode: HandoffMode::Suggest,
                active: MAC,
                has_track: true,
            }
        }

        fn at(&self, secs: u64) -> Instant {
            self.start + Duration::from_secs(secs)
        }

        fn signal(
            &self,
            secs: u64,
            device: &str,
            signal: ConnectSignalKind,
            output: Option<&str>,
            playing: bool,
        ) -> Signal {
            let kind = self
                .devices
                .iter()
                .find(|d| d.id == device)
                .map_or(ConnectKind::Phone, |d| d.kind);
            Signal {
                device: device.into(),
                kind,
                signal,
                output: output.map(str::to_string),
                playing,
                at: self.at(secs),
            }
        }

        fn push(&mut self, s: Signal) {
            self.history.push(s);
        }

        /// Решение по новому сигналу; он же уходит в историю.
        fn send(&mut self, s: Signal) -> Option<Decision> {
            let facts = Facts {
                mode: self.mode,
                active: Some(self.active),
                has_track: self.has_track,
                devices: &self.devices,
                history: &self.history,
                now: s.at,
            };
            let d = decide(&facts, &s);
            self.history.push(s);
            d
        }
    }

    use ConnectSignalKind::*;

    #[test]
    fn headphones_moving_from_mac_to_phone_are_followed() {
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        let d = w
            .send(w.signal(8, PHONE, OutputAdded, Some("AirPods  захара"), false))
            .expect("наушники те же");
        assert_eq!(
            d,
            Decision {
                to: PHONE.into(),
                from: MAC.into(),
                reason: HandoffReason::Headphones,
                output: Some("AirPods  захара".into()),
                transfer: false,
                play: true,
            }
        );
        // «Сразу» — перенос без вопроса
        let mut w = World::new();
        w.mode = HandoffMode::Auto;
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        let d = w
            .send(w.signal(8, PHONE, OutputAdded, Some("AirPods Захара"), false))
            .unwrap();
        assert!(d.transfer);
    }

    #[test]
    fn order_of_signals_does_not_matter() {
        let mut w = World::new();
        w.mode = HandoffMode::Auto;
        w.push(w.signal(0, PHONE, OutputAdded, Some("AirPods Захара"), false));
        let d = w
            .send(w.signal(3, MAC, OutputLost, Some("AirPods Захара"), true))
            .expect("телефон сообщил раньше");
        assert_eq!((d.to.as_str(), d.transfer), (PHONE, true));
        // Но если с телефона наушники с тех пор ушли — нет
        let mut w = World::new();
        w.push(w.signal(0, PHONE, OutputAdded, Some("AirPods Захара"), false));
        w.push(w.signal(2, PHONE, OutputLost, Some("AirPods Захара"), false));
        assert_eq!(
            w.send(w.signal(3, MAC, OutputLost, Some("AirPods Захара"), true)),
            None
        );
    }

    #[test]
    fn phone_opened_later_with_the_same_headphones_continues() {
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        // Через 5 минут достали iPhone и открыли приложение
        let d = w.send(w.signal(300, PHONE, Foreground, Some("AirPods Захара"), false));
        assert_eq!(d.map(|d| d.reason), Some(HandoffReason::Headphones));
        // Через полчаса — уже нет: это другая история
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        assert_eq!(
            w.send(w.signal(1900, PHONE, Foreground, Some("AirPods Захара"), false)),
            None
        );
        // А подключение (а не открытие) ждёт не дольше полутора минут
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        assert_eq!(
            w.send(w.signal(200, PHONE, OutputAdded, Some("AirPods Захара"), false)),
            None
        );
    }

    #[test]
    fn false_alarms_are_ignored() {
        // Другие наушники
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        assert_eq!(
            w.send(w.signal(5, PHONE, OutputAdded, Some("Sony WH-1000XM5"), false)),
            None
        );
        // На Mac музыка стояла на паузе — переносить нечего
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), false));
        assert_eq!(
            w.send(w.signal(5, PHONE, OutputAdded, Some("AirPods Захара"), false)),
            None
        );
        // Наушники вернулись на Mac раньше, чем телефон их увидел
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        w.push(w.signal(4, MAC, OutputAdded, Some("AirPods Захара"), false));
        assert_eq!(
            w.send(w.signal(6, PHONE, OutputAdded, Some("AirPods Захара"), false)),
            None
        );
        // Выключено, или трека нет
        let mut w = World::new();
        w.mode = HandoffMode::Off;
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        assert_eq!(
            w.send(w.signal(5, PHONE, OutputAdded, Some("AirPods Захара"), false)),
            None
        );
        let mut w = World::new();
        w.has_track = false;
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        assert_eq!(
            w.send(w.signal(5, PHONE, OutputAdded, Some("AirPods Захара"), false)),
            None
        );
        // Сигнал без наушников (колонка)
        let mut w = World::new();
        w.push(w.signal(0, MAC, OutputLost, Some("AirPods Захара"), true));
        assert_eq!(w.send(w.signal(5, PHONE, Foreground, None, false)), None);
    }

    #[test]
    fn music_never_goes_to_a_watch_a_tab_or_an_offline_device() {
        for (id, online) in [("watch", true), ("tab", true), (PHONE, false)] {
            let mut w = World::new();
            for d in &mut w.devices {
                if d.id == PHONE {
                    d.online = online;
                }
            }
            w.push(w.signal(0, MAC, OutputLost, Some("AirPods"), true));
            assert_eq!(
                w.send(w.signal(5, id, OutputAdded, Some("AirPods"), false)),
                None,
                "{id}"
            );
        }
    }

    #[test]
    fn back_from_phone_to_mac() {
        let mut w = World::new();
        w.active = PHONE;
        w.push(w.signal(0, PHONE, OutputLost, Some("AirPods Захара"), true));
        let d = w
            .send(w.signal(4, MAC, OutputAdded, Some("AirPods Захара"), false))
            .unwrap();
        assert_eq!((d.to.as_str(), d.from.as_str()), (MAC, PHONE));
    }

    #[test]
    fn locked_computer_only_suggests() {
        let mut w = World::new();
        w.mode = HandoffMode::Auto;
        w.push(w.signal(0, MAC, Locked, None, false));
        // Заблокирован минуту — рано
        assert_eq!(
            w.send(w.signal(60, PHONE, OutputAdded, Some("Sony WH-1000XM5"), false)),
            None
        );
        let d = w
            .send(w.signal(400, PHONE, OutputAdded, Some("Sony WH-1000XM5"), false))
            .unwrap();
        assert_eq!(d.reason, HandoffReason::Away);
        assert!(!d.transfer, "даже в режиме «Сразу» — только предложение");
        // Разблокировали — человек вернулся
        let mut w = World::new();
        w.push(w.signal(0, MAC, Locked, None, false));
        w.push(w.signal(100, MAC, Unlocked, None, false));
        assert_eq!(
            w.send(w.signal(400, PHONE, Foreground, Some("AirPods"), false)),
            None
        );
        // Телефон без наушников — не повод
        let mut w = World::new();
        w.push(w.signal(0, MAC, Locked, None, false));
        assert_eq!(w.send(w.signal(400, PHONE, Foreground, None, false)), None);
    }

    #[test]
    fn names_compare_by_letters_and_digits() {
        assert_eq!(
            name_key("AirPods Pro (Захар)"),
            name_key("airpods pro захар")
        );
        assert_ne!(name_key("AirPods Pro"), name_key("AirPods Max"));
        assert_eq!(name_key("—"), "");
    }

    #[test]
    fn memory_forgets_and_does_not_repeat() {
        let m = Memory::default();
        let user = Uuid::now_v7();
        let start = Instant::now();
        let s = |secs: u64| Signal {
            device: MAC.into(),
            kind: ConnectKind::Laptop,
            signal: Locked,
            output: None,
            playing: false,
            at: start + Duration::from_secs(secs),
        };
        assert!(m.remember(user, s(0)).is_empty());
        assert_eq!(m.remember(user, s(10)).len(), 1);
        assert!(m.remember(user, s(10 + KEEP.as_secs() + 1)).is_empty());
        let d = Decision {
            to: PHONE.into(),
            from: MAC.into(),
            reason: HandoffReason::Headphones,
            output: None,
            transfer: false,
            play: true,
        };
        assert!(m.fresh(user, &d, start));
        assert!(!m.fresh(user, &d, start + Duration::from_secs(30)));
        assert!(m.fresh(user, &d, start + REPEAT_AFTER));
    }
}
