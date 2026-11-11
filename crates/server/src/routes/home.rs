//! Главная: «Продолжить», «Миксы дня», «Радар новинок», «Новинки подписок», «Открытия недели», «Недавно
//! понравилось», «Забытые любимые», «Ваши артисты». Миксы и открытия — плейлисты вида `mix`, которые
//! собирает ночной пересчёт вкуса ([`crate::reco::taste`]). Порядок полок — от времени суток слушателя.

use std::collections::HashMap;

use axum::{Router, extract::State, routing::get};
use chrono::Utc;
use dalywave_protocol::home::{Home, Shelf, ShelfItem, ShelfKind};
use uuid::Uuid;

use crate::{
    auth::Principal,
    catalog::views,
    error::AppResult,
    extract::Json,
    reco::{DayPart, Settings, wave},
    routes::playlists,
    state::AppState,
};

const SHELF_SIZE: i64 = 20;
/// Треков в «Радаре новинок» и сколько из одного релиза.
const RADAR_SIZE: i64 = 30;
const RADAR_PER_RELEASE: i64 = 3;

/// Порядок полок по времени суток: утром — свежее (миксы, новинки), днём — открытия, вечером — любимое
/// и забытое, ночью — миксы и знакомое. «Продолжить» всегда рядом с верхом.
pub fn shelf_order(part: DayPart) -> [&'static str; 8] {
    match part {
        DayPart::Morning => [
            "continue",
            "daily-mixes",
            "release-radar",
            "new-releases",
            "discover-weekly",
            "recent-likes",
            "forgotten-favorites",
            "top-artists",
        ],
        DayPart::Day => [
            "continue",
            "discover-weekly",
            "daily-mixes",
            "release-radar",
            "new-releases",
            "recent-likes",
            "top-artists",
            "forgotten-favorites",
        ],
        DayPart::Evening => [
            "continue",
            "daily-mixes",
            "forgotten-favorites",
            "discover-weekly",
            "recent-likes",
            "release-radar",
            "new-releases",
            "top-artists",
        ],
        DayPart::Night => [
            "daily-mixes",
            "continue",
            "forgotten-favorites",
            "recent-likes",
            "discover-weekly",
            "release-radar",
            "new-releases",
            "top-artists",
        ],
    }
}

/// «Радар новинок»: до трёх самых популярных треков каждого релиза за 30 дней у подписок, лайкнутых
/// артистов и тех, кого дослушивал хотя бы трижды за три месяца; «не рекомендовать» и «не нравится» — мимо.
async fn release_radar(st: &AppState, user: Uuid) -> AppResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar!(
        r#"WITH mine AS (
               SELECT artist_id FROM followed_artists WHERE user_id = $1
               UNION
               SELECT ta.artist_id FROM liked_tracks l
               JOIN track_artists ta ON ta.track_id = l.track_id AND ta.role = 'main'
               WHERE l.user_id = $1 AND l.created_at > taste_reset_at($1)
               UNION
               SELECT ta.artist_id FROM play_events p
               JOIN track_artists ta ON ta.track_id = p.track_id AND ta.role = 'main'
               WHERE p.user_id = $1 AND p.started_at > now() - interval '90 days'
                 AND (p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms)
               GROUP BY ta.artist_id HAVING count(*) >= 3
           ), releases AS (
               SELECT DISTINCT al.id, al.release_date FROM artist_releases r
               JOIN albums al ON al.id = r.album_id
               WHERE r.artist_id IN (SELECT artist_id FROM mine)
                 AND al.release_date >= current_date - 30 AND al.release_date <= current_date
                 AND NOT EXISTS (SELECT 1 FROM blocked_items b
                                 WHERE b.user_id = $1 AND b.kind = 'artist' AND b.ref_id = al.artist_id)
           )
           SELECT pick.id AS "id!" FROM releases rl
           CROSS JOIN LATERAL (
               SELECT t.id, t.rank FROM tracks t
               WHERE t.album_id = rl.id
                 AND NOT EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $1 AND b.kind = 'track' AND b.ref_id = t.id)
                 AND NOT COALESCE((SELECT f.kind = 'dislike' FROM feedback_events f
                                   WHERE f.user_id = $1 AND f.track_id = t.id AND f.kind IN ('dislike', 'undislike')
                                   ORDER BY f.at DESC LIMIT 1), false)
               ORDER BY t.rank DESC NULLS LAST, t.disc_number, t.track_position, t.id
               LIMIT $2
           ) pick
           ORDER BY rl.release_date DESC, rl.id, pick.rank DESC NULLS LAST, pick.id
           LIMIT $3"#,
        user,
        RADAR_PER_RELEASE,
        RADAR_SIZE,
    )
    .fetch_all(&st.db)
    .await?)
}

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
               -- История из архива Spotify питает вкус, но «Продолжить» — только своё недавнее
               WHERE pe.user_id = $1 AND pe.source <> 'import'
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
    let mix_ids = sqlx::query_scalar!(
        "SELECT id FROM playlists WHERE user_id = $1 AND kind = 'mix' AND rules ->> 'generator' = 'daily_mix'
         ORDER BY (rules ->> 'slot')::int, id",
        user,
    )
    .fetch_all(&st.db)
    .await?;
    let mixes: HashMap<Uuid, _> = playlists::summaries(&st.db, user, Some(&mix_ids), false)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let discovery_ids = sqlx::query_scalar!(
        "SELECT i.track_id FROM playlists p JOIN playlist_items i ON i.playlist_id = p.id
         WHERE p.user_id = $1 AND p.kind = 'mix' AND p.rules ->> 'generator' = 'discover_weekly'
         ORDER BY i.position, i.id",
        user,
    )
    .fetch_all(&st.db)
    .await?;
    let forgotten_ids = wave::forgotten(&st.db, user, SHELF_SIZE).await?;
    let radar_ids = release_radar(&st, user).await?;

    let shelves = [
        shelf(
            "continue",
            "Продолжить",
            ShelfKind::Continue,
            continue_items(&st, user).await?,
        ),
        shelf(
            "daily-mixes",
            "Миксы дня",
            ShelfKind::DailyMixes,
            mix_ids
                .iter()
                .filter_map(|id| mixes.get(id).cloned().map(ShelfItem::Playlist))
                .collect(),
        ),
        shelf(
            "release-radar",
            "Радар новинок",
            ShelfKind::ReleaseRadar,
            views::tracks(&st.db, &radar_ids, user)
                .await?
                .into_iter()
                .map(ShelfItem::Track)
                .collect(),
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
            "discover-weekly",
            "Открытия недели",
            ShelfKind::DiscoverWeekly,
            views::tracks(&st.db, &discovery_ids, user)
                .await?
                .into_iter()
                .map(ShelfItem::Track)
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
            "forgotten-favorites",
            "Забытые любимые",
            ShelfKind::ForgottenFavorites,
            views::tracks(&st.db, &forgotten_ids, user)
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
    let settings = Settings::load(&st.db).await?;
    let order = shelf_order(DayPart::of_hour(settings.local_hour(Utc::now())));
    let mut shelves: Vec<Shelf> = shelves.into_iter().flatten().collect();
    shelves.sort_by_key(|s| {
        order
            .iter()
            .position(|id| *id == s.id)
            .unwrap_or(order.len())
    });
    Ok(Json(Home {
        shelves,
        generated_at: Utc::now(),
    }))
}
