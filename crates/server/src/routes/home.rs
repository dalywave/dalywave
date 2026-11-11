//! Главная: «Продолжить», «Новинки подписок», «Недавно понравилось», «Ваши артисты».
//! Миксы и открытия добавит этап 2 — новыми видами полок той же формы.

use std::collections::HashMap;

use axum::{Router, extract::State, routing::get};
use chrono::Utc;
use dalywave_protocol::home::{Home, Shelf, ShelfItem, ShelfKind};
use uuid::Uuid;

use crate::{
    auth::Principal, catalog::views, error::AppResult, extract::Json, routes::playlists,
    state::AppState,
};

const SHELF_SIZE: i64 = 20;

pub fn routes() -> Router<AppState> {
    Router::new().route("/home", get(home))
}

fn shelf(id: &str, title: &str, kind: ShelfKind, items: Vec<ShelfItem>) -> Option<Shelf> {
    (!items.is_empty()).then(|| Shelf {
        id: id.into(),
        title: title.into(),
        kind,
        items,
    })
}

/// Недавние источники прослушивания: альбом, плейлист или артист; без источника — альбом трека.
async fn continue_items(st: &AppState, user: Uuid) -> AppResult<Vec<ShelfItem>> {
    let rows = sqlx::query!(
        r#"WITH recent AS (
               SELECT pe.started_at,
                      CASE WHEN pe.context_type IN ('album', 'playlist', 'artist') AND pe.context_id IS NOT NULL
                           THEN pe.context_type ELSE 'album' END AS kind,
                      CASE WHEN pe.context_type IN ('album', 'playlist', 'artist') AND pe.context_id IS NOT NULL
                           THEN pe.context_id ELSE t.album_id::text END AS ref
               FROM play_events pe JOIN tracks t ON t.id = pe.track_id
               WHERE pe.user_id = $1
               ORDER BY pe.started_at DESC
               LIMIT 300
           )
           SELECT kind AS "kind!", ref AS "ref!", max(started_at) AS "last!"
           FROM recent WHERE ref IS NOT NULL
           GROUP BY kind, ref
           ORDER BY max(started_at) DESC
           LIMIT 12"#,
        user,
    )
    .fetch_all(&st.db)
    .await?;
    let refs: Vec<(String, Uuid)> = rows
        .into_iter()
        .filter_map(|r| Some((r.kind, r.r#ref.parse().ok()?)))
        .collect();
    let ids = |kind: &str| -> Vec<Uuid> {
        refs.iter()
            .filter(|(k, _)| k == kind)
            .map(|(_, id)| *id)
            .collect()
    };
    let albums: HashMap<Uuid, _> = views::albums(&st.db, &ids("album"), user)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let artists: HashMap<Uuid, _> = views::artists(&st.db, &ids("artist"), user)
        .await?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    let playlist_ids = ids("playlist");
    let lists: HashMap<Uuid, _> = playlists::summaries(&st.db, user, Some(&playlist_ids), false)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    Ok(refs
        .iter()
        .filter_map(|(kind, id)| match kind.as_str() {
            "album" => albums.get(id).cloned().map(ShelfItem::Album),
            "artist" => artists.get(id).cloned().map(ShelfItem::Artist),
            "playlist" => lists.get(id).cloned().map(ShelfItem::Playlist),
            _ => None,
        })
        .collect())
}

#[utoipa::path(
    get,
    path = "/api/home",
    tag = "library",
    security(("session" = []), ("device" = [])),
    responses((status = 200, description = "Полки главной; пустых нет", body = Home)),
)]
pub async fn home(State(st): State<AppState>, principal: Principal) -> AppResult<Json<Home>> {
    let user = principal.user.id;
    let new_release_ids = sqlx::query_scalar!(
        r#"SELECT al.id AS "id!" FROM albums al
           WHERE al.id IN (SELECT r.album_id FROM followed_artists f
                           JOIN artist_releases r ON r.artist_id = f.artist_id WHERE f.user_id = $1)
             AND al.release_date >= current_date - 60 AND al.release_date <= current_date
           ORDER BY al.release_date DESC, al.id DESC
           LIMIT $2"#,
        user,
        SHELF_SIZE,
    )
    .fetch_all(&st.db)
    .await?;
    let liked_ids = sqlx::query_scalar!(
        "SELECT track_id FROM liked_tracks WHERE user_id = $1 ORDER BY created_at DESC, track_id DESC LIMIT $2",
        user,
        SHELF_SIZE,
    )
    .fetch_all(&st.db)
    .await?;
    let top_artist_ids = sqlx::query_scalar!(
        r#"SELECT ta.artist_id AS "artist_id!" FROM play_events pe
           JOIN track_artists ta ON ta.track_id = pe.track_id AND ta.role = 'main'
           WHERE pe.user_id = $1 AND pe.started_at > now() - interval '30 days'
           GROUP BY ta.artist_id
           ORDER BY count(*) DESC, max(pe.started_at) DESC, ta.artist_id
           LIMIT 12"#,
        user,
    )
    .fetch_all(&st.db)
    .await?;

    let shelves = [
        shelf(
            "continue",
            "Продолжить",
            ShelfKind::Continue,
            continue_items(&st, user).await?,
        ),
        shelf(
            "new-releases",
            "Новинки подписок",
            ShelfKind::NewReleases,
            views::albums(&st.db, &new_release_ids, user)
                .await?
                .into_iter()
                .map(ShelfItem::Album)
                .collect(),
        ),
        shelf(
            "recent-likes",
            "Недавно понравилось",
            ShelfKind::RecentLikes,
            views::tracks(&st.db, &liked_ids, user)
                .await?
                .into_iter()
                .map(ShelfItem::Track)
                .collect(),
        ),
        shelf(
            "top-artists",
            "Ваши артисты",
            ShelfKind::TopArtists,
            views::artists(&st.db, &top_artist_ids, user)
                .await?
                .into_iter()
                .map(ShelfItem::Artist)
                .collect(),
        ),
    ];
    Ok(Json(Home {
        shelves: shelves.into_iter().flatten().collect(),
        generated_at: Utc::now(),
    }))
}
