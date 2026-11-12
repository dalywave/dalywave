//! Чужие ссылки на музыку → что искать в каталоге. Deezer и свои ссылки разбираются по адресу;
//! Spotify, Apple Music и Bandcamp — по тегам Open Graph публичной страницы (API Spotify не
//! используется), YouTube и SoundCloud — по oEmbed. Сервер ходит только на известные хосты и только
//! по https: произвольные адреса из ссылки не открываются.

use std::time::Duration;

use dalywave_protocol::{catalog::DeezerKind, share::ShareKind};
use unicode_normalization::UnicodeNormalization;
use url::Url;
use uuid::Uuid;

/// Больше страницы не читаем: теги Open Graph — в начале, в `<head>`.
const PAGE_LIMIT: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(8);
/// Самые длинные ссылки «Поделиться» — сотни символов; длиннее — не ссылка, а мусор.
pub const MAX_TEXT: usize = 4096;

pub const YOUTUBE_OEMBED: &str = "https://www.youtube.com/oembed";
pub const SOUNDCLOUD_OEMBED: &str = "https://soundcloud.com/oembed";

/// Что за ссылка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    Own {
        kind: ShareKind,
        id: Uuid,
    },
    Deezer {
        kind: DeezerKind,
        id: i64,
    },
    /// `link.deezer.com/s/…`, `deezer.page.link/…`: настоящий адрес — в редиректе
    DeezerShort(Url),
    /// `kind` неизвестен у коротких `spotify.link/…` — узнаём по итоговому адресу страницы
    Spotify {
        kind: Option<ShareKind>,
        url: Url,
    },
    Apple {
        kind: ShareKind,
        url: Url,
    },
    Youtube {
        url: Url,
    },
    Soundcloud {
        url: Url,
    },
    Bandcamp {
        kind: ShareKind,
        url: Url,
    },
}

/// Первая http(s)-ссылка в тексте: «Поделиться» из приложений часто шлёт «Название — ссылка».
pub fn extract_url(text: &str) -> Option<Url> {
    let start = text.find("https://").or_else(|| text.find("http://"))?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>' | '«' | '»'))
        .unwrap_or(rest.len());
    let raw = rest[..end].trim_end_matches(['.', ',', ')', ';', '!', '?']);
    Url::parse(raw)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
}

fn segments(url: &Url) -> Vec<&str> {
    url.path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default()
}

fn share_kind(s: &str) -> Option<ShareKind> {
    s.parse().ok()
}

/// Разбор адреса. `own_hosts` — хосты DalyWave (из `PUBLIC_URL` и боевой).
pub fn parse(url: &Url, own_hosts: &[&str]) -> Option<Parsed> {
    let full = url.host_str()?.to_ascii_lowercase();
    let host = full.strip_prefix("www.").unwrap_or(&full).to_string();
    let path = segments(url);
    if own_hosts
        .iter()
        .any(|h| h.eq_ignore_ascii_case(&host) || h.eq_ignore_ascii_case(&full))
    {
        return match path.as_slice() {
            [kind, id] => Some(Parsed::Own {
                kind: share_kind(kind)?,
                id: id.parse().ok()?,
            }),
            _ => None,
        };
    }
    match host.as_str() {
        "deezer.com" => {
            // /track/1, /ru/track/1, /us/album/2
            let at = path
                .iter()
                .position(|p| matches!(*p, "track" | "album" | "artist"))?;
            let kind = match path[at] {
                "track" => DeezerKind::Track,
                "album" => DeezerKind::Album,
                _ => DeezerKind::Artist,
            };
            let id = path.get(at + 1)?.parse::<i64>().ok().filter(|i| *i > 0)?;
            Some(Parsed::Deezer { kind, id })
        }
        "link.deezer.com" | "deezer.page.link" => Some(Parsed::DeezerShort(url.clone())),
        "open.spotify.com" | "play.spotify.com" => {
            let rest: Vec<&str> = path
                .iter()
                .copied()
                .skip_while(|p| p.starts_with("intl-") || *p == "embed")
                .collect();
            let kind = match rest.as_slice() {
                ["track", _, ..] => ShareKind::Track,
                ["album", _, ..] => ShareKind::Album,
                ["artist", _, ..] => ShareKind::Artist,
                _ => return None,
            };
            Some(Parsed::Spotify {
                kind: Some(kind),
                url: https(url),
            })
        }
        "spotify.link" => Some(Parsed::Spotify {
            kind: None,
            url: https(url),
        }),
        "music.apple.com" | "itunes.apple.com" => {
            let has_track = url.query_pairs().any(|(k, _)| k == "i");
            let kind = if path.contains(&"song") || (path.contains(&"album") && has_track) {
                ShareKind::Track
            } else if path.contains(&"album") {
                ShareKind::Album
            } else if path.contains(&"artist") {
                ShareKind::Artist
            } else {
                return None;
            };
            Some(Parsed::Apple {
                kind,
                url: https(url),
            })
        }
        "youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be" => {
            youtube_watch(url).map(|url| Parsed::Youtube { url })
        }
        "soundcloud.com" | "m.soundcloud.com" | "on.soundcloud.com" => {
            (!path.is_empty()).then(|| Parsed::Soundcloud { url: https(url) })
        }
        h if h.ends_with(".bandcamp.com") => {
            let kind = match path.first() {
                Some(&"track") => ShareKind::Track,
                Some(&"album") => ShareKind::Album,
                _ => return None,
            };
            Some(Parsed::Bandcamp {
                kind,
                url: https(url),
            })
        }
        _ => None,
    }
}

fn https(url: &Url) -> Url {
    let mut u = url.clone();
    let _ = u.set_scheme("https");
    u.set_fragment(None);
    u
}

/// Любая ссылка YouTube — к виду `https://www.youtube.com/watch?v=…`: его понимает oEmbed.
fn youtube_watch(url: &Url) -> Option<Url> {
    let host = url.host_str()?;
    let path = segments(url);
    let id = if host.ends_with("youtu.be") {
        path.first().map(|s| s.to_string())
    } else {
        match path.as_slice() {
            ["watch"] => url
                .query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.into_owned()),
            ["shorts" | "embed" | "live", id, ..] => Some(id.to_string()),
            _ => None,
        }
    }?;
    let ok = !id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| Url::parse(&format!("https://www.youtube.com/watch?v={id}")).ok())?
}

/// Хосты, куда сервер может сходить при разборе ссылки (и куда разрешены редиректы).
pub fn allowed_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    matches!(
        host.as_str(),
        "open.spotify.com"
            | "spotify.link"
            | "music.apple.com"
            | "itunes.apple.com"
            | "www.youtube.com"
            | "soundcloud.com"
            | "on.soundcloud.com"
            | "m.soundcloud.com"
            | "link.deezer.com"
            | "deezer.page.link"
            | "www.deezer.com"
            | "deezer.com"
    ) || host.ends_with(".bandcamp.com")
}

/// Что искать: название и исполнитель, как их удалось достать со страницы.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Wanted {
    pub title: String,
    pub artist: Option<String>,
}

impl Wanted {
    pub fn query(&self) -> String {
        match &self.artist {
            Some(a) if !a.is_empty() => format!("{a} {}", self.title),
            _ => self.title.clone(),
        }
    }
}

// ---- страницы и oEmbed ----

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("сервис не ответил: {0}")]
    Network(#[from] reqwest::Error),
    #[error("сервис ответил {0}")]
    Status(u16),
    #[error("ссылка ведёт не туда")]
    Host,
}

/// HTTP для разбора ссылок. `stub` — в тестах: все запросы уходят на заглушку с тем же путём.
pub struct Links {
    http: reqwest::Client,
    no_redirect: reqwest::Client,
    stub: Option<String>,
}

impl Links {
    pub fn new() -> anyhow::Result<Self> {
        Self::build(None)
    }

    /// Все внешние запросы — на `origin` (например `http://127.0.0.1:1234`), путь и query — как есть.
    pub fn stubbed(origin: &str) -> anyhow::Result<Self> {
        Self::build(Some(origin.trim_end_matches('/').to_string()))
    }

    fn build(stub: Option<String>) -> anyhow::Result<Self> {
        let testing = stub.is_some();
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            let ok = attempt.previous().len() < 5
                && (testing
                    || (attempt.url().scheme() == "https"
                        && attempt.url().host_str().is_some_and(allowed_host)));
            if ok { attempt.follow() } else { attempt.stop() }
        });
        // Страницы отдают теги Open Graph и без браузера; по этому заголовку видно, кто пришёл
        let agent = concat!(
            "Mozilla/5.0 (compatible; DalyWave/",
            env!("CARGO_PKG_VERSION"),
            "; +https://wave.daly.rest)"
        );
        let http = reqwest::Client::builder()
            .user_agent(agent)
            .timeout(TIMEOUT)
            .redirect(policy)
            .build()?;
        let no_redirect = reqwest::Client::builder()
            .user_agent(agent)
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            no_redirect,
            stub,
        })
    }

    fn target(&self, url: &Url) -> Result<String, FetchError> {
        match &self.stub {
            Some(origin) => {
                let mut t = format!("{origin}{}", url.path());
                if let Some(q) = url.query() {
                    t.push('?');
                    t.push_str(q);
                }
                Ok(t)
            }
            None if url.scheme() == "https" && url.host_str().is_some_and(allowed_host) => {
                Ok(url.to_string())
            }
            None => Err(FetchError::Host),
        }
    }

    /// Страница (не больше 1 МБ) и адрес, на котором она оказалась после редиректов.
    pub async fn page(&self, url: &Url) -> Result<(Url, String), FetchError> {
        let mut res = self
            .http
            .get(self.target(url)?)
            .header(reqwest::header::ACCEPT_LANGUAGE, "en")
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(FetchError::Status(res.status().as_u16()));
        }
        // В тестах итоговый адрес — заглушка: для разбора нужен исходный
        let landed = match self.stub {
            Some(_) => url.clone(),
            None => res.url().clone(),
        };
        let mut body = Vec::new();
        while let Some(chunk) = res.chunk().await? {
            body.extend_from_slice(&chunk);
            if body.len() >= PAGE_LIMIT {
                body.truncate(PAGE_LIMIT);
                break;
            }
        }
        Ok((landed, String::from_utf8_lossy(&body).into_owned()))
    }

    /// Куда ведёт короткая ссылка: один запрос без перехода, адрес из `Location`.
    pub async fn redirect(&self, url: &Url) -> Result<Url, FetchError> {
        let res = self.no_redirect.get(self.target(url)?).send().await?;
        if !res.status().is_redirection() {
            return Err(FetchError::Status(res.status().as_u16()));
        }
        res.headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| url.join(v).ok())
            .ok_or(FetchError::Host)
    }

    /// oEmbed YouTube или SoundCloud: название и автор.
    pub async fn oembed(&self, endpoint: &str, page: &Url) -> Result<OEmbed, FetchError> {
        let mut api = Url::parse(endpoint).map_err(|_| FetchError::Host)?;
        api.query_pairs_mut()
            .append_pair("format", "json")
            .append_pair("url", page.as_str());
        let res = self.http.get(self.target(&api)?).send().await?;
        if !res.status().is_success() {
            return Err(FetchError::Status(res.status().as_u16()));
        }
        Ok(res.json().await?)
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct OEmbed {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author_name: String,
}

// ---- теги страницы ----

/// Значение `<meta property|name="{key}" content="…">`.
pub fn meta(html: &str, key: &str) -> Option<String> {
    let mut rest = html;
    while let Some(at) = find_ci(rest, "<meta") {
        rest = &rest[at + 5..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        let name = attr(tag, "property").or_else(|| attr(tag, "name"));
        if name.is_some_and(|n| n.eq_ignore_ascii_case(key))
            && let Some(content) = attr(tag, "content")
        {
            let v = decode_entities(&content).trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
        rest = &rest[end..];
    }
    None
}

fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Атрибут тега: `name="…"`, `name='…'`.
fn attr(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    loop {
        let at = find_ci(rest, name)?;
        let before_ok = at == 0
            || rest[..at]
                .chars()
                .last()
                .is_some_and(|c| c.is_whitespace() || c == '"' || c == '\'');
        let after = rest[at + name.len()..].trim_start();
        rest = &rest[at + name.len()..];
        if !before_ok {
            continue;
        }
        let Some(after) = after.strip_prefix('=') else {
            continue;
        };
        let after = after.trim_start();
        let quote = after.chars().next()?;
        if quote == '"' || quote == '\'' {
            let body = &after[1..];
            let end = body.find(quote)?;
            return Some(body[..end].to_string());
        }
        let end = after
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(after.len());
        return Some(after[..end].to_string());
    }
}

pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Невидимые знаки направления текста, которые Apple ставит в начало заголовков.
fn strip_marks(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\u{200e}' | '\u{200f}' | '\u{200b}' | '\u{feff}'))
        .collect::<String>()
        .trim()
        .to_string()
}

fn split_last<'a>(s: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    let at = s.rfind(sep)?;
    let (a, b) = (s[..at].trim(), s[at + sep.len()..].trim());
    (!a.is_empty() && !b.is_empty()).then_some((a, b))
}

/// Spotify: название — `og:title`, исполнитель — `music:musician_description` или первый осмысленный
/// кусок `og:description` («Artist · Album · Song · 2020», «Listen to X on Spotify. Song · Artist · 2020.»).
pub fn spotify(html: &str, kind: ShareKind) -> Option<Wanted> {
    let title = strip_marks(&meta(html, "og:title")?);
    if kind == ShareKind::Artist {
        return Some(Wanted {
            title,
            artist: None,
        });
    }
    let artist = meta(html, "music:musician_description").or_else(|| {
        let desc = meta(html, "og:description")?;
        let desc = match desc.find(" on Spotify. ") {
            Some(at) => desc[at + " on Spotify. ".len()..].to_string(),
            None => desc,
        };
        desc.split(" · ")
            .map(|s| s.trim().trim_end_matches('.').trim())
            .find(|s| {
                let low = s.to_lowercase();
                !s.is_empty()
                    && !s.chars().all(|c| c.is_ascii_digit())
                    && !low.ends_with(" songs")
                    && !low.ends_with(" song")
                    && !matches!(
                        low.as_str(),
                        "song"
                            | "album"
                            | "single"
                            | "ep"
                            | "compilation"
                            | "песня"
                            | "альбом"
                            | "сингл"
                    )
                    && normalize(s) != normalize(&title)
            })
            .map(str::to_string)
    });
    Some(Wanted { title, artist })
}

/// Apple Music: «Song by Artist on Apple Music»; другие языки — заголовок как есть.
pub fn apple(html: &str) -> Option<Wanted> {
    let raw = strip_marks(&meta(html, "og:title")?);
    let mut t = raw.as_str();
    for suffix in [
        " on Apple Music",
        " — Apple Music",
        " - Apple Music",
        " в Apple Music",
    ] {
        if let Some(s) = t.strip_suffix(suffix) {
            t = s.trim();
        }
    }
    Some(match split_last(t, " by ") {
        Some((title, artist)) => Wanted {
            title: title.to_string(),
            artist: Some(artist.to_string()),
        },
        None => Wanted {
            title: t.to_string(),
            artist: None,
        },
    })
}

/// Bandcamp: «Title, by Artist».
pub fn bandcamp(html: &str) -> Option<Wanted> {
    let raw = strip_marks(&meta(html, "og:title")?);
    Some(match split_last(&raw, ", by ") {
        Some((title, artist)) => Wanted {
            title: title.to_string(),
            artist: Some(artist.to_string()),
        },
        None => Wanted {
            title: raw,
            artist: None,
        },
    })
}

/// SoundCloud oEmbed: «Title by Artist».
pub fn soundcloud(o: &OEmbed) -> Option<Wanted> {
    let title = o.title.trim();
    if title.is_empty() {
        return None;
    }
    Some(match split_last(title, " by ") {
        Some((t, a)) => Wanted {
            title: clean_video_title(t),
            artist: Some(a.to_string()),
        },
        None => Wanted {
            title: clean_video_title(title),
            artist: (!o.author_name.is_empty()).then(|| o.author_name.clone()),
        },
    })
}

/// YouTube oEmbed: «Artist - Title (Official Video)» или название + канал «Artist - Topic».
pub fn youtube(o: &OEmbed) -> Option<Wanted> {
    let title = clean_video_title(&o.title);
    if title.is_empty() {
        return None;
    }
    for sep in [" - ", " – ", " — "] {
        if let Some(at) = title.find(sep) {
            let (artist, rest) = (title[..at].trim(), title[at + sep.len()..].trim());
            if !artist.is_empty() && !rest.is_empty() {
                return Some(Wanted {
                    title: rest.to_string(),
                    artist: Some(artist.to_string()),
                });
            }
        }
    }
    let channel = o.author_name.trim();
    let channel = channel.strip_suffix(" - Topic").unwrap_or(channel);
    let channel = channel.strip_suffix("VEVO").unwrap_or(channel).trim();
    Some(Wanted {
        title,
        artist: (!channel.is_empty()).then(|| channel.to_string()),
    })
}

/// Без «(Official Video)», «[Lyrics]», «| Премьера клипа» и т. п. — только то, что есть в каталоге.
pub fn clean_video_title(title: &str) -> String {
    const NOISE: &[&str] = &[
        "official",
        "video",
        "audio",
        "lyric",
        "visuali",
        "music video",
        "mv",
        "m/v",
        "hd",
        "hq",
        "4k",
        "clip",
        "клип",
        "премьера",
        "текст",
        "full album",
    ];
    let t = strip_marks(title);
    let t = match t.find(" | ") {
        Some(at) => t[..at].to_string(),
        None => t,
    };
    let mut out = String::new();
    let mut rest = t.as_str();
    while let Some(open) = rest.find(['(', '[']) {
        let close_ch = if rest[open..].starts_with('(') {
            ')'
        } else {
            ']'
        };
        let Some(len) = rest[open..].find(close_ch) else {
            break;
        };
        let inner = rest[open + 1..open + len].to_lowercase();
        out.push_str(&rest[..open]);
        if !NOISE.iter().any(|n| {
            inner == *n
                || inner.starts_with(&format!("{n} "))
                || inner.contains(&format!(" {n}"))
                || (n.len() > 3 && inner.contains(n))
        }) {
            out.push_str(&rest[open..open + len + 1]);
        }
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- сравнение с найденным ----

/// Без регистра, диакритики и знаков: «Beyoncé — Halo!» и «beyonce halo» совпадают.
pub fn normalize(s: &str) -> String {
    let folded: String = s
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Название без версий в скобках и «feat.»: «Devil (Remastered) feat. X» → «devil».
fn base_title(s: &str) -> String {
    let mut t = s.to_string();
    for open in ['(', '['] {
        if let Some(at) = t.find(open) {
            t.truncate(at);
        }
    }
    let low = t.to_lowercase();
    for cut in [" feat.", " feat ", " ft.", " ft ", " - "] {
        if let Some(at) = low.find(cut) {
            t.truncate(at);
            break;
        }
    }
    normalize(&t)
}

fn close(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && (a == b || a.contains(b) || b.contains(a))
}

pub fn title_matches(wanted: &str, found: &str) -> bool {
    close(&normalize(wanted), &normalize(found)) || close(&base_title(wanted), &base_title(found))
}

pub fn artist_matches(wanted: &str, found: &[&str]) -> bool {
    let w = normalize(wanted);
    found.iter().any(|a| close(&w, &normalize(a)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Option<Parsed> {
        parse(&Url::parse(s).unwrap(), &["wave.daly.rest", "localhost"])
    }

    #[test]
    fn urls_are_extracted_from_shared_text() {
        assert_eq!(
            extract_url("Послушай «Devil» https://www.deezer.com/track/1?utm=x, круто")
                .unwrap()
                .as_str(),
            "https://www.deezer.com/track/1?utm=x"
        );
        assert!(extract_url("просто текст").is_none());
        assert_eq!(
            extract_url("(https://youtu.be/abc).").unwrap().as_str(),
            "https://youtu.be/abc"
        );
    }

    #[test]
    fn links_are_recognised() {
        let id = Uuid::now_v7();
        assert_eq!(
            p(&format!("https://wave.daly.rest/album/{id}")),
            Some(Parsed::Own {
                kind: ShareKind::Album,
                id
            })
        );
        assert_eq!(p("https://wave.daly.rest/library"), None);
        assert_eq!(
            p("https://www.deezer.com/ru/track/3661045852"),
            Some(Parsed::Deezer {
                kind: DeezerKind::Track,
                id: 3661045852
            })
        );
        assert_eq!(
            p("https://deezer.com/album/12"),
            Some(Parsed::Deezer {
                kind: DeezerKind::Album,
                id: 12
            })
        );
        assert!(matches!(
            p("https://link.deezer.com/s/30abc"),
            Some(Parsed::DeezerShort(_))
        ));
        assert!(matches!(
            p("https://open.spotify.com/intl-de/track/4uLU6hMCjMI75M1A2tKUQC?si=1"),
            Some(Parsed::Spotify {
                kind: Some(ShareKind::Track),
                ..
            })
        ));
        assert!(matches!(
            p("https://open.spotify.com/album/1"),
            Some(Parsed::Spotify {
                kind: Some(ShareKind::Album),
                ..
            })
        ));
        assert_eq!(p("https://open.spotify.com/playlist/1"), None);
        assert!(matches!(
            p("https://music.apple.com/us/album/devil/1700000000?i=1700000001"),
            Some(Parsed::Apple {
                kind: ShareKind::Track,
                ..
            })
        ));
        assert!(matches!(
            p("https://music.apple.com/ru/album/devil/1700000000"),
            Some(Parsed::Apple {
                kind: ShareKind::Album,
                ..
            })
        ));
        for yt in [
            "https://youtu.be/dQw4w9WgXcQ?si=x",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=1",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/shorts/dQw4w9WgXcQ",
        ] {
            assert_eq!(
                p(yt),
                Some(Parsed::Youtube {
                    url: Url::parse("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap()
                }),
                "{yt}"
            );
        }
        assert!(matches!(
            p("https://soundcloud.com/artist/track"),
            Some(Parsed::Soundcloud { .. })
        ));
        assert!(matches!(
            p("https://lowborn.bandcamp.com/track/devil"),
            Some(Parsed::Bandcamp {
                kind: ShareKind::Track,
                ..
            })
        ));
        assert_eq!(p("https://example.com/track/1"), None);
        assert_eq!(
            p("https://evil.com/?u=https://open.spotify.com/track/1"),
            None
        );
    }

    #[test]
    fn only_known_hosts_are_fetched() {
        assert!(allowed_host("open.spotify.com"));
        assert!(allowed_host("lowborn.bandcamp.com"));
        assert!(!allowed_host("bandcamp.com.evil.net"));
        assert!(!allowed_host("169.254.169.254"));
        let links = Links::new().unwrap();
        assert!(
            links
                .target(&Url::parse("http://open.spotify.com/track/1").unwrap())
                .is_err()
        );
        assert!(
            links
                .target(&Url::parse("https://localhost/track/1").unwrap())
                .is_err()
        );
    }

    #[test]
    fn og_tags_are_read() {
        let html = r#"<html><head><META property="og:title" content="Light Me Up &amp; Co"/>
            <meta content='Lowborn · Light Me Up · Song · 2024' property='og:description'>
            <meta name="twitter:title" content="x"></head>"#;
        assert_eq!(meta(html, "og:title").unwrap(), "Light Me Up & Co");
        assert_eq!(
            meta(html, "og:description").unwrap(),
            "Lowborn · Light Me Up · Song · 2024"
        );
        assert!(meta(html, "og:image").is_none());
    }

    #[test]
    fn spotify_descriptions() {
        let page = |title: &str, desc: &str| {
            format!(
                r#"<meta property="og:title" content="{title}"><meta property="og:description" content="{desc}">"#
            )
        };
        let w = spotify(
            &page("Devil", "Lowborn · Devil · Song · 2024"),
            ShareKind::Track,
        )
        .unwrap();
        assert_eq!(w.artist.as_deref(), Some("Lowborn"));
        assert_eq!(w.title, "Devil");
        let w = spotify(
            &page(
                "Devil",
                "Listen to Devil on Spotify. Song · Lowborn · 2024.",
            ),
            ShareKind::Track,
        )
        .unwrap();
        assert_eq!(w.artist.as_deref(), Some("Lowborn"));
        let w = spotify(
            &page("Modern Sadness", "Lowborn · EP · 2024 · 4 songs"),
            ShareKind::Album,
        )
        .unwrap();
        assert_eq!(w.artist.as_deref(), Some("Lowborn"));
        let w = spotify(
            &page("Lowborn", "Artist · 10K monthly listeners."),
            ShareKind::Artist,
        )
        .unwrap();
        assert_eq!(w.artist, None);
    }

    #[test]
    fn titles_are_cleaned() {
        assert_eq!(
            youtube(&OEmbed {
                title: "Lowborn - Devil (Official Music Video) [4K]".into(),
                author_name: "LowbornVEVO".into()
            })
            .unwrap(),
            Wanted {
                title: "Devil".into(),
                artist: Some("Lowborn".into())
            }
        );
        assert_eq!(
            youtube(&OEmbed {
                title: "Devil".into(),
                author_name: "Lowborn - Topic".into()
            })
            .unwrap(),
            Wanted {
                title: "Devil".into(),
                artist: Some("Lowborn".into())
            }
        );
        assert_eq!(
            clean_video_title("Song (feat. X) (Lyrics)"),
            "Song (feat. X)"
        );
        assert_eq!(clean_video_title("Песня | Премьера клипа 2024"), "Песня");
        assert_eq!(
            clean_video_title("Song (Live at Wembley)"),
            "Song (Live at Wembley)"
        );
        assert_eq!(
            apple(
                r#"<meta property="og:title" content="&#x200e;Devil by Lowborn on Apple Music">"#
            )
            .unwrap(),
            Wanted {
                title: "Devil".into(),
                artist: Some("Lowborn".into())
            }
        );
        assert_eq!(
            bandcamp(r#"<meta property="og:title" content="Devil, by Lowborn">"#).unwrap(),
            Wanted {
                title: "Devil".into(),
                artist: Some("Lowborn".into())
            }
        );
        assert_eq!(
            soundcloud(&OEmbed {
                title: "Devil by Lowborn".into(),
                author_name: "Lowborn".into()
            })
            .unwrap()
            .artist
            .as_deref(),
            Some("Lowborn")
        );
    }

    #[test]
    fn matching_is_forgiving_but_not_blind() {
        assert!(title_matches("Devil", "Devil (Remastered 2011)"));
        assert!(title_matches("Beyoncé — Halo!", "beyonce halo"));
        assert!(title_matches("Light Me Up feat. vista", "Light Me Up"));
        assert!(!title_matches("Devil", "Doctor Doctor"));
        assert!(artist_matches("Lowborn", &["Lowborn", "vista"]));
        assert!(!artist_matches("Anberlin", &["Lowborn"]));
    }
}
