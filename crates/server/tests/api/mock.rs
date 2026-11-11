//! Заглушка daly.rest на случайном локальном порту: discovery, token с id_token ES256, JWKS, userinfo.
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
use dalywave_server::config::OidcConfig;
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const CLIENT_ID: &str = "dalywave-test";
pub const CLIENT_SECRET: &str = "test secret+/=";

fn b64(data: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

fn random() -> String {
    let mut buf = [0u8; 24];
    getrandom::fill(&mut buf).unwrap();
    b64(buf)
}

/// Человек с аккаунтом на «daly.rest».
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

    fn claims(&self, with_roles: bool) -> Value {
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
            c["roles"] = json!(self.roles);
        }
        c
    }
}

struct Grant {
    person: Person,
    nonce: String,
    challenge: String,
    redirect_uri: String,
}

struct Inner {
    key: SigningKey,
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
    inner: Arc<Mutex<Inner>>,
}

impl MockProvider {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let key = loop {
            let mut seed = [0u8; 32];
            getrandom::fill(&mut seed).unwrap();
            if let Ok(key) = SigningKey::from_slice(&seed) {
                break key;
            }
        };
        let provider = Self {
            issuer,
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

    pub fn oidc_config(&self) -> OidcConfig {
        OidcConfig {
            issuer: self.issuer.clone(),
            client_id: CLIENT_ID.into(),
            client_secret: CLIENT_SECRET.into(),
            title: "daly.rest".into(),
            admin_roles: vec!["admin".into()],
        }
    }

    /// Человек вошёл на «daly.rest» по адресу, который выдал наш сервер. Возвращает (code, state).
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
        assert_eq!(q["scope"], "openid email profile");
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
    Json(json!({
        "issuer": iss,
        "authorization_endpoint": format!("{iss}/authorize"),
        "token_endpoint": format!("{iss}/token"),
        "userinfo_endpoint": format!("{iss}/userinfo"),
        "jwks_uri": format!("{iss}/jwks"),
        "response_types_supported": ["code"],
        "id_token_signing_alg_values_supported": ["ES256"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
    }))
}

async fn jwks(State(p): State<MockProvider>) -> Json<Value> {
    let inner = p.inner.lock().unwrap();
    let point = inner.key.verifying_key().to_sec1_point(false);
    let bytes = point.as_bytes();
    Json(json!({ "keys": [{
        "kty": "EC", "crv": "P-256", "use": "sig", "alg": "ES256", "kid": inner.kid,
        "x": b64(&bytes[1..33]), "y": b64(&bytes[33..65]),
    }]}))
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
    if basic_client(&headers) != Some((CLIENT_ID.into(), CLIENT_SECRET.into())) {
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
    let mut claims = grant.person.claims(inner.roles_in_id_token);
    claims["iss"] = json!(p.issuer);
    claims["aud"] = json!(CLIENT_ID);
    claims["iat"] = json!(now);
    claims["exp"] = json!(now + 300);
    claims["nonce"] = json!(inner.nonce_override.clone().unwrap_or(grant.nonce));
    let header = json!({ "alg": "ES256", "typ": "JWT", "kid": inner.kid });
    let input = format!("{}.{}", b64(header.to_string()), b64(claims.to_string()));
    let signature: Signature = inner.key.sign(input.as_bytes());
    let id_token = format!("{input}.{}", b64(signature.to_bytes()));
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
        Some(person) => Json(person.claims(true)).into_response(),
        None => oauth_error(StatusCode::UNAUTHORIZED, "invalid_token"),
    }
}
