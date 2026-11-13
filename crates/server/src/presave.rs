//! Предсохранение релизов. Анонсы приходят вместе с дискографией артиста: Deezer отдаёт в
//! `artist/{id}/albums` и ещё не вышедшие альбомы — с датой выхода в будущем и без треков.
//!
//! «Сохранить заранее» записывает альбом в `album_presaves`. Раз в час обслуживание воркера ([`deliver`])
//! смотрит, у кого день выхода уже настал **по его часовому поясу** (из настроек уведомлений), обновляет
//! альбом из Deezer и, когда треки появились, кладёт альбом в медиатеку, ставит треки на скачивание
//! (как сохранённый альбом) и присылает уведомление «Вышел альбом — уже в медиатеке». Метка уведомления —
//! та же, что у новинок подписок (`release:{альбом}`), так что о релизе артиста из подписок придёт одно.
//!
//! Перенос: Deezer сдвинул дату — альбом ждёт новую. Отмена: альбом пропал из Deezer — предсохранение
//! гаснет (`cancelled_at`) и пропадает из «Скоро выйдет».

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use dalywave_protocol::{
    catalog::RecordType,
    jobs::AcquireReason,
    library::UpcomingRelease,
    notifications::{NotificationItem, NotificationKind},
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
};
use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    acquire,
    catalog::{
        self, CatalogError,
        views::{self, ImageKind, image_set},
    },
    deezer::{Deezer, DeezerError},
    jobs::lanes,
    notify::{self, Draft},
    realtime::{self, Audience},
};

/// Самый восточный часовой пояс (UTC+14): раньше него день выхода не наступает ни у кого.
const MAX_AHEAD_MIN: i64 = 14 * 60;

/// Сегодняшняя дата у слушателя со смещением `offset_min` от UTC.
pub fn local_today(offset_min: i16, now: DateTime<Utc>) -> NaiveDate {
    (now + Duration::minutes(i64::from(offset_min))).date_naive()
}

/// Вышел ли релиз с датой `release` у слушателя со смещением `offset_min`.
pub fn released(release: NaiveDate, offset_min: i16, now: DateTime<Utc>) -> bool {
    local_today(offset_min, now) >= release
}

/// Сегодня у слушателя: часовой пояс — из настроек уведомлений, иначе — сервиса.
pub async fn user_today(
    conn: &mut PgConnection,
    user: Uuid,
    now: DateTime<Utc>,
) -> Result<NaiveDate, sqlx::Error> {
    let offset = notify::prefs(conn, user).await?.utc_offset_min;
    Ok(local_today(offset, now))
}

async fn upcoming_views(
    db: &PgPool,
    user: Uuid,
    rows: Vec<(Uuid, bool, bool)>,
) -> Result<Vec<UpcomingRelease>, sqlx::Error> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let flags: HashMap<Uuid, (bool, bool)> =
        rows.into_iter().map(|(id, p, f)| (id, (p, f))).collect();
    Ok(views::albums(db, &ids, user)
        .await?
        .into_iter()
        .map(|album| {
            let (presaved, followed) = flags.get(&album.id).copied().unwrap_or_default();
            UpcomingRelease {
                album,
                presaved,
                followed,
            }
        })
        .collect())
}

/// Анонсы артиста для его страницы («Скоро»): ближайшие первыми.
pub async fn for_artist(
    db: &PgPool,
    user: Uuid,
    artist_id: Uuid,
    today: NaiveDate,
) -> Result<Vec<UpcomingRelease>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT al.id,
                  EXISTS (SELECT 1 FROM album_presaves p WHERE p.user_id = $2 AND p.album_id = al.id
                          AND p.delivered_at IS NULL AND p.cancelled_at IS NULL) AS "presaved!",
                  EXISTS (SELECT 1 FROM followed_artists f WHERE f.user_id = $2 AND f.artist_id = $1) AS "followed!"
           FROM artist_releases r JOIN albums al ON al.id = r.album_id
           WHERE r.artist_id = $1 AND al.release_date > $3
           ORDER BY al.release_date, al.id"#,
        artist_id,
        user,
        today,
    )
    .fetch_all(db)
    .await?;
    upcoming_views(
        db,
        user,
        rows.into_iter()
            .map(|r| (r.id, r.presaved, r.followed))
            .collect(),
    )
    .await
}

/// «Скоро выйдет»: сохранённое заранее и анонсы артистов из подписок, ближайшие первыми. Сохранённое
/// заранее, но ещё не доставленное (день настал, треков ещё нет) тоже здесь — до доставки.
pub async fn upcoming(
    db: &PgPool,
    user: Uuid,
    today: NaiveDate,
) -> Result<Vec<UpcomingRelease>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH presaved AS (
               SELECT album_id FROM album_presaves
               WHERE user_id = $1 AND delivered_at IS NULL AND cancelled_at IS NULL
           ), announced AS (
               SELECT DISTINCT r.album_id FROM followed_artists f
               JOIN artist_releases r ON r.artist_id = f.artist_id
               JOIN albums al ON al.id = r.album_id
               WHERE f.user_id = $1 AND al.release_date > $2
           )
           SELECT al.id,
                  al.id IN (SELECT album_id FROM presaved) AS "presaved!",
                  EXISTS (SELECT 1 FROM followed_artists f JOIN artist_releases r ON r.artist_id = f.artist_id
                          WHERE f.user_id = $1 AND r.album_id = al.id) AS "followed!"
           FROM albums al
           WHERE al.id IN (SELECT album_id FROM presaved UNION SELECT album_id FROM announced)
           ORDER BY al.release_date NULLS LAST, al.id"#,
        user,
        today,
    )
    .fetch_all(db)
    .await?;
    upcoming_views(
        db,
        user,
        rows.into_iter()
            .map(|r| (r.id, r.presaved, r.followed))
            .collect(),
    )
    .await
}

/// Итог прохода [`deliver`] — для журнала воркера.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Delivered {
    /// Альбомов положено в медиатеки (по слушателям).
    pub delivered: usize,
    /// День настал, а треков в Deezer ещё нет — повторим через час.
    pub waiting: usize,
    /// Дата выхода сдвинулась вперёд.
    pub postponed: usize,
    /// Альбом пропал из Deezer — предсохранения погашены.
    pub cancelled: usize,
    /// Deezer не ответил — повторим через час.
    pub failed: usize,
}

struct Pending {
    release_date: Option<NaiveDate>,
    deezer_id: Option<i64>,
    users: Vec<Uuid>,
}

/// Раз в час: предсохранённое, у кого вышло, — в медиатеку.
pub async fn deliver(
    db: &PgPool,
    deezer: &Deezer,
    now: DateTime<Utc>,
) -> Result<Delivered, sqlx::Error> {
    // Отбор с запасом на самый ранний часовой пояс; точный день — по каждому слушателю ниже
    let horizon = (now + Duration::minutes(MAX_AHEAD_MIN)).date_naive();
    let rows = sqlx::query!(
        "SELECT p.user_id, p.album_id, al.release_date, al.deezer_id
         FROM album_presaves p JOIN albums al ON al.id = p.album_id
         WHERE p.delivered_at IS NULL AND p.cancelled_at IS NULL
           AND (al.release_date IS NULL OR al.release_date <= $1)
         ORDER BY p.album_id, p.created_at",
        horizon,
    )
    .fetch_all(db)
    .await?;
    let mut albums: BTreeMap<Uuid, Pending> = BTreeMap::new();
    for r in rows {
        albums
            .entry(r.album_id)
            .or_insert(Pending {
                release_date: r.release_date,
                deezer_id: r.deezer_id,
                users: Vec::new(),
            })
            .users
            .push(r.user_id);
    }
    let mut report = Delivered::default();
    let mut offsets: HashMap<Uuid, i16> = HashMap::new();
    let mut conn = db.acquire().await?;
    for (album_id, pending) in albums {
        for u in &pending.users {
            if !offsets.contains_key(u) {
                let offset = notify::prefs(&mut conn, *u).await?.utc_offset_min;
                offsets.insert(*u, offset);
            }
        }
        let due = |date: Option<NaiveDate>| -> Vec<Uuid> {
            pending
                .users
                .iter()
                .copied()
                .filter(|u| date.is_none_or(|d| released(d, offsets[u], now)))
                .collect()
        };
        if due(pending.release_date).is_empty() {
            continue;
        }
        // Свежий альбом из Deezer: треки, перенос даты или отмена
        if let Some(deezer_id) = pending.deezer_id {
            match catalog::load_album(db, deezer, deezer_id).await {
                Ok(_) => {}
                Err(CatalogError::Deezer(DeezerError::NotFound)) => {
                    let gone = sqlx::query!(
                        "UPDATE album_presaves SET cancelled_at = now()
                         WHERE album_id = $1 AND delivered_at IS NULL AND cancelled_at IS NULL",
                        album_id
                    )
                    .execute(db)
                    .await?;
                    report.cancelled += gone.rows_affected() as usize;
                    tracing::info!(%album_id, "presaved release disappeared from Deezer");
                    continue;
                }
                Err(CatalogError::Db(e)) => return Err(e),
                Err(CatalogError::Deezer(e)) => {
                    tracing::warn!(%album_id, error = %e, "could not refresh a presaved release");
                    report.failed += 1;
                    continue;
                }
            }
        }
        let album = sqlx::query!(
            r#"SELECT al.release_date, al.title, al.record_type, al.cover_key, al.cover_source,
                      ar.name AS artist_name,
                      (SELECT count(*) FROM tracks t WHERE t.album_id = al.id) AS "tracks!"
               FROM albums al JOIN artists ar ON ar.id = al.artist_id WHERE al.id = $1"#,
            album_id
        )
        .fetch_one(db)
        .await?;
        let users = due(album.release_date);
        if users.is_empty() {
            report.postponed += 1;
            continue;
        }
        if album.tracks == 0 {
            report.waiting += 1;
            continue;
        }
        let record_type: RecordType = album.record_type.parse().unwrap_or(RecordType::Album);
        let what = match record_type {
            RecordType::Album => "Вышел альбом",
            RecordType::Ep => "Вышел EP",
            RecordType::Single => "Вышел сингл",
            RecordType::Compile => "Вышел сборник",
        };
        let image = image_set(
            ImageKind::Albums,
            album_id,
            album.cover_key.as_deref(),
            album.cover_source.as_deref(),
        )
        .map(|i| i.small);
        let link = format!("/album/{album_id}");
        let tracks = sqlx::query_scalar!(
            "SELECT id FROM tracks WHERE album_id = $1 ORDER BY disc_number, track_position NULLS LAST, id",
            album_id
        )
        .fetch_all(db)
        .await?;
        for user in users {
            let mut tx = db.begin().await?;
            let saved = sqlx::query!(
                "INSERT INTO saved_albums (user_id, album_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                user,
                album_id
            )
            .execute(&mut *tx)
            .await?;
            if saved.rows_affected() > 0 {
                realtime::publish(
                    &mut *tx,
                    Audience::User(user),
                    ServerEvent::LibraryChanged(LibraryChange {
                        entities: vec![SyncEntity::SavedAlbums],
                    }),
                )
                .await;
            }
            sqlx::query!(
                "UPDATE album_presaves SET delivered_at = now() WHERE user_id = $1 AND album_id = $2",
                user,
                album_id
            )
            .execute(&mut *tx)
            .await?;
            let draft = Draft::new(
                NotificationKind::Release,
                format!("{what} «{}»", album.title),
                format!("{} · уже в медиатеке", album.artist_name),
            )
            .link(link.clone())
            .image(image.clone())
            .item(NotificationItem {
                title: album.title.clone(),
                subtitle: album.artist_name.clone(),
                link: Some(link.clone()),
                image: image.clone(),
            })
            .group("release")
            // Та же метка, что у новинок подписок: об одном релизе — одно уведомление
            .once(format!("release:{album_id}"));
            notify::emit_in(&mut tx, user, draft).await?;
            tx.commit().await?;
            report.delivered += 1;
            for id in &tracks {
                let req = acquire::Request::new(*id, AcquireReason::Album, lanes::LIBRARY).by(user);
                if let Err(e) = acquire::request(db, deezer, req).await {
                    tracing::warn!(track = %id, error = %e, "could not queue a presaved track for download");
                }
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn release_day_follows_the_listener_time_zone() {
        // 12 декабря, 21:30 UTC: в Москве (UTC+3) уже 13-е, в Нью-Йорке (UTC−5) ещё 12-е
        let now = Utc.with_ymd_and_hms(2026, 12, 12, 21, 30, 0).unwrap();
        let release = d(2026, 12, 13);
        assert!(released(release, 180, now), "Москва: полночь прошла");
        assert!(!released(release, -300, now), "Нью-Йорк: ещё вчера");
        assert!(!released(release, 0, now), "UTC: ещё вчера");
        assert!(released(release, 840, now), "Киритимати, UTC+14");
    }

    #[test]
    fn release_day_boundaries() {
        let release = d(2026, 12, 11);
        // Ровно полночь по Москве
        let midnight = Utc.with_ymd_and_hms(2026, 12, 10, 21, 0, 0).unwrap();
        assert!(released(release, 180, midnight));
        assert!(!released(release, 180, midnight - Duration::seconds(1)));
        // Прошедшая дата — вышел; через год — нет
        assert!(released(d(2020, 1, 1), 0, midnight));
        assert!(!released(d(2027, 12, 11), 0, midnight));
        assert_eq!(local_today(-720, midnight), d(2026, 12, 10));
        assert_eq!(local_today(840, midnight), d(2026, 12, 11));
    }
}
