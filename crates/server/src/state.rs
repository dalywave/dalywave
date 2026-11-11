use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts},
    http::{HeaderMap, request::Parts},
};
use axum_extra::extract::cookie::Key;
use sqlx::PgPool;

use crate::{
    config::Config, deezer::Deezer, ml::Ml, oidc::Oidc, ratelimit::RateLimiter, realtime::Hub,
    storage::Storage,
};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    /// Шифрует cookie `dw_oidc`. Выведен из SECRET_KEY, поэтому вход переживает перезапуск
    pub cookie_key: Key,
    pub limiter: Arc<RateLimiter>,
    /// Клиент daly.rest; `None`, если вход через провайдера не настроен
    pub oidc: Option<Arc<Oidc>>,
    pub started: Instant,
    pub deezer: Arc<Deezer>,
    pub storage: Arc<Storage>,
    /// События для открытых WebSocket этого процесса
    pub hub: Arc<Hub>,
    /// Сервис анализа звука: векторы описаний словами (поиск по звучанию, волна по описанию)
    pub ml: Arc<Ml>,
}

impl AppState {
    /// Хранилище — из `S3_*`; без настроек — в памяти процесса.
    pub fn new(db: PgPool, config: Config) -> anyhow::Result<Self> {
        let storage = match &config.s3 {
            Some(s3) => Storage::from_config(s3)?,
            None => Storage::memory(),
        };
        Self::with_storage(db, config, storage)
    }

    pub fn with_storage(db: PgPool, config: Config, storage: Storage) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("dalywave/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(10))
            // Ответы token и userinfo приходят напрямую; редирект там означает ошибку настройки
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let oidc = config
            .oidc
            .clone()
            .map(|c| Arc::new(Oidc::new(c, http.clone())));
        let deezer = Deezer::new(&config.deezer.url, config.deezer.rate_api)?;
        let ml = Ml::new(&config.ml.url)?;
        Ok(Self {
            deezer: Arc::new(deezer),
            storage: Arc::new(storage),
            hub: Arc::new(Hub::default()),
            ml: Arc::new(ml),
            cookie_key: Key::derive_from(&config.secret_key),
            db,
            config: Arc::new(config),
            http,
            limiter: Arc::new(RateLimiter::default()),
            oidc,
            started: Instant::now(),
        })
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

/// IP клиента. Заголовкам верим, только если запрос пришёл из частной сети: это Traefik в docker
/// или dev-прокси Nuxt на этой же машине. Иначе их может подставить кто угодно.
pub struct ClientIp(pub IpAddr);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
        Ok(Self(client_ip(peer, &parts.headers)))
    }
}

pub fn client_ip(peer: IpAddr, headers: &HeaderMap) -> IpAddr {
    let peer = peer.to_canonical();
    if !is_private(peer) {
        return peer;
    }
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    // Перед Traefik стоит Cloudflare: адрес посетителя он кладёт в CF-Connecting-IP.
    // Traefik заменяет чужой X-Forwarded-For своим, поэтому первый адрес в нём — тот, кто к нему пришёл
    header("cf-connecting-ip")
        .and_then(|v| v.trim().parse::<IpAddr>().ok())
        .or_else(|| {
            header("x-forwarded-for")?
                .split(',')
                .next()?
                .trim()
                .parse()
                .ok()
        })
        .map(|ip| ip.to_canonical())
        .unwrap_or(peer)
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn forwarded_headers_only_from_private_peers() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.7, 172.18.0.1"),
        );
        let traefik: IpAddr = "172.18.0.5".parse().unwrap();
        let stranger: IpAddr = "198.51.100.1".parse().unwrap();
        assert_eq!(
            client_ip(traefik, &h),
            "203.0.113.7".parse::<IpAddr>().unwrap()
        );
        assert_eq!(client_ip(stranger, &h), stranger);

        h.insert("cf-connecting-ip", HeaderValue::from_static("2001:db8::1"));
        assert_eq!(
            client_ip(traefik, &h),
            "2001:db8::1".parse::<IpAddr>().unwrap()
        );

        let garbage = HeaderMap::from_iter([(
            axum::http::header::HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_static("not-an-ip"),
        )]);
        assert_eq!(client_ip(traefik, &garbage), traefik);
        let mapped: IpAddr = "::ffff:10.0.0.9".parse().unwrap();
        assert_eq!(
            client_ip(mapped, &HeaderMap::new()),
            "10.0.0.9".parse::<IpAddr>().unwrap()
        );
    }
}
