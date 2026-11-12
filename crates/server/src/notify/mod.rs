//! Уведомления: центр уведомлений человека, рассылка в окно приложения (WebSocket) и push на устройства.
//!
//! Событие ([`emit`]) сразу ложится в центр уведомлений, а рассылку делает очередь задач (полоса
//! «Уведомления»): `notify_send` ждёт окно сбора (несколько релизов за пару минут — одно уведомление
//! «3 новых релиза»), закрывает группу, шлёт `notification.new` открытым окнам и ставит по задаче
//! `notify_push` на каждое устройство — с повторами, в тихие часы — с отсрочкой до утра. Прочитанное
//! до отправки push не уходит. «Токен недействителен» выключает подписку.
//!
//! Каналы push — [`push`]: APNs ([`apns`]), Web Push ([`webpush`]), позже FCM. Поводы для уведомлений
//! (релизы подписок, перенос, итоги года, спорные треки, источники) — [`events`].

pub mod apns;
pub mod events;
pub mod push;
pub mod webpush;

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Timelike, Utc};
use dalywave_protocol::{
    jobs::kind,
    notifications::{
        ApnsEnvironment, Notification, NotificationItem, NotificationKind, NotificationKindSetting,
        NotificationSettings, NotificationsRead, PushChannel, QuietHours,
    },
    realtime::ServerEvent,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, PgExecutor, PgPool};
use uuid::Uuid;

use crate::{
    jobs::{
        self, JobRow, NewJob, lanes,
        worker::{Context, JobError, JobResult},
    },
    realtime::{self, Audience},
};
use push::{Delivery, Message, Target};

/// Попыток доставки на устройство: служба push лежит — повтор через 30 с, 1, 2, 4 минуты.
const PUSH_ATTEMPTS: i16 = 5;
/// Сколько хранится центр уведомлений.
const KEEP_DAYS: i32 = 90;
/// Однократные метки (итоги года — раз в год) живут дольше года.
const KEEP_MARKS_DAYS: i32 = 400;
/// Самое длинное событие в одном уведомлении (остальное — числом).
const MAX_ITEMS: usize = 20;

/// Сколько ждать новых событий того же вида, прежде чем разослать уведомление.
pub fn gather(kind: NotificationKind) -> Duration {
    match kind {
        // Сверка подписок идёт артист за артистом: релизы одного вечера — одним уведомлением
        NotificationKind::Release => Duration::from_secs(120),
        // Спорные копятся пачкой, пока качается импорт или дискография
        NotificationKind::Review => Duration::from_secs(15 * 60),
        _ => Duration::ZERO,
    }
}

/// Новое событие для человека.
#[derive(Debug, Clone)]
pub struct Draft {
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    /// Путь сайта: `/album/{id}`, `/import`.
    pub link: Option<String>,
    pub image: Option<String>,
    /// Событие для списка группы (у релиза — альбом).
    pub item: Option<NotificationItem>,
    /// События с одним ключом группы, пока уведомление не разослано, собираются в одно.
    pub group: Option<String>,
    /// Однократность: событие с этой меткой у человека уже было — второго уведомления нет.
    pub mark: Option<String>,
}

impl Draft {
    pub fn new(kind: NotificationKind, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            body: body.into(),
            link: None,
            image: None,
            item: None,
            group: None,
            mark: None,
        }
    }

    pub fn link(mut self, link: impl Into<String>) -> Self {
        self.link = Some(link.into());
        self
    }

    pub fn image(mut self, image: Option<String>) -> Self {
        self.image = image;
        self
    }

    pub fn item(mut self, item: NotificationItem) -> Self {
        self.item = Some(item);
        self
    }

    pub fn group(mut self, key: impl Into<String>) -> Self {
        self.group = Some(key.into());
        self
    }

    pub fn once(mut self, mark: impl Into<String>) -> Self {
        self.mark = Some(mark.into());
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendPayload {
    pub notification_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPayload {
    pub notification_id: Uuid,
    pub subscription_id: Uuid,
}

/// Настройки человека; строки нет — по умолчанию, часовой пояс — сервиса.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefs {
    pub disabled: Vec<NotificationKind>,
    pub quiet: QuietHours,
    pub utc_offset_min: i16,
}

impl Prefs {
    pub fn enabled(&self, kind: NotificationKind) -> bool {
        !kind.configurable() || !self.disabled.contains(&kind)
    }

    /// Настройки для экрана: виды, доступные человеку.
    pub fn view(&self, admin: bool) -> NotificationSettings {
        NotificationSettings {
            kinds: NotificationKind::ALL
                .iter()
                .filter(|k| k.configurable() && (admin || !k.admin_only()))
                .map(|&kind| NotificationKindSetting {
                    kind,
                    enabled: self.enabled(kind),
                })
                .collect(),
            quiet: self.quiet,
            utc_offset_min: self.utc_offset_min,
        }
    }
}

pub async fn prefs(db: &mut PgConnection, user: Uuid) -> Result<Prefs, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT disabled_kinds, quiet_enabled, quiet_from, quiet_to, utc_offset_min
         FROM notification_settings WHERE user_id = $1",
        user
    )
    .fetch_optional(&mut *db)
    .await?;
    let server_offset = crate::reco::Settings::load(&mut *db).await?.utc_offset_min;
    Ok(match row {
        None => Prefs {
            disabled: Vec::new(),
            quiet: QuietHours::default(),
            utc_offset_min: server_offset,
        },
        Some(r) => Prefs {
            disabled: r
                .disabled_kinds
                .iter()
                .filter_map(|k| k.parse().ok())
                .collect(),
            quiet: QuietHours {
                enabled: r.quiet_enabled,
                from: r.quiet_from as u16,
                to: r.quiet_to as u16,
            },
            utc_offset_min: r.utc_offset_min.unwrap_or(server_offset),
        },
    })
}

/// Сколько ждать конца тихих часов; `None` — сейчас не тихо.
pub fn quiet_wait(quiet: &QuietHours, utc_offset_min: i16, now: DateTime<Utc>) -> Option<Duration> {
    if !quiet.enabled || quiet.from == quiet.to {
        return None;
    }
    let local = now + chrono::Duration::minutes(i64::from(utc_offset_min));
    let second = i64::from(local.num_seconds_from_midnight());
    let (from, to) = (i64::from(quiet.from) * 60, i64::from(quiet.to) * 60);
    let quiet_now = if from < to {
        (from..to).contains(&second)
    } else {
        second >= from || second < to
    };
    quiet_now.then(|| Duration::from_secs((to - second).rem_euclid(86_400) as u64))
}

/// Записать событие в центр уведомлений и поставить рассылку. `None` — вид выключен или событие
/// с этой меткой уже было.
pub async fn emit(db: &PgPool, user: Uuid, draft: Draft) -> Result<Option<Uuid>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let id = emit_in(&mut tx, user, draft).await?;
    tx.commit().await?;
    Ok(id)
}

/// [`emit`] внутри чужой транзакции: уведомление появится вместе с данными.
pub async fn emit_in(
    tx: &mut PgConnection,
    user: Uuid,
    draft: Draft,
) -> Result<Option<Uuid>, sqlx::Error> {
    if let Some(mark) = &draft.mark {
        let fresh = sqlx::query_scalar!(
            "INSERT INTO notification_marks (user_id, key) VALUES ($1, $2)
             ON CONFLICT DO NOTHING RETURNING true AS \"fresh!\"",
            user,
            mark
        )
        .fetch_optional(&mut *tx)
        .await?;
        if fresh.is_none() {
            return Ok(None);
        }
    }
    if !prefs(tx, user).await?.enabled(draft.kind) {
        return Ok(None);
    }
    let kind = draft.kind.as_str();
    let items: Vec<NotificationItem> = draft.item.clone().into_iter().collect();
    let open = match &draft.group {
        Some(group) => {
            sqlx::query!(
                r#"SELECT id, items, count FROM notifications
                   WHERE user_id = $1 AND kind = $2 AND group_key = $3 AND sent_at IS NULL
                   FOR UPDATE"#,
                user,
                kind,
                group
            )
            .fetch_optional(&mut *tx)
            .await?
        }
        None => None,
    };
    let id = match open {
        Some(row) => {
            let mut all: Vec<NotificationItem> =
                serde_json::from_value(row.items).unwrap_or_default();
            all.extend(items);
            all.truncate(MAX_ITEMS);
            let count = row.count + 1;
            let (title, body, link, image) = regroup(&draft, &all, count);
            sqlx::query!(
                "UPDATE notifications SET title = $2, body = $3, link = $4, image = $5, items = $6,
                        count = $7, updated_at = now()
                 WHERE id = $1",
                row.id,
                title,
                body,
                link,
                image,
                json!(all),
                count,
            )
            .execute(&mut *tx)
            .await?;
            row.id
        }
        None => {
            sqlx::query_scalar!(
            "INSERT INTO notifications (user_id, kind, group_key, title, body, link, image, items)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            user,
            kind,
            draft.group,
            draft.title,
            draft.body,
            draft.link,
            draft.image,
            json!(items),
        )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    jobs::enqueue(
        tx,
        NewJob::new(
            kind::NOTIFY_SEND,
            lanes::NOTIFY,
            SendPayload {
                notification_id: id,
            },
        )
        .dedupe(format!("notify_send:{id}"))
        .delay(gather(draft.kind))
        .max_attempts(5),
    )
    .await?;
    Ok(Some(id))
}

/// Заголовок и текст уведомления, в которое собралось несколько событий.
fn regroup(
    draft: &Draft,
    items: &[NotificationItem],
    count: i32,
) -> (String, String, Option<String>, Option<String>) {
    match draft.kind {
        NotificationKind::Release => {
            let title = format!(
                "{count} {}",
                plural(count, ["новый релиз", "новых релиза", "новых релизов"])
            );
            let mut artists: Vec<&str> = Vec::new();
            for it in items {
                if !it.subtitle.is_empty() && !artists.contains(&it.subtitle.as_str()) {
                    artists.push(&it.subtitle);
                }
            }
            let body = match artists.len() {
                0 => String::new(),
                1 => artists[0].to_string(),
                2 => format!("{} и {}", artists[0], artists[1]),
                n => format!("{}, {} и ещё {}", artists[0], artists[1], n - 2),
            };
            let image = items.iter().find_map(|i| i.image.clone());
            (title, body, Some("/notifications".into()), image)
        }
        _ => (
            draft.title.clone(),
            draft.body.clone(),
            draft.link.clone(),
            draft.image.clone(),
        ),
    }
}

/// «1 релиз», «2 релиза», «5 релизов».
pub fn plural(n: i32, forms: [&str; 3]) -> &str {
    let n = n.unsigned_abs();
    let (d10, d100) = (n % 10, n % 100);
    if d10 == 1 && d100 != 11 {
        forms[0]
    } else if (2..=4).contains(&d10) && !(12..=14).contains(&d100) {
        forms[1]
    } else {
        forms[2]
    }
}

struct Row {
    id: Uuid,
    user_id: Uuid,
    kind: String,
    title: String,
    body: String,
    link: Option<String>,
    image: Option<String>,
    items: serde_json::Value,
    count: i32,
    created_at: DateTime<Utc>,
    read_at: Option<DateTime<Utc>>,
}

impl Row {
    fn kind(&self) -> NotificationKind {
        self.kind.parse().unwrap_or(NotificationKind::Test)
    }

    fn view(self) -> Notification {
        let kind = self.kind();
        let items: Vec<NotificationItem> = serde_json::from_value(self.items).unwrap_or_default();
        Notification {
            id: self.id,
            kind,
            title: self.title,
            body: self.body,
            link: self.link,
            image: self.image,
            count: self.count,
            // Одно событие уже сказано заголовком
            items: if self.count > 1 { items } else { Vec::new() },
            created_at: self.created_at,
            read_at: self.read_at,
        }
    }
}

pub async fn unread(db: impl PgExecutor<'_>, user: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM notifications WHERE user_id = $1 AND read_at IS NULL"#,
        user
    )
    .fetch_one(db)
    .await
}

/// Страница центра уведомлений: новые первыми, `before` — курсор предыдущей страницы.
pub async fn list(
    db: &PgPool,
    user: Uuid,
    before: Option<(DateTime<Utc>, Uuid)>,
    limit: i64,
) -> Result<(Vec<Notification>, Option<String>), sqlx::Error> {
    let (at, id) = before.unzip();
    let mut rows = sqlx::query_as!(
        Row,
        "SELECT id, user_id, kind, title, body, link, image, items, count, created_at, read_at
         FROM notifications
         WHERE user_id = $1 AND ($2::timestamptz IS NULL OR (created_at, id) < ($2, $3))
         ORDER BY created_at DESC, id DESC
         LIMIT $4",
        user,
        at,
        id,
        limit + 1,
    )
    .fetch_all(db)
    .await?;
    let next = crate::cursor::page(&mut rows, limit, |r| (r.created_at, r.id));
    Ok((rows.into_iter().map(Row::view).collect(), next))
}

/// Отметить прочитанными (`ids` пусто — все) и сообщить остальным устройствам.
pub async fn mark_read(
    db: &PgPool,
    user: Uuid,
    ids: &[Uuid],
) -> Result<NotificationsRead, sqlx::Error> {
    let mut tx = db.begin().await?;
    let all = ids.is_empty();
    let changed = sqlx::query_scalar!(
        "UPDATE notifications SET read_at = now()
         WHERE user_id = $1 AND read_at IS NULL AND ($2 OR id = ANY($3))
         RETURNING id",
        user,
        all,
        ids,
    )
    .fetch_all(&mut *tx)
    .await?;
    let read = NotificationsRead {
        ids: if all { Vec::new() } else { changed },
        all,
        unread: unread(&mut *tx, user).await?,
    };
    realtime::publish(
        &mut *tx,
        Audience::User(user),
        ServerEvent::NotificationRead(read.clone()),
    )
    .await;
    tx.commit().await?;
    Ok(read)
}

/// Чистка: старые уведомления и метки.
pub async fn cleanup(db: &PgPool) -> Result<u64, sqlx::Error> {
    let a = sqlx::query!(
        "DELETE FROM notifications WHERE created_at < now() - make_interval(days => $1)",
        KEEP_DAYS
    )
    .execute(db)
    .await?
    .rows_affected();
    let b = sqlx::query!(
        "DELETE FROM notification_marks WHERE created_at < now() - make_interval(days => $1)",
        KEEP_MARKS_DAYS
    )
    .execute(db)
    .await?
    .rows_affected();
    Ok(a + b)
}

/// Подписки, на которые сейчас можно слать: не выключены, сессия жива, канал включён на сервере.
async fn live_subscriptions(
    db: &mut PgConnection,
    user: Uuid,
    channels: &[&str],
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT p.id FROM push_subscriptions p
         LEFT JOIN sessions s ON s.id = p.session_id
         WHERE p.user_id = $1 AND p.disabled_at IS NULL AND p.channel = ANY($2)
           AND (p.session_id IS NULL OR (s.revoked_at IS NULL AND s.expires_at > now()))
         ORDER BY p.created_at",
        user,
        channels as &[&str],
    )
    .fetch_all(db)
    .await
}

/// Каналы, которые есть у этого процесса.
fn channels(ctx: &Context) -> Vec<&'static str> {
    [PushChannel::Apns, PushChannel::Webpush, PushChannel::Fcm]
        .into_iter()
        .filter(|c| ctx.push.enabled(*c))
        .map(PushChannel::as_str)
        .collect()
}

/// Задача `notify_send`: закрыть группу, сообщить окнам, поставить push на устройства.
pub async fn run_send(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: SendPayload = job.payload()?;
    let mut tx = ctx.db.begin().await?;
    let row = sqlx::query_as!(
        Row,
        "UPDATE notifications SET sent_at = now() WHERE id = $1 AND sent_at IS NULL
         RETURNING id, user_id, kind, title, body, link, image, items, count, created_at, read_at",
        p.notification_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(mut row) = row else {
        return Ok(json!({ "skipped": "already_sent" }).into());
    };
    let kind = row.kind();
    // Спорные: к рассылке часть могли уже решить — в тексте то, что ждёт сейчас
    if kind == NotificationKind::Review {
        match events::review_text(&mut tx).await? {
            Some((title, body)) => {
                sqlx::query!(
                    "UPDATE notifications SET title = $2, body = $3 WHERE id = $1",
                    row.id,
                    title,
                    body
                )
                .execute(&mut *tx)
                .await?;
                (row.title, row.body) = (title, body);
            }
            None => {
                sqlx::query!("DELETE FROM notifications WHERE id = $1", row.id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(json!({ "skipped": "resolved" }).into());
            }
        }
    }
    let user = row.user_id;
    let already_read = row.read_at.is_some();
    realtime::publish(
        &mut *tx,
        Audience::User(user),
        ServerEvent::NotificationNew(row.view()),
    )
    .await;
    if already_read {
        tx.commit().await?;
        return Ok(json!({ "skipped": "read" }).into());
    }
    let prefs = prefs(&mut tx, user).await?;
    // Пробное просят сейчас — оно приходит и в тихие часы
    let wait = (kind != NotificationKind::Test)
        .then(|| quiet_wait(&prefs.quiet, prefs.utc_offset_min, Utc::now()))
        .flatten();
    let subs = live_subscriptions(&mut tx, user, &channels(&ctx)).await?;
    for sub in &subs {
        let mut job = NewJob::new(
            kind::NOTIFY_PUSH,
            lanes::NOTIFY,
            PushPayload {
                notification_id: p.notification_id,
                subscription_id: *sub,
            },
        )
        .dedupe(format!("notify_push:{}:{sub}", p.notification_id))
        .max_attempts(PUSH_ATTEMPTS);
        if let Some(wait) = wait {
            job = job.delay(wait);
        }
        jobs::enqueue(&mut tx, job).await?;
    }
    tx.commit().await?;
    Ok(json!({
        "devices": subs.len(),
        "quietSecs": wait.map(|w| w.as_secs()),
    })
    .into())
}

/// Задача `notify_push`: одно уведомление на одно устройство.
pub async fn run_push(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: PushPayload = job.payload()?;
    let db = &ctx.db;
    let Some(n) = sqlx::query!(
        "SELECT user_id, kind, title, body, link, image, read_at FROM notifications WHERE id = $1",
        p.notification_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(json!({ "skipped": "deleted" }).into());
    };
    // Увидели в приложении, пока push ждал утра или повтора
    if n.read_at.is_some() {
        return Ok(json!({ "skipped": "read" }).into());
    }
    let Some(sub) = sqlx::query!(
        r#"SELECT p.channel AS "channel!", p.token AS "token!", p.p256dh, p.auth, p.environment
         FROM push_subscriptions p LEFT JOIN sessions s ON s.id = p.session_id
         WHERE p.id = $1 AND p.disabled_at IS NULL
           AND (p.session_id IS NULL OR (s.revoked_at IS NULL AND s.expires_at > now()))"#,
        p.subscription_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(json!({ "skipped": "unsubscribed" }).into());
    };
    let channel: PushChannel = sub
        .channel
        .parse()
        .map_err(|_| JobError::permanent("неизвестный канал"))?;
    let message = Message {
        id: p.notification_id,
        kind: n.kind.parse().unwrap_or(NotificationKind::Test),
        title: n.title,
        body: n.body,
        link: n.link.unwrap_or_else(|| "/notifications".into()),
        image: n.image,
        unread: unread(db, n.user_id).await?,
    };
    let target = Target {
        channel,
        token: &sub.token,
        p256dh: sub.p256dh.as_deref(),
        auth: sub.auth.as_deref(),
        environment: sub.environment.as_deref().and_then(|e| e.parse().ok()),
    };
    let (delivery, env) = ctx.push.send(&target, &message).await;
    match delivery {
        Delivery::Sent => {
            sqlx::query!(
                "UPDATE push_subscriptions SET last_sent_at = now(),
                        environment = COALESCE($2, environment), updated_at = now()
                 WHERE id = $1",
                p.subscription_id,
                env.map(ApnsEnvironment::as_str),
            )
            .execute(db)
            .await?;
            Ok(json!({ "sent": channel }).into())
        }
        Delivery::Gone(reason) => {
            tracing::info!(subscription = %p.subscription_id, %channel, %reason, "push subscription is gone");
            sqlx::query!(
                "UPDATE push_subscriptions SET disabled_at = now(), disabled_reason = $2, updated_at = now()
                 WHERE id = $1",
                p.subscription_id,
                reason.chars().take(500).collect::<String>(),
            )
            .execute(db)
            .await?;
            Ok(json!({ "gone": reason }).into())
        }
        Delivery::Retry(m) => Err(JobError::retry(m)),
        Delivery::Failed(m) => Err(JobError::permanent(m)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 12, h, m, 0).unwrap()
    }

    #[test]
    fn quiet_hours_cross_midnight() {
        let q = QuietHours::default(); // 23:00–08:00
        // Москва: 21:00 UTC = 00:00 — до 08:00 восемь часов
        assert_eq!(
            quiet_wait(&q, 180, at(21, 0)),
            Some(Duration::from_secs(8 * 3600))
        );
        assert_eq!(quiet_wait(&q, 180, at(19, 59)), None, "22:59 — ещё не тихо");
        assert_eq!(
            quiet_wait(&q, 180, at(20, 0)),
            Some(Duration::from_secs(9 * 3600))
        );
        assert_eq!(quiet_wait(&q, 180, at(5, 0)), None, "08:00 — уже можно");
        assert_eq!(
            quiet_wait(&q, 180, at(4, 30)),
            Some(Duration::from_secs(30 * 60))
        );
        let off = QuietHours {
            enabled: false,
            ..q
        };
        assert_eq!(quiet_wait(&off, 180, at(21, 0)), None);
    }

    #[test]
    fn quiet_hours_within_a_day() {
        let q = QuietHours {
            enabled: true,
            from: 13 * 60,
            to: 15 * 60,
        };
        assert_eq!(
            quiet_wait(&q, 0, at(14, 0)),
            Some(Duration::from_secs(3600))
        );
        assert_eq!(quiet_wait(&q, 0, at(15, 0)), None);
        assert_eq!(quiet_wait(&q, 0, at(12, 59)), None);
        let same = QuietHours {
            enabled: true,
            from: 600,
            to: 600,
        };
        assert_eq!(quiet_wait(&same, 0, at(10, 0)), None);
    }

    #[test]
    fn russian_plurals() {
        let f = ["новый релиз", "новых релиза", "новых релизов"];
        assert_eq!(plural(1, f), "новый релиз");
        assert_eq!(plural(3, f), "новых релиза");
        assert_eq!(plural(5, f), "новых релизов");
        assert_eq!(plural(11, f), "новых релизов");
        assert_eq!(plural(21, f), "новый релиз");
        assert_eq!(plural(112, f), "новых релизов");
    }

    #[test]
    fn releases_regroup() {
        let draft = Draft::new(NotificationKind::Release, "x", "y");
        let item = |t: &str, a: &str| NotificationItem {
            title: t.into(),
            subtitle: a.into(),
            link: Some("/album/1".into()),
            image: None,
        };
        let items = vec![
            item("A", "Lowborn"),
            item("B", "Saluki"),
            item("C", "Lowborn"),
        ];
        let (title, body, link, _) = regroup(&draft, &items, 3);
        assert_eq!(title, "3 новых релиза");
        assert_eq!(body, "Lowborn и Saluki");
        assert_eq!(link.as_deref(), Some("/notifications"));
        let items = vec![
            item("A", "a"),
            item("B", "b"),
            item("C", "c"),
            item("D", "d"),
        ];
        assert_eq!(regroup(&draft, &items, 4).1, "a, b и ещё 2");
    }

    #[test]
    fn settings_hide_admin_kinds() {
        let p = Prefs {
            disabled: vec![NotificationKind::Import],
            quiet: QuietHours::default(),
            utc_offset_min: 180,
        };
        let v = p.view(false);
        assert!(v.kinds.iter().all(|k| !k.kind.admin_only()));
        assert!(!v.kinds.iter().any(|k| k.kind == NotificationKind::Test));
        assert!(
            !v.kinds
                .iter()
                .find(|k| k.kind == NotificationKind::Import)
                .unwrap()
                .enabled
        );
        assert!(
            p.view(true)
                .kinds
                .iter()
                .any(|k| k.kind == NotificationKind::Review)
        );
        assert!(p.enabled(NotificationKind::Test));
    }
}
