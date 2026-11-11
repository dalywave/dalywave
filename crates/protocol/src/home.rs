//! Главная: полки, которые интерфейс показывает как есть. Новые виды полок добавляются в [`ShelfKind`],
//! форма полки и элементов при этом не меняется.

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
    /// «Миксы дня»: по плейлисту на грань вкуса (вид `mix`), ~30 треков, обновляются каждую ночь.
    DailyMixes,
    /// «Открытия недели»: треки индекса каталога, которых ещё не слышал; обновляются раз в неделю.
    DiscoverWeekly,
    /// «Забытые любимые»: любимое, которое давно не играло.
    ForgottenFavorites,
    /// «Радар новинок»: треки свежих (30 дней) релизов подписок и артистов, которых часто слушаешь.
    ReleaseRadar,
}

names!(ShelfKind {
    Continue => "continue",
    NewReleases => "new_releases",
    RecentLikes => "recent_likes",
    TopArtists => "top_artists",
    DailyMixes => "daily_mixes",
    DiscoverWeekly => "discover_weekly",
    ForgottenFavorites => "forgotten_favorites",
    ReleaseRadar => "release_radar",
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
    /// Постоянный id полки: `continue`, `daily-mixes`, `new-releases`, `discover-weekly`, `recent-likes`,
    /// `forgotten-favorites`, `top-artists`, `release-radar`.
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
