//! Короткие подписанные ссылки на файлы через этот же API: `/api/files/<ключ>?e=<срок>&s=<подпись>`.
//! Нужны хранилищу на диске (у него своих ссылок нет) и своей музыке из папок на сервере. Подпись —
//! HMAC-SHA256 ключом, выведенным из `DALYWAVE_SECRET_KEY`, по методу, ключу, сроку и имени скачивания;
//! вход для таких ссылок не нужен — как у ссылок S3, их можно отдать плееру `<audio>`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use url::Url;

use super::{Presign, StorageError};

type HmacSha256 = Hmac<Sha256>;

/// Путь ссылок от адреса сайта.
pub const PREFIX: &str = "/api/files/";

#[derive(Clone)]
pub struct Links {
    key: [u8; 32],
    /// Адрес API для клиентов (адрес сайта)
    public: Url,
    /// Адрес API для сервиса анализа
    ml: Url,
}

impl std::fmt::Debug for Links {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Links")
            .field("public", &self.public.as_str())
            .field("ml", &self.ml.as_str())
            .finish_non_exhaustive()
    }
}

/// Что разрешает проверенная ссылка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    pub put: bool,
    /// Отдать с `Content-Disposition: attachment` под этим именем.
    pub download: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

impl Links {
    pub fn new(secret: &[u8; 32], public_url: &str, ml_url: Option<&str>) -> anyhow::Result<Self> {
        let hk = hkdf::Hkdf::<Sha256>::new(Some(b"dalywave files"), secret);
        let mut key = [0u8; 32];
        hk.expand(b"file links v1", &mut key)
            .map_err(|_| anyhow::anyhow!("file link key"))?;
        let base = |u: &str| Url::parse(&format!("{}/", u.trim_end_matches('/')));
        Ok(Self {
            key,
            public: base(public_url)?,
            ml: base(ml_url.unwrap_or(public_url))?,
        })
    }

    /// Для тестов и хранилища в памяти: адрес `http://storage.invalid`.
    pub fn local() -> Self {
        match Self::new(&[0; 32], "http://storage.invalid", None) {
            Ok(l) => l,
            Err(e) => unreachable!("local links: {e}"),
        }
    }

    fn mac(&self, method: &str, key: &str, expires: u64, download: &str) -> HmacSha256 {
        let mut mac = match <HmacSha256 as KeyInit>::new_from_slice(&self.key) {
            Ok(m) => m,
            Err(e) => unreachable!("hmac accepts any key length: {e}"),
        };
        for part in [method, key, &expires.to_string(), download] {
            mac.update(part.as_bytes());
            mac.update(b"\n");
        }
        mac
    }

    /// Подписанная ссылка на ключ.
    pub fn sign(&self, key: &str, how: Presign<'_>, ttl: Duration) -> Result<Url, StorageError> {
        let expires = now() + ttl.as_secs().max(1);
        let (method, download, base) = match how {
            Presign::Get => ("GET", "", &self.public),
            Presign::GetForMl => ("GET", "", &self.ml),
            Presign::Download(name) => ("GET", name, &self.public),
            Presign::Put => ("PUT", "", &self.public),
        };
        let sig = crate::crypto::b64(
            self.mac(method, key, expires, download)
                .finalize()
                .into_bytes(),
        );
        let mut url = base
            .join(PREFIX.trim_start_matches('/'))
            .map_err(|e| StorageError::Key(e.to_string()))?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| StorageError::Key(key.to_string()))?;
            segments.pop_if_empty();
            segments.extend(key.split('/'));
        }
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("e", &expires.to_string());
            if !download.is_empty() {
                q.append_pair("dl", download);
            }
            q.append_pair("s", &sig);
        }
        Ok(url)
    }

    /// Проверка ссылки: срок не вышел, подпись сходится. `None` — ссылка не принимается.
    pub fn verify(
        &self,
        method: &str,
        key: &str,
        expires: u64,
        download: Option<&str>,
        signature: &str,
    ) -> Option<Signed> {
        if expires < now() {
            return None;
        }
        let put = method == "PUT";
        let method = if put { "PUT" } else { "GET" };
        let sig = crate::crypto::b64_decode(signature)?;
        self.mac(method, key, expires, download.unwrap_or(""))
            .verify_slice(&sig)
            .ok()?;
        Some(Signed {
            put,
            download: download.map(str::to_string),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(url: &Url) -> (String, u64, Option<String>, String) {
        let q: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();
        let key = url
            .path()
            .strip_prefix(PREFIX)
            .unwrap()
            .split('/')
            .map(percent_decode)
            .collect::<Vec<_>>()
            .join("/");
        (
            key,
            q["e"].parse().unwrap(),
            q.get("dl").cloned(),
            q["s"].clone(),
        )
    }

    fn percent_decode(s: &str) -> String {
        url::form_urlencoded::parse(format!("x={}", s.replace('+', "%2B")).as_bytes())
            .next()
            .unwrap()
            .1
            .into_owned()
    }

    #[test]
    fn signed_links_roundtrip() {
        let links = Links::new(
            &[3; 32],
            "https://wave.example.com/",
            Some("http://api:8400"),
        )
        .unwrap();
        let key = "library/music/Кино/01 Группа крови.flac";
        let url = links
            .sign(key, Presign::Get, Duration::from_secs(60))
            .unwrap();
        assert!(
            url.as_str()
                .starts_with("https://wave.example.com/api/files/library/music/"),
            "{url}"
        );
        let (k, e, dl, s) = parts(&url);
        assert_eq!(k, key);
        assert_eq!(
            links.verify("GET", &k, e, dl.as_deref(), &s),
            Some(Signed {
                put: false,
                download: None
            })
        );
        assert_eq!(
            links.verify("HEAD", &k, e, None, &s).map(|s| s.put),
            Some(false)
        );
        // Другой ключ, метод или срок — не та подпись
        assert!(
            links
                .verify("GET", "library/music/other.flac", e, None, &s)
                .is_none()
        );
        assert!(links.verify("PUT", &k, e, None, &s).is_none());
        assert!(links.verify("GET", &k, e + 1, None, &s).is_none());
        assert!(links.verify("GET", &k, e, None, "garbage").is_none());
        // Чужой секрет
        let other = Links::new(&[4; 32], "https://wave.example.com", None).unwrap();
        assert!(other.verify("GET", &k, e, None, &s).is_none());

        let ml = links
            .sign("audio/r/f.flac", Presign::GetForMl, Duration::from_secs(60))
            .unwrap();
        assert_eq!(ml.host_str(), Some("api"));
        assert_eq!(ml.port(), Some(8400));

        let dl = links
            .sign(
                "releases/a.dmg",
                Presign::Download("DalyWave 1.dmg"),
                Duration::from_secs(60),
            )
            .unwrap();
        let (k, e, name, s) = parts(&dl);
        assert_eq!(name.as_deref(), Some("DalyWave 1.dmg"));
        assert!(
            links.verify("GET", &k, e, None, &s).is_none(),
            "имя скачивания входит в подпись"
        );
        assert!(links.verify("GET", &k, e, Some("other.dmg"), &s).is_none());
        assert!(links.verify("GET", &k, e, name.as_deref(), &s).is_some());

        let put = links
            .sign("audio/r/new.m4a", Presign::Put, Duration::from_secs(60))
            .unwrap();
        let (k, e, _, s) = parts(&put);
        assert_eq!(
            links.verify("PUT", &k, e, None, &s).map(|s| s.put),
            Some(true)
        );
        assert!(links.verify("GET", &k, e, None, &s).is_none());
    }

    #[test]
    fn expired_links_are_refused() {
        let links = Links::local();
        let url = links
            .sign("audio/x.flac", Presign::Get, Duration::from_secs(60))
            .unwrap();
        let (k, _, _, s) = parts(&url);
        assert!(links.verify("GET", &k, now() - 1, None, &s).is_none());
    }
}
