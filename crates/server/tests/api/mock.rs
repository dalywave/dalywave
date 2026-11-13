//! Подставной провайдер OIDC на случайном локальном порту: discovery, token с подписанным id_token
//! (ES256, RS256 или EdDSA), JWKS, userinfo. По умолчанию изображает daly.rest; [`MockOptions`]
//! делают из него Authentik, Keycloak или Pocket ID (группы, client_secret_post, публичный клиент).
//! Страницу входа не изображаем: тест сам «входит» через [`MockProvider::approve`].

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Form, Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use dalywave_server::config::{LEGACY_PROVIDER_ID, OidcConfig};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair, RSA_PKCS1_SHA256, RsaKeyPair},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const CLIENT_ID: &str = "dalywave-test";
pub const CLIENT_SECRET: &str = "test secret+/=";

/// Тестовый ключ RSA 2048 (PKCS#8): только для подписи id_token подставного провайдера.
const RSA_KEY: &[u8] = include_bytes!("../fixtures/oidc/test_rsa.pk8");

fn b64(data: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

fn random() -> String {
    let mut buf = [0u8; 24];
    getrandom::fill(&mut buf).unwrap();
    b64(buf)
}

/// Человек с аккаунтом у провайдера.
#[derive(Debug, Clone)]
pub struct Person {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: String,
    pub roles: Vec<String>,
}

impl Person {
    pub fn new(sub: &str) -> Self {
        Self {
            sub: sub.into(),
            email: Some(format!("{sub}@example.com")),
            email_verified: true,
            name: format!("Person {sub}"),
            roles: Vec::new(),
        }
    }

    pub fn admin(sub: &str) -> Self {
        Self {
            roles: vec!["admin".into()],
            ..Self::new(sub)
        }
    }

    fn claims(&self, roles_claim: &str, with_roles: bool) -> Value {
        let mut c = json!({
            "sub": self.sub,
            "name": self.name,
            "preferred_username": self.sub,
            "picture": format!("https://cdn.example.com/{}.png", self.sub),
        });
        if let Some(email) = &self.email {
            c["email"] = json!(email);
            c["email_verified"] = json!(self.email_verified);
        }
        if with_roles {
            // `realm_access.roles` — вложенный claim, как у Keycloak
            match roles_claim.split_once('.') {
                Some((outer, inner)) => {
                    let mut nested = serde_json::Map::new();
                    nested.insert(inner.into(), json!(self.roles));
                    c[outer] = Value::Object(nested);
                }
                None => c[roles_claim] = json!(self.roles),
            }
        }
        c
    }
}

/// Чем подписан id_token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alg {
    Es256,
    Rs256,
    EdDsa,
}

/// Как ведёт себя провайдер.
#[derive(Debug, Clone)]
pub struct MockOptions {
    pub alg: Alg,
    /// Claim ролей или групп
    pub roles_claim: String,
    /// Секрет только в теле (client_secret_post): discovery не называет basic
    pub post_only: bool,
    /// Публичный клиент: секрета нет вовсе
    pub public: bool,
}

impl Default for MockOptions {
    fn default() -> Self {
        Self {
            alg: Alg::Es256,
            roles_claim: "roles".into(),
            post_only: false,
            public: false,
        }
    }
}

enum Key {
    Es(SigningKey),
    Rsa(RsaKeyPair),
    Ed(Ed25519KeyPair),
}

struct Grant {
    person: Person,
    nonce: String,
    challenge: String,
    redirect_uri: String,
}

struct Inner {
    key: Key,
    kid: String,
    grants: HashMap<String, Grant>,
    access: HashMap<String, Person>,
    roles_in_id_token: bool,
    nonce_override: Option<String>,
    userinfo_calls: usize,
}

#[derive(Clone)]
pub struct MockProvider {
    pub issuer: String,
    pub options: Arc<MockOptions>,
    inner: Arc<Mutex<Inner>>,
}

/// SEQUENCE { INTEGER n, INTEGER e } из DER открытого ключа RSA → (n, e) без ведущих нулей.
fn rsa_components(der: &[u8]) -> (Vec<u8>, Vec<u8>) {
    fn read(buf: &[u8]) -> (u8, &[u8], &[u8]) {
        let tag = buf[0];
        let (len, start) = if buf[1] & 0x80 == 0 {
            (buf[1] as usize, 2)
        } else {
            let n = (buf[1] & 0x7f) as usize;
            let len = buf[2..2 + n]
                .iter()
                .fold(0usize, |acc, b| (acc << 8) | *b as usize);
            (len, 2 + n)
        };
        (tag, &buf[start..start + len], &buf[start + len..])
    }
    let (_, seq, _) = read(der);
    let (_, n, rest) = read(seq);
    let (_, e, _) = read(rest);
    let trim = |v: &[u8]| v.iter().skip_while(|b| **b == 0).copied().collect();
    (trim(n), trim(e))
}

impl MockProvider {
    pub async fn start() -> Self {
        Self::start_with(MockOptions::default()).await
    }

    pub async fn start_with(options: MockOptions) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let key = match options.alg {
            Alg::Es256 => Key::Es(loop {
                let mut seed = [0u8; 32];
                getrandom::fill(&mut seed).unwrap();
                if let Ok(key) = SigningKey::from_slice(&seed) {
                    break key;
                }
            }),
            Alg::Rs256 => Key::Rsa(RsaKeyPair::from_pkcs8(RSA_KEY).unwrap()),
            Alg::EdDsa => {
                let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
                Key::Ed(Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap())
            }
        };
        let provider = Self {
            issuer,
            options: Arc::new(options),
            inner: Arc::new(Mutex::new(Inner {
                key,
                kid: random(),
                grants: HashMap::new(),
                access: HashMap::new(),
                roles_in_id_token: true,
                nonce_override: None,
                userinfo_calls: 0,
            })),
        };
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .route("/userinfo", get(userinfo))
            .with_state(provider.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        provider
    }

    /// Как daly.rest из прежних переменных: id `dalyrest`, старый адрес возврата, почте верим.
    pub fn oidc_config(&self) -> OidcConfig {
        let mut c = OidcConfig::new(
            LEGACY_PROVIDER_ID,
            &self.issuer,
            CLIENT_ID,
            Some(CLIENT_SECRET),
        );
        c.title = "daly.rest".into();
        c.trust_email = true;
        c.legacy_callback = true;
        c
    }

    /// Новый провайдер `id` из переменных `DALYWAVE_OIDC_<ID>_*`.
    pub fn provider_config(&self, id: &str, title: &str) -> OidcConfig {
        let secret = (!self.options.public).then_some(CLIENT_SECRET);
        let mut c = OidcConfig::new(id, &self.issuer, CLIENT_ID, secret);
        c.title = title.into();
        c.admin_claim = Some(self.options.roles_claim.clone());
        c
    }

    /// Человек вошёл у провайдера по адресу, который выдал наш сервер. Возвращает (code, state).
    pub fn approve(&self, authorize_url: &str, person: &Person) -> (String, String) {
        let url = url::Url::parse(authorize_url).unwrap();
        assert!(
            authorize_url.starts_with(&format!("{}/authorize?", self.issuer)),
            "{authorize_url}"
        );
        let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], CLIENT_ID);
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["code_challenge_method"], "S256");
        assert!(q["scope"].starts_with("openid"), "{}", q["scope"]);
        let code = random();
        let grant = Grant {
            person: person.clone(),
            nonce: q["nonce"].clone(),
            challenge: q["code_challenge"].clone(),
            redirect_uri: q["redirect_uri"].clone(),
        };
        self.inner
            .lock()
            .unwrap()
            .grants
            .insert(code.clone(), grant);
        (code, q["state"].clone())
    }

    /// Без роли в id_token клиент обязан спросить userinfo.
    pub fn roles_in_id_token(&self, on: bool) {
        self.inner.lock().unwrap().roles_in_id_token = on;
    }

    pub fn override_nonce(&self, nonce: &str) {
        self.inner.lock().unwrap().nonce_override = Some(nonce.into());
    }

    pub fn userinfo_calls(&self) -> usize {
        self.inner.lock().unwrap().userinfo_calls
    }
}

fn oauth_error(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({ "error": error }))).into_response()
}

async fn discovery(State(p): State<MockProvider>) -> Json<Value> {
    let iss = &p.issuer;
    let methods = if p.options.post_only {
        json!(["client_secret_post"])
    } else {
        json!(["client_secret_basic", "client_secret_post"])
    };
    Json(json!({
        "issuer": iss,
        "authorization_endpoint": format!("{iss}/authorize"),
        "token_endpoint": format!("{iss}/token"),
        "userinfo_endpoint": format!("{iss}/userinfo"),
        "jwks_uri": format!("{iss}/jwks"),
        "response_types_supported": ["code"],
        "id_token_signing_alg_values_supported": ["ES256", "RS256", "EdDSA"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": methods,
    }))
}

async fn jwks(State(p): State<MockProvider>) -> Json<Value> {
    let inner = p.inner.lock().unwrap();
    let key = match &inner.key {
        Key::Es(key) => {
            let point = key.verifying_key().to_sec1_point(false);
            let bytes = point.as_bytes();
            json!({ "kty": "EC", "crv": "P-256", "alg": "ES256",
                    "x": b64(&bytes[1..33]), "y": b64(&bytes[33..65]) })
        }
        Key::Rsa(key) => {
            let (n, e) = rsa_components(key.public().as_ref());
            json!({ "kty": "RSA", "alg": "RS256", "n": b64(n), "e": b64(e) })
        }
        Key::Ed(key) => {
            json!({ "kty": "OKP", "crv": "Ed25519", "alg": "EdDSA", "x": b64(key.public_key().as_ref()) })
        }
    };
    let mut key = key;
    key["use"] = json!("sig");
    key["kid"] = json!(inner.kid);
    Json(json!({ "keys": [key] }))
}

fn basic_client(headers: &HeaderMap) -> Option<(String, String)> {
    let raw = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Basic ")?;
    let decoded = String::from_utf8(STANDARD.decode(raw).ok()?).ok()?;
    let (id, secret) = decoded.split_once(':')?;
    let unescape = |s: &str| {
        url::form_urlencoded::parse(format!("v={s}").as_bytes())
            .next()
            .map(|(_, v)| v.into_owned())
    };
    Some((unescape(id)?, unescape(secret)?))
}

async fn token(
    State(p): State<MockProvider>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let posted = (
        form.get("client_id").cloned(),
        form.get("client_secret").cloned(),
    );
    let client_ok = if p.options.public {
        posted == (Some(CLIENT_ID.into()), None) && basic_client(&headers).is_none()
    } else if p.options.post_only {
        posted == (Some(CLIENT_ID.into()), Some(CLIENT_SECRET.into()))
    } else {
        basic_client(&headers) == Some((CLIENT_ID.into(), CLIENT_SECRET.into()))
    };
    if !client_ok {
        return oauth_error(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    if form.get("grant_type").map(String::as_str) != Some("authorization_code") {
        return oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type");
    }
    let mut inner = p.inner.lock().unwrap();
    let Some(grant) = form.get("code").and_then(|c| inner.grants.remove(c)) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    if form.get("redirect_uri") != Some(&grant.redirect_uri) {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
    if b64(Sha256::digest(verifier.as_bytes())) != grant.challenge {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant");
    }
    let now = chrono::Utc::now().timestamp();
    let mut claims = grant
        .person
        .claims(&p.options.roles_claim, inner.roles_in_id_token);
    claims["iss"] = json!(p.issuer);
    claims["aud"] = json!(CLIENT_ID);
    claims["iat"] = json!(now);
    claims["exp"] = json!(now + 300);
    claims["nonce"] = json!(inner.nonce_override.clone().unwrap_or(grant.nonce));
    let alg = match p.options.alg {
        Alg::Es256 => "ES256",
        Alg::Rs256 => "RS256",
        Alg::EdDsa => "EdDSA",
    };
    let header = json!({ "alg": alg, "typ": "JWT", "kid": inner.kid });
    let input = format!("{}.{}", b64(header.to_string()), b64(claims.to_string()));
    let signature = match &inner.key {
        Key::Es(key) => {
            let sig: Signature = key.sign(input.as_bytes());
            sig.to_bytes().to_vec()
        }
        Key::Rsa(key) => {
            let mut sig = vec![0u8; key.public().modulus_len()];
            key.sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                input.as_bytes(),
                &mut sig,
            )
            .unwrap();
            sig
        }
        Key::Ed(key) => key.sign(input.as_bytes()).as_ref().to_vec(),
    };
    let id_token = format!("{input}.{}", b64(signature));
    let access = random();
    inner.access.insert(access.clone(), grant.person);
    Json(json!({ "access_token": access, "token_type": "Bearer", "expires_in": 3600, "id_token": id_token }))
        .into_response()
}

async fn userinfo(State(p): State<MockProvider>, headers: HeaderMap) -> Response {
    let mut inner = p.inner.lock().unwrap();
    inner.userinfo_calls += 1;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match token.and_then(|t| inner.access.get(t)) {
        Some(person) => Json(person.claims(&p.options.roles_claim, true)).into_response(),
        None => oauth_error(StatusCode::UNAUTHORIZED, "invalid_token"),
    }
}
