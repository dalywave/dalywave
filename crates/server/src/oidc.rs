//! Клиент OpenID Connect для входа через daly.rest: authorization code + PKCE (S256),
//! id_token ES256 с ключами из JWKS, роли из claim `roles`.
//!
//! Своя небольшая реализация вместо крейта openidconnect: провайдер один, алгоритм один,
//! claim `roles` нестандартный, а openidconnect тянет RSA, Ed25519, вторую версию reqwest и sha2.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, bail, ensure};
use axum::http::header;
use base64::{Engine, engine::general_purpose::STANDARD};
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::sync::RwLock;

use crate::{
    config::OidcConfig,
    crypto::{b64_decode, secrets_match},
};

pub const SCOPE: &str = "openid email profile";
const DISCOVERY_TTL: Duration = Duration::from_secs(3600);
const JWKS_TTL: Duration = Duration::from_secs(3600);
/// Неизвестный kid заставляет перечитать JWKS, но не чаще этого
const JWKS_REFRESH_GAP: Duration = Duration::from_secs(30);
/// Допуск на расхождение часов с провайдером
const LEEWAY_S: i64 = 60;

#[derive(Debug, Clone, Deserialize)]
pub struct Discovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: String,
}

struct Jwk {
    kid: Option<String>,
    key: VerifyingKey,
}

/// Профиль человека по данным провайдера.
#[derive(Debug, Clone)]
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    /// Роли daly.rest: `["admin"]` у админов
    pub roles: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    /// nonce в id_token не тот, что выдан этому браузеру: ответ подменили или повторили
    #[error("id_token nonce does not match")]
    Nonce,
    #[error(transparent)]
    Provider(#[from] anyhow::Error),
}

type Cached<T> = RwLock<Option<(Instant, Arc<T>)>>;

pub struct Oidc {
    pub config: OidcConfig,
    http: reqwest::Client,
    discovery: Cached<Discovery>,
    keys: Cached<Vec<Jwk>>,
}

enum KeyMatch {
    Verified,
    /// Ключа с таким kid нет: возможно, провайдер его только что сменил
    Unknown,
    Invalid,
}

impl Oidc {
    pub fn new(config: OidcConfig, http: reqwest::Client) -> Self {
        Self {
            config,
            http,
            discovery: RwLock::new(None),
            keys: RwLock::new(None),
        }
    }

    /// `/.well-known/openid-configuration` провайдера, кешируется на час.
    pub async fn discovery(&self) -> anyhow::Result<Arc<Discovery>> {
        if let Some((at, doc)) = &*self.discovery.read().await
            && at.elapsed() < DISCOVERY_TTL
        {
            return Ok(doc.clone());
        }
        let url = format!("{}/.well-known/openid-configuration", self.config.issuer);
        let doc: Discovery = serde_json::from_value(self.get_json(&url, None).await?)
            .context("bad discovery document")?;
        // OIDC Discovery 4.3: документ должен принадлежать тому же issuer, что настроен у нас
        ensure!(
            doc.issuer.trim_end_matches('/') == self.config.issuer,
            "discovery issuer {} does not match {}",
            doc.issuer,
            self.config.issuer
        );
        let doc = Arc::new(doc);
        *self.discovery.write().await = Some((Instant::now(), doc.clone()));
        Ok(doc)
    }

    pub fn authorize_url(
        &self,
        doc: &Discovery,
        redirect_uri: &str,
        state: &str,
        nonce: &str,
        code_challenge: &str,
    ) -> anyhow::Result<String> {
        let mut url =
            url::Url::parse(&doc.authorization_endpoint).context("bad authorization_endpoint")?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", SCOPE)
            .append_pair("state", state)
            .append_pair("nonce", nonce)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url.into())
    }

    /// Меняет код на токены (только здесь, на сервере), проверяет id_token и собирает профиль.
    /// Роли берутся из id_token, а если их там нет — из userinfo.
    pub async fn exchange(
        &self,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
        nonce: &str,
    ) -> Result<Identity, OidcError> {
        let doc = self.discovery().await?;
        let tokens = self.redeem(&doc, code, verifier, redirect_uri).await?;
        let id_token = tokens["id_token"]
            .as_str()
            .context("no id_token in the token response")?;
        let claims = self.verify(id_token, &doc).await?;
        let token_nonce = claims.get("nonce").and_then(Value::as_str).unwrap_or("");
        if !secrets_match(token_nonce, nonce) {
            return Err(OidcError::Nonce);
        }
        let mut identity = identity(&doc.issuer, &claims)?;
        if roles(claims.get("roles")).is_none() {
            let access = tokens["access_token"]
                .as_str()
                .context("no access_token for userinfo")?;
            self.fill_from_userinfo(&doc, access, &mut identity).await?;
        }
        Ok(identity)
    }

    async fn redeem(
        &self,
        doc: &Discovery,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> anyhow::Result<Value> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("code_verifier", verifier)
            .finish();
        let res = self
            .http
            .post(&doc.token_endpoint)
            .header(
                header::AUTHORIZATION,
                basic_auth(&self.config.client_id, &self.config.client_secret),
            )
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ACCEPT, "application/json")
            .body(body)
            .send()
            .await
            .context("token request failed")?;
        let status = res.status();
        let tokens: Value = res.json().await.context("token response is not JSON")?;
        // В теле ошибки только код OAuth, секретов там нет
        ensure!(
            status.is_success(),
            "token endpoint answered {status}: {}",
            tokens["error"]
        );
        Ok(tokens)
    }

    async fn fill_from_userinfo(
        &self,
        doc: &Discovery,
        access_token: &str,
        identity: &mut Identity,
    ) -> anyhow::Result<()> {
        let endpoint = doc
            .userinfo_endpoint
            .as_deref()
            .context("provider has no userinfo endpoint")?;
        let info = self.get_json(endpoint, Some(access_token)).await?;
        let info = info.as_object().context("userinfo is not an object")?;
        // OIDC Core 5.3.4: userinfo обязан быть про того же человека
        ensure!(
            info.get("sub").and_then(Value::as_str) == Some(identity.subject.as_str()),
            "userinfo sub mismatch"
        );
        identity.roles = roles(info.get("roles")).unwrap_or_default();
        if identity.email.is_none() {
            identity.email = text(info.get("email"));
            identity.email_verified = flag(info.get("email_verified"));
        }
        if identity.name.is_none() {
            identity.name = display_name(info);
        }
        if identity.avatar_url.is_none() {
            identity.avatar_url = text(info.get("picture"));
        }
        Ok(())
    }

    /// Подпись ES256 и claims iss, aud, azp, exp, iat. nonce сверяет вызывающий.
    async fn verify(&self, token: &str, doc: &Discovery) -> anyhow::Result<Map<String, Value>> {
        let mut parts = token.split('.');
        let (Some(head), Some(payload), Some(sig), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            bail!("id_token is not a compact JWS");
        };
        let header: Value =
            serde_json::from_slice(&b64_decode(head).context("bad id_token header")?)?;
        // Только ES256: никаких none и подмены алгоритма
        ensure!(
            header["alg"] == "ES256",
            "unexpected id_token alg {}",
            header["alg"]
        );
        let signature = Signature::from_slice(&b64_decode(sig).context("bad id_token signature")?)
            .context("bad ES256 signature")?;
        let signed = &token.as_bytes()[..head.len() + 1 + payload.len()];
        let kid = header["kid"].as_str();

        let mut keys = self.keys(false).await?;
        let mut result = check(&keys.1, kid, signed, &signature);
        if matches!(result, KeyMatch::Unknown) && keys.0.elapsed() >= JWKS_REFRESH_GAP {
            keys = self.keys(true).await?;
            result = check(&keys.1, kid, signed, &signature);
        }
        ensure!(
            matches!(result, KeyMatch::Verified),
            "id_token signature is invalid"
        );

        let claims: Map<String, Value> =
            serde_json::from_slice(&b64_decode(payload).context("bad id_token payload")?)
                .context("bad id_token claims")?;
        check_claims(
            &claims,
            &doc.issuer,
            &self.config.client_id,
            chrono::Utc::now().timestamp(),
        )?;
        Ok(claims)
    }

    async fn keys(&self, refresh: bool) -> anyhow::Result<(Instant, Arc<Vec<Jwk>>)> {
        if !refresh
            && let Some((at, keys)) = &*self.keys.read().await
            && at.elapsed() < JWKS_TTL
        {
            return Ok((*at, keys.clone()));
        }
        let doc = self.discovery().await?;
        let jwks = self.get_json(&doc.jwks_uri, None).await?;
        let keys = Arc::new(parse_jwks(&jwks));
        ensure!(!keys.is_empty(), "provider JWKS has no P-256 keys");
        let at = Instant::now();
        *self.keys.write().await = Some((at, keys.clone()));
        Ok((at, keys))
    }

    async fn get_json(&self, url: &str, bearer: Option<&str>) -> anyhow::Result<Value> {
        let mut req = self
            .http
            .get(url)
            .header(header::ACCEPT, "application/json");
        if let Some(token) = bearer {
            req = req.bearer_auth(token);
        }
        let res = req.send().await.with_context(|| format!("GET {url}"))?;
        let status = res.status();
        ensure!(status.is_success(), "GET {url} answered {status}");
        res.json()
            .await
            .with_context(|| format!("GET {url}: not JSON"))
    }
}

/// client_secret_basic: по RFC 6749 2.3.1 обе части перед base64 кодируются как form-urlencoded.
fn basic_auth(client_id: &str, secret: &str) -> String {
    let enc = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
    format!(
        "Basic {}",
        STANDARD.encode(format!("{}:{}", enc(client_id), enc(secret)))
    )
}

fn parse_jwks(doc: &Value) -> Vec<Jwk> {
    let Some(list) = doc["keys"].as_array() else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|k| {
            if k["kty"] != "EC" || k["crv"] != "P-256" {
                return None;
            }
            if k.get("use").is_some_and(|u| u != "sig")
                || k.get("alg").is_some_and(|a| a != "ES256")
            {
                return None;
            }
            let x = b64_decode(k["x"].as_str()?)?;
            let y = b64_decode(k["y"].as_str()?)?;
            if x.len() != 32 || y.len() != 32 {
                return None;
            }
            // Несжатая точка SEC1: 0x04 || x || y
            let point: Vec<u8> = [&[4u8][..], &x, &y].concat();
            let key = VerifyingKey::from_sec1_bytes(&point).ok()?;
            Some(Jwk {
                kid: k["kid"].as_str().map(str::to_string),
                key,
            })
        })
        .collect()
}

fn check(keys: &[Jwk], kid: Option<&str>, signed: &[u8], signature: &Signature) -> KeyMatch {
    let candidates: Vec<&Jwk> = match kid {
        Some(kid) => keys
            .iter()
            .filter(|k| k.kid.as_deref() == Some(kid))
            .collect(),
        None => keys.iter().collect(),
    };
    if candidates.is_empty() {
        return KeyMatch::Unknown;
    }
    if candidates
        .iter()
        .any(|k| k.key.verify(signed, signature).is_ok())
    {
        KeyMatch::Verified
    } else {
        KeyMatch::Invalid
    }
}

fn check_claims(
    claims: &Map<String, Value>,
    issuer: &str,
    client_id: &str,
    now: i64,
) -> anyhow::Result<()> {
    ensure!(
        claims.get("iss").and_then(Value::as_str) == Some(issuer),
        "id_token iss mismatch"
    );
    let audience_ok = match claims.get("aud") {
        Some(Value::String(aud)) => aud == client_id,
        Some(Value::Array(list)) => list.iter().any(|a| a == client_id),
        _ => false,
    };
    ensure!(audience_ok, "id_token is issued for another client");
    // OIDC Core 3.1.3.7: при нескольких аудиториях azp обязан указывать на нас
    let many = matches!(claims.get("aud"), Some(Value::Array(list)) if list.len() > 1);
    let azp = claims.get("azp").and_then(Value::as_str);
    ensure!(
        azp.map_or(!many, |a| a == client_id),
        "id_token azp mismatch"
    );
    let exp = claims
        .get("exp")
        .and_then(Value::as_i64)
        .context("id_token has no exp")?;
    ensure!(exp + LEEWAY_S > now, "id_token has expired");
    if let Some(iat) = claims.get("iat").and_then(Value::as_i64) {
        ensure!(iat - LEEWAY_S <= now, "id_token is issued in the future");
    }
    Ok(())
}

fn text(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Некоторые провайдеры присылают email_verified строкой.
fn flag(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    }
}

/// Роли только списком строк; что-то другое считаем отсутствием claim.
fn roles(v: Option<&Value>) -> Option<Vec<String>> {
    v?.as_array()?
        .iter()
        .map(|r| r.as_str().map(str::to_string))
        .collect()
}

fn display_name(c: &Map<String, Value>) -> Option<String> {
    text(c.get("name")).or_else(|| text(c.get("preferred_username")))
}

fn identity(issuer: &str, c: &Map<String, Value>) -> anyhow::Result<Identity> {
    let subject = text(c.get("sub")).context("id_token has no sub")?;
    Ok(Identity {
        issuer: issuer.to_string(),
        subject,
        email: text(c.get("email")),
        email_verified: flag(c.get("email_verified")),
        name: display_name(c),
        avatar_url: text(c.get("picture")),
        roles: roles(c.get("roles")).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn claims(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn claims_are_checked() {
        let good = json!({"iss": "https://daly.rest", "aud": "dw", "exp": 2_000, "iat": 1_000, "sub": "u1"});
        assert!(check_claims(&claims(good.clone()), "https://daly.rest", "dw", 1_500).is_ok());
        assert!(
            check_claims(&claims(good.clone()), "https://evil.example", "dw", 1_500).is_err(),
            "чужой iss"
        );
        assert!(
            check_claims(&claims(good.clone()), "https://daly.rest", "other", 1_500).is_err(),
            "чужой aud"
        );
        assert!(
            check_claims(&claims(good.clone()), "https://daly.rest", "dw", 2_100).is_err(),
            "истёк"
        );
        assert!(
            check_claims(&claims(good), "https://daly.rest", "dw", 500).is_err(),
            "из будущего"
        );

        let many = json!({"iss": "i", "aud": ["dw", "x"], "exp": 2_000, "sub": "u"});
        assert!(
            check_claims(&claims(many.clone()), "i", "dw", 1_500).is_err(),
            "нет azp при нескольких aud"
        );
        let mut with_azp = many;
        with_azp["azp"] = json!("dw");
        assert!(check_claims(&claims(with_azp), "i", "dw", 1_500).is_ok());
    }

    #[test]
    fn roles_and_flags() {
        assert_eq!(
            roles(Some(&json!(["admin"]))),
            Some(vec!["admin".to_string()])
        );
        assert_eq!(roles(Some(&json!([]))), Some(vec![]));
        assert_eq!(roles(Some(&json!("admin"))), None);
        assert_eq!(roles(Some(&json!([1]))), None);
        assert_eq!(roles(None), None);
        assert!(flag(Some(&json!("true"))));
        assert!(!flag(Some(&json!(false))));
    }

    #[test]
    fn basic_auth_is_form_encoded() {
        let header = basic_auth("my app", "s+cret");
        let raw = STANDARD
            .decode(header.trim_start_matches("Basic "))
            .unwrap();
        assert_eq!(String::from_utf8(raw).unwrap(), "my+app:s%2Bcret");
    }
}
