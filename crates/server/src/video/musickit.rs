//! Apple Music API по ключу MusicKit: альбом в каталоге по UPC или ISRC трека и его живая обложка
//! (`editorialVideo`: `motionDetailSquare`, `motionDetailTall` — HLS).
//!
//! Токен разработчика — JWT (ES256) с Key ID в заголовке и Team ID в `iss`; живёт до полугода,
//! сервер подписывает его сам и держит в памяти, пока до конца срока больше суток.
//!
//! Внимание: на 2026-11 Apple отдаёт `editorialVideo` только своим приложениям — сторонним
//! токенам разработчика поле не приходит (ответ инженера Apple на форумах разработчиков, проверено
//! вживую). Поэтому обычный итог поиска — «живой обложки нет»; код готов к тому дню, когда поле
//! откроют, и такие альбомы перепроверяются раз в месяц.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use chrono::Utc;
use p256::{
    SecretKey,
    ecdsa::{Signature, SigningKey, signature::Signer},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{config::MusicKitConfig, crypto::b64};

/// Срок токена: до шести месяцев по правилам Apple, берём с запасом.
const TOKEN_TTL: Duration = Duration::from_secs(150 * 24 * 3600);
/// Перевыпуск, когда до конца срока осталось меньше.
const TOKEN_MARGIN: Duration = Duration::from_secs(24 * 3600);
/// Сколько альбомов-кандидатов по ISRC проверять на живую обложку.
const MAX_CANDIDATES: usize = 6;

#[derive(Debug, thiserror::Error)]
pub enum MusicKitError {
    #[error("Apple Music недоступен: {0}")]
    Network(String),
    #[error("Apple Music не принял токен (HTTP {0})")]
    Auth(u16),
    #[error("Apple Music занят")]
    Busy,
    #[error("Apple Music ответил HTTP {0}")]
    Status(u16),
    #[error("Apple Music прислал непонятный ответ: {0}")]
    Decode(String),
}

impl MusicKitError {
    /// Повтор позже поможет.
    pub fn transient(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Busy | Self::Status(500..=599)
        )
    }
}

/// Что известно об альбоме в Apple Music.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// id альбома в каталоге Apple Music; `None` — альбом не нашёлся.
    pub apple_album_id: Option<String>,
    /// HLS квадратной живой обложки.
    pub square: Option<String>,
    /// HLS вертикальной (3:4 или выше).
    pub tall: Option<String>,
}

impl Found {
    pub fn has_video(&self) -> bool {
        self.square.is_some() || self.tall.is_some()
    }
}

/// Что известно о нашем альбоме для поиска.
#[derive(Debug, Clone, Default)]
pub struct AlbumQuery {
    pub title: String,
    pub upc: Option<String>,
    /// ISRC треков альбома (первые несколько).
    pub isrcs: Vec<String>,
}

pub struct MusicKit {
    http: reqwest::Client,
    signing: SigningKey,
    config: MusicKitConfig,
    token: Mutex<Option<(String, Instant)>>,
}

#[derive(Debug, Deserialize)]
struct Resource {
    id: String,
    #[serde(default)]
    attributes: Value,
    #[serde(default)]
    relationships: Value,
}

#[derive(Debug, Deserialize)]
struct Page {
    #[serde(default)]
    data: Vec<Resource>,
}

impl MusicKit {
    /// Ключ .p8 (PKCS#8 PEM) — из файла `key_path`.
    pub fn load(config: &MusicKitConfig) -> anyhow::Result<Self> {
        let pem = std::fs::read_to_string(&config.key_path).map_err(|e| {
            anyhow::anyhow!(
                "cannot read the MusicKit key {}: {e}",
                config.key_path.display()
            )
        })?;
        Self::from_pem(config, &pem)
    }

    pub fn from_pem(config: &MusicKitConfig, pem: &str) -> anyhow::Result<Self> {
        let secret = SecretKey::from_pem(pem.trim())
            .map_err(|e| anyhow::anyhow!("the MusicKit key is not a P-256 .p8 key: {e}"))?;
        let http = reqwest::Client::builder()
            .user_agent(crate::info::USER_AGENT)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            http,
            signing: SigningKey::from(&secret),
            config: config.clone(),
            token: Mutex::new(None),
        })
    }

    /// Токен разработчика: прежний, пока до конца срока больше суток.
    pub fn developer_token(&self) -> String {
        let mut cached = self.token.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((token, at)) = cached.as_ref()
            && at.elapsed() + TOKEN_MARGIN < TOKEN_TTL
        {
            return token.clone();
        }
        let now = Utc::now().timestamp();
        let header = b64(json!({ "alg": "ES256", "kid": self.config.key_id }).to_string());
        let claims = b64(json!({
            "iss": self.config.team_id,
            "iat": now,
            "exp": now + TOKEN_TTL.as_secs() as i64,
        })
        .to_string());
        let input = format!("{header}.{claims}");
        let signature: Signature = self.signing.sign(input.as_bytes());
        let token = format!("{input}.{}", b64(signature.to_bytes()));
        *cached = Some((token.clone(), Instant::now()));
        token
    }

    fn forget_token(&self) {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    async fn get(&self, path_and_query: &str) -> Result<Option<Page>, MusicKitError> {
        let url = format!("{}{path_and_query}", self.config.url);
        let res = self
            .http
            .get(&url)
            .bearer_auth(self.developer_token())
            .send()
            .await
            .map_err(|e| MusicKitError::Network(e.without_url().to_string()))?;
        let status = res.status().as_u16();
        match status {
            200..=299 => {
                let bytes = res
                    .bytes()
                    .await
                    .map_err(|e| MusicKitError::Network(e.without_url().to_string()))?;
                serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|e| MusicKitError::Decode(e.to_string()))
            }
            404 => Ok(None),
            401 | 403 => {
                self.forget_token();
                Err(MusicKitError::Auth(status))
            }
            429 => Err(MusicKitError::Busy),
            s => Err(MusicKitError::Status(s)),
        }
    }

    fn catalog(&self) -> String {
        format!("/v1/catalog/{}", self.config.storefront)
    }

    /// Альбомы по id с живой обложкой (`extend=editorialVideo`).
    async fn albums(&self, ids: &[String]) -> Result<Vec<Resource>, MusicKitError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let q = format!(
            "{}/albums?ids={}&extend=editorialVideo&omit[resource]=relationships",
            self.catalog(),
            ids.join(",")
        );
        Ok(self.get(&q).await?.map(|p| p.data).unwrap_or_default())
    }

    /// Альбом и его живая обложка: сначала по UPC, иначе по ISRC треков — из альбомов с этой песней
    /// выбирается не сборник с тем же названием.
    pub async fn find(&self, query: &AlbumQuery) -> Result<Found, MusicKitError> {
        if let Some(upc) = query.upc.as_deref().filter(|u| valid_code(u)) {
            let q = format!(
                "{}/albums?filter[upc]={upc}&extend=editorialVideo&omit[resource]=relationships",
                self.catalog()
            );
            if let Some(album) = self.get(&q).await?.and_then(|p| p.data.into_iter().next()) {
                return Ok(found(&album));
            }
        }
        for isrc in query.isrcs.iter().filter(|i| valid_code(i)).take(3) {
            let q = format!(
                "{}/songs?filter[isrc]={isrc}&include=albums",
                self.catalog()
            );
            let Some(page) = self.get(&q).await? else {
                continue;
            };
            let ids = candidate_albums(&page.data, &query.title);
            if ids.is_empty() {
                continue;
            }
            let albums = self.albums(&ids).await?;
            // Порядок кандидатов — по сходству; первым — тот, где живая обложка есть
            let mut best: Option<Found> = None;
            for id in &ids {
                let Some(album) = albums.iter().find(|a| &a.id == id) else {
                    continue;
                };
                let f = found(album);
                if f.has_video() {
                    return Ok(f);
                }
                best.get_or_insert(f);
            }
            if let Some(f) = best {
                return Ok(f);
            }
        }
        Ok(Found::default())
    }
}

/// UPC и ISRC — только буквы и цифры: в адрес запроса больше ничего не попадёт.
fn valid_code(code: &str) -> bool {
    (8..=20).contains(&code.len()) && code.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Ссылки живой обложки альбома.
fn found(album: &Resource) -> Found {
    let video = |keys: &[&str]| {
        keys.iter().find_map(|k| {
            album
                .attributes
                .pointer(&format!("/editorialVideo/{k}/video"))
                .and_then(Value::as_str)
                .filter(|u| u.starts_with("https://") || u.starts_with("http://"))
                .map(str::to_string)
        })
    };
    Found {
        apple_album_id: Some(album.id.clone()),
        square: video(&["motionDetailSquare", "motionSquareVideo1x1"]),
        tall: video(&[
            "motionDetailTall",
            "motionTallVideo3x4",
            "motionDetailPortrait",
        ]),
    }
}

/// Альбомы песни по сходству с нашим: то же название — первым, сборники — последними.
fn candidate_albums(songs: &[Resource], title: &str) -> Vec<String> {
    let want = crate::info::loose(&strip_edition(title));
    let mut scored: Vec<(i32, usize, String, bool)> = Vec::new();
    for song in songs {
        let Some(albums) = song
            .relationships
            .pointer("/albums/data")
            .and_then(Value::as_array)
        else {
            continue;
        };
        let album_name = song
            .attributes
            .get("albumName")
            .and_then(Value::as_str)
            .unwrap_or_default();
        for a in albums {
            let Some(id) = a.get("id").and_then(Value::as_str) else {
                continue;
            };
            if scored.iter().any(|s| s.2 == id) {
                continue;
            }
            let name = crate::info::loose(&strip_edition(album_name));
            let compilation = a
                .pointer("/attributes/isCompilation")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut score = 0;
            if !want.is_empty() && name == want {
                score += 10;
            } else if !want.is_empty() && (name.contains(&want) || want.contains(&name)) {
                score += 5;
            }
            if compilation {
                score -= 8;
            }
            scored.push((score, scored.len(), id.to_string(), compilation));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    // Название не совпало ни с одним (перевод, другая запись): тогда — первые не сборники
    let matched = scored.first().is_some_and(|s| s.0 > 0);
    scored
        .into_iter()
        .filter(|s| if matched { s.0 > 0 } else { !s.3 })
        .take(if matched { MAX_CANDIDATES } else { 3 })
        .map(|s| s.2)
        .collect()
}

/// «Midnights (3am Edition)», «GUTS - Deluxe» → основа названия.
fn strip_edition(title: &str) -> String {
    let base = crate::info::apple::search_title(title);
    base.split(" - ").next().unwrap_or(&base).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn songs(json: &str) -> Vec<Resource> {
        serde_json::from_str::<Page>(json).unwrap().data
    }

    #[test]
    fn albums_with_the_same_title_come_first() {
        let data = songs(
            r#"{"data": [
                {"id": "1", "attributes": {"albumName": "Best Hits Of 2022"},
                 "relationships": {"albums": {"data": [{"id": "10", "attributes": {"isCompilation": true}}]}}},
                {"id": "2", "attributes": {"albumName": "Midnights (3am Edition)"},
                 "relationships": {"albums": {"data": [{"id": "20"}]}}},
                {"id": "3", "attributes": {"albumName": "Midnights"},
                 "relationships": {"albums": {"data": [{"id": "30"}]}}},
                {"id": "4", "attributes": {"albumName": "Anti-Hero - Single"},
                 "relationships": {"albums": {"data": [{"id": "40"}]}}}
            ]}"#,
        );
        assert_eq!(candidate_albums(&data, "Midnights"), vec!["20", "30"]);
        // Название не совпало: первые три не сборника
        assert_eq!(candidate_albums(&data, "Полночь"), vec!["20", "30", "40"]);
    }

    #[test]
    fn editorial_video_is_parsed() {
        let album: Resource = serde_json::from_str(
            r#"{"id": "1739659134", "attributes": {"name": "X", "editorialVideo": {
                "motionDetailSquare": {"video": "https://mvod.itunes.apple.com/a/square.m3u8"},
                "motionDetailTall": {"video": "https://mvod.itunes.apple.com/a/tall.m3u8"},
                "motionSquareVideo1x1": {"video": "https://mvod.itunes.apple.com/a/other.m3u8"}
            }}}"#,
        )
        .unwrap();
        let f = found(&album);
        assert_eq!(
            f.square.as_deref(),
            Some("https://mvod.itunes.apple.com/a/square.m3u8")
        );
        assert_eq!(
            f.tall.as_deref(),
            Some("https://mvod.itunes.apple.com/a/tall.m3u8")
        );
        let plain: Resource =
            serde_json::from_str(r#"{"id": "1", "attributes": {"name": "X"}}"#).unwrap();
        assert!(!found(&plain).has_video());
        assert_eq!(found(&plain).apple_album_id.as_deref(), Some("1"));
    }

    #[test]
    fn codes_are_checked_before_going_into_a_url() {
        assert!(valid_code("USUG12205736"));
        assert!(valid_code("00602448762689"));
        assert!(!valid_code("USUG1220&x=1"));
        assert!(!valid_code("short"));
    }

    /// Живая проверка с настоящим ключом: `DALYWAVE_MUSICKIT_KEY_PATH=… cargo test -p dalywave-server
    /// live_animated_covers -- --ignored --nocapture`. Печатает, что Apple отдал по популярным альбомам.
    #[tokio::test]
    #[ignore = "ходит в Apple Music с настоящим ключом"]
    async fn live_animated_covers() {
        let Ok(key_path) = std::env::var("DALYWAVE_MUSICKIT_KEY_PATH") else {
            return;
        };
        let config = MusicKitConfig {
            key_path: key_path.into(),
            key_id: std::env::var("DALYWAVE_MUSICKIT_KEY_ID").unwrap_or("TNR3H4SF5N".into()),
            team_id: std::env::var("DALYWAVE_MUSICKIT_TEAM_ID").unwrap_or("Y5KR5JPCRG".into()),
            storefront: std::env::var("DALYWAVE_MUSICKIT_STOREFRONT").unwrap_or("us".into()),
            url: "https://api.music.apple.com".into(),
        };
        let mk = MusicKit::load(&config).unwrap();
        // Альбомы, у которых в Apple Music есть живая обложка
        let albums = [
            ("HIT ME HARD AND SOFT", "USUM72401994"),
            ("UTOPIA", "USSM12305574"),
            ("GNX", "USUG12408692"),
            ("SOS", "USRC12204589"),
            ("GUTS", "USUG12305936"),
            ("Dawn FM", "USUG12200046"),
            ("Midnights", "USUG12205736"),
            ("The Tortured Poets Department", "USUG12401376"),
            ("Short n' Sweet", "USUM72406937"),
            ("Hurry Up Tomorrow", "USUG12406388"),
        ];
        for (title, isrc) in albums {
            let q = AlbumQuery {
                title: title.into(),
                upc: None,
                isrcs: vec![isrc.into()],
            };
            match mk.find(&q).await {
                Ok(f) => println!(
                    "{title}: album {:?}, square {}, tall {}",
                    f.apple_album_id,
                    f.square.is_some(),
                    f.tall.is_some()
                ),
                Err(e) => println!("{title}: {e}"),
            }
        }
    }

    #[test]
    fn edition_suffixes_are_ignored() {
        assert_eq!(strip_edition("Midnights (3am Edition)"), "Midnights");
        assert_eq!(strip_edition("GUTS - Deluxe"), "GUTS");
    }
}
