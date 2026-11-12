//! Каталог: артисты, альбомы, треки и поиск.
//!
//! Картинки — адреса нашего сервера вида `/api/images/albums/{id}/500.jpg?v=…`: сервер отдаёт копию из
//! хранилища или, пока копии нет, перенаправляет на Deezer. `v` меняется вместе с исходной картинкой.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::library::DownloadPolicy;

/// Есть ли аудио записи на сервере. Интерфейс показывает это значками.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudioStatus {
    /// Ещё не скачивалась: скачается при первом запуске.
    None,
    /// Стоит в очереди на скачивание.
    Queued,
    /// Скачивается сейчас.
    Acquiring,
    /// Файл на сервере.
    Ready,
    /// Кандидаты не прошли проверку — ждёт решения в разделе «Спорные».
    Disputed,
    /// В источниках нет.
    Unavailable,
    /// Скачать не удалось.
    Failed,
}

names!(AudioStatus {
    None => "none",
    Queued => "queued",
    Acquiring => "acquiring",
    Ready => "ready",
    Disputed => "disputed",
    Unavailable => "unavailable",
    Failed => "failed",
});

/// Тип релиза у Deezer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    Album,
    Ep,
    Single,
    /// Сборник.
    Compile,
}

names!(RecordType { Album => "album", Ep => "ep", Single => "single", Compile => "compile" });

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtistRole {
    Main,
    /// Приглашённый артист (feat.).
    Featured,
}

names!(ArtistRole { Main => "main", Featured => "featured" });

/// Картинка в трёх размерах.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageSet {
    /// 250×250.
    pub small: String,
    /// 500×500.
    pub medium: String,
    /// 1000×1000.
    pub large: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackArtist {
    pub id: Uuid,
    pub name: String,
    pub role: ArtistRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRef {
    pub id: Uuid,
    pub title: String,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
}

/// Трек каталога — позиция в альбоме. Одна запись (ISRC) может быть у нескольких треков.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: Uuid,
    #[schema(required = true)]
    pub deezer_id: Option<i64>,
    pub title: String,
    /// Версия из названия у Deezer: «Slowed», «Remastered 2011». Пустая строка, если её нет.
    pub version: String,
    pub duration_ms: i32,
    pub explicit: bool,
    pub disc_number: i16,
    #[schema(required = true)]
    pub track_position: Option<i16>,
    /// Сначала основные артисты, затем приглашённые.
    pub artists: Vec<TrackArtist>,
    #[schema(required = true)]
    pub album: Option<AlbumRef>,
    /// `null`, пока ISRC трека неизвестен.
    #[schema(required = true)]
    pub recording_id: Option<Uuid>,
    #[schema(required = true)]
    pub isrc: Option<String>,
    pub audio_status: AudioStatus,
    pub liked: bool,
    /// У explicit-трека — чистая версия той же песни в каталоге: её играет фильтр «Без нецензурного».
    #[serde(default)]
    pub clean_id: Option<Uuid>,
}

/// Карточка артиста в списках.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtistSummary {
    pub id: Uuid,
    #[schema(required = true)]
    pub deezer_id: Option<i64>,
    pub name: String,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
    #[schema(required = true)]
    pub fans: Option<i32>,
    #[schema(required = true)]
    pub albums_count: Option<i32>,
    pub followed: bool,
}

/// Карточка альбома в списках.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: Uuid,
    #[schema(required = true)]
    pub deezer_id: Option<i64>,
    pub title: String,
    pub artist: ArtistRef,
    pub record_type: RecordType,
    #[schema(required = true)]
    pub release_date: Option<NaiveDate>,
    pub explicit: bool,
    #[schema(required = true)]
    pub tracks_count: Option<i32>,
    #[schema(required = true)]
    pub duration_ms: Option<i32>,
    #[schema(required = true)]
    pub image: Option<ImageSet>,
    /// 3–5 основных цветов обложки (`#rrggbb`, самый заметный первым); `null`, пока обложка не скопирована.
    #[schema(required = true)]
    pub palette: Option<Vec<String>>,
    pub saved: bool,
}

/// Страница альбома.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetail {
    pub album: AlbumSummary,
    #[schema(required = true)]
    pub upc: Option<String>,
    #[schema(required = true)]
    pub label: Option<String>,
    pub genres: Vec<String>,
    #[schema(required = true)]
    pub fans: Option<i32>,
    /// По порядку: диск, затем позиция.
    pub tracks: Vec<Track>,
    /// Сумма длительностей треков.
    pub total_duration_ms: i64,
    /// Сколько треков уже на сервере.
    pub ready_tracks: i64,
}

/// Дискография по типам релизов, новые первыми.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Releases {
    pub albums: Vec<AlbumSummary>,
    pub eps: Vec<AlbumSummary>,
    pub singles: Vec<AlbumSummary>,
    pub compilations: Vec<AlbumSummary>,
}

/// Подписка на артиста.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowState {
    pub policy: DownloadPolicy,
    pub notify: bool,
    pub followed_at: DateTime<Utc>,
}

/// Страница артиста.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetail {
    pub artist: ArtistSummary,
    pub top_tracks: Vec<Track>,
    pub releases: Releases,
    pub related: Vec<ArtistSummary>,
    /// `null`, если подписки нет.
    #[schema(required = true)]
    pub follow: Option<FollowState>,
    /// Сколько треков дискографии уже на сервере («скачано 48 из 132»).
    pub tracks_on_server: i64,
    pub tracks_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrackLookup {
    /// До 500 id; неизвестные пропускаются.
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackList {
    pub items: Vec<Track>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchScope {
    /// Весь каталог Deezer.
    #[default]
    Catalog,
    /// Медиатека: лайки, сохранённые альбомы, подписки и всё, что уже на сервере.
    Library,
}

names!(SearchScope { Catalog => "catalog", Library => "library" });

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// Строка поиска.
    pub q: String,
    /// `catalog` (по умолчанию) или `library`.
    pub scope: Option<SearchScope>,
    /// Через запятую: `track`, `album`, `artist`. По умолчанию все.
    pub types: Option<String>,
    /// Сколько результатов каждого вида: 1–50, по умолчанию 20.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub scope: SearchScope,
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumSummary>,
    pub artists: Vec<ArtistSummary>,
}

/// Что искать в Deezer по его id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeezerKind {
    Artist,
    Album,
    Track,
}

names!(DeezerKind { Artist => "artist", Album => "album", Track => "track" });

/// Наша сущность для id Deezer: поле `type` говорит, какая.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CatalogEntity {
    Artist(ArtistSummary),
    Album(AlbumSummary),
    Track(Track),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_json() {
        for s in AudioStatus::ALL {
            assert_eq!(serde_json::to_value(s).unwrap(), s.as_str());
            assert_eq!(s.as_str().parse::<AudioStatus>().unwrap(), *s);
        }
        for t in RecordType::ALL {
            assert_eq!(serde_json::to_value(t).unwrap(), t.as_str());
        }
        assert_eq!(
            serde_json::to_value(ArtistRole::Featured).unwrap(),
            "featured"
        );
    }

    #[test]
    fn entity_is_tagged() {
        let artist = ArtistSummary {
            id: Uuid::nil(),
            deezer_id: Some(1),
            name: "Lowborn".into(),
            image: None,
            fans: None,
            albums_count: None,
            followed: false,
        };
        let json = serde_json::to_value(CatalogEntity::Artist(artist)).unwrap();
        assert_eq!(json["type"], "artist");
        assert_eq!(json["name"], "Lowborn");
        assert_eq!(json["deezerId"], 1);
    }
}
