//! Wikidata и Википедия: элемент по свойству (`haswbstatement:P2722=<id Deezer>`), его утверждения и
//! ссылки на статьи, подписи элементов по-русски и начало статьи (REST `page/summary`).
//! Ключ не нужен; Wikimedia просит User-Agent с контактом и не частить.

use std::{collections::HashMap, time::Duration};

use serde::Deserialize;
use serde_json::Value;

use super::{InfoError, Paced};

const GAP: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Snak {
    #[serde(default)]
    pub datavalue: Option<DataValue>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DataValue {
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Claim {
    #[serde(default)]
    pub mainsnak: Snak,
    /// `preferred`, `normal`, `deprecated`.
    #[serde(default)]
    pub rank: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Sitelink {
    pub title: String,
}

/// Элемент Wikidata: утверждения по свойствам и ссылки на статьи.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Entity {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub claims: HashMap<String, Vec<Claim>>,
    #[serde(default)]
    pub sitelinks: HashMap<String, Sitelink>,
}

impl Entity {
    /// Значения свойства: сначала предпочтительные, без устаревших.
    fn values(&self, prop: &str) -> Vec<&Value> {
        let Some(claims) = self.claims.get(prop) else {
            return Vec::new();
        };
        let mut good: Vec<&Claim> = claims.iter().filter(|c| c.rank != "deprecated").collect();
        good.sort_by_key(|c| u8::from(c.rank != "preferred"));
        good.into_iter()
            .filter_map(|c| c.mainsnak.datavalue.as_ref().map(|d| &d.value))
            .collect()
    }

    /// Элементы, на которые указывает свойство (`Q…`).
    pub fn items(&self, prop: &str) -> Vec<String> {
        self.values(prop)
            .into_iter()
            .filter_map(|v| v.get("id").and_then(Value::as_str).map(str::to_string))
            .collect()
    }

    /// Год из первого значения-даты (`+2015-00-00T00:00:00Z`).
    pub fn year(&self, prop: &str) -> Option<i32> {
        self.values(prop).into_iter().find_map(|v| {
            let time = v.get("time")?.as_str()?;
            let (sign, rest) = time.split_at(1);
            let year: i32 = rest.split('-').next()?.parse().ok()?;
            (sign == "+" && year > 0).then_some(year)
        })
    }

    /// Это человек (`P31 = Q5`), а не группа.
    pub fn is_human(&self) -> bool {
        self.items("P31").iter().any(|q| q == "Q5")
    }
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    title: String,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    #[serde(default)]
    search: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    query: Option<SearchQuery>,
}

#[derive(Debug, Deserialize)]
struct Label {
    value: String,
}

#[derive(Debug, Deserialize)]
struct Labelled {
    #[serde(default)]
    labels: HashMap<String, Label>,
}

#[derive(Debug, Deserialize)]
struct Entities<T> {
    #[serde(default = "HashMap::new")]
    entities: HashMap<String, T>,
}

/// Начало статьи Википедии.
#[derive(Debug, Clone, Deserialize)]
pub struct Summary {
    /// `standard`, `disambiguation`…
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub extract: String,
    #[serde(default)]
    pub content_urls: Option<ContentUrls>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentUrls {
    pub desktop: PageUrl,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageUrl {
    pub page: String,
}

pub struct Wiki {
    wikidata: String,
    /// С `{lang}` вместо языка
    wikipedia: String,
    paced: Paced,
}

impl Wiki {
    pub fn new(wikidata: &str, wikipedia: &str) -> anyhow::Result<Self> {
        Ok(Self {
            wikidata: wikidata.trim_end_matches('/').to_string(),
            wikipedia: wikipedia.trim_end_matches('/').to_string(),
            paced: Paced::new(GAP)?,
        })
    }

    fn api(&self, query: &[(&str, &str)]) -> Result<url::Url, InfoError> {
        let mut url = url::Url::parse(&format!("{}/w/api.php", self.wikidata))
            .map_err(|e| InfoError::Network(e.to_string()))?;
        url.query_pairs_mut()
            .extend_pairs(query.iter().copied())
            .append_pair("format", "json");
        Ok(url)
    }

    /// Элемент, у которого свойство `prop` равно `value` (`P2722` — id артиста Deezer, `P434` — id
    /// MusicBrainz). Несколько элементов с одним id — неоднозначно: `None`.
    pub async fn find_by_claim(
        &self,
        prop: &str,
        value: &str,
    ) -> Result<Option<String>, InfoError> {
        let clean: String = value
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        if clean.is_empty() {
            return Ok(None);
        }
        let q = format!("haswbstatement:{prop}={clean}");
        let url = self.api(&[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", &q),
            ("srlimit", "2"),
            ("srprop", ""),
        ])?;
        let hits = self
            .paced
            .get::<SearchResponse>(url)
            .await?
            .and_then(|r| r.query)
            .map(|q| q.search)
            .unwrap_or_default();
        Ok(match hits.as_slice() {
            [one] if one.title.starts_with('Q') => Some(one.title.clone()),
            _ => None,
        })
    }

    /// Утверждения и ссылки на статьи (русскую и английскую).
    pub async fn entity(&self, id: &str) -> Result<Option<Entity>, InfoError> {
        let url = self.api(&[
            ("action", "wbgetentities"),
            ("ids", id),
            ("props", "sitelinks|claims"),
            ("sitefilter", "ruwiki|enwiki"),
        ])?;
        let found = self.paced.get::<Entities<Entity>>(url).await?;
        Ok(found
            .and_then(|mut e| e.entities.remove(id))
            .filter(|e| !e.id.is_empty()))
    }

    /// Подписи элементов: по-русски, иначе по-английски.
    pub async fn labels(&self, ids: &[String]) -> Result<HashMap<String, String>, InfoError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let joined = ids.iter().take(50).cloned().collect::<Vec<_>>().join("|");
        let url = self.api(&[
            ("action", "wbgetentities"),
            ("ids", &joined),
            ("props", "labels"),
            ("languages", "ru|en"),
        ])?;
        let found = self.paced.get::<Entities<Labelled>>(url).await?;
        Ok(found
            .map(|e| e.entities)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, e)| {
                let label = e.labels.get("ru").or_else(|| e.labels.get("en"))?;
                Some((id, label.value.clone()))
            })
            .collect())
    }

    /// Начало статьи. `lang` — `ru` или `en`.
    pub async fn summary(&self, lang: &str, title: &str) -> Result<Option<Summary>, InfoError> {
        let base = self.wikipedia.replace("{lang}", lang);
        let mut url = url::Url::parse(&format!("{base}/api/rest_v1/page/summary/"))
            .map_err(|e| InfoError::Network(e.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| InfoError::Network("bad wikipedia url".into()))?
            .pop_if_empty()
            .push(&title.replace(' ', "_"));
        self.paced.get(url).await
    }
}
