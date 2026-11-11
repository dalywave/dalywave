use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use dalywave_server::{AppState, Config};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

use crate::mock::{MockProvider, Person};

pub const PUBLIC_URL: &str = "http://localhost:3400";
pub const ADMIN_PASSWORD: &str = "correct horse battery staple, but longer";

pub fn config() -> Config {
    Config {
        database_url: String::new(),
        listen: "127.0.0.1:0".parse().unwrap(),
        public_url: PUBLIC_URL.into(),
        secret_key: [7; 32],
        static_dir: None,
        oidc: None,
        admin_emails: Vec::new(),
        admin_password: None,
    }
}

/// Как запрос подписан.
pub enum Auth {
    None,
    /// Значение для заголовка Cookie, например `dw_session=dws_…`
    Cookie(String),
    Bearer(String),
}

pub struct Resp {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
    pub text: String,
}

impl Resp {
    pub fn location(&self) -> String {
        self.headers
            .get(header::LOCATION)
            .expect("no Location")
            .to_str()
            .unwrap()
            .to_string()
    }

    /// Полная строка Set-Cookie для cookie с этим именем.
    pub fn set_cookie(&self, name: &str) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .find(|v| v.starts_with(&format!("{name}=")))
    }

    /// `name=value` для заголовка Cookie, если cookie поставлена (а не стёрта).
    pub fn cookie(&self, name: &str) -> Option<String> {
        let pair = self.set_cookie(name)?.split(';').next()?.to_string();
        (pair.len() > name.len() + 1).then_some(pair)
    }

    pub fn code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or_default()
    }
}

pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    pub provider: MockProvider,
}

impl TestApp {
    pub async fn new(db: PgPool) -> Self {
        Self::with(db, |_| {}).await
    }

    pub async fn with(db: PgPool, tweak: impl FnOnce(&mut Config)) -> Self {
        let provider = MockProvider::start().await;
        let mut config = config();
        config.oidc = Some(provider.oidc_config());
        tweak(&mut config);
        let state = AppState::new(db, config).unwrap();
        Self {
            router: dalywave_server::app(state.clone()),
            state,
            provider,
        }
    }

    pub async fn send(&self, req: Request<Body>) -> Resp {
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Resp {
            status,
            headers,
            body,
            text,
        }
    }

    /// Запрос как из интерфейса: изменяющие запросы с cookie несут Origin сайта.
    pub async fn call(&self, method: Method, uri: &str, body: Option<Value>, auth: &Auth) -> Resp {
        let mut req = Request::builder().method(method.clone()).uri(uri);
        match auth {
            Auth::None => {}
            Auth::Cookie(c) => {
                req = req.header(header::COOKIE, c);
                if !method.is_safe() {
                    req = req.header(header::ORIGIN, PUBLIC_URL);
                }
            }
            Auth::Bearer(t) => req = req.header(header::AUTHORIZATION, format!("Bearer {t}")),
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        };
        self.send(req.unwrap()).await
    }

    pub async fn get(&self, uri: &str, auth: &Auth) -> Resp {
        self.call(Method::GET, uri, None, auth).await
    }

    /// Начало входа: адрес daly.rest и cookie `dw_oidc`.
    pub async fn start(&self, uri: &str) -> (String, String) {
        let res = self.get(uri, &Auth::None).await;
        assert_eq!(res.status, StatusCode::FOUND, "{}", res.text);
        let cookie = res.cookie("dw_oidc").expect("dw_oidc cookie");
        (res.location(), cookie)
    }

    pub async fn callback(&self, code: &str, state: &str, oidc_cookie: Option<&str>) -> Resp {
        let mut req =
            Request::builder().uri(format!("/api/auth/oidc/callback?code={code}&state={state}"));
        if let Some(c) = oidc_cookie {
            req = req.header(header::COOKIE, c);
        }
        self.send(req.body(Body::empty()).unwrap()).await
    }

    /// Полный вход через «daly.rest». Возвращает cookie `dw_session=…`.
    pub async fn login(&self, person: &Person) -> String {
        let (url, cookie) = self.start("/api/auth/oidc/start").await;
        let (code, state) = self.provider.approve(&url, person);
        let res = self.callback(&code, &state, Some(&cookie)).await;
        assert_eq!(res.status, StatusCode::FOUND);
        assert_eq!(res.location(), format!("{PUBLIC_URL}/"), "sign-in failed");
        res.cookie("dw_session").expect("dw_session cookie")
    }

    /// Админ — человек, которому daly.rest прислал роль admin.
    pub async fn admin(&self, sub: &str) -> (String, Auth) {
        let cookie = self.login(&Person::admin(sub)).await;
        let me = self
            .get("/api/auth/me", &Auth::Cookie(cookie.clone()))
            .await;
        (
            me.body["user"]["id"].as_str().unwrap().to_string(),
            Auth::Cookie(cookie),
        )
    }

    pub async fn user_id(&self, auth: &Auth) -> String {
        self.get("/api/auth/me", auth).await.body["user"]["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    pub async fn audit_kinds(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT kind FROM audit_events ORDER BY id")
            .fetch_all(&self.state.db)
            .await
            .unwrap()
    }
}
