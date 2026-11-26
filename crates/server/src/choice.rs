//! Где заиграет музыка, выбранная на другом устройстве. Музыка играет на ПК, человек на ноутбуке выбирает
//! альбом: иногда он ушёл за ноутбук и хочет слушать там, иногда ПК стоит у колонок и ноутбук — пульт.
//!
//! Решение — на сервере, под блокировкой строки `playback_state`: два пульта, выбравшие музыку
//! одновременно, получат согласованные ответы (второй увидит, что звук уже на первом). Пульт шлёт
//! команду `playTracks`/`startWave` с `pick`, сервер решает ([`decide`], чистая функция) и отвечает
//! `connect.placed`:
//! - «там» — команда уходит активному устройству, как раньше;
//! - «здесь» — активным сразу становится пульт (прежнее активное получает `connect.active` и встаёт на
//!   паузу, его очередь остаётся в последнем снимке), а пульт включает музыку сам.
//!
//! Правила по порядку:
//! 1. Настройка выбирающего устройства «Всегда здесь» / «Там, где уже играет».
//! 2. Телефон с наушниками: выбранное на нём играет на нём.
//! 3. Запомненная поправка для пары устройств (см. ниже).
//! 4. Активное — медиаплеер (и играет или играло последние [`MEDIA_PLAYER_GRACE`]): туда.
//! 5. Активный телефон играет в наушники: туда — музыка остаётся в ушах.
//! 6. Присутствие: у активного ввод был меньше [`IDLE_AFTER`] назад — там им пользуются, туда; дольше —
//!    человек отошёл, сюда (он только что нажал здесь). Приложения на компьютерах знают простой всей
//!    системы, сайт — только ввод на своей странице (признак слабее: человек мог работать в другом окне).
//! 7. О присутствии ничего не известно — туда, как раньше.
//!
//! Поправки: если после автоматического решения человек в течение [`CORRECTION_WINDOW`] перенёс музыку
//! на второе устройство пары (кнопкой в тосте или в «Где играть»), это поправка. Две одинаковые подряд —
//! предпочтение для пары на [`LEARN_FOR`] (или до обратной поправки) и `connect.learned` тому, кто поправил.

use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::{
    connect::{
        ConnectActive, ConnectCommand, ConnectDevicePatch, ConnectDevices, ConnectKind,
        ConnectLearned, ConnectPick, ConnectPlaced, ConnectPresence, ConnectSend, PlaceReason,
        PlayChoice, RemoteCommand,
    },
    realtime::ServerEvent,
};

use crate::{
    connect::{self, Link, Scope},
    error::{AppError, AppResult},
    realtime::{self, Audience},
    state::AppState,
};

/// Ввода на активном устройстве не было дольше — человек от него отошёл.
pub const IDLE_AFTER: Duration = Duration::minutes(2);
/// Медиаплеер на паузе остаётся местом для музыки столько после того, как играл.
pub const MEDIA_PLAYER_GRACE: Duration = Duration::minutes(30);
/// Перенос на второе устройство пары в пределах этого после решения — поправка.
pub const CORRECTION_WINDOW: Duration = Duration::minutes(2);
/// Сколько одинаковых поправок подряд запоминаются.
pub const LEARN_AFTER: i16 = 2;
/// Сколько помнить предпочтение.
pub const LEARN_FOR: Duration = Duration::hours(12);
/// Простой больше этого не нужен (и не переполнит интервал).
const MAX_IDLE_MS: u64 = 30 * 24 * 3600 * 1000;

/// Устройство, где выбрали музыку.
#[derive(Debug, Clone, Copy)]
pub struct Chooser {
    pub choice: PlayChoice,
    pub headphones: bool,
    /// Телефон или планшет.
    pub handheld: bool,
}

/// Устройство, где играет звук.
#[derive(Debug, Clone, Copy, Default)]
pub struct Active {
    pub media_player: bool,
    pub playing: bool,
    pub handheld: bool,
    pub headphones: bool,
    pub last_input_at: Option<DateTime<Utc>>,
    /// Когда устройство в последний раз сообщало о себе (а значит, играло).
    pub presence_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub chooser: Chooser,
    pub active: Active,
    /// Запомненная поправка для пары: `true` — играть там, где выбрали.
    pub learned: Option<bool>,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// Играть там, где выбрали.
    pub here: bool,
    pub reason: PlaceReason,
}

const fn place(here: bool, reason: PlaceReason) -> Placement {
    Placement { here, reason }
}

/// Где играть музыку, выбранную на пульте, пока звук на активном устройстве.
pub fn decide(f: &Facts) -> Placement {
    match f.chooser.choice {
        PlayChoice::Here => return place(true, PlaceReason::Setting),
        PlayChoice::There => return place(false, PlaceReason::Setting),
        PlayChoice::Auto => {}
    }
    if f.chooser.handheld && f.chooser.headphones {
        return place(true, PlaceReason::Headphones);
    }
    if let Some(here) = f.learned {
        return place(here, PlaceReason::Learned);
    }
    let a = &f.active;
    let played_lately = a.playing
        || a.presence_at
            .is_some_and(|at| f.now - at <= MEDIA_PLAYER_GRACE);
    if a.media_player && played_lately {
        return place(false, PlaceReason::MediaPlayer);
    }
    if a.handheld && a.headphones && a.playing {
        return place(false, PlaceReason::Headphones);
    }
    match a.last_input_at {
        None => place(false, PlaceReason::Unknown),
        Some(at) if f.now - at < IDLE_AFTER => place(false, PlaceReason::InUse),
        Some(_) => place(true, PlaceReason::Away),
    }
}

/// Решение по поправке: куда перенесли относительно решения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Correction {
    pub streak_here: Option<bool>,
    pub streak: i16,
    pub prefer: Option<bool>,
    /// Предпочтение только что запомнено.
    pub learned: bool,
}

/// Новая поправка `here` (перенесли туда, где выбирали) к серии `streak_here`×`streak` и запомненному
/// `prefer`: две одинаковые подряд — запомнить, обратная — забыть.
pub fn correct(
    streak_here: Option<bool>,
    streak: i16,
    prefer: Option<bool>,
    here: bool,
) -> Correction {
    let streak = if streak_here == Some(here) {
        streak.saturating_add(1)
    } else {
        1
    };
    if streak >= LEARN_AFTER {
        return Correction {
            streak_here: Some(here),
            streak: 0,
            prefer: Some(here),
            learned: true,
        };
    }
    Correction {
        streak_here: Some(here),
        streak,
        prefer: prefer.filter(|p| *p == here),
        learned: false,
    }
}

fn dir(here: bool) -> &'static str {
    if here { "here" } else { "there" }
}

fn handheld(kind: &str) -> bool {
    matches!(
        kind.parse::<ConnectKind>(),
        Ok(ConnectKind::Phone | ConnectKind::Tablet)
    )
}

/// Новая музыка — то, что выбирают (а не управление тем, что уже играет).
pub fn is_new_music(cmd: &RemoteCommand) -> bool {
    matches!(
        cmd,
        RemoteCommand::PlayTracks(_) | RemoteCommand::StartWave(_)
    )
}

/// Пульт `from` выбрал музыку: решить, где ей играть, и ответить `connect.placed`.
pub async fn pick(
    st: &AppState,
    scope: Scope,
    from: String,
    send: ConnectSend,
    pick: ConnectPick,
) -> AppResult<Option<ServerEvent>> {
    let mut tx = st.db.begin().await?;
    // Блокировка — на всё решение: второй пульт ждёт и видит уже новое активное устройство
    let row = sqlx::query!(
        r#"SELECT active_connect_id, (state ->> 'playing')::boolean AS "playing?"
           FROM playback_state WHERE scope = $1 FOR UPDATE"#,
        scope.id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let (active, playing) = match row {
        Some(r) => (r.active_connect_id, r.playing.unwrap_or(false)),
        None => (None, false),
    };
    let placed = |here: bool, device: &str, other: &str, reason: PlaceReason| {
        Some(ServerEvent::ConnectPlaced(ConnectPlaced {
            id: send.id.clone(),
            here,
            device_id: device.to_string(),
            other: other.to_string(),
            reason,
        }))
    };
    // Звук нигде не играет или уже здесь — просто здесь
    let Some(active) = active.filter(|a| *a != from) else {
        return Ok(placed(true, &from, &from, PlaceReason::Setting));
    };
    let rows = sqlx::query!(
        r#"SELECT id, kind, choice, media_player, last_input_at, headphones, presence_at,
                  coalesce((capabilities->>'remote')::boolean, false) AS "remote!",
                  (connection IS NOT NULL AND last_seen_at > now() - make_interval(secs => $3)) AS "online!"
           FROM connect_devices WHERE scope = $1 AND id = ANY($2)"#,
        scope.id,
        &[from.clone(), active.clone()][..],
        dalywave_protocol::connect::ONLINE_WINDOW_SECS as f64,
    )
    .fetch_all(&mut *tx)
    .await?;
    let me = rows.iter().find(|r| r.id == from);
    let there = rows.iter().find(|r| r.id == active);
    // Пульт без звука (часы) играть не может; активное пропало — играть здесь
    let Some(me) = me.filter(|m| !m.remote) else {
        return forward(st, scope, from, send, &active, PlaceReason::Setting);
    };
    let Some(there) = there.filter(|t| t.online) else {
        tx.rollback().await?;
        return Ok(placed(true, &from, &active, PlaceReason::Away));
    };
    let learned = sqlx::query_scalar!(
        "SELECT prefer FROM connect_choices
         WHERE scope = $1 AND from_id = $2 AND to_id = $3 AND prefer_until > now()",
        scope.id,
        from,
        active,
    )
    .fetch_optional(&mut *tx)
    .await?
    .flatten()
    .map(|p| p == "here");
    let now = Utc::now();
    let facts = Facts {
        chooser: Chooser {
            choice: me.choice.parse().unwrap_or_default(),
            headphones: pick.headphones,
            handheld: handheld(&me.kind),
        },
        active: Active {
            media_player: there.media_player,
            playing,
            handheld: handheld(&there.kind),
            headphones: there.headphones,
            last_input_at: there.last_input_at,
            presence_at: there.presence_at,
        },
        learned,
        now,
    };
    let p = decide(&facts);
    tracing::info!(user = %scope.user, %from, to = %active, here = p.here, reason = %p.reason, "music placed");
    if p.reason != PlaceReason::Setting {
        // Решение автоматики — чтобы узнать поправку. Прежнее решение никто не поправил: серия прервалась
        sqlx::query!(
            "INSERT INTO connect_choices (scope, from_id, to_id, decided, decided_at)
             VALUES ($1, $2, $3, $4, now())
             ON CONFLICT (scope, from_id, to_id) DO UPDATE SET
                 streak = CASE WHEN connect_choices.decided_at IS NOT NULL THEN 0 ELSE connect_choices.streak END,
                 decided = EXCLUDED.decided, decided_at = now()",
            scope.id,
            from,
            active,
            dir(p.here),
        )
        .execute(&mut *tx)
        .await?;
    }
    if !p.here {
        tx.commit().await?;
        return forward(st, scope, from, send, &active, p.reason);
    }
    // Здесь: пульт сразу становится активным (снимок прежнего остаётся до первого своего)
    sqlx::query!(
        "UPDATE playback_state SET active_connect_id = $2 WHERE scope = $1",
        scope.id,
        from,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    realtime::deliver(
        &st.hub,
        Audience::User(scope.id),
        ServerEvent::ConnectActive(ConnectActive {
            device_id: Some(from.clone()),
            from: Some(active.clone()),
            transfer: false,
            state: None,
        }),
    );
    connect::announce(&st.db, &st.hub, scope).await?;
    Ok(placed(true, &from, &active, p.reason))
}

/// Музыка играет там: команда — активному устройству, пульту — где она заиграет.
fn forward(
    st: &AppState,
    scope: Scope,
    from: String,
    send: ConnectSend,
    active: &str,
    reason: PlaceReason,
) -> AppResult<Option<ServerEvent>> {
    realtime::deliver(
        &st.hub,
        Audience::Device {
            user: scope.id,
            device: active.to_string(),
        },
        ServerEvent::ConnectCommand(ConnectCommand {
            id: send.id.clone(),
            from: from.clone(),
            command: send.command,
        }),
    );
    Ok(Some(ServerEvent::ConnectPlaced(ConnectPlaced {
        id: send.id,
        here: false,
        device_id: active.to_string(),
        other: from,
        reason,
    })))
}

/// Музыку перенесли на `to` по просьбе устройства `by`: если это поправка недавнего решения — учесть.
pub async fn transferred(st: &AppState, scope: Scope, by: &str, to: &str) -> AppResult<()> {
    let mut tx = st.db.begin().await?;
    let row = sqlx::query!(
        "SELECT from_id, to_id, decided, streak_dir, streak, prefer FROM connect_choices
         WHERE scope = $1 AND decided_at > now() - make_interval(secs => $2)
         ORDER BY decided_at DESC LIMIT 1 FOR UPDATE",
        scope.id,
        CORRECTION_WINDOW.num_seconds() as f64,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(r) = row else {
        return Ok(());
    };
    // Решили «здесь» — перенесли туда, где играло; решили «там» — перенесли туда, где выбирали
    let here = match r.decided.as_deref() {
        Some("here") if to == r.to_id => false,
        Some("there") if to == r.from_id => true,
        _ => return Ok(()),
    };
    let c = correct(
        r.streak_dir.as_deref().map(|d| d == "here"),
        r.streak,
        r.prefer.as_deref().map(|p| p == "here"),
        here,
    );
    let until = Utc::now() + LEARN_FOR;
    sqlx::query!(
        "UPDATE connect_choices SET decided_at = NULL, streak_dir = $4, streak = $5, prefer = $6,
             prefer_until = CASE WHEN $7 THEN $8 WHEN $6::text IS NULL THEN NULL ELSE prefer_until END
         WHERE scope = $1 AND from_id = $2 AND to_id = $3",
        scope.id,
        r.from_id,
        r.to_id,
        c.streak_here.map(dir),
        c.streak,
        c.prefer.map(dir),
        c.learned,
        until,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(user = %scope.user, from = %r.from_id, to = %r.to_id, here, learned = c.learned, "music placement corrected");
    if c.learned {
        realtime::deliver(
            &st.hub,
            Audience::Device {
                user: scope.id,
                device: by.to_string(),
            },
            ServerEvent::ConnectLearned(ConnectLearned {
                from: r.from_id,
                to: r.to_id,
                here,
                until,
            }),
        );
    }
    Ok(())
}

/// `connect.presence`: как давно трогали устройство, где играет звук.
pub async fn presence(
    st: &AppState,
    scope: Scope,
    link: &Link,
    p: ConnectPresence,
) -> AppResult<Option<ServerEvent>> {
    let Some(device) = link.device.as_deref() else {
        return Ok(None);
    };
    let idle = p.idle_ms.min(MAX_IDLE_MS) as f64 / 1000.0;
    sqlx::query!(
        "UPDATE connect_devices
         SET last_input_at = now() - make_interval(secs => $4), input_source = $5, headphones = $6,
             presence_at = now()
         WHERE scope = $1 AND id = $2 AND connection = $3",
        scope.id,
        device,
        link.connection,
        idle,
        p.source.as_str(),
        p.headphones,
    )
    .execute(&st.db)
    .await?;
    Ok(None)
}

/// Настройки устройства `id`: видны и меняются с любого устройства человека.
pub async fn update_device(
    st: &AppState,
    scope: Scope,
    id: &str,
    patch: ConnectDevicePatch,
) -> AppResult<ConnectDevices> {
    let done = sqlx::query!(
        "UPDATE connect_devices SET choice = coalesce($3, choice), media_player = coalesce($4, media_player)
         WHERE scope = $1 AND id = $2",
        scope.id,
        id,
        patch.choice.map(PlayChoice::as_str),
        patch.media_player,
    )
    .execute(&st.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    connect::announce(&st.db, &st.hub, scope).await?;
    connect::devices(&st.db, scope.id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        "2026-11-13T12:00:00Z".parse().unwrap()
    }

    fn ago(secs: i64) -> Option<DateTime<Utc>> {
        Some(now() - Duration::seconds(secs))
    }

    fn laptop() -> Chooser {
        Chooser {
            choice: PlayChoice::Auto,
            headphones: false,
            handheld: false,
        }
    }

    fn pc(last_input: i64) -> Active {
        Active {
            playing: true,
            last_input_at: ago(last_input),
            presence_at: ago(10),
            ..Active::default()
        }
    }

    fn facts(chooser: Chooser, active: Active) -> Facts {
        Facts {
            chooser,
            active,
            learned: None,
            now: now(),
        }
    }

    #[test]
    fn someone_at_the_active_device_keeps_the_music_there() {
        let p = decide(&facts(laptop(), pc(30)));
        assert_eq!(p, place(false, PlaceReason::InUse));
        // Ровно на границе — уже отошёл
        let p = decide(&facts(laptop(), pc(120)));
        assert_eq!(p, place(true, PlaceReason::Away));
    }

    #[test]
    fn nobody_at_the_active_device_plays_here() {
        let p = decide(&facts(laptop(), pc(10 * 60)));
        assert_eq!(p, place(true, PlaceReason::Away));
        // Пауза давно, ввода не было: тоже здесь
        let paused = Active {
            playing: false,
            presence_at: ago(3600),
            ..pc(3600)
        };
        assert!(decide(&facts(laptop(), paused)).here);
    }

    #[test]
    fn unknown_presence_keeps_the_old_behaviour() {
        let active = Active {
            playing: true,
            ..Active::default()
        };
        assert_eq!(
            decide(&facts(laptop(), active)),
            place(false, PlaceReason::Unknown)
        );
    }

    #[test]
    fn explicit_setting_wins_over_everything() {
        let here = Chooser {
            choice: PlayChoice::Here,
            ..laptop()
        };
        let media = Active {
            media_player: true,
            ..pc(5)
        };
        let mut f = facts(here, media);
        f.learned = Some(false);
        assert_eq!(decide(&f), place(true, PlaceReason::Setting));
        let there = Chooser {
            choice: PlayChoice::There,
            headphones: true,
            handheld: true,
        };
        let mut f = facts(there, pc(3600));
        f.learned = Some(true);
        assert_eq!(decide(&f), place(false, PlaceReason::Setting));
    }

    #[test]
    fn media_player_takes_the_music_regardless_of_idle() {
        let media = Active {
            media_player: true,
            ..pc(6 * 3600)
        };
        assert_eq!(
            decide(&facts(laptop(), media)),
            place(false, PlaceReason::MediaPlayer)
        );
        // На паузе недавно — всё ещё медиаплеер
        let paused = Active {
            playing: false,
            presence_at: ago(10 * 60),
            ..media
        };
        assert_eq!(
            decide(&facts(laptop(), paused)),
            place(false, PlaceReason::MediaPlayer)
        );
        // Давно молчит — обычное правило: никого нет — здесь
        let asleep = Active {
            presence_at: ago(2 * 3600),
            ..paused
        };
        assert_eq!(
            decide(&facts(laptop(), asleep)),
            place(true, PlaceReason::Away)
        );
    }

    #[test]
    fn phone_with_headphones_plays_on_the_phone() {
        let phone = Chooser {
            choice: PlayChoice::Auto,
            headphones: true,
            handheld: true,
        };
        let media = Active {
            media_player: true,
            ..pc(5)
        };
        assert_eq!(
            decide(&facts(phone, media)),
            place(true, PlaceReason::Headphones)
        );
        // Наушники у ноутбука — не повод: правило про телефон
        let laptop_headphones = Chooser {
            headphones: true,
            ..laptop()
        };
        assert_eq!(
            decide(&facts(laptop_headphones, pc(5))),
            place(false, PlaceReason::InUse)
        );
    }

    #[test]
    fn music_stays_in_the_phone_headphones() {
        let phone = Active {
            handheld: true,
            headphones: true,
            ..pc(3600)
        };
        assert_eq!(
            decide(&facts(laptop(), phone)),
            place(false, PlaceReason::Headphones)
        );
        // Телефон на паузе — наушники не держат
        let paused = Active {
            playing: false,
            ..phone
        };
        assert!(decide(&facts(laptop(), paused)).here);
    }

    #[test]
    fn learned_preference_beats_presence_and_media_player() {
        let mut f = facts(laptop(), pc(5));
        f.learned = Some(true);
        assert_eq!(decide(&f), place(true, PlaceReason::Learned));
        let media = Active {
            media_player: true,
            ..pc(5)
        };
        let mut f = facts(laptop(), media);
        f.learned = Some(true);
        assert_eq!(decide(&f), place(true, PlaceReason::Learned));
        let away = facts(laptop(), pc(3600));
        assert!(decide(&away).here);
        let mut away = away;
        away.learned = Some(false);
        assert_eq!(decide(&away), place(false, PlaceReason::Learned));
    }

    #[test]
    fn two_same_corrections_in_a_row_are_remembered() {
        let first = correct(None, 0, None, false);
        assert_eq!(first.streak, 1);
        assert!(!first.learned);
        let second = correct(first.streak_here, first.streak, first.prefer, false);
        assert!(second.learned);
        assert_eq!(second.prefer, Some(false));
        assert_eq!(second.streak, 0);
    }

    #[test]
    fn opposite_correction_resets_the_streak_and_forgets() {
        let c = correct(Some(false), 1, None, true);
        assert_eq!((c.streak_here, c.streak, c.learned), (Some(true), 1, false));
        // Запомнено «там», поправили на «здесь» — забыто
        let c = correct(Some(false), 0, Some(false), true);
        assert_eq!(c.prefer, None);
        assert!(!c.learned);
        // Поправка в ту же сторону, что запомнено, — помнится дальше
        let c = correct(Some(true), 0, Some(true), true);
        assert_eq!(c.prefer, Some(true));
    }
}
