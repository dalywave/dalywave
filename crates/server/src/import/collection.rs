//! Медиатека другого сервиса в общем виде: треки (лайкнутые — в «Мне нравится») и плейлисты по
//! порядку. Из неё собираются позиции импорта «Яндекс Музыки» и YouTube Music.

use std::collections::HashMap;

use super::matching::normalize;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
    /// Устойчивый id в источнике: `ym:<id>`, `yt:<id видео>`. Ключ кеша сопоставлений.
    pub source_ref: Option<String>,
    pub liked: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Playlist {
    pub name: String,
    pub description: String,
    /// Индексы в [`Collection::tracks`] по порядку плейлиста.
    pub tracks: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Collection {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    by_ref: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
}

/// Больше позиций за один перенос не берём: это десятки тысяч поисков в Deezer.
pub const MAX_TRACKS: usize = 50_000;

impl Collection {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn full(&self) -> bool {
        self.tracks.len() >= MAX_TRACKS
    }

    /// Трек по id источника или по имени: один и тот же трек из лайков и плейлистов — одна позиция.
    /// Пустое название допустимо только с id (имя узнается позже, как у видео из Takeout).
    pub fn add(&mut self, t: Track) -> Option<usize> {
        let source_ref = t.source_ref.clone().filter(|r| !r.trim().is_empty());
        let title = t.title.trim();
        if title.is_empty() && source_ref.is_none() {
            return None;
        }
        let name_key = (!title.is_empty())
            .then(|| format!("{}|{}", normalize(&t.artist), normalize(title)))
            .filter(|k| k != "|");
        let found = source_ref
            .as_ref()
            .and_then(|r| self.by_ref.get(r).copied())
            .or_else(|| {
                // Без id одинаковые имена — одно; с разными id — разные записи (версии, перезаливы)
                if source_ref.is_some() {
                    None
                } else {
                    name_key.as_ref().and_then(|k| self.by_name.get(k).copied())
                }
            });
        let idx = match found {
            Some(i) => i,
            None => {
                if self.full() {
                    return None;
                }
                self.tracks.push(Track {
                    artist: t.artist.trim().to_string(),
                    title: title.to_string(),
                    album: None,
                    duration_ms: None,
                    source_ref: source_ref.clone(),
                    liked: false,
                });
                self.tracks.len() - 1
            }
        };
        let cur = &mut self.tracks[idx];
        if cur.title.is_empty() && !title.is_empty() {
            cur.title = title.to_string();
            cur.artist = t.artist.trim().to_string();
        }
        if cur.album.is_none() {
            cur.album = t
                .album
                .as_deref()
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .map(String::from);
        }
        if cur.duration_ms.is_none() {
            cur.duration_ms = t.duration_ms.filter(|d| *d > 0);
        }
        cur.liked |= t.liked;
        if let Some(r) = source_ref {
            self.by_ref.insert(r, idx);
        }
        if let Some(k) = name_key {
            self.by_name.entry(k).or_insert(idx);
        }
        Some(idx)
    }

    /// Плейлист из уже добавленных треков. Пустое название — «Плейлист».
    pub fn playlist(&mut self, name: &str, description: &str, tracks: Vec<usize>) {
        let name = name.trim();
        self.playlists.push(Playlist {
            name: if name.is_empty() {
                "Плейлист".into()
            } else {
                name.chars().take(200).collect()
            },
            description: description.trim().chars().take(2000).collect(),
            tracks,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(artist: &str, title: &str, r: Option<&str>) -> Track {
        Track {
            artist: artist.into(),
            title: title.into(),
            source_ref: r.map(String::from),
            ..Track::default()
        }
    }

    #[test]
    fn same_track_is_one_position() {
        let mut c = Collection::default();
        let a = c
            .add(Track {
                liked: true,
                ..t("Lowborn", "Devil", Some("ym:1"))
            })
            .unwrap();
        let b = c.add(t("Lowborn", "Devil", Some("ym:1"))).unwrap();
        assert_eq!(a, b);
        assert!(c.tracks[0].liked);
        // Без id — по имени
        let x = c.add(t("Mzlff", "Ёлка", None)).unwrap();
        let y = c.add(t("MZLFF", "ёлка", None)).unwrap();
        assert_eq!(x, y);
        // Видео без имени, имя приходит позже
        let v = c.add(t("", "", Some("yt:abc"))).unwrap();
        let w = c.add(t("Rushex", "Night Drive", Some("yt:abc"))).unwrap();
        assert_eq!(v, w);
        assert_eq!(c.tracks[v].title, "Night Drive");
        assert_eq!(c.add(t("A", "", None)), None);
        c.playlist(" ", "", vec![a, x]);
        assert_eq!(c.playlists[0].name, "Плейлист");
    }
}
