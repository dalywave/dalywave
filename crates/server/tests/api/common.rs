use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use dalywave_server::{
    AppState, Config,
    config::{
        DeezerConfig, Features, FetcherConfig, InfoConfig, LibraryConfig, LrclibConfig, MlConfig,
        StorageConfig, YandexMusicConfig,
    },
    deezer::Deezer,
    jobs::{
        handlers,
        worker::{Context, Outcome, Settings, Worker},
    },
    ml::Ml,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::{
    deezer::DeezerMock,
    mock::{MockProvider, Person},
};

pub const PUBLIC_URL: &str = "http://localhost:3400";
pub const ADMIN_PASSWORD: &str = "correct horse battery staple, but longer";

pub fn config() -> Config {
    Config {
        database_url: String::new(),
        listen: "127.0.0.1:0".parse().unwrap(),
        public_url: PUBLIC_URL.into(),
        secret_key: [7; 32],
        static_dir: None,
        oidc: Vec::new(),
        // Как на wave.daly.rest: вход только через провайдера. Тесты своих аккаунтов включают сами
        local_auth: false,
        admin_emails: Vec::new(),
        admin_password: None,
        s3: None,
        storage: StorageConfig::default(),
        library: LibraryConfig::default(),
        // Тесты написаны для wave.daly.rest: Deezer и загрузчик включены
        features: Features {
            deezer: true,
            downloader: true,
        },
        deezer: DeezerConfig::default(),
        // Long-poll загрузчика в тестах — доли секунды, а не 25 с
        fetcher: FetcherConfig {
            claim_wait: Duration::from_millis(300),
            ..FetcherConfig::default()
        },
        // Настоящего сервиса анализа в тестах нет: адрес, где никто не слушает (заглушку ставят сами тесты)
        ml: MlConfig {
            url: "http://127.0.0.1:9".into(),
        },
        // Тексты — тоже без сети: тесты текстов ставят свою заглушку LRCLIB
        lrclib: LrclibConfig {
            url: "http://127.0.0.1:9".into(),
        },
        // «Яндекс Музыка» — тоже без сети: тесты переноса ставят свою заглушку
        yandex_music: YandexMusicConfig {
            url: "http://127.0.0.1:9".into(),
        },
        // Сведения о музыке — тоже без сети: тесты чартов, авторов и «Об артисте» ставят свою заглушку
        info: InfoConfig {
            apple_charts_url: "http://127.0.0.1:9".into(),
            musicbrainz_url: "http://127.0.0.1:9".into(),
            wikidata_url: "http://127.0.0.1:9".into(),
            wikipedia_url: "http://127.0.0.1:9/{lang}".into(),
            musickit: None,
        },
        release_token: None,
        // push — без каналов: тесты уведомлений ставят свои поддельные APNs и службу Web Push
        push: Default::default(),
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
    pub bytes: Vec<u8>,
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
    pub deezer: DeezerMock,
}

impl TestApp {
    pub async fn new(db: PgPool) -> Self {
        Self::with(db, |_| {}).await
    }

    pub async fn with(db: PgPool, tweak: impl FnOnce(&mut Config)) -> Self {
        let provider = MockProvider::start().await;
        let deezer = DeezerMock::start().await;
        let mut config = config();
        config.oidc = vec![provider.oidc_config()];
        config.deezer.url = deezer.base.clone();
        tweak(&mut config);
        let mut state = AppState::new(db, config).unwrap();
        // Квота и повторы в тестах — без пятисекундных пауз
        state.deezer = Arc::new(fast_deezer(&deezer.base).enabled(state.config.features.deezer));
        // MusicBrainz в тестах — без секундной паузы между запросами
        state.info = Arc::new(dalywave_server::info::Sources::fast(&state.config.info).unwrap());
        Self {
            router: dalywave_server::app(state.clone()),
            state,
            provider,
            deezer,
        }
    }

    /// Исполнитель задач с теми же базой, хранилищем и Deezer, что у API.
    pub fn worker(&self) -> Worker {
        let ctx = Context::new(
            self.state.db.clone(),
            self.state.config.clone(),
            Arc::new(fast_deezer(&self.deezer.base).enabled(self.state.config.features.deezer)),
            self.state.storage.clone(),
        )
        .unwrap();
        Worker::new(
            Arc::new(ctx),
            handlers::registry(&self.state.config.fetcher),
            Settings::default(),
        )
    }

    /// Исполнитель со своим клиентом сервиса анализа (короткие паузы при «занят»).
    pub fn worker_with_ml(&self, ml: Ml) -> Worker {
        let ctx = Context::with_ml(
            self.state.db.clone(),
            self.state.config.clone(),
            Arc::new(fast_deezer(&self.deezer.base).enabled(self.state.config.features.deezer)),
            self.state.storage.clone(),
            Arc::new(ml),
        )
        .unwrap();
        Worker::new(
            Arc::new(ctx),
            handlers::registry(&self.state.config.fetcher),
            Settings::default(),
        )
    }

    /// Одна задача полосы до конца.
    pub async fn run_job(&self, lane: &str) -> Option<(String, Outcome)> {
        self.worker()
            .run_once(lane)
            .await
            .unwrap()
            .map(|(job, outcome)| (job.kind, outcome))
    }

    /// Настоящий сервер на случайном порту — для WebSocket. Слушатель NOTIFY запущен.
    pub async fn serve(&self) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = self
            .router
            .clone()
            .into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        dalywave_server::realtime::spawn_listener(self.state.db.clone(), self.state.hub.clone());
        addr
    }

    /// Слушатель — человек с ролью listener (выдаёт админ `boss`).
    pub async fn listener(&self, sub: &str) -> Auth {
        let (_, admin) = self.admin(&format!("{sub}-boss")).await;
        let auth = Auth::Cookie(self.login(&Person::new(sub)).await);
        let id = self.user_id(&auth).await;
        let res = self
            .call(
                Method::PATCH,
                &format!("/api/admin/users/{id}"),
                Some(json!({ "roles": ["listener"] })),
                &admin,
            )
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        auth
    }

    /// Наш id сущности Deezer (загружается из заглушки при первом обращении).
    pub async fn resolve(&self, kind: &str, deezer_id: i64, auth: &Auth) -> Value {
        let res = self
            .get(&format!("/api/catalog/deezer/{kind}/{deezer_id}"), auth)
            .await;
        assert_eq!(res.status, StatusCode::OK, "{}", res.text);
        res.body
    }

    pub async fn resolve_id(&self, kind: &str, deezer_id: i64, auth: &Auth) -> String {
        self.resolve(kind, deezer_id, auth).await["id"]
            .as_str()
            .unwrap()
            .to_string()
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
            bytes: bytes.to_vec(),
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

    pub async fn put(&self, uri: &str, auth: &Auth) -> Resp {
        self.call(Method::PUT, uri, None, auth).await
    }

    pub async fn delete(&self, uri: &str, auth: &Auth) -> Resp {
        self.call(Method::DELETE, uri, None, auth).await
    }

    pub async fn post(&self, uri: &str, body: Value, auth: &Auth) -> Resp {
        self.call(Method::POST, uri, Some(body), auth).await
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

pub fn fast_deezer(base: &str) -> Deezer {
    Deezer::new(base, 40)
        .unwrap()
        .with_backoff(Duration::from_millis(10), Duration::from_millis(10))
}
