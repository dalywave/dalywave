//! Открытый чарт Apple Music (`/api/v2/{страна}/music/most-played/{n}/songs.json`): самые
//! прослушиваемые песни страны. Нужен для России — у Deezer её чарта нет. Ключ не нужен; в ответе
//! только название и артисты, трек каталога находится поиском Deezer (см. [`crate::charts`]).

use std::time::Duration;

use serde::Deserialize;

use super::{InfoError, Paced};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    /// id песни в Apple Music: по нему запоминается найденный трек.
    pub id: String,
    pub name: String,
    /// «Miyagi & Эндшпиль», «By Индия, Xcho & МОТ».
    pub artist_name: String,
}

#[derive(Debug, Deserialize)]
struct Feed {
    #[serde(default)]
    results: Vec<Song>,
}

#[derive(Debug, Deserialize)]
struct Response {
    feed: Feed,
}

pub struct AppleCharts {
    base: String,
    paced: Paced,
}

impl AppleCharts {
    pub fn new(base: &str) -> anyhow::Result<Self> {
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            paced: Paced::new(Duration::from_millis(500))?,
        })
    }

    /// Самые прослушиваемые песни страны (`ru`), до 100, по местам.
    pub async fn most_played(&self, country: &str, limit: u32) -> Result<Vec<Song>, InfoError> {
        let country: String = country
            .chars()
            .filter(char::is_ascii_alphabetic)
            .take(2)
            .collect();
        let url = url::Url::parse(&format!(
            "{}/api/v2/{country}/music/most-played/{}/songs.json",
            self.base,
            limit.clamp(1, 100)
        ))
        .map_err(|e| InfoError::Network(e.to_string()))?;
        Ok(self
            .paced
            .get::<Response>(url)
            .await?
            .map(|r| r.feed.results)
            .unwrap_or_default())
    }
}

/// Артисты строки чарта по отдельности: «By Индия, Xcho & МОТ» → три имени.
pub fn split_artists(name: &str) -> Vec<String> {
    let mut parts = vec![name.to_string()];
    // « и » не делит: «Король и Шут» — одна группа
    for sep in [", ", " & ", " x ", " X ", " feat. ", " ft. "] {
        parts = parts
            .into_iter()
            .flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    parts
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Название для поиска: без «(feat. …)» и пометок в скобках.
pub fn search_title(name: &str) -> String {
    let mut depth = 0u32;
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.is_empty() {
        name.trim().to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_is_parsed() {
        let r: Response = serde_json::from_str(
            r#"{"feed": {"title": "Топ-песни", "results": [
                {"artistName": "МАЯК X", "id": "6820477982", "name": "Украсть тебя", "kind": "songs"}
            ]}}"#,
        )
        .unwrap();
        assert_eq!(r.feed.results[0].artist_name, "МАЯК X");
    }

    #[test]
    fn artists_and_titles_for_search() {
        assert_eq!(
            split_artists("By Индия, Xcho & МОТ"),
            vec!["By Индия", "Xcho", "МОТ"]
        );
        assert_eq!(split_artists("Zivert"), vec!["Zivert"]);
        assert_eq!(search_title("I Got Love (feat. Рем Дигга)"), "I Got Love");
        assert_eq!(
            search_title("Beni Al (Ta Ki Seni Görene Kadar) [Beni Al Afro House Remix]"),
            "Beni Al"
        );
        assert_eq!(search_title("(Intro)"), "(Intro)");
    }
}
