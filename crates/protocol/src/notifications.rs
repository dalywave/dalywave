//! Уведомления (`/api/notifications*`): центр уведомлений человека, настройки по видам и тихие часы,
//! подписки устройств на push. Новое уведомление приходит и по WebSocket — `notification.new`,
//! прочтение на другом устройстве — `notification.read`.
//!
//! Каналы push: APNs (iPhone, iPad), Web Push (сайт: Chrome, Firefox, Safari, сайт на экране «Домой»
//! iPhone), позже FCM (Android) — канал добавляется одним модулем сервера.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Вид уведомления.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Новый релиз артиста из подписок (несколько за раз — одно уведомление).
    Release,
    /// Перенос музыки из другого сервиса завершён.
    Import,
    /// «Итоги года» открыты.
    Rewind,
    /// Администратору: спорные треки ждут решения.
    Review,
    /// Администратору: YouTube перестал отдавать звук.
    Sources,
    /// Пробное уведомление из настроек.
    Test,
}

names!(NotificationKind {
    Release => "release",
    Import => "import",
    Rewind => "rewind",
    Review => "review",
    Sources => "sources",
    Test => "test",
});

impl NotificationKind {
    /// Только для администраторов.
    pub fn admin_only(self) -> bool {
        matches!(self, Self::Review | Self::Sources)
    }

    /// Можно выключить в настройках (пробное приходит всегда — его просят сами).
    pub fn configurable(self) -> bool {
        !matches!(self, Self::Test)
    }
}

/// Одно событие внутри уведомления: у «3 новых релиза» — три альбома.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    /// Путь сайта, например `/album/{id}`.
    #[serde(default)]
    #[schema(required = true)]
    pub link: Option<String>,
    /// Обложка: адрес `/api/images/…`.
    #[serde(default)]
    #[schema(required = true)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: Uuid,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    /// Куда ведёт нажатие: путь сайта (`/album/{id}`, `/import`, `/rewind?year=2026`). Приложения
    /// открывают его как `dalywave://album/{id}`.
    #[schema(required = true)]
    pub link: Option<String>,
    /// Картинка: адрес `/api/images/…`.
    #[schema(required = true)]
    pub image: Option<String>,
    /// Сколько событий собрано в уведомлении.
    pub count: i32,
    /// События по отдельности (пусто, если событие одно и всё сказано в заголовке).
    pub items: Vec<NotificationItem>,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub read_at: Option<DateTime<Utc>>,
}

/// Страница центра уведомлений: новые первыми.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPage {
    pub items: Vec<Notification>,
    /// Непрочитанных всего.
    pub unread: i64,
    /// Курсор следующей страницы (`?before=`); `null` — дальше ничего.
    #[schema(required = true)]
    pub next: Option<String>,
}

/// Страница центра уведомлений.
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct NotificationQuery {
    /// `next` из предыдущей страницы.
    pub before: Option<String>,
    /// Сколько вернуть: 1–100, по умолчанию 30.
    pub limit: Option<u32>,
}

/// Отметить прочитанными: перечисленные или (пустой список) все.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationReadRequest {
    #[serde(default)]
    pub ids: Vec<Uuid>,
}

/// Событие WebSocket `notification.read` и ответ на отметку прочтения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsRead {
    /// Прочитанные; пусто вместе с `all` — все.
    pub ids: Vec<Uuid>,
    pub all: bool,
    /// Непрочитанных осталось.
    pub unread: i64,
}

/// Вид уведомлений включён или нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationKindSetting {
    pub kind: NotificationKind,
    pub enabled: bool,
}

/// Тихие часы: в это время push не приходят — они ждут утра (центр уведомлений и окно приложения
/// получают их сразу). Время — минуты от полуночи по часовому поясу человека; `from > to` — через полночь.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuietHours {
    pub enabled: bool,
    /// 0…1439, например 1380 — 23:00.
    pub from: u16,
    /// 0…1439, например 480 — 08:00.
    pub to: u16,
}

impl Default for QuietHours {
    fn default() -> Self {
        Self {
            enabled: true,
            from: 23 * 60,
            to: 8 * 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    /// Виды, доступные этому человеку (админские — только администраторам).
    pub kinds: Vec<NotificationKindSetting>,
    pub quiet: QuietHours,
    /// Часовой пояс для тихих часов: минуты к UTC (Москва — 180).
    pub utc_offset_min: i16,
}

/// Изменить часть настроек.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationSettingsPatch {
    /// Только перечисленные виды; остальные как были.
    #[serde(default)]
    pub kinds: Option<Vec<NotificationKindSetting>>,
    #[serde(default)]
    pub quiet: Option<QuietHours>,
    #[serde(default)]
    pub utc_offset_min: Option<i16>,
}

/// Канал доставки push.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PushChannel {
    /// Apple Push Notification service: приложение на iPhone и iPad.
    Apns,
    /// Web Push (VAPID): сайт в браузере и сайт на экране «Домой».
    Webpush,
    /// Firebase Cloud Messaging: приложение на Android (позже).
    Fcm,
}

names!(PushChannel { Apns => "apns", Webpush => "webpush", Fcm => "fcm" });

/// Среда APNs: отладочные сборки получают токены песочницы, TestFlight и App Store — боевые.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApnsEnvironment {
    Sandbox,
    #[default]
    Production,
}

names!(ApnsEnvironment { Sandbox => "sandbox", Production => "production" });

/// Подписка браузера — `PushSubscription.toJSON()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebPushSubscription {
    pub endpoint: String,
    pub keys: WebPushKeys,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebPushKeys {
    /// Открытый ключ браузера P-256, base64url.
    pub p256dh: String,
    /// Секрет аутентификации, base64url.
    pub auth: String,
}

/// Подписать это устройство на push. Повтор с тем же токеном обновляет подписку (токен APNs стоит
/// присылать при каждом запуске приложения).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushRegistration {
    pub channel: PushChannel,
    /// APNs и FCM: токен устройства (у APNs — hex).
    #[serde(default)]
    pub token: Option<String>,
    /// Web Push: подписка браузера.
    #[serde(default)]
    pub subscription: Option<WebPushSubscription>,
    /// APNs: среда сборки. По умолчанию `production`.
    #[serde(default)]
    pub environment: Option<ApnsEnvironment>,
    /// Как назвать устройство в списке («iPhone», «Chrome»). По умолчанию — по сессии.
    #[serde(default)]
    pub name: Option<String>,
    /// Часовой пояс устройства (минуты к UTC) — для тихих часов.
    #[serde(default)]
    pub utc_offset_min: Option<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PushRegistered {
    pub id: Uuid,
}

/// Отписать устройство: токен APNs/FCM или адрес подписки браузера.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushUnregister {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// Устройство, подписанное на push.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PushDevice {
    pub id: Uuid,
    pub channel: PushChannel,
    pub name: String,
    /// Подписка этой сессии (этого браузера или приложения).
    pub current: bool,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub last_sent_at: Option<DateTime<Utc>>,
}

/// Какие каналы push есть на сервере и какие устройства человека подписаны.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PushInfo {
    /// Открытый ключ VAPID (base64url) для `pushManager.subscribe`; `null` — Web Push на сервере выключен.
    #[schema(required = true)]
    pub web_push_key: Option<String>,
    /// APNs настроен: приложению на iPhone и iPad есть смысл просить разрешение.
    pub apns: bool,
    pub fcm: bool,
    pub devices: Vec<PushDevice>,
}

/// Ответ на пробное уведомление.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PushTest {
    /// На сколько устройств ушёл push.
    pub devices: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shape() {
        let r: PushRegistration = serde_json::from_value(serde_json::json!({
            "channel": "webpush",
            "subscription": { "endpoint": "https://push.example/1", "keys": { "p256dh": "a", "auth": "b" } },
            "utcOffsetMin": 180,
        }))
        .unwrap();
        assert_eq!(r.channel, PushChannel::Webpush);
        assert_eq!(r.utc_offset_min, Some(180));
        assert_eq!(
            serde_json::to_value(NotificationKind::Release).unwrap(),
            "release"
        );
        assert!(NotificationKind::Review.admin_only());
        assert!(!NotificationKind::Test.configurable());
        assert_eq!(QuietHours::default().from, 1380);
    }
}
