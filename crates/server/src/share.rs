//! Карточка ссылки «Поделиться» и превью для мессенджеров: страница `/{kind}/{id}` — тот же
//! index.html интерфейса, но с тегами Open Graph. Отдаётся всем: роботам мессенджеров вход не нужен,
//! а сама страница в интерфейсе всё равно попросит войти.

use dalywave_protocol::share::{ShareCard, ShareKind};
use sqlx::PgPool;
use uuid::Uuid;

use crate::catalog::views::{ImageKind, image_set};

/// Адрес картинки годится для превью: у Deezer он публичный.
fn public(source: Option<String>) -> Option<String> {
    source.filter(|s| s.starts_with("https://") || s.starts_with("http://"))
}

/// Карточка по id. `owner` — для плейлистов: чужой плейлист не находится. Без `owner` (превью
/// ссылки) — любой.
pub async fn card(
    db: &PgPool,
    public_url: &str,
    kind: ShareKind,
    id: Uuid,
    owner: Option<Uuid>,
) -> Result<Option<ShareCard>, sqlx::Error> {
    let found = match kind {
        ShareKind::Track => sqlx::query!(
            r#"SELECT t.title, al.cover_source,
                      ARRAY(SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                            WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id) AS "artists!"
               FROM tracks t LEFT JOIN albums al ON al.id = t.album_id
               WHERE t.id = $1"#,
            id
        )
        .fetch_optional(db)
        .await?
        .map(|r| (r.title, r.artists.join(", "), public(r.cover_source))),
        ShareKind::Album => sqlx::query!(
            "SELECT al.title, al.cover_source, a.name AS artist
             FROM albums al JOIN artists a ON a.id = al.artist_id
             WHERE al.id = $1",
            id
        )
        .fetch_optional(db)
        .await?
        .map(|r| (r.title, r.artist, public(r.cover_source))),
        ShareKind::Artist => sqlx::query!(
            "SELECT name, picture_source FROM artists WHERE id = $1",
            id
        )
        .fetch_optional(db)
        .await?
        .map(|r| (r.name, String::new(), public(r.picture_source))),
        ShareKind::Playlist => sqlx::query!(
            r#"SELECT p.title, p.description, p.user_id, p.cover_key,
                      (SELECT al.cover_source FROM playlist_items i
                       JOIN tracks t ON t.id = i.track_id
                       JOIN albums al ON al.id = t.album_id
                       WHERE i.playlist_id = p.id AND al.cover_source IS NOT NULL
                       ORDER BY i.position LIMIT 1) AS cover_source
               FROM playlists p WHERE p.id = $1"#,
            id
        )
        .fetch_optional(db)
        .await?
        .filter(|r| owner.is_none_or(|o| o == r.user_id))
        .map(|r| {
            // Своя обложка плейлиста отдаётся сервером без входа (/api/images открыты)
            let own = image_set(ImageKind::Playlists, id, r.cover_key.as_deref(), None)
                .map(|set| format!("{public_url}{}", set.large));
            (r.title, r.description, own.or(public(r.cover_source)))
        }),
    };
    Ok(found.map(|(title, subtitle, image)| ShareCard {
        kind,
        id,
        title,
        subtitle,
        image,
        url: format!("{public_url}/{kind}/{id}"),
    }))
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Теги Open Graph и Twitter — перед `</head>` index.html. Нет `</head>` — страница как есть.
pub fn inject(index: &str, card: &ShareCard) -> String {
    let og_type = match card.kind {
        ShareKind::Track => "music.song",
        ShareKind::Album => "music.album",
        ShareKind::Artist => "profile",
        ShareKind::Playlist => "music.playlist",
    };
    let description = if card.subtitle.is_empty() {
        "DalyWave".to_string()
    } else {
        card.subtitle.clone()
    };
    let mut tags = vec![
        ("property", "og:site_name", "DalyWave".to_string()),
        ("property", "og:type", og_type.to_string()),
        ("property", "og:title", card.title.clone()),
        ("property", "og:description", description.clone()),
        ("property", "og:url", card.url.clone()),
        ("name", "twitter:card", "summary".to_string()),
        ("name", "twitter:title", card.title.clone()),
        ("name", "twitter:description", description),
    ];
    if let Some(image) = &card.image {
        tags.push(("property", "og:image", image.clone()));
        tags.push(("property", "og:image:alt", card.title.clone()));
        tags.push(("name", "twitter:image", image.clone()));
    }
    let mut head = String::new();
    for (attr, key, value) in tags {
        head.push_str(&format!(
            "<meta {attr}=\"{key}\" content=\"{}\">",
            escape(&value)
        ));
    }
    match index.find("</head>") {
        Some(at) => format!("{}{head}{}", &index[..at], &index[at..]),
        None => index.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_escaped_and_placed_in_head() {
        let card = ShareCard {
            kind: ShareKind::Track,
            id: Uuid::nil(),
            title: "Rock & \"Roll\" <script>".into(),
            subtitle: "A, B".into(),
            image: Some("https://cdn-images.dzcdn.net/images/cover/x/1000x1000.jpg".into()),
            url: "https://wave.daly.rest/track/00000000-0000-0000-0000-000000000000".into(),
        };
        let html = inject(
            "<html><head><title>DalyWave</title></head><body></body></html>",
            &card,
        );
        assert!(html.contains(
            r#"<meta property="og:title" content="Rock &amp; &quot;Roll&quot; &lt;script&gt;">"#
        ));
        assert!(html.contains(r#"<meta property="og:type" content="music.song">"#));
        assert!(html.contains("og:image\" content=\"https://cdn-images.dzcdn.net/"));
        assert!(html.find("og:title").unwrap() < html.find("</head>").unwrap());
        assert!(!html.contains("<script>"));
        assert_eq!(inject("no head", &card), "no head");
    }
}
