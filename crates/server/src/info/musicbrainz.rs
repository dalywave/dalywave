//! Клиент MusicBrainz (https://musicbrainz.org/doc/MusicBrainz_API): запись по ISRC, её отношения
//! (продюсеры, звукорежиссёры, произведение с авторами) и ссылки артиста. Ключ не нужен; просят не
//! чаще одного запроса в секунду и User-Agent с контактом, а занятый сервер отвечает 503.

use std::time::Duration;

use serde::Deserialize;
use uuid::Uuid;

use super::{InfoError, Paced};

/// MusicBrainz просит не чаще раза в секунду; с запасом.
const GAP: Duration = Duration::from_millis(1100);

#[derive(Debug, Clone, Deserialize)]
pub struct ArtistRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credit {
    #[serde(default)]
    pub name: String,
    pub artist: ArtistRef,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UrlRef {
    pub resource: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Work {
    pub id: Uuid,
    #[serde(default)]
    pub title: String,
    /// Авторы произведения — при `work-level-rels`.
    #[serde(default)]
    pub relations: Vec<Relation>,
}

/// Отношение: `type` — «producer», «mix», «performance», «composer»…; цель — артист, произведение или ссылка.
#[derive(Debug, Clone, Deserialize)]
pub struct Relation {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "target-type", default)]
    pub target_type: String,
    /// «assistant», «additional», «co», «executive»…
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(default)]
    pub work: Option<Work>,
    #[serde(default)]
    pub url: Option<UrlRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Recording {
    pub id: Uuid,
    #[serde(default)]
    pub title: String,
    /// Миллисекунды.
    #[serde(default)]
    pub length: Option<i64>,
    /// В поиске: 0–100.
    #[serde(default)]
    pub score: Option<i64>,
    #[serde(rename = "artist-credit", default)]
    pub artist_credit: Vec<Credit>,
    #[serde(default)]
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub relations: Vec<Relation>,
}

impl Artist {
    /// Страницы артиста в Deezer из ссылок MusicBrainz (их бывает несколько — по порядку).
    pub fn deezer_ids(&self) -> Vec<i64> {
        self.relations
            .iter()
            .filter_map(|r| r.url.as_ref())
            .filter_map(|u| deezer_artist_id(&u.resource))
            .collect()
    }
}

/// `https://www.deezer.com/artist/123`, `https://www.deezer.com/en/artist/123` → 123.
pub fn deezer_artist_id(resource: &str) -> Option<i64> {
    let url = url::Url::parse(resource).ok()?;
    let host = url.host_str()?;
    if host != "deezer.com" && !host.ends_with(".deezer.com") {
        return None;
    }
    let mut parts = url.path_segments()?.filter(|p| !p.is_empty());
    let mut prev = parts.next()?;
    for part in parts {
        if prev == "artist" {
            return part.parse().ok().filter(|id| *id > 0);
        }
        prev = part;
    }
    None
}

#[derive(Debug, Deserialize)]
struct Search {
    #[serde(default)]
    recordings: Vec<Recording>,
}

pub struct MusicBrainz {
    base: String,
    paced: Paced,
}

impl MusicBrainz {
    pub fn new(base: &str) -> anyhow::Result<Self> {
        Self::with_gap(base, GAP, Duration::from_millis(1500))
    }

    pub fn with_gap(base: &str, gap: Duration, backoff: Duration) -> anyhow::Result<Self> {
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            paced: Paced::new(gap)?.with_backoff(backoff),
        })
    }

    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<url::Url, InfoError> {
        let mut url = url::Url::parse(&format!("{}{path}", self.base))
            .map_err(|e| InfoError::Network(e.to_string()))?;
        url.query_pairs_mut()
            .extend_pairs(query.iter().copied())
            .append_pair("fmt", "json");
        Ok(url)
    }

    /// Записи с этим ISRC (поиск: у прямого `/isrc/…` MusicBrainz чаще отвечает «занят»).
    pub async fn recordings_by_isrc(&self, isrc: &str) -> Result<Vec<Recording>, InfoError> {
        let code: String = isrc
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_uppercase();
        if code.is_empty() {
            return Ok(Vec::new());
        }
        let q = format!("isrc:{code}");
        let url = self.url("/recording", &[("query", &q), ("limit", "10")])?;
        Ok(self
            .paced
            .get::<Search>(url)
            .await?
            .map(|s| s.recordings)
            .unwrap_or_default())
    }

    /// Запись с отношениями: кто продюсировал, сводил, и произведение с авторами.
    pub async fn recording(&self, id: Uuid) -> Result<Option<Recording>, InfoError> {
        let url = self.url(
            &format!("/recording/{id}"),
            &[(
                "inc",
                "artist-credits+artist-rels+work-rels+work-level-rels",
            )],
        )?;
        self.paced.get(url).await
    }

    /// Артист со ссылками: Deezer, Wikidata…
    pub async fn artist(&self, id: Uuid) -> Result<Option<Artist>, InfoError> {
        let url = self.url(&format!("/artist/{id}"), &[("inc", "url-rels")])?;
        self.paced.get(url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deezer_links_of_an_artist() {
        assert_eq!(
            deezer_artist_id("https://www.deezer.com/artist/10797684"),
            Some(10797684)
        );
        assert_eq!(
            deezer_artist_id("https://www.deezer.com/en/artist/7889120"),
            Some(7889120)
        );
        assert_eq!(deezer_artist_id("https://www.deezer.com/album/1"), None);
        assert_eq!(deezer_artist_id("https://evil.com/artist/1"), None);
        assert_eq!(deezer_artist_id("https://notdeezer.com/artist/1"), None);
        assert_eq!(deezer_artist_id("not a url"), None);
    }
}
