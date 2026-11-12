//! Web Push: сайт в браузере (Chrome, Firefox, Safari) и сайт на экране «Домой» iPhone.
//! Подпись сервера — VAPID (RFC 8292, JWT ES256), содержимое шифруется ключами браузера — aes128gcm
//! (RFC 8291). Служба push (FCM для Chrome, Mozilla, Apple) видит только зашифрованные байты.

use std::time::Duration;

use aes_gcm::{
    Aes128Gcm, KeyInit,
    aead::{Aead, Payload},
};
use chrono::Utc;
use hkdf::Hkdf;
use p256::{
    PublicKey, SecretKey,
    ecdsa::{Signature, SigningKey, signature::Signer},
};
use serde_json::json;
use sha2::Sha256;

use super::push::{Delivery, Message, Target};
use crate::{
    config::VapidConfig,
    crypto::{b64, b64_decode, random_bytes},
};

/// Размер записи aes128gcm: всё уведомление умещается в одну запись.
const RECORD_SIZE: u32 = 4096;
/// Служба push держит уведомление для выключенного устройства сутки, дальше оно уже не новость.
const TTL: Duration = Duration::from_secs(24 * 3600);
/// Подпись VAPID живёт 12 часов (служба принимает до суток).
const JWT_LIFETIME: i64 = 12 * 3600;

/// Открытый ключ (base64url, несжатая точка 65 байт) по закрытому.
pub fn public_key(private: &[u8; 32]) -> Option<String> {
    let secret = SecretKey::from_slice(private).ok()?;
    Some(b64(secret.public_key().to_sec1_bytes()))
}

/// Новая пара VAPID в base64url: (открытый, закрытый).
pub fn generate_keys() -> anyhow::Result<(String, String)> {
    loop {
        let bytes = random_bytes::<32>()?;
        // Почти любые 32 байта — ключ; ноль и числа больше порядка кривой — нет
        if let Some(public) = public_key(&bytes) {
            return Ok((public, b64(bytes)));
        }
    }
}

pub struct WebPush {
    http: reqwest::Client,
    signing: SigningKey,
    public_key: String,
    subject: String,
}

impl WebPush {
    pub fn new(config: &VapidConfig, http: reqwest::Client) -> anyhow::Result<Self> {
        let signing = SigningKey::from_slice(&config.private_key)
            .map_err(|e| anyhow::anyhow!("VAPID key: {e}"))?;
        Ok(Self {
            http,
            signing,
            public_key: config.public_key.clone(),
            subject: config.subject.clone(),
        })
    }

    /// Заголовок `Authorization: vapid t=…, k=…` для службы push по адресу подписки.
    fn authorization(&self, endpoint: &str) -> Option<String> {
        let url = url::Url::parse(endpoint).ok()?;
        let audience = url.origin().ascii_serialization();
        let header = b64(json!({ "typ": "JWT", "alg": "ES256" }).to_string());
        let claims = b64(json!({
            "aud": audience,
            "exp": Utc::now().timestamp() + JWT_LIFETIME,
            "sub": self.subject,
        })
        .to_string());
        let input = format!("{header}.{claims}");
        let signature: Signature = self.signing.sign(input.as_bytes());
        Some(format!(
            "vapid t={input}.{}, k={}",
            b64(signature.to_bytes()),
            self.public_key
        ))
    }

    pub async fn send(&self, target: &Target<'_>, message: &Message) -> Delivery {
        let (Some(p256dh), Some(auth)) = (target.p256dh, target.auth) else {
            return Delivery::Gone("у подписки нет ключей шифрования".into());
        };
        let payload = json!({
            "id": message.id,
            "kind": message.kind,
            "title": message.title,
            "body": message.body,
            "link": message.link,
            "image": message.image,
            "unread": message.unread,
        })
        .to_string();
        let body = match encrypt(p256dh, auth, payload.as_bytes()) {
            Ok(b) => b,
            Err(e) => return Delivery::Gone(format!("ключи подписки не подходят: {e}")),
        };
        let Some(authorization) = self.authorization(target.token) else {
            return Delivery::Gone("адрес подписки — не ссылка".into());
        };
        let res = self
            .http
            .post(target.token)
            .header("authorization", authorization)
            .header("content-encoding", "aes128gcm")
            .header("content-type", "application/octet-stream")
            .header("ttl", TTL.as_secs().to_string())
            .header("urgency", "normal")
            .body(body)
            .send()
            .await;
        let res = match res {
            Ok(r) => r,
            Err(e) => return Delivery::Retry(format!("служба Web Push недоступна: {e}")),
        };
        let status = res.status();
        if status.is_success() {
            return Delivery::Sent;
        }
        let text: String = res
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(300)
            .collect();
        match status.as_u16() {
            // Подписки больше нет: браузер отписался, сайт удалён с экрана «Домой»
            404 | 410 => Delivery::Gone(format!("HTTP {status}")),
            429 | 500..=599 => Delivery::Retry(format!("HTTP {status}: {text}")),
            _ => Delivery::Failed(format!("HTTP {status}: {text}")),
        }
    }
}

/// Шифрование содержимого для браузера (RFC 8291, aes128gcm): одноразовый ключ сервера, ECDH с ключом
/// браузера, HKDF с его секретом.
pub fn encrypt(p256dh: &str, auth: &str, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
    let server = loop {
        if let Ok(key) = SecretKey::from_slice(&random_bytes::<32>()?) {
            break key;
        }
    };
    encrypt_with(&server, &random_bytes::<16>()?, p256dh, auth, plaintext)
}

fn encrypt_with(
    server: &SecretKey,
    salt: &[u8; 16],
    p256dh: &str,
    auth: &str,
    plaintext: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let ua_public = b64_decode(p256dh).ok_or_else(|| anyhow::anyhow!("p256dh is not base64url"))?;
    let auth_secret = b64_decode(auth).ok_or_else(|| anyhow::anyhow!("auth is not base64url"))?;
    let ua = PublicKey::from_sec1_bytes(&ua_public).map_err(|_| anyhow::anyhow!("bad p256dh"))?;
    let as_public = server.public_key().to_sec1_bytes();
    let shared = server.diffie_hellman(&ua);

    // IKM = HKDF(auth, ecdh, "WebPush: info" || 0 || ua_public || as_public)
    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(&ua_public);
    key_info.extend_from_slice(&as_public);
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&auth_secret), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|_| anyhow::anyhow!("hkdf"))?;
    let prk = Hkdf::<Sha256>::new(Some(salt), &ikm);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    prk.expand(b"Content-Encoding: aes128gcm\0", &mut cek)
        .map_err(|_| anyhow::anyhow!("hkdf"))?;
    prk.expand(b"Content-Encoding: nonce\0", &mut nonce)
        .map_err(|_| anyhow::anyhow!("hkdf"))?;

    // Одна запись: содержимое и разделитель последней записи 0x02
    let mut padded = plaintext.to_vec();
    padded.push(2);
    anyhow::ensure!(
        padded.len() + 16 <= RECORD_SIZE as usize,
        "notification is too large"
    );
    let cipher = Aes128Gcm::new_from_slice(&cek).map_err(|_| anyhow::anyhow!("aes key"))?;
    let sealed = cipher
        .encrypt(
            &aes_gcm::Nonce::from(nonce),
            Payload {
                msg: &padded,
                aad: b"",
            },
        )
        .map_err(|_| anyhow::anyhow!("aes-gcm"))?;

    let mut out = Vec::with_capacity(16 + 4 + 1 + as_public.len() + sealed.len());
    out.extend_from_slice(salt);
    out.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    out.push(as_public.len() as u8);
    out.extend_from_slice(&as_public);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Расшифровка на стороне браузера — для тестов (и поддельной службы push в них).
pub fn decrypt(ua_private: &[u8], auth: &[u8], body: &[u8]) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(body.len() > 21, "too short");
    let salt = &body[..16];
    let id_len = body[20] as usize;
    let as_public = &body[21..21 + id_len];
    let sealed = &body[21 + id_len..];
    let ua = SecretKey::from_slice(ua_private).map_err(|_| anyhow::anyhow!("ua key"))?;
    let ua_public = ua.public_key().to_sec1_bytes();
    let server = PublicKey::from_sec1_bytes(as_public).map_err(|_| anyhow::anyhow!("as key"))?;
    let shared = ua.diffie_hellman(&server);
    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(&ua_public);
    key_info.extend_from_slice(as_public);
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(auth), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|_| anyhow::anyhow!("hkdf"))?;
    let prk = Hkdf::<Sha256>::new(Some(salt), &ikm);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    prk.expand(b"Content-Encoding: aes128gcm\0", &mut cek)
        .map_err(|_| anyhow::anyhow!("hkdf"))?;
    prk.expand(b"Content-Encoding: nonce\0", &mut nonce)
        .map_err(|_| anyhow::anyhow!("hkdf"))?;
    let cipher = Aes128Gcm::new_from_slice(&cek).map_err(|_| anyhow::anyhow!("aes key"))?;
    let mut plain = cipher
        .decrypt(&aes_gcm::Nonce::from(nonce), sealed)
        .map_err(|_| anyhow::anyhow!("aes-gcm"))?;
    // Разделитель записи и нули дополнения — с конца
    while plain.last() == Some(&0) {
        plain.pop();
    }
    anyhow::ensure!(plain.pop() == Some(2), "no record delimiter");
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Vec<u8> {
        b64_decode(s).unwrap()
    }

    /// Пример из RFC 8291, раздел 5.
    #[test]
    fn rfc8291_example() {
        let server =
            SecretKey::from_slice(&d("yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw")).unwrap();
        assert_eq!(
            b64(server.public_key().to_sec1_bytes()),
            "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8"
        );
        let salt: [u8; 16] = d("DGv6ra1nlYgDCS1FRnbzlw").try_into().unwrap();
        let body = encrypt_with(
            &server,
            &salt,
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
            "BTBZMqHH6r4Tts7J_aSIgg",
            b"When I grow up, I want to be a watermelon",
        )
        .unwrap();
        assert_eq!(
            b64(&body),
            "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN"
        );
        let plain = decrypt(
            &d("q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94"),
            &d("BTBZMqHH6r4Tts7J_aSIgg"),
            &body,
        )
        .unwrap();
        assert_eq!(plain, b"When I grow up, I want to be a watermelon");
    }

    #[test]
    fn roundtrip_with_fresh_keys() {
        let (ua_public, ua_private) = generate_keys().unwrap();
        let auth = b64(random_bytes::<16>().unwrap());
        let body = encrypt(&ua_public, &auth, "Новый релиз".as_bytes()).unwrap();
        let plain = decrypt(&d(&ua_private), &d(&auth), &body).unwrap();
        assert_eq!(plain, "Новый релиз".as_bytes());
        assert!(encrypt("not a key", &auth, b"x").is_err());
    }

    #[test]
    fn vapid_header_is_a_signed_jwt() {
        let (public, private) = generate_keys().unwrap();
        let mut key = [0u8; 32];
        key.copy_from_slice(&d(&private));
        let wp = WebPush::new(
            &VapidConfig {
                public_key: public.clone(),
                private_key: key,
                subject: "https://wave.daly.rest".into(),
            },
            reqwest::Client::new(),
        )
        .unwrap();
        let header = wp
            .authorization("https://fcm.googleapis.com/fcm/send/abc")
            .unwrap();
        let (t, k) = header
            .strip_prefix("vapid t=")
            .unwrap()
            .split_once(", k=")
            .unwrap();
        assert_eq!(k, public);
        let parts: Vec<&str> = t.split('.').collect();
        assert_eq!(parts.len(), 3);
        let claims: serde_json::Value = serde_json::from_slice(&d(parts[1])).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        assert_eq!(claims["sub"], "https://wave.daly.rest");
        use p256::ecdsa::{VerifyingKey, signature::Verifier};
        let vk = VerifyingKey::from_sec1_bytes(&d(&public)).unwrap();
        let sig = Signature::from_slice(&d(parts[2])).unwrap();
        vk.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig)
            .unwrap();
    }
}
