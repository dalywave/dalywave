//! Поводы для уведомлений. Только то, что правда полезно узнать вне приложения:
//! - новый релиз артиста из подписок (раз на релиз; несколько за вечер — одно уведомление);
//! - перенос из другого сервиса закончился (сколько перенесено, сколько проверить);
//! - «Итоги года» открыты (раз в год, вечером);
//! - администратору: спорные треки ждут решения (не чаще раза в день), YouTube перестал отдавать
//!   звук (повторяется несколько раз за полчаса; не чаще раза в день).

use chrono::{DateTime, Duration, Timelike, Utc};
use dalywave_protocol::{
    catalog::RecordType,
    library::DownloadPolicy,
    notifications::{NotificationItem, NotificationKind},
};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Draft, emit, plural};
use crate::catalog::views::{ImageKind, image_set};

/// Релиз считается новым две недели после выхода.
const FRESH_DAYS: i64 = 14;
/// Сбои YouTube, которые значат «звук не отдают» (а не «эту запись не нашли»).
pub const BLOCKED_CODES: [&str; 4] = ["bot_check", "po_token", "http_403", "cookies_missing"];
/// Столько таких сбоев за полчаса — уже не случайность.
const BLOCKED_THRESHOLD: i64 = 3;
/// Итоги года — вечером по времени сервиса.
const REWIND_HOURS: std::ops::Range<u32> = 18..21;

/// Новые релизы артиста — его подписчикам (у кого уведомления по артисту включены), по их же правилам
/// докачки: синглы не нужны — и уведомления о синглах нет. Звать после сверки дискографии.
pub async fn new_releases(db: &PgPool, artist_id: Uuid) -> Result<usize, sqlx::Error> {
    let today = Utc::now().date_naive();
    let rows = sqlx::query!(
        r#"SELECT f.user_id, f.download_policy, al.id AS album_id, al.title, al.record_type,
                  al.artist_id AS main_artist, al.cover_key, al.cover_source, ar.name AS artist_name,
                  (SELECT count(*) FROM tracks t JOIN recordings r ON r.id = t.recording_id
                   WHERE t.album_id = al.id AND r.audio_status <> 'ready') AS "missing!",
                  (SELECT count(*) FROM tracks t WHERE t.album_id = al.id) AS "tracks!"
           FROM followed_artists f
           JOIN users u ON u.id = f.user_id AND NOT u.disabled
           JOIN artist_releases rel ON rel.artist_id = f.artist_id
           JOIN albums al ON al.id = rel.album_id
           JOIN artists ar ON ar.id = f.artist_id
           WHERE f.artist_id = $1 AND f.notify
             AND al.release_date IS NOT NULL AND al.release_date <= $2 AND al.release_date >= $3
             -- Вышедшее до подписки — не новость
             AND al.release_date >= (f.created_at AT TIME ZONE 'UTC')::date
           ORDER BY al.release_date, al.id"#,
        artist_id,
        today,
        today - Duration::days(FRESH_DAYS),
    )
    .fetch_all(db)
    .await?;
    let mut sent = 0;
    for r in rows {
        let policy: DownloadPolicy = serde_json::from_value(r.download_policy).unwrap_or_default();
        let record_type: RecordType = r.record_type.parse().unwrap_or(RecordType::Album);
        if !crate::catalog::wanted(&policy, record_type, &r.title, r.main_artist == artist_id) {
            continue;
        }
        // Треков ещё нет в каталоге — значит, и сказать пока нечего
        if r.tracks == 0 {
            continue;
        }
        let what = match record_type {
            RecordType::Album => "Новый альбом",
            RecordType::Ep => "Новый EP",
            RecordType::Single => "Новый сингл",
            RecordType::Compile => "Новый сборник",
        };
        let ready = if r.missing == 0 {
            "можно слушать"
        } else {
            "скоро можно слушать"
        };
        let image = image_set(
            ImageKind::Albums,
            r.album_id,
            r.cover_key.as_deref(),
            r.cover_source.as_deref(),
        )
        .map(|i| i.small);
        let link = format!("/album/{}", r.album_id);
        let draft = Draft::new(
            NotificationKind::Release,
            format!("{} — {}", r.artist_name, r.title),
            format!("{what} · {ready}"),
        )
        .link(link.clone())
        .image(image.clone())
        .item(NotificationItem {
            title: r.title.clone(),
            subtitle: r.artist_name.clone(),
            link: Some(link),
            image,
        })
        .group("release")
        .once(format!("release:{}", r.album_id));
        if emit(db, r.user_id, draft).await?.is_some() {
            sent += 1;
        }
    }
    Ok(sent)
}

/// Перенос закончился: сколько перенесено и сколько не нашлось.
pub async fn import_done(db: &PgPool, import_id: Uuid) -> Result<(), sqlx::Error> {
    let Some(r) = sqlx::query!(
        r#"SELECT i.user_id, i.total, i.matched,
                  (SELECT count(*) FROM import_items WHERE import_id = i.id AND status = 'unmatched') AS "unmatched!"
           FROM imports i WHERE i.id = $1 AND i.status = 'done'"#,
        import_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(());
    };
    if r.total == 0 {
        return Ok(());
    }
    let mut body = format!("Перенесено {} из {}", r.matched, r.total);
    if r.unmatched > 0 {
        let n = i32::try_from(r.unmatched).unwrap_or(i32::MAX);
        body.push_str(&format!(
            " · {} {}",
            n,
            plural(n, ["ждёт проверки", "ждут проверки", "ждут проверки"])
        ));
    }
    let draft = Draft::new(NotificationKind::Import, "Перенос завершён", body)
        .link("/import")
        .once(format!("import:{import_id}"));
    emit(db, r.user_id, draft).await?;
    Ok(())
}

/// «Итоги года» открыты: тем, кто слушал в этом году и ещё не смотрел, — один раз, вечером.
/// Из обслуживания воркера раз в час.
pub async fn rewind_ready(db: &PgPool, now: DateTime<Utc>) -> Result<usize, sqlx::Error> {
    let off = crate::reco::Settings::load(db).await?.utc_offset_min;
    let local = now + Duration::minutes(i64::from(off));
    if !REWIND_HOURS.contains(&local.hour()) {
        return Ok(0);
    }
    let event = crate::rewind::event::load(db).await?;
    let Some(year) = event.open_year(crate::rewind::local_date(now, off)) else {
        return Ok(0);
    };
    let (since, until) = crate::rewind::range(year, now, off);
    let users = sqlx::query_scalar!(
        "SELECT u.id FROM users u
         WHERE NOT u.disabled AND u.roles && ARRAY['listener', 'admin']
           AND EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = u.id AND p.started_at >= $1 AND p.started_at < $2)
           AND NOT EXISTS (SELECT 1 FROM rewind_seen s WHERE s.user_id = u.id AND s.year = $3)
           AND NOT EXISTS (SELECT 1 FROM notification_marks m WHERE m.user_id = u.id AND m.key = $4)",
        since,
        until,
        year,
        format!("rewind:{year}"),
    )
    .fetch_all(db)
    .await?;
    let mut sent = 0;
    for user in users {
        let draft = Draft::new(
            NotificationKind::Rewind,
            format!("Итоги {year} готовы"),
            "Минуты, артист и трек года — всё в одной истории",
        )
        .link(format!("/rewind?year={year}"))
        .once(format!("rewind:{year}"));
        if emit(db, user, draft).await?.is_some() {
            sent += 1;
        }
    }
    Ok(sent)
}

async fn admins(db: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!("SELECT id FROM users WHERE 'admin' = ANY(roles) AND NOT disabled")
        .fetch_all(db)
        .await
}

/// Запись ушла в «Спорные». Уведомление соберётся за 15 минут, текст — по тому, что ждёт к рассылке.
pub async fn disputed(db: &PgPool) -> Result<(), sqlx::Error> {
    let day = Utc::now().format("%Y-%m-%d");
    for admin in admins(db).await? {
        let draft = Draft::new(NotificationKind::Review, "Спорные треки ждут решения", "")
            .link("/admin/disputed")
            .group("review")
            .once(format!("review:{day}"));
        emit(db, admin, draft).await?;
    }
    Ok(())
}

/// Заголовок и текст «спорных» к рассылке; `None` — решать уже нечего.
pub async fn review_text(db: &mut PgConnection) -> Result<Option<(String, String)>, sqlx::Error> {
    let n = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM acquisitions WHERE status = 'disputed'"#
    )
    .fetch_one(db)
    .await?;
    if n == 0 {
        return Ok(None);
    }
    let n = i32::try_from(n).unwrap_or(i32::MAX);
    Ok(Some((
        format!(
            "{n} {}",
            plural(
                n,
                [
                    "спорный трек ждёт решения",
                    "спорных трека ждут решения",
                    "спорных треков ждут решения",
                ]
            )
        ),
        "Выберите запись или отметьте, что её нет".into(),
    )))
}

/// Скачивание упёрлось в YouTube (проверка на бота, нет cookies, 403). Если за полчаса так уже
/// несколько раз — администраторам, не чаще раза в день.
pub async fn source_blocked(db: &PgPool, code: &str) -> Result<(), sqlx::Error> {
    if !BLOCKED_CODES.contains(&code) {
        return Ok(());
    }
    let recent = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM acquisition_attempts
           WHERE outcome = 'postponed' AND error_code = ANY($1)
             AND created_at > now() - interval '30 minutes'"#,
        &BLOCKED_CODES.map(String::from) as &[String],
    )
    .fetch_one(db)
    .await?;
    if recent < BLOCKED_THRESHOLD {
        return Ok(());
    }
    let day = Utc::now().format("%Y-%m-%d");
    for admin in admins(db).await? {
        let draft = Draft::new(
            NotificationKind::Sources,
            "YouTube перестал отдавать звук",
            "Скачивание стоит — загляните в «Источники»",
        )
        .link("/admin/sources")
        .once(format!("sources:{day}"));
        emit(db, admin, draft).await?;
    }
    Ok(())
}
