//! Перенос из «Яндекс Музыки». Официального API нет; это тот же JSON API, которым пользуются её
//! приложения (`api.music.yandex.net`).
//!
//! Что проверено вживую (2026-11-12, с адреса в Германии — там же стоит сервер): без входа API
//! отвечает `451 Unavailable For Legal Reasons` на любой запрос — Яндекс не отдаёт каталог за пределами
//! своих стран; старые обработчики сайта (`/handlers/playlist.jsx`) убраны, а новая страница плейлиста
//! собирается в браузере и треков в HTML не содержит. Поэтому сначала запрос идёт без входа (из
//! страны, где Яндекс отвечает, публичный плейлист так и переносится), а на «нужен вход» человек
//! вставляет свой ключ входа — OAuth-токен «Яндекс Музыки». Ключ живёт только в этом запросе: в базу,
//! задачи и журналы он не попадает, в URL не ставится (только заголовок `Authorization`).
//!
//! - плейлист: `GET /users/{логин}/playlists/{номер}` (номер 3 — «Мне нравится»), общая ссылка нового
//!   сайта — `GET /playlist/{id}`;
//! - «Мне нравится»: `GET /users/{логин}/likes/tracks` (только id) → `POST /tracks` пачками;
//! - весь аккаунт (ключ без ссылки): `GET /account/status` → лайки и `GET /users/{uid}/playlists/list`.

use std::time::Duration;

use futures_util::{StreamExt, stream};
use reqwest::{RequestBuilder, StatusCode};
use serde_json::Value;

use super::collection::{self, Collection};

/// Номер плейлиста «Мне нравится».
pub const LIKES_KIND: i64 = 3;
/// Треков за один `POST /tracks`.
const TRACKS_BATCH: usize = 250;
/// Плейлистов аккаунта за один перенос.
const MAX_PLAYLISTS: usize = 300;
/// Один запрос к Яндексу.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Куда ведёт ссылка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// `…/users/<логин>/playlists/<номер>`
    Playlist { owner: String, kind: i64 },
    /// `…/playlists/<id>` — общая ссылка нового сайта.
    Shared(String),
    /// `…/users/<логин>/tracks` или плейлист номер 3.
    Likes { owner: String },
}

/// Разбирает ссылку на плейлист или «Мне нравится». `None` — это не ссылка «Яндекс Музыки».
pub fn parse_link(raw: &str) -> Option<Link> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let url = url::Url::parse(&with_scheme).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if !host.starts_with("music.yandex.") && host != "next.music.yandex.ru" {
        return None;
    }
    let parts: Vec<String> = url
        .path_segments()?
        .filter(|p| !p.is_empty())
        .map(|p| {
            url::form_urlencoded::parse(format!("x={p}").as_bytes())
                .next()
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default()
        })
        .collect();
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    let playlist = |owner: &str, kind: &str| {
        let kind: i64 = kind.parse().ok()?;
        Some(if kind == LIKES_KIND {
            Link::Likes {
                owner: owner.to_string(),
            }
        } else {
            Link::Playlist {
                owner: owner.to_string(),
                kind,
            }
        })
    };
    match parts.as_slice() {
        ["users", owner, "playlists", kind, ..] => playlist(owner, kind),
        ["iframe", "playlist", owner, kind, ..] => playlist(owner, kind),
        ["users", owner, "tracks" | "likes", ..] => Some(Link::Likes {
            owner: owner.to_string(),
        }),
        ["playlists", id, ..] if valid_id(id) => Some(Link::Shared(id.to_string())),
        _ => None,
    }
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Ключ входа как есть или вся адресная строка после входа (`…#access_token=…&token_type=…`).
pub fn parse_token(raw: &str) -> Option<String> {
    let s = raw.trim();
    let s = match s.find("access_token=") {
        Some(i) => s[i + "access_token=".len()..]
            .split(['&', '#', ' ', '\n'])
            .next()
            .unwrap_or(""),
        None => s,
    };
    let s = s.trim().trim_matches(['"', '\'']);
    (s.len() >= 20
        && s.len() <= 400
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
    .then(|| s.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum YandexError {
    #[error("«Яндекс Музыка» не отдаёт это без входа — нужен ключ входа")]
    LoginRequired,
    #[error("ключ входа не подошёл — возьмите новый")]
    TokenRejected,
    #[error("плейлист не нашёлся или скрыт владельцем")]
    NotFound,
    #[error("«Яндекс Музыка» не отвечает серверу из его страны")]
    Region,
    #[error("«Яндекс Музыка» не ответила: {0}")]
    Unavailable(String),
    #[error("«Яндекс Музыка» ответила непонятно")]
    Format,
}

/// Что получилось собрать.
#[derive(Debug, Default)]
pub struct Fetched {
    pub collection: Collection,
    /// Название для истории переносов: имя плейлиста или «Мне нравится».
    pub title: Option<String>,
}

pub struct Yandex<'a> {
    http: &'a reqwest::Client,
    base: &'a str,
    token: Option<&'a str>,
}

impl<'a> Yandex<'a> {
    pub fn new(http: &'a reqwest::Client, base: &'a str, token: Option<&'a str>) -> Self {
        Self { http, base, token }
    }

    async fn send(&self, req: RequestBuilder) -> Result<Value, YandexError> {
        let mut req = req
            .timeout(TIMEOUT)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(t) = self.token {
            req = req.header(reqwest::header::AUTHORIZATION, format!("OAuth {t}"));
        }
        // Текст ошибки reqwest — без адреса: в нём ничего секретного нет, но и пользы в журнале тоже
        let res = req
            .send()
            .await
            .map_err(|e| YandexError::Unavailable(e.without_url().to_string()))?;
        let status = res.status();
        let authed = self.token.is_some();
        match status {
            s if s.is_success() => {}
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS if authed => return Err(YandexError::Region),
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS => return Err(YandexError::LoginRequired),
            StatusCode::UNAUTHORIZED if authed => return Err(YandexError::TokenRejected),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN if !authed => {
                return Err(YandexError::LoginRequired);
            }
            StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => return Err(YandexError::NotFound),
            s => return Err(YandexError::Unavailable(format!("HTTP {}", s.as_u16()))),
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| YandexError::Unavailable(e.without_url().to_string()))?;
        match body.get("result") {
            Some(r) if !r.is_null() => Ok(r.clone()),
            _ => Err(YandexError::Format),
        }
    }

    async fn get(&self, path: &str) -> Result<Value, YandexError> {
        self.send(self.http.get(format!("{}{path}", self.base)))
            .await
    }

    /// Собирает треки по ссылке, а без неё (с ключом) — «Мне нравится» и плейлисты аккаунта.
    pub async fn fetch(&self, link: Option<&Link>) -> Result<Fetched, YandexError> {
        let mut out = Fetched::default();
        match link {
            Some(Link::Playlist { owner, kind }) => {
                let pl = self
                    .get(&format!("/users/{}/playlists/{kind}", seg(owner)))
                    .await?;
                out.title = Some(self.add_playlist(&mut out.collection, &pl).await?);
            }
            Some(Link::Shared(id)) => {
                let pl = self.get(&format!("/playlist/{}", seg(id))).await?;
                out.title = Some(self.add_playlist(&mut out.collection, &pl).await?);
            }
            Some(Link::Likes { owner }) => {
                self.add_likes(&mut out.collection, owner).await?;
                out.title = Some("Мне нравится".into());
            }
            None => {
                if self.token.is_none() {
                    return Err(YandexError::LoginRequired);
                }
                let status = self.get("/account/status").await?;
                let uid = match &status["account"]["uid"] {
                    Value::Number(n) => n.to_string(),
                    Value::String(s) if !s.is_empty() => s.clone(),
                    _ => return Err(YandexError::Format),
                };
                self.add_likes(&mut out.collection, &uid).await?;
                let list = self.get(&format!("/users/{uid}/playlists/list")).await?;
                let kinds: Vec<i64> = list
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|p| p["trackCount"].as_i64().unwrap_or(1) > 0)
                            .filter_map(|p| p["kind"].as_i64())
                            .filter(|k| *k != LIKES_KIND)
                            .take(MAX_PLAYLISTS)
                            .collect()
                    })
                    .unwrap_or_default();
                // Плейлисты — по нескольку сразу, раскладываются в исходном порядке
                let fetched: Vec<Result<Value, YandexError>> = stream::iter(kinds)
                    .map(|k| {
                        let path = format!("/users/{uid}/playlists/{k}");
                        async move { self.get(&path).await }
                    })
                    .buffered(4)
                    .collect()
                    .await;
                for pl in fetched {
                    match pl {
                        Ok(pl) => {
                            self.add_playlist(&mut out.collection, &pl).await?;
                        }
                        // Плейлист удалили, пока шёл перенос, — без него
                        Err(YandexError::NotFound) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        Ok(out)
    }

    /// Треки плейлиста по порядку. Если в ответе только id — дозапрашиваются пачкой.
    async fn add_playlist(&self, c: &mut Collection, pl: &Value) -> Result<String, YandexError> {
        let title = pl["title"].as_str().unwrap_or("").trim().to_string();
        let entries = pl["tracks"].as_array().cloned().unwrap_or_default();
        let mut tracks: Vec<Option<Value>> = Vec::with_capacity(entries.len());
        let mut missing = Vec::new();
        for e in &entries {
            if e["track"].is_object() {
                tracks.push(Some(e["track"].clone()));
            } else if let Some(id) = track_ref(e) {
                missing.push((tracks.len(), id));
                tracks.push(None);
            }
        }
        if !missing.is_empty() {
            let ids: Vec<String> = missing.iter().map(|(_, id)| id.clone()).collect();
            let full = self.tracks(&ids).await?;
            for (slot, id) in missing {
                let bare = id.split(':').next().unwrap_or(&id).to_string();
                tracks[slot] = full
                    .iter()
                    .find(|t| id_of(t).as_deref() == Some(bare.as_str()))
                    .cloned();
            }
        }
        let positions: Vec<usize> = tracks
            .iter()
            .flatten()
            .filter_map(|t| track(t, false).and_then(|t| c.add(t)))
            .collect();
        let likes = pl["kind"].as_i64() == Some(LIKES_KIND);
        if likes {
            for i in positions {
                c.tracks[i].liked = true;
            }
        } else if !positions.is_empty() {
            c.playlist(&title, pl["description"].as_str().unwrap_or(""), positions);
        }
        Ok(if likes {
            "Мне нравится".into()
        } else {
            title
        })
    }

    async fn add_likes(&self, c: &mut Collection, owner: &str) -> Result<(), YandexError> {
        let lib = self
            .get(&format!("/users/{}/likes/tracks", seg(owner)))
            .await?;
        let ids: Vec<String> = lib["library"]["tracks"]
            .as_array()
            .map(|a| a.iter().filter_map(track_ref).collect())
            .unwrap_or_default();
        for t in self.tracks(&ids).await? {
            if let Some(t) = track(&t, true) {
                c.add(t);
            }
        }
        Ok(())
    }

    /// Полные треки по id (`123` или `123:456` с альбомом), в том же порядке.
    async fn tracks(&self, ids: &[String]) -> Result<Vec<Value>, YandexError> {
        let mut out = Vec::with_capacity(ids.len());
        for chunk in ids
            .chunks(TRACKS_BATCH)
            .take(collection::MAX_TRACKS / TRACKS_BATCH)
        {
            let form = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("track-ids", &chunk.join(","))
                .append_pair("with-positions", "false")
                .finish();
            let res = self
                .send(
                    self.http
                        .post(format!("{}/tracks", self.base))
                        .header(
                            reqwest::header::CONTENT_TYPE,
                            "application/x-www-form-urlencoded",
                        )
                        .body(form),
                )
                .await?;
            out.extend(res.as_array().cloned().unwrap_or_default());
        }
        Ok(out)
    }
}

/// Часть пути: логин и id без `/` и прочего.
fn seg(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn id_of(t: &Value) -> Option<String> {
    match &t["id"] {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// `id` или `id:albumId` из короткой записи (лайки, плейлист без треков).
fn track_ref(e: &Value) -> Option<String> {
    let id = id_of(e)?;
    let album = match &e["albumId"] {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    Some(match album {
        Some(a) if !id.contains(':') => format!("{id}:{a}"),
        _ => id,
    })
}

/// Трек «Яндекс Музыки» → позиция. Подкасты и аудиокниги не переносим.
pub fn track(t: &Value, liked: bool) -> Option<collection::Track> {
    let kind = t["type"].as_str().unwrap_or("music");
    if kind.contains("podcast") || kind.contains("audiobook") || kind.contains("fairy") {
        return None;
    }
    let mut title = t["title"].as_str()?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    if let Some(v) = t["version"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        title = format!("{title} ({v})");
    }
    let artists: Vec<&str> = t["artists"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["name"].as_str())
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some(collection::Track {
        artist: artists.join(", "),
        title,
        album: t["albums"][0]["title"].as_str().map(String::from),
        duration_ms: t["durationMs"].as_i64().and_then(|d| i32::try_from(d).ok()),
        source_ref: id_of(t).map(|id| format!("ym:{id}")),
        liked,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn links() {
        assert_eq!(
            parse_link("https://music.yandex.ru/users/daly/playlists/1001?utm_source=web"),
            Some(Link::Playlist {
                owner: "daly".into(),
                kind: 1001
            })
        );
        assert_eq!(
            parse_link("music.yandex.com/users/daly.rest/playlists/3"),
            Some(Link::Likes {
                owner: "daly.rest".into()
            })
        );
        assert_eq!(
            parse_link("https://music.yandex.ru/users/daly/tracks"),
            Some(Link::Likes {
                owner: "daly".into()
            })
        );
        assert_eq!(
            parse_link("https://music.yandex.ru/playlists/lk.7c1b0f6e-1d1b-4c7a-9f0e-2b8a3f0e1d2c"),
            Some(Link::Shared(
                "lk.7c1b0f6e-1d1b-4c7a-9f0e-2b8a3f0e1d2c".into()
            ))
        );
        assert_eq!(parse_link("https://music.yandex.ru/album/123"), None);
        assert_eq!(parse_link("https://evil.example/users/x/playlists/1"), None);
        assert_eq!(parse_link(""), None);
    }

    #[test]
    fn tokens() {
        let t = "y0_AgAAAAAAbcdefghijklmnopqrstuvwxyz0123456789";
        assert_eq!(parse_token(t).as_deref(), Some(t));
        assert_eq!(
            parse_token(&format!(
                "https://music.yandex.ru/#access_token={t}&token_type=bearer&expires_in=31536000"
            ))
            .as_deref(),
            Some(t)
        );
        assert_eq!(parse_token("short"), None);
        assert_eq!(parse_token("has spaces inside of the token value"), None);
    }

    #[test]
    fn tracks_become_positions() {
        let t = json!({
            "id": "33311009", "title": "Devil", "version": "Remix", "durationMs": 162000,
            "artists": [{"name": "Lowborn"}, {"name": "vista"}],
            "albums": [{"id": 1, "title": "Devil"}]
        });
        let p = track(&t, true).unwrap();
        assert_eq!(p.artist, "Lowborn, vista");
        assert_eq!(p.title, "Devil (Remix)");
        assert_eq!(p.album.as_deref(), Some("Devil"));
        assert_eq!(p.duration_ms, Some(162_000));
        assert_eq!(p.source_ref.as_deref(), Some("ym:33311009"));
        assert!(p.liked);
        let podcast = json!({"id": 5, "title": "Выпуск", "type": "podcast-episode"});
        assert!(track(&podcast, false).is_none());
        assert_eq!(
            track_ref(&json!({"id": "1", "albumId": 2})).as_deref(),
            Some("1:2")
        );
        assert_eq!(track_ref(&json!({"id": 7})).as_deref(), Some("7"));
    }
}
