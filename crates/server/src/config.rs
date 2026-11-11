//! Настройки из переменных `DALYWAVE_*`. Локально их дополняет `.env` в корне репозитория.

use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, bail};

pub const DEFAULT_DATABASE_URL: &str = "postgres://dalywave:dalywave@localhost:5400/dalywave";
pub const DEFAULT_PUBLIC_URL: &str = "http://localhost:3400";
pub const DEFAULT_LISTEN: &str = "0.0.0.0:8400";
/// Короче пароль не принимается: аварийный вход не должен подбираться перебором.
pub const MIN_ADMIN_PASSWORD_LEN: usize = 32;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    /// Где слушать: в docker — 0.0.0.0:8400, за ним Traefik
    pub listen: SocketAddr,
    /// Адрес сайта без `/` в конце. Из него адрес возврата OIDC, разрешённый Origin и флаг Secure у cookie.
    pub public_url: String,
    /// 32 байта из `SECRET_KEY`: из них выводится ключ шифрования cookie.
    pub secret_key: [u8; 32],
    /// Собранный интерфейс; без него API отдаёт только `/api`.
    pub static_dir: Option<PathBuf>,
    pub oidc: Option<OidcConfig>,
    /// Почты (в нижнем регистре), которые получают роль admin при входе, если провайдер их подтвердил.
    pub admin_emails: Vec<String>,
    /// Пароль аварийного входа. `None` — вход паролем выключен.
    pub admin_password: Option<String>,
}

/// Клиент daly.rest. Включается, только когда заданы issuer, client_id и секрет.
#[derive(Clone)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    /// Подпись кнопки входа.
    pub title: String,
    /// Роли провайдера, которые делают человека администратором DalyWave.
    pub admin_roles: Vec<String>,
}

fn list(value: Option<String>) -> Vec<String> {
    value
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    /// Разбор настроек из любого источника: переменные окружения или словарь в тестах.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let var = |name: &str| {
            lookup(&format!("DALYWAVE_{name}"))
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let public_url = var("PUBLIC_URL").unwrap_or_else(|| DEFAULT_PUBLIC_URL.into());
        let parsed =
            url::Url::parse(&public_url).context("DALYWAVE_PUBLIC_URL is not a valid URL")?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            bail!("DALYWAVE_PUBLIC_URL must be an http(s) address, e.g. https://wave.daly.rest");
        }
        let public_url = public_url.trim_end_matches('/').to_string();

        let secret_key = match var("SECRET_KEY") {
            Some(hex) => parse_secret(&hex)?,
            // Локально можно без ключа: cookie просто не переживут перезапуск
            None if public_url.starts_with("http://") => {
                tracing::warn!(
                    "DALYWAVE_SECRET_KEY is not set: using a random key, sign-in state resets on restart"
                );
                crate::crypto::random_bytes::<32>()?
            }
            None => {
                bail!("DALYWAVE_SECRET_KEY is required: 64 hex characters (openssl rand -hex 32)")
            }
        };

        let oidc = match (
            var("OIDC_ISSUER"),
            var("OIDC_CLIENT_ID"),
            var("OIDC_CLIENT_SECRET"),
        ) {
            (Some(issuer), Some(client_id), Some(client_secret)) => Some(OidcConfig {
                issuer: issuer.trim_end_matches('/').to_string(),
                client_id,
                client_secret,
                title: var("OIDC_TITLE").unwrap_or_else(|| "daly.rest".into()),
                admin_roles: match list(var("OIDC_ADMIN_ROLES")) {
                    roles if roles.is_empty() => vec!["admin".into()],
                    roles => roles,
                },
            }),
            (None, None, None) => None,
            _ => bail!(
                "DALYWAVE_OIDC_ISSUER, DALYWAVE_OIDC_CLIENT_ID and DALYWAVE_OIDC_CLIENT_SECRET go together"
            ),
        };

        let admin_password = var("ADMIN_PASSWORD").filter(|p| {
            let ok = p.chars().count() >= MIN_ADMIN_PASSWORD_LEN;
            if !ok {
                tracing::warn!("DALYWAVE_ADMIN_PASSWORD is shorter than {MIN_ADMIN_PASSWORD_LEN} characters: password sign-in is off");
            }
            ok
        });

        Ok(Self {
            database_url: var("DATABASE_URL").unwrap_or_else(|| DEFAULT_DATABASE_URL.into()),
            listen: var("LISTEN")
                .unwrap_or_else(|| DEFAULT_LISTEN.into())
                .parse()
                .context("DALYWAVE_LISTEN must look like 0.0.0.0:8400")?,
            public_url,
            secret_key,
            static_dir: var("STATIC_DIR").map(PathBuf::from),
            oidc,
            admin_emails: list(var("ADMIN_EMAILS"))
                .into_iter()
                .map(|e| e.to_lowercase())
                .collect(),
            admin_password,
        })
    }

    /// Cookie с флагом Secure браузер не вернёт по http, поэтому флаг ставится только для https.
    pub fn cookie_secure(&self) -> bool {
        self.public_url.starts_with("https://")
    }

    /// Origin сайта: схема, хост и порт.
    pub fn public_origin(&self) -> String {
        url::Url::parse(&self.public_url)
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_else(|_| self.public_url.clone())
    }

    pub fn oidc_redirect_uri(&self) -> String {
        format!("{}/api/auth/oidc/callback", self.public_url)
    }

    /// Абсолютный адрес страницы интерфейса по пути вида `/library?tab=albums`.
    pub fn page(&self, path: &str) -> String {
        url::Url::parse(&self.public_url)
            .and_then(|base| base.join(path))
            .map(String::from)
            .unwrap_or_else(|_| format!("{}/", self.public_url))
    }

    /// Страница входа с причиной ошибки: `cancelled`, `expired`, `state`, `disabled`, `provider`.
    pub fn login_error(&self, code: &str, next: &str) -> String {
        let mut url = self.page("/login");
        url.push_str("?error=");
        url.push_str(code);
        if next != "/" {
            url.push_str("&next=");
            url.extend(url::form_urlencoded::byte_serialize(next.as_bytes()));
        }
        url
    }
}

fn parse_secret(hex: &str) -> anyhow::Result<[u8; 32]> {
    let bytes = hex::decode(hex).context("DALYWAVE_SECRET_KEY must be hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("DALYWAVE_SECRET_KEY must be 64 hex characters (32 bytes)"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn parse(pairs: &[(&str, &str)]) -> anyhow::Result<Config> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (format!("DALYWAVE_{k}"), v.to_string()))
            .collect();
        Config::from_lookup(|name| map.get(name).cloned())
    }

    #[test]
    fn local_defaults() {
        let c = parse(&[]).unwrap();
        assert_eq!(c.public_url, DEFAULT_PUBLIC_URL);
        assert_eq!(c.listen, DEFAULT_LISTEN.parse().unwrap());
        assert_eq!(c.database_url, DEFAULT_DATABASE_URL);
        assert!(c.oidc.is_none() && c.admin_password.is_none() && c.static_dir.is_none());
        assert!(!c.cookie_secure());
        assert_eq!(
            c.oidc_redirect_uri(),
            "http://localhost:3400/api/auth/oidc/callback"
        );
    }

    #[test]
    fn production_needs_a_secret() {
        assert!(parse(&[("PUBLIC_URL", "https://wave.daly.rest")]).is_err());
        let key = "ab".repeat(32);
        let c = parse(&[
            ("PUBLIC_URL", "https://wave.daly.rest/"),
            ("SECRET_KEY", &key),
        ])
        .unwrap();
        assert_eq!(c.public_url, "https://wave.daly.rest");
        assert!(c.cookie_secure());
        assert_eq!(c.public_origin(), "https://wave.daly.rest");
        assert_eq!(c.secret_key, [0xab; 32]);
        assert!(parse(&[("PUBLIC_URL", "ftp://wave.daly.rest")]).is_err());
    }

    #[test]
    fn short_password_is_off() {
        assert!(
            parse(&[("ADMIN_PASSWORD", "too short")])
                .unwrap()
                .admin_password
                .is_none()
        );
        let long = "x".repeat(MIN_ADMIN_PASSWORD_LEN);
        assert_eq!(
            parse(&[("ADMIN_PASSWORD", &long)]).unwrap().admin_password,
            Some(long)
        );
    }

    #[test]
    fn oidc_goes_together() {
        assert!(parse(&[("OIDC_ISSUER", "https://daly.rest")]).is_err());
        let c = parse(&[
            ("OIDC_ISSUER", "https://daly.rest/"),
            ("OIDC_CLIENT_ID", "dalywave-37a20828"),
            ("OIDC_CLIENT_SECRET", "s"),
            ("ADMIN_EMAILS", "Owner@Example.com, other@example.com"),
        ])
        .unwrap();
        let oidc = c.oidc.unwrap();
        assert_eq!(oidc.issuer, "https://daly.rest");
        assert_eq!(oidc.title, "daly.rest");
        assert_eq!(oidc.admin_roles, vec!["admin"]);
        assert_eq!(
            c.admin_emails,
            vec!["owner@example.com", "other@example.com"]
        );
    }

    #[test]
    fn pages_stay_on_site() {
        let c = parse(&[]).unwrap();
        assert_eq!(
            c.page("/library?tab=albums"),
            "http://localhost:3400/library?tab=albums"
        );
        assert_eq!(
            c.page("/поиск"),
            "http://localhost:3400/%D0%BF%D0%BE%D0%B8%D1%81%D0%BA"
        );
        assert_eq!(
            c.login_error("state", "/"),
            "http://localhost:3400/login?error=state"
        );
        assert_eq!(
            c.login_error("expired", "/a?b=1"),
            "http://localhost:3400/login?error=expired&next=%2Fa%3Fb%3D1"
        );
    }

    #[test]
    fn secret_must_be_32_bytes() {
        assert!(parse_secret(&"ab".repeat(32)).is_ok());
        assert!(parse_secret(&"ab".repeat(16)).is_err());
        assert!(parse_secret("not hex").is_err());
    }

    #[test]
    fn lists_are_trimmed() {
        assert_eq!(
            list(Some(" a@x.ru , ,b@y.ru".into())),
            vec!["a@x.ru", "b@y.ru"]
        );
        assert!(list(None).is_empty());
    }
}
