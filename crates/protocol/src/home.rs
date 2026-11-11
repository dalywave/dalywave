//! Главная: полки, которые интерфейс показывает как есть. Новые виды полок (миксы, открытия)
//! добавятся в [`ShelfKind`], форма полки и элементов при этом не меняется.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    catalog::{AlbumSummary, ArtistSummary, Track},
    library::PlaylistSummary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShelfKind {
    /// «Продолжить»: недавние альбомы, плейлисты и артисты, с которых слушал.
    Continue,
    /// «Новинки подписок»: релизы артистов из подписок за 60 дней.
    NewReleases,
    /// «Недавно понравилось».
    RecentLikes,
    /// «Ваши артисты»: больше всего прослушиваний за 30 дней.
    TopArtists,
}

names!(ShelfKind {
    Continue => "continue",
    NewReleases => "new_releases",
    RecentLikes => "recent_likes",
    TopArtists => "top_artists",
});

/// Элемент полки: поле `type` говорит, что внутри.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShelfItem {
    Track(Track),
    Album(AlbumSummary),
    Artist(ArtistSummary),
    Playlist(PlaylistSummary),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    /// Постоянный id полки: `continue`, `new-releases`, `recent-likes`, `top-artists`.
    pub id: String,
    /// Заголовок по-русски; английский интерфейс берёт свой по `id`.
    pub title: String,
    pub kind: ShelfKind,
    pub items: Vec<ShelfItem>,
}

/// Пустые полки не приходят.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Home {
    pub shelves: Vec<Shelf>,
    pub generated_at: DateTime<Utc>,
}
