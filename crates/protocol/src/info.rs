//! Сведения о музыке: чарты (`/api/charts`), отрывок с припева перед включением
//! (`/api/tracks/{id}/hook`), кто сделал трек (`/api/tracks/{id}/credits`) и об артисте
//! (`/api/artists/{id}/about`).
//!
//! Источники бесплатные и без ключей: Deezer (чарты мира и жанров), открытый чарт Apple Music
//! (Россия: у Deezer своего нет), MusicBrainz (авторы и звукорежиссёры), Wikidata и Википедия.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::catalog::{AlbumSummary, ArtistSummary, ImageSet, Track};

// ───────────── Чарты ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    /// Чарт страны.
    Country,
    /// Весь мир.
    World,
    /// Жанр.
    Genre,
}

names!(ChartKind { Country => "country", World => "world", Genre => "genre" });

/// Чарт в списке.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChartSummary {
    /// `ru`, `world`, `genre-116`.
    pub id: String,
    /// По-русски; интерфейс переводит по `id`.
    pub title: String,
    pub kind: ChartKind,
    /// Обложка первого трека. `null`, пока чарт не собран.
    #[schema(required = true)]
    pub image: Option<ImageSet>,
    /// Когда чарт собран. `null` — ещё ни разу.
    #[schema(required = true)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChartList {
    pub items: Vec<ChartSummary>,
}

/// Чарт целиком: треки по местам, альбомы и артисты — по трекам чарта.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChartDetail {
    pub chart: ChartSummary,
    /// По местам: первый — №1.
    pub tracks: Vec<Track>,
    /// Альбомы и EP, чьи треки стоят выше всего.
    pub albums: Vec<AlbumSummary>,
    /// Артисты, чьи треки стоят выше всего.
    pub artists: Vec<ArtistSummary>,
    /// Чарт собирается прямо сейчас: стоит спросить ещё раз через несколько секунд.
    pub updating: bool,
}

// ───────────── Отрывок с припева ─────────────

/// Откуда место отрывка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookFrom {
    /// Припев по кривой энергии полного анализа.
    Analysis,
    /// Без анализа: 30 % трека.
    Estimate,
    /// Звука на сервере нет: 30-секундное превью Deezer.
    Preview,
}

names!(HookFrom { Analysis => "analysis", Estimate => "estimate", Preview => "preview" });

/// Ответ `GET /api/tracks/{id}/hook`: что играть ~15 секунд перед включением, не трогая очередь.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackHook {
    pub track_id: Uuid,
    /// Звук трека на сервере: отрывок — из него, с `startMs` (ссылку плеер берёт как обычно).
    pub ready: bool,
    /// Начало отрывка от начала звука, мс. У превью — 0: Deezer сам режет его с припева.
    pub start_ms: i64,
    /// Сколько звучит отрывок, мс.
    pub length_ms: i64,
    pub from: HookFrom,
    /// Звука на сервере нет — подписанная ссылка на MP3-превью Deezer (живёт недолго, не хранить).
    /// `null` вместе с `ready: false` — послушать нечего.
    #[schema(required = true)]
    pub preview_url: Option<String>,
}

// ───────────── Кто сделал трек ─────────────

/// Роль в работе над треком (отношения MusicBrainz).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CreditRole {
    /// Музыка и слова.
    Writer,
    /// Музыка.
    Composer,
    /// Слова.
    Lyricist,
    /// Аранжировка.
    Arranger,
    Producer,
    /// Звукорежиссёр.
    Engineer,
    /// Запись.
    Recording,
    /// Сведение.
    Mix,
    Mastering,
    /// Программирование.
    Programming,
}

names!(CreditRole {
    Writer => "writer",
    Composer => "composer",
    Lyricist => "lyricist",
    Arranger => "arranger",
    Producer => "producer",
    Engineer => "engineer",
    Recording => "recording",
    Mix => "mix",
    Mastering => "mastering",
    Programming => "programming",
});

impl CreditRole {
    /// Авторы песни (музыка, слова), а не запись.
    pub fn is_writing(self) -> bool {
        matches!(self, Self::Writer | Self::Composer | Self::Lyricist)
    }
}

/// Человек и что он делал. Один человек — одна строка, роли по порядку важности.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreditPerson {
    /// id в MusicBrainz.
    pub id: Uuid,
    pub name: String,
    pub roles: Vec<CreditRole>,
    /// Артист каталога, если MusicBrainz знает его страницу в Deezer: нажатие ведёт к его трекам.
    /// `null` — просто имя.
    #[schema(required = true)]
    pub artist_id: Option<Uuid>,
}

/// Ответ `GET /api/tracks/{id}/credits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackCredits {
    pub track_id: Uuid,
    /// Авторы музыки и слов.
    pub writing: Vec<CreditPerson>,
    /// Продюсеры и звукорежиссёры.
    pub production: Vec<CreditPerson>,
    #[schema(required = true)]
    pub label: Option<String>,
    #[schema(required = true)]
    pub year: Option<i32>,
    /// Страницы людей ещё ищутся: через несколько секунд у части имён появятся ссылки.
    pub resolving: bool,
}

// ───────────── Об артисте ─────────────

/// Годы на сцене. `to: null` — по сей день.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveYears {
    pub from: i32,
    #[schema(required = true)]
    pub to: Option<i32>,
}

/// Коротко об артисте из Википедии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtistAbout {
    /// Начало статьи: несколько предложений, без ударений.
    pub summary: String,
    /// Статья целиком — «Читать дальше».
    pub url: String,
    /// Язык статьи: `ru` или `en`.
    pub lang: String,
    /// Группа, а не человек.
    pub group: bool,
    #[schema(required = true)]
    pub active: Option<ActiveYears>,
    /// Город рождения или основания.
    #[schema(required = true)]
    pub origin: Option<String>,
    /// До трёх жанров.
    pub genres: Vec<String>,
}

/// Ответ `GET /api/artists/{id}/about`. `about: null` — статьи нет, блока нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfo {
    pub artist_id: Uuid,
    #[schema(required = true)]
    pub about: Option<ArtistAbout>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_json() {
        for r in CreditRole::ALL {
            assert_eq!(serde_json::to_value(r).unwrap(), r.as_str());
        }
        for k in ChartKind::ALL {
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
        }
        assert_eq!(serde_json::to_value(HookFrom::Preview).unwrap(), "preview");
        assert!(CreditRole::Lyricist.is_writing() && !CreditRole::Mix.is_writing());
    }
}
