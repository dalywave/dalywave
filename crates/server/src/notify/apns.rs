//! APNs — уведомления приложению на iPhone и iPad. HTTP/2 к api.push.apple.com, вход — токен JWT
//! (ES256) по ключу .p8 из портала разработчика: Key ID и Team ID в заголовке и утверждениях.
//! Токен живёт час; Apple просит обновлять его не чаще раза в 20 минут — обновляем раз в 50.
//!
//! Среда — у подписки: отладочная сборка из Xcode получает токены песочницы, TestFlight и App Store —
//! боевые. Если APNs ответил «чужой токен» (`BadDeviceToken`), пробуем другую среду один раз: так
//! переживается сборка, неверно назвавшая свою среду.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use chrono::Utc;
use dalywave_protocol::notifications::ApnsEnvironment;
use p256::{
    SecretKey,
    ecdsa::{Signature, SigningKey, signature::Signer},
};
use serde::Deserialize;
use serde_json::json;

use super::push::{Delivery, Message, Target};
use crate::{config::ApnsConfig, crypto::b64};

const TOKEN_REFRESH: Duration = Duration::from_secs(50 * 60);
/// Выключенное устройство получит уведомление в течение суток, дальше оно уже не новость.
const EXPIRATION: i64 = 24 * 3600;

pub struct Apns {
    http: reqwest::Client,
    signing: SigningKey,
    config: ApnsConfig,
    token: Mutex<Option<(String, Instant)>>,
}

#[derive(Deserialize)]
struct Reason {
    #[serde(default)]
    reason: String,
}

/// Чем кончилась одна попытка к одной среде.
enum Answer {
    Sent,
    /// Токен не этой среды (или вовсе не токен).
    WrongToken(String),
    Other(Delivery),
}

impl Apns {
    /// Ключ .p8 (PKCS#8 PEM) — из файла `key_path`.
    pub fn load(config: &ApnsConfig, http: reqwest::Client) -> anyhow::Result<Self> {
        let pem = std::fs::read_to_string(&config.key_path).map_err(|e| {
            anyhow::anyhow!(
                "cannot read the APNs key {}: {e}",
                config.key_path.display()
            )
        })?;
        Self::from_pem(config, &pem, http)
    }

    pub fn from_pem(config: &ApnsConfig, pem: &str, http: reqwest::Client) -> anyhow::Result<Self> {
        let secret = SecretKey::from_pem(pem.trim())
            .map_err(|e| anyhow::anyhow!("the APNs key is not a P-256 .p8 key: {e}"))?;
        Ok(Self {
            http,
            signing: SigningKey::from(&secret),
            config: config.clone(),
            token: Mutex::new(None),
        })
    }

    /// Токен входа: прежний, пока ему меньше 50 минут.
    fn bearer(&self) -> String {
        let mut cached = self.token.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((token, at)) = cached.as_ref()
            && at.elapsed() < TOKEN_REFRESH
        {
            return token.clone();
        }
        let header = b64(json!({ "alg": "ES256", "kid": self.config.key_id }).to_string());
        let claims =
            b64(json!({ "iss": self.config.team_id, "iat": Utc::now().timestamp() }).to_string());
        let input = format!("{header}.{claims}");
        let signature: Signature = self.signing.sign(input.as_bytes());
        let token = format!("{input}.{}", b64(signature.to_bytes()));
        *cached = Some((token.clone(), Instant::now()));
        token
    }

    /// Apple отверг токен входа (истёк, часы сервера): следующий запрос подпишется заново.
    fn forget_bearer(&self) {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn base(&self, env: ApnsEnvironment) -> &str {
        match env {
            ApnsEnvironment::Production => &self.config.production_url,
            ApnsEnvironment::Sandbox => &self.config.sandbox_url,
        }
    }

    /// Полезная нагрузка: системный баннер, звук, число на значке и ссылка `dalywave://…` для нажатия.
    pub fn payload(message: &Message) -> serde_json::Value {
        json!({
            "aps": {
                "alert": { "title": message.title, "body": message.body },
                "sound": "default",
                "badge": message.unread,
                "thread-id": message.kind,
            },
            "link": message.app_link(),
            "id": message.id,
        })
    }

    /// Отправить; при «чужом токене» — ещё раз в другую среду. Вторая среда, если сработала, — в ответе.
    pub async fn send(
        &self,
        target: &Target<'_>,
        message: &Message,
    ) -> (Delivery, Option<ApnsEnvironment>) {
        let env = target.environment.unwrap_or_default();
        match self.attempt(env, target.token, message).await {
            Answer::Sent => (Delivery::Sent, None),
            Answer::Other(d) => (d, None),
            Answer::WrongToken(reason) => {
                let other = match env {
                    ApnsEnvironment::Production => ApnsEnvironment::Sandbox,
                    ApnsEnvironment::Sandbox => ApnsEnvironment::Production,
                };
                match self.attempt(other, target.token, message).await {
                    Answer::Sent => (Delivery::Sent, Some(other)),
                    Answer::Other(d) => (d, None),
                    Answer::WrongToken(_) => (Delivery::Gone(reason), None),
                }
            }
        }
    }

    async fn attempt(&self, env: ApnsEnvironment, token: &str, message: &Message) -> Answer {
        if token.is_empty() || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Answer::Other(Delivery::Gone("токен APNs — не hex".into()));
        }
        let url = format!("{}/3/device/{token}", self.base(env));
        let res = self
            .http
            .post(&url)
            .header("authorization", format!("bearer {}", self.bearer()))
            .header("apns-topic", &self.config.topic)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .header(
                "apns-expiration",
                (Utc::now().timestamp() + EXPIRATION).to_string(),
            )
            // Собранное уведомление («3 новых релиза») заменяет на экране прежнее с тем же id
            .header("apns-collapse-id", message.id.to_string())
            .json(&Self::payload(message))
            .send()
            .await;
        let res = match res {
            Ok(r) => r,
            Err(e) => return Answer::Other(Delivery::Retry(format!("APNs недоступен: {e}"))),
        };
        let status = res.status();
        if status.is_success() {
            return Answer::Sent;
        }
        let reason = res
            .json::<Reason>()
            .await
            .map(|r| r.reason)
            .unwrap_or_default();
        let text = format!("APNs {status}: {reason}");
        match (status.as_u16(), reason.as_str()) {
            (400, "BadDeviceToken") => Answer::WrongToken(text),
            // Приложение удалено или уведомления выключены
            (410, _) | (400, "DeviceTokenNotForTopic") => Answer::Other(Delivery::Gone(text)),
            (403, "ExpiredProviderToken" | "InvalidProviderToken") => {
                self.forget_bearer();
                Answer::Other(Delivery::Retry(text))
            }
            (429, _) | (500..=599, _) => Answer::Other(Delivery::Retry(text)),
            _ => Answer::Other(Delivery::Failed(text)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ключ только для тестов (сгенерирован для них, нигде не зарегистрирован).
    pub const TEST_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg6SaekJxSZC4o4BWu
a0FcuNrh88dL09FQibl8gPEHg++hRANCAARwCAOsN4ViqE37jC8jSECODRd13ia4
hJQ3qUm5USAhKHOp+Ru9zn9P/xGT/50v4MOZUMS4WiYwXyy2BRKUh+p/
-----END PRIVATE KEY-----";

    fn apns() -> Apns {
        let config = ApnsConfig {
            key_path: "/dev/null".into(),
            key_id: "ABC123DEFG".into(),
            team_id: "Y5KR5JPCRG".into(),
            topic: "rest.daly.wave".into(),
            production_url: "http://127.0.0.1:9".into(),
            sandbox_url: "http://127.0.0.1:9".into(),
        };
        Apns::from_pem(&config, TEST_KEY, reqwest::Client::new()).unwrap()
    }

    #[test]
    fn bearer_is_cached_and_signed() {
        let a = apns();
        let t = a.bearer();
        assert_eq!(t, a.bearer(), "токен не пересоздаётся на каждый запрос");
        let parts: Vec<&str> = t.split('.').collect();
        let header: serde_json::Value =
            serde_json::from_slice(&crate::crypto::b64_decode(parts[0]).unwrap()).unwrap();
        assert_eq!(header["kid"], "ABC123DEFG");
        let claims: serde_json::Value =
            serde_json::from_slice(&crate::crypto::b64_decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["iss"], "Y5KR5JPCRG");
        a.forget_bearer();
        assert!(a.token.lock().unwrap().is_none());
    }

    #[test]
    fn rejects_garbage_keys() {
        let mut config = apns().config;
        config.key_path = "/nonexistent/AuthKey.p8".into();
        assert!(Apns::load(&config, reqwest::Client::new()).is_err());
        assert!(Apns::from_pem(&config, "not a key", reqwest::Client::new()).is_err());
    }
}
