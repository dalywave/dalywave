//! Клиент OpenID Connect: authorization code + PKCE (S256), id_token с ключами из JWKS
//! (RS256/PS256, ES256/ES384, EdDSA; HS256 — по секрету клиента), роли или группы из настроенного claim.
//! Провайдеров несколько ([`crate::config::OidcConfig`]): у каждого свой [`Oidc`] со своим кешем.
//!
//! Своя небольшая реализация вместо крейта openidconnect: он тянет вторую версию reqwest и sha2,
//! а подписи проверяет ring, который уже есть в сборке через rustls.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, bail, ensure};
use axum::http::header;
use base64::{Engine, engine::general_purpose::STANDARD};
use ring::{hmac, signature};
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::sync::RwLock;

use crate::{
    config::{ClientAuth, OidcConfig},
    crypto::{b64_decode, secrets_match},
};

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
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Option<Vec<String>>,
}

/// Открытый ключ из JWKS.
#[derive(Debug)]
enum Key {
    Rsa {
        n: Vec<u8>,
        e: Vec<u8>,
    },
    /// Несжатая точка SEC1: 0x04 || x || y
    P256(Vec<u8>),
    P384(Vec<u8>),
    Ed25519(Vec<u8>),
}

#[derive(Debug)]
struct Jwk {
    kid: Option<String>,
    /// Алгоритм, если ключ его называет: тогда другим алгоритмом им не проверяем
    alg: Option<String>,
    key: Key,
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
    /// Значения claim ролей или групп из настроек провайдера (`roles`, `groups`…)
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

    pub fn id(&self) -> &str {
        &self.config.id
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
            .append_pair("scope", &self.config.scopes)
            .append_pair("state", state)
            .append_pair("nonce", nonce)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url.into())
    }

    /// Меняет код на токены (только здесь, на сервере), проверяет id_token и собирает профиль.
    /// Claim ролей берётся из id_token, а если его там нет — из userinfo.
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
        let claim = self.config.admin_claim.as_deref();
        let mut identity = identity(&doc.issuer, &claims, claim)?;
        let roles_missing = claim.is_some_and(|c| roles(lookup(&claims, c)).is_none());
        let profile_missing = identity.email.is_none() && identity.name.is_none();
        if (roles_missing || profile_missing)
            && doc.userinfo_endpoint.is_some()
            && let Some(access) = tokens["access_token"].as_str()
        {
            self.fill_from_userinfo(&doc, access, &mut identity).await?;
        }
        Ok(identity)
    }

    fn client_auth(&self, doc: &Discovery) -> ClientAuth {
        if let Some(auth) = self.config.client_auth {
            return auth;
        }
        // По умолчанию (RFC 8414) провайдер умеет client_secret_basic; post — только если basic он не называет
        match &doc.token_endpoint_auth_methods_supported {
            Some(list)
                if !list.iter().any(|m| m == "client_secret_basic")
                    && list.iter().any(|m| m == "client_secret_post") =>
            {
                ClientAuth::Post
            }
            _ => ClientAuth::Basic,
        }
    }

    async fn redeem(
        &self,
        doc: &Discovery,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> anyhow::Result<Value> {
        let mut req = self.http.post(&doc.token_endpoint);
        // Сериализатор формы не Send: тело собирается целиком до первого await
        let body = {
            let mut form = url::form_urlencoded::Serializer::new(String::new());
            form.append_pair("grant_type", "authorization_code")
                .append_pair("code", code)
                .append_pair("redirect_uri", redirect_uri)
                .append_pair("code_verifier", verifier);
            match (&self.config.client_secret, self.client_auth(doc)) {
                (Some(secret), ClientAuth::Basic) => {
                    req = req.header(
                        header::AUTHORIZATION,
                        basic_auth(&self.config.client_id, secret),
                    );
                }
                (Some(secret), ClientAuth::Post) => {
                    form.append_pair("client_id", &self.config.client_id)
                        .append_pair("client_secret", secret);
                }
                // Публичный клиент: только client_id, защищает PKCE
                (None, _) => {
                    form.append_pair("client_id", &self.config.client_id);
                }
            }
            form.finish()
        };
        let res = req
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
        if let Some(claim) = self.config.admin_claim.as_deref()
            && let Some(found) = roles(lookup(info, claim))
        {
            identity.roles = found;
        }
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

    /// Подпись и claims iss, aud, azp, exp, iat. nonce сверяет вызывающий.
    async fn verify(&self, token: &str, doc: &Discovery) -> anyhow::Result<Map<String, Value>> {
        let mut parts = token.split('.');
        let (Some(head), Some(payload), Some(sig), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            bail!("id_token is not a compact JWS");
        };
        let header: Value =
            serde_json::from_slice(&b64_decode(head).context("bad id_token header")?)?;
        let alg = header["alg"].as_str().unwrap_or_default();
        let signature = b64_decode(sig).context("bad id_token signature")?;
        let signed = &token.as_bytes()[..head.len() + 1 + payload.len()];

        if let Some(hash) = hmac_alg(alg) {
            // HS256 подписан секретом клиента (так по умолчанию у Authentik без ключа подписи).
            // Публичный клиент секрета не имеет — такой токен не принимаем
            let secret = self
                .config
                .client_secret
                .as_deref()
                .context("HS* id_token needs a client secret")?;
            let key = hmac::Key::new(hash, secret.as_bytes());
            ensure!(
                hmac::verify(&key, signed, &signature).is_ok(),
                "id_token signature is invalid"
            );
        } else {
            // Только известные асимметричные алгоритмы: никаких none и подмены алгоритма
            ensure!(
                verifier(alg).is_some(),
                "unexpected id_token alg {}",
                header["alg"]
            );
            let kid = header["kid"].as_str();
            let mut keys = self.keys(false).await?;
            let mut result = check(&keys.1, kid, alg, signed, &signature);
            if matches!(result, KeyMatch::Unknown) && keys.0.elapsed() >= JWKS_REFRESH_GAP {
                keys = self.keys(true).await?;
                result = check(&keys.1, kid, alg, signed, &signature);
            }
            ensure!(
                matches!(result, KeyMatch::Verified),
                "id_token signature is invalid"
            );
        }

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
        ensure!(!keys.is_empty(), "provider JWKS has no usable signing keys");
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

fn hmac_alg(alg: &str) -> Option<hmac::Algorithm> {
    match alg {
        "HS256" => Some(hmac::HMAC_SHA256),
        "HS384" => Some(hmac::HMAC_SHA384),
        "HS512" => Some(hmac::HMAC_SHA512),
        _ => None,
    }
}

/// Проверка подписи для алгоритма JWS (RFC 7518, RFC 8037).
fn verifier(alg: &str) -> Option<&'static dyn signature::VerificationAlgorithm> {
    Some(match alg {
        "RS256" => &signature::RSA_PKCS1_2048_8192_SHA256,
        "RS384" => &signature::RSA_PKCS1_2048_8192_SHA384,
        "RS512" => &signature::RSA_PKCS1_2048_8192_SHA512,
        "PS256" => &signature::RSA_PSS_2048_8192_SHA256,
        "PS384" => &signature::RSA_PSS_2048_8192_SHA384,
        "PS512" => &signature::RSA_PSS_2048_8192_SHA512,
        "ES256" => &signature::ECDSA_P256_SHA256_FIXED,
        "ES384" => &signature::ECDSA_P384_SHA384_FIXED,
        "EdDSA" => &signature::ED25519,
        _ => return None,
    })
}

/// Подходит ли ключ к алгоритму: RSA — к RS*/PS*, P-256 — к ES256 и т. д.
fn fits(key: &Key, alg: &str) -> bool {
    match key {
        Key::Rsa { .. } => alg.starts_with("RS") || alg.starts_with("PS"),
        Key::P256(_) => alg == "ES256",
        Key::P384(_) => alg == "ES384",
        Key::Ed25519(_) => alg == "EdDSA",
    }
}

fn verify_with(key: &Key, alg: &str, signed: &[u8], sig: &[u8]) -> bool {
    let Some(algorithm) = verifier(alg) else {
        return false;
    };
    if !fits(key, alg) {
        return false;
    }
    match key {
        Key::Rsa { n, e } => signature::RsaPublicKeyComponents { n, e }
            .verify(
                match alg {
                    "RS256" => &signature::RSA_PKCS1_2048_8192_SHA256,
                    "RS384" => &signature::RSA_PKCS1_2048_8192_SHA384,
                    "RS512" => &signature::RSA_PKCS1_2048_8192_SHA512,
                    "PS256" => &signature::RSA_PSS_2048_8192_SHA256,
                    "PS384" => &signature::RSA_PSS_2048_8192_SHA384,
                    _ => &signature::RSA_PSS_2048_8192_SHA512,
                },
                signed,
                sig,
            )
            .is_ok(),
        Key::P256(point) | Key::P384(point) | Key::Ed25519(point) => {
            signature::UnparsedPublicKey::new(algorithm, point)
                .verify(signed, sig)
                .is_ok()
        }
    }
}

fn parse_jwks(doc: &Value) -> Vec<Jwk> {
    let Some(list) = doc["keys"].as_array() else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|k| {
            if k.get("use").is_some_and(|u| u != "sig") {
                return None;
            }
            let field = |name: &str| b64_decode(k[name].as_str()?);
            let key = match (k["kty"].as_str()?, k["crv"].as_str()) {
                ("RSA", _) => Key::Rsa {
                    n: field("n")?,
                    e: field("e")?,
                },
                ("EC", Some(crv @ ("P-256" | "P-384"))) => {
                    let len = if crv == "P-256" { 32 } else { 48 };
                    let (x, y) = (field("x")?, field("y")?);
                    if x.len() != len || y.len() != len {
                        return None;
                    }
                    let point: Vec<u8> = [&[4u8][..], &x, &y].concat();
                    if crv == "P-256" {
                        Key::P256(point)
                    } else {
                        Key::P384(point)
                    }
                }
                ("OKP", Some("Ed25519")) => {
                    let x = field("x")?;
                    if x.len() != 32 {
                        return None;
                    }
                    Key::Ed25519(x)
                }
                _ => return None,
            };
            Some(Jwk {
                kid: k["kid"].as_str().map(str::to_string),
                alg: k["alg"].as_str().map(str::to_string),
                key,
            })
        })
        .collect()
}

fn check(keys: &[Jwk], kid: Option<&str>, alg: &str, signed: &[u8], sig: &[u8]) -> KeyMatch {
    let candidates: Vec<&Jwk> = keys
        .iter()
        .filter(|k| kid.is_none() || k.kid.as_deref() == kid)
        .filter(|k| k.alg.as_deref().is_none_or(|a| a == alg) && fits(&k.key, alg))
        .collect();
    if candidates.is_empty() {
        return KeyMatch::Unknown;
    }
    if candidates
        .iter()
        .any(|k| verify_with(&k.key, alg, signed, sig))
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

/// Claim по пути через точку: `realm_access.roles` у Keycloak.
fn lookup<'a>(claims: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let mut value = claims.get(parts.next()?)?;
    for part in parts {
        value = value.get(part)?;
    }
    Some(value)
}

/// Роли или группы: список строк или одна строка. Что-то другое считаем отсутствием claim.
fn roles(v: Option<&Value>) -> Option<Vec<String>> {
    match v? {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(list) => list
            .iter()
            .map(|r| r.as_str().map(str::to_string))
            .collect(),
        _ => None,
    }
}

fn display_name(c: &Map<String, Value>) -> Option<String> {
    text(c.get("name")).or_else(|| text(c.get("preferred_username")))
}

fn identity(
    issuer: &str,
    c: &Map<String, Value>,
    roles_claim: Option<&str>,
) -> anyhow::Result<Identity> {
    let subject = text(c.get("sub")).context("id_token has no sub")?;
    Ok(Identity {
        issuer: issuer.to_string(),
        subject,
        email: text(c.get("email")),
        email_verified: flag(c.get("email_verified")),
        name: display_name(c),
        avatar_url: text(c.get("picture")),
        roles: roles_claim
            .and_then(|claim| roles(lookup(c, claim)))
            .unwrap_or_default(),
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
        assert_eq!(roles(Some(&json!("admin"))), Some(vec!["admin".into()]));
        assert_eq!(roles(Some(&json!([1]))), None);
        assert_eq!(roles(Some(&json!({"a": 1}))), None);
        assert_eq!(roles(None), None);
        assert!(flag(Some(&json!("true"))));
        assert!(!flag(Some(&json!(false))));
    }

    #[test]
    fn nested_claims() {
        let c = claims(json!({"sub": "u", "realm_access": {"roles": ["wave-admin", "x"]}}));
        let id = identity("i", &c, Some("realm_access.roles")).unwrap();
        assert_eq!(id.roles, vec!["wave-admin", "x"]);
        assert!(identity("i", &c, Some("groups")).unwrap().roles.is_empty());
        assert!(identity("i", &c, None).unwrap().roles.is_empty());
    }

    #[test]
    fn jwks_kinds() {
        let p256 = crate::crypto::b64([1u8; 32]);
        let doc = json!({"keys": [
            {"kty": "RSA", "n": "AQAB", "e": "AQAB", "kid": "r"},
            {"kty": "EC", "crv": "P-256", "x": p256, "y": p256, "kid": "e"},
            {"kty": "EC", "crv": "P-521", "x": p256, "y": p256},
            {"kty": "OKP", "crv": "Ed25519", "x": p256, "alg": "EdDSA"},
            {"kty": "RSA", "n": "AQAB", "e": "AQAB", "use": "enc"},
            {"kty": "oct", "k": "AQAB"},
        ]});
        let keys = parse_jwks(&doc);
        assert_eq!(keys.len(), 3, "{keys:?}");
        assert!(matches!(keys[0].key, Key::Rsa { .. }));
        assert!(matches!(keys[1].key, Key::P256(_)));
        assert!(matches!(keys[2].key, Key::Ed25519(_)));
        assert!(fits(&keys[0].key, "PS256") && !fits(&keys[0].key, "ES256"));
        assert!(verifier("none").is_none() && verifier("HS256").is_none());
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
