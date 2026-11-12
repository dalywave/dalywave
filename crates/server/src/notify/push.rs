//! Каналы push. Каждый канал — свой модуль с `send(target, message) -> Delivery`; здесь выбор канала по
//! подписке. Android (FCM) добавляется так же: модуль `fcm`, поле в [`Push`], ветка в [`Push::send`]
//! и настройки в `config::PushConfig`.

use std::time::Duration;

use dalywave_protocol::notifications::{ApnsEnvironment, NotificationKind, PushChannel};
use uuid::Uuid;

use super::{apns::Apns, webpush::WebPush};
use crate::config::PushConfig;

/// Что уходит на устройство.
#[derive(Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    /// Путь сайта (`/album/{id}`); приложения — [`Message::app_link`].
    pub link: String,
    pub image: Option<String>,
    /// Непрочитанных у человека — число на значке приложения.
    pub unread: i64,
}

impl Message {
    /// Ссылка для приложений: `/album/{id}` → `dalywave://album/{id}`.
    pub fn app_link(&self) -> String {
        format!("dalywave://{}", self.link.trim_start_matches('/'))
    }
}

/// Куда: одна подписка.
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
    pub channel: PushChannel,
    /// APNs, FCM — токен устройства; Web Push — адрес подписки.
    pub token: &'a str,
    pub p256dh: Option<&'a str>,
    pub auth: Option<&'a str>,
    pub environment: Option<ApnsEnvironment>,
}

/// Чем кончилась доставка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    Sent,
    /// Подписки больше нет («токен недействителен»): выключить её.
    Gone(String),
    /// Сбой службы или сети: повторить позже.
    Retry(String),
    /// Повтор не поможет, но и подписка, может быть, жива.
    Failed(String),
}

/// Каналы, настроенные на сервере.
#[derive(Default)]
pub struct Push {
    pub apns: Option<Apns>,
    pub webpush: Option<WebPush>,
}

impl Push {
    /// Каналы из настроек. Канал, который не поднялся (нет файла ключа), выключен — остальное работает.
    pub fn from_config(config: &PushConfig) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("dalywave/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let apns = match &config.apns {
            Some(c) => match Apns::load(c, http.clone()) {
                Ok(a) => Some(a),
                Err(e) => {
                    tracing::warn!(error = %e, "APNs is off");
                    None
                }
            },
            None => None,
        };
        let webpush = match &config.vapid {
            Some(v) => Some(WebPush::new(v, http)?),
            None => None,
        };
        Ok(Self { apns, webpush })
    }

    pub fn enabled(&self, channel: PushChannel) -> bool {
        match channel {
            PushChannel::Apns => self.apns.is_some(),
            PushChannel::Webpush => self.webpush.is_some(),
            PushChannel::Fcm => false,
        }
    }

    /// Доставить; для APNs — ещё и среда, если подписка назвала свою неверно.
    pub async fn send(
        &self,
        target: &Target<'_>,
        message: &Message,
    ) -> (Delivery, Option<ApnsEnvironment>) {
        let off = || Delivery::Failed(format!("канал {} на сервере выключен", target.channel));
        match target.channel {
            PushChannel::Apns => match &self.apns {
                Some(apns) => apns.send(target, message).await,
                None => (off(), None),
            },
            PushChannel::Webpush => match &self.webpush {
                Some(wp) => (wp.send(target, message).await, None),
                None => (off(), None),
            },
            PushChannel::Fcm => (off(), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_links() {
        let m = Message {
            id: Uuid::nil(),
            kind: NotificationKind::Rewind,
            title: String::new(),
            body: String::new(),
            link: "/rewind?year=2026".into(),
            image: None,
            unread: 1,
        };
        assert_eq!(m.app_link(), "dalywave://rewind?year=2026");
    }
}
