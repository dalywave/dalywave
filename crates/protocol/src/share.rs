//! «Поделиться»: карточка ссылки на трек, альбом, артиста или плейлист и разбор чужих ссылок
//! (Deezer, Spotify, Apple Music, YouTube, SoundCloud, Bandcamp) в сущность каталога.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::catalog::{AlbumSummary, ArtistSummary, Track};

/// Что открывает ссылка `https://wave.daly.rest/{kind}/{id}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShareKind {
    Track,
    Album,
    Artist,
    Playlist,
}

names!(ShareKind { Track => "track", Album => "album", Artist => "artist", Playlist => "playlist" });

/// Карточка для ссылки: то же видят превью в мессенджерах и статус в Discord.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShareCard {
    pub kind: ShareKind,
    pub id: Uuid,
    pub title: String,
    /// Трек — исполнители через запятую, альбом — главный исполнитель, плейлист — описание,
    /// артист — пусто.
    pub subtitle: String,
    /// Публичный адрес обложки (у Deezer), который открывается без входа; `null` — обложки нет.
    #[schema(required = true)]
    pub image: Option<String>,
    /// Ссылка на страницу в DalyWave.
    pub url: String,
}

/// Ссылка или текст, в котором она есть (так приходит «Поделиться» из других приложений).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolveLinkRequest {
    pub url: String,
}

/// Откуда ссылка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LinkSource {
    Dalywave,
    Deezer,
    Spotify,
    Apple,
    Youtube,
    Soundcloud,
    Bandcamp,
}

names!(LinkSource {
    Dalywave => "dalywave",
    Deezer => "deezer",
    Spotify => "spotify",
    Apple => "apple",
    Youtube => "youtube",
    Soundcloud => "soundcloud",
    Bandcamp => "bandcamp",
});

/// Найденное по ссылке: заполнено поле своего вида (`track`, `album`, `artist` или `playlistId`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLink {
    pub source: LinkSource,
    pub kind: ShareKind,
    /// Текст, по которому искали в каталоге (для ссылок не на Deezer и не на DalyWave).
    #[schema(required = true)]
    pub query: Option<String>,
    #[schema(required = true)]
    pub track: Option<Track>,
    #[schema(required = true)]
    pub album: Option<AlbumSummary>,
    #[schema(required = true)]
    pub artist: Option<ArtistSummary>,
    #[schema(required = true)]
    pub playlist_id: Option<Uuid>,
}
