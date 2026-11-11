//! Ответы публичного API Deezer. Поля, которых может не быть, — `Option` или со значением по умолчанию:
//! один и тот же объект приходит урезанным в списках и полным в своём запросе.

use serde::{Deserialize, Deserializer};

/// Пустая строка у Deezer означает «нет значения».
fn non_empty<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let v = Option::<String>::deserialize(d)?;
    Ok(v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

#[derive(Debug, Clone, Deserialize)]
pub struct Page<T> {
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    #[serde(default)]
    pub total: Option<i64>,
    /// Адрес следующей страницы.
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    #[serde(default, deserialize_with = "non_empty")]
    pub picture_xl: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub picture_big: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub md5_image: Option<String>,
    #[serde(default)]
    pub nb_album: Option<i32>,
    #[serde(default)]
    pub nb_fan: Option<i32>,
    /// У участников трека и альбома: `Main` или `Featured`.
    #[serde(default)]
    pub role: Option<String>,
}

impl Artist {
    /// Есть ли полная карточка (так приходят артист, поиск артистов и похожие).
    pub fn has_card(&self) -> bool {
        self.nb_fan.is_some()
    }

    pub fn picture(&self) -> Option<String> {
        image_source(
            "artist",
            [&self.picture_xl, &self.picture_big],
            &self.md5_image,
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Genre {
    /// Нужен чартам и подборкам жанра; в жанрах альбома тоже есть.
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    pub id: i64,
    pub title: String,
    #[serde(default, deserialize_with = "non_empty")]
    pub upc: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub cover_xl: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub cover_big: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub md5_image: Option<String>,
    #[serde(default)]
    pub genres: Option<Page<Genre>>,
    #[serde(default, deserialize_with = "non_empty")]
    pub label: Option<String>,
    #[serde(default)]
    pub nb_tracks: Option<i32>,
    /// Секунды.
    #[serde(default)]
    pub duration: Option<i32>,
    #[serde(default)]
    pub fans: Option<i32>,
    #[serde(default, deserialize_with = "non_empty")]
    pub release_date: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub record_type: Option<String>,
    #[serde(default)]
    pub explicit_lyrics: Option<bool>,
    #[serde(default)]
    pub contributors: Vec<Artist>,
    /// Главный артист. В списке релизов артиста его нет.
    #[serde(default)]
    pub artist: Option<Artist>,
}

impl Album {
    pub fn cover(&self) -> Option<String> {
        image_source("cover", [&self.cover_xl, &self.cover_big], &self.md5_image)
    }

    pub fn release_date(&self) -> Option<chrono::NaiveDate> {
        self.release_date
            .as_deref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }

    pub fn genre_names(&self) -> Vec<String> {
        self.genres
            .as_ref()
            .map(|g| g.data.iter().map(|g| g.name.clone()).collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub id: i64,
    pub title: String,
    /// Название без версии.
    #[serde(default, deserialize_with = "non_empty")]
    pub title_short: Option<String>,
    #[serde(default, deserialize_with = "non_empty")]
    pub title_version: Option<String>,
    /// Есть в треке, в треках альбома и в поиске; в популярном артиста и в чартах — нет.
    #[serde(default, deserialize_with = "non_empty")]
    pub isrc: Option<String>,
    /// Секунды.
    pub duration: i32,
    #[serde(default)]
    pub track_position: Option<i16>,
    #[serde(default)]
    pub disk_number: Option<i16>,
    #[serde(default)]
    pub rank: Option<i32>,
    #[serde(default)]
    pub explicit_lyrics: Option<bool>,
    #[serde(default)]
    pub artist: Option<Artist>,
    #[serde(default)]
    pub album: Option<Album>,
    /// Все участники с ролями; есть в треке и в популярном артиста.
    #[serde(default)]
    pub contributors: Vec<Artist>,
    /// 30-секундное превью MP3. Ссылка подписана (`hdnea=exp=…`) и живёт недолго: её не хранят,
    /// а берут свежую перед анализом.
    #[serde(default, deserialize_with = "non_empty")]
    pub preview: Option<String>,
}

impl Track {
    /// Название без версии и версия отдельно: «ELA DANCA» и «(Slowed)».
    pub fn split_title(&self) -> (String, String) {
        let version = self.title_version.clone().unwrap_or_default();
        let title = match &self.title_short {
            Some(short) => short.clone(),
            None if !version.is_empty() => self
                .title
                .strip_suffix(&version)
                .map(|t| t.trim_end().to_string())
                .unwrap_or_else(|| self.title.clone()),
            None => self.title.clone(),
        };
        (title, version)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chart {
    #[serde(default = "empty_page")]
    pub tracks: Page<Track>,
    #[serde(default = "empty_page")]
    pub albums: Page<Album>,
    #[serde(default = "empty_page")]
    pub artists: Page<Artist>,
}

fn empty_page<T>() -> Page<T> {
    Page {
        data: Vec::new(),
        total: None,
        next: None,
    }
}

/// Ошибка API, которая приходит с HTTP 200: `{"error": {"type", "message", "code"}}`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub code: i64,
}

/// Адрес картинки 1000×1000. У пустой картинки Deezer отдаёт заглушку с пустым md5 (`/images/artist//…`).
fn image_source(kind: &str, urls: [&Option<String>; 2], md5: &Option<String>) -> Option<String> {
    let placeholder = format!("/images/{kind}//");
    let url = urls
        .into_iter()
        .flatten()
        .find(|u| u.starts_with("http") && !u.contains(&placeholder))
        .cloned();
    url.map(|u| u.replace("/500x500-", "/1000x1000-"))
        .or_else(|| {
            // У будущих релизов адресов ещё нет, а md5 уже есть: CDN отдаёт картинку по нему
            md5.as_deref()
                .filter(|m| m.len() == 32 && m.chars().all(|c| c.is_ascii_hexdigit()))
                .map(|m| {
                    format!(
                        "https://cdn-images.dzcdn.net/images/{kind}/{m}/1000x1000-000000-80-0-0.jpg"
                    )
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_split_into_name_and_version() {
        let mut t: Track = serde_json::from_str(
            r#"{"id": 1, "title": "ELA DANCA (Slowed)", "title_short": "ELA DANCA", "title_version": "(Slowed)", "duration": 120}"#,
        )
        .unwrap();
        assert_eq!(t.split_title(), ("ELA DANCA".into(), "(Slowed)".into()));
        t.title_short = None;
        assert_eq!(t.split_title(), ("ELA DANCA".into(), "(Slowed)".into()));
        let plain: Track = serde_json::from_str(
            r#"{"id": 2, "title": "Devil", "title_version": "", "duration": 1}"#,
        )
        .unwrap();
        assert_eq!(plain.split_title(), ("Devil".into(), String::new()));
        assert!(plain.isrc.is_none());
    }

    #[test]
    fn images_skip_placeholders() {
        let a: Artist = serde_json::from_str(
            r#"{"id": 1, "name": "x", "picture_xl": "https://e-cdns-images.dzcdn.net/images/artist//1000x1000-000000-80-0-0.jpg", "md5_image": ""}"#,
        )
        .unwrap();
        assert_eq!(a.picture(), None);
        let album: Album = serde_json::from_str(
            r#"{"id": 1, "title": "melancholy", "cover_xl": null, "cover": "", "md5_image": "041a7401ead0c50765000792dc75b14e"}"#,
        )
        .unwrap();
        assert_eq!(
            album.cover().as_deref(),
            Some(
                "https://cdn-images.dzcdn.net/images/cover/041a7401ead0c50765000792dc75b14e/1000x1000-000000-80-0-0.jpg"
            )
        );
        assert_eq!(album.release_date(), None);
    }
}
