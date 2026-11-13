//! Настройки клипов из админки (`server_settings.clip_*`): качество, кусок или клип целиком
//! (в такт с треком), сроки хранения, потолок места и автопоиск. Потолок можно задать и переменной
//! `DALYWAVE_CLIPS_MAX_GB` — тогда в админке он только для чтения.

use dalywave_protocol::admin_settings::{CLIP_HEIGHTS, ClipSettings, ClipSettingsPatch};
use sqlx::{PgConnection, PgExecutor};

use crate::error::{AppResult, bad};

const GB: i64 = 1024 * 1024 * 1024;

/// Средний битрейт клипа без звука по высоте кадра, кбит/с: оценка размера, пока клипов нет.
fn kbps(height: i32) -> i64 {
    match height {
        ..=360 => 500,
        361..=480 => 800,
        481..=720 => 1_600,
        _ => 3_000,
    }
}

/// Средняя длина клипа целиком, с (для оценки размера).
const WHOLE_TYPICAL_S: i64 = 220;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub height: i32,
    pub whole: bool,
    pub sync: bool,
    pub length_ms: i32,
    pub keep_days: i32,
    pub keep_liked_days: i32,
    pub max_bytes: i64,
    pub auto: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            height: 480,
            whole: false,
            sync: true,
            length_ms: 12_000,
            keep_days: 7,
            keep_liked_days: 60,
            max_bytes: 3 * GB,
            auto: true,
        }
    }
}

/// Потолок из `DALYWAVE_CLIPS_MAX_GB`, если задан.
pub fn env_max_gb() -> Option<i32> {
    std::env::var("DALYWAVE_CLIPS_MAX_GB")
        .ok()
        .and_then(|v| v.trim().parse::<i32>().ok())
        .filter(|v| *v > 0)
}

struct Row {
    clip_height: i32,
    clip_whole: bool,
    clip_sync: bool,
    clip_length_s: i32,
    clip_keep_days: i32,
    clip_keep_liked_days: i32,
    clip_max_gb: i32,
    clip_auto: bool,
}

async fn row(db: impl PgExecutor<'_>) -> Result<Row, sqlx::Error> {
    sqlx::query_as!(
        Row,
        "SELECT clip_height, clip_whole, clip_sync, clip_length_s, clip_keep_days, clip_keep_liked_days,
                clip_max_gb, clip_auto
         FROM server_settings WHERE id"
    )
    .fetch_one(db)
    .await
}

pub async fn load(db: impl PgExecutor<'_>) -> Result<Settings, sqlx::Error> {
    let r = row(db).await?;
    let gb = env_max_gb().unwrap_or(r.clip_max_gb);
    Ok(Settings {
        height: r.clip_height,
        whole: r.clip_whole,
        sync: r.clip_sync,
        length_ms: r.clip_length_s * 1000,
        keep_days: r.clip_keep_days,
        keep_liked_days: r.clip_keep_liked_days,
        max_bytes: i64::from(gb) * GB,
        auto: r.clip_auto,
    })
}

/// Вид для админки: настройки, сколько занято и средний размер клипа при нынешнем качестве.
pub async fn view(conn: &mut PgConnection) -> Result<ClipSettings, sqlx::Error> {
    let r = row(&mut *conn).await?;
    let stored = sqlx::query!(
        r#"SELECT count(*) AS "n!", COALESCE(sum(size_bytes), 0)::bigint AS "bytes!",
                  (SELECT avg(size_bytes)::bigint FROM track_clips
                   WHERE state = 'ready' AND height = $1 AND whole = $2) AS avg
           FROM track_clips WHERE state = 'ready'"#,
        r.clip_height,
        r.clip_whole,
    )
    .fetch_one(&mut *conn)
    .await?;
    let seconds = if r.clip_whole {
        WHOLE_TYPICAL_S
    } else {
        i64::from(r.clip_length_s)
    };
    let estimate = kbps(r.clip_height) * 1000 / 8 * seconds;
    let env = env_max_gb();
    Ok(ClipSettings {
        height: r.clip_height,
        whole: r.clip_whole,
        sync: r.clip_sync,
        length_s: r.clip_length_s,
        keep_days: r.clip_keep_days,
        keep_liked_days: r.clip_keep_liked_days,
        max_gb: env.unwrap_or(r.clip_max_gb),
        max_gb_from_env: env.is_some(),
        auto_search: r.clip_auto,
        stored: stored.n,
        stored_bytes: stored.bytes,
        avg_bytes: stored.avg.filter(|a| *a > 0).unwrap_or(estimate),
    })
}

/// Проверяет и сохраняет изменения. Потолок, заданный переменной окружения, не меняется.
pub async fn update(conn: &mut PgConnection, p: &ClipSettingsPatch) -> AppResult<()> {
    if p.height.is_some_and(|h| !CLIP_HEIGHTS.contains(&h)) {
        return Err(bad("высота клипа — 360, 480, 720 или 1080"));
    }
    if p.length_s.is_some_and(|l| !(6..=30).contains(&l)) {
        return Err(bad("длина куска — от 6 до 30 секунд"));
    }
    if p.keep_days.is_some_and(|d| !(1..=365).contains(&d)) {
        return Err(bad("срок хранения — от 1 до 365 дней"));
    }
    if p.keep_liked_days.is_some_and(|d| !(1..=730).contains(&d)) {
        return Err(bad("срок хранения любимых — от 1 до 730 дней"));
    }
    if p.max_gb.is_some_and(|g| !(1..=2000).contains(&g)) {
        return Err(bad("потолок места — от 1 до 2000 ГБ"));
    }
    if p.max_gb.is_some() && env_max_gb().is_some() {
        return Err(bad(
            "потолок задан переменной DALYWAVE_CLIPS_MAX_GB — меняется на сервере",
        ));
    }
    sqlx::query!(
        "UPDATE server_settings SET
             clip_height = COALESCE($1, clip_height), clip_whole = COALESCE($2, clip_whole),
             clip_sync = COALESCE($3, clip_sync), clip_length_s = COALESCE($4, clip_length_s),
             clip_keep_days = COALESCE($5, clip_keep_days),
             clip_keep_liked_days = COALESCE($6, clip_keep_liked_days),
             clip_max_gb = COALESCE($7, clip_max_gb), clip_auto = COALESCE($8, clip_auto),
             updated_at = now()
         WHERE id",
        p.height,
        p.whole,
        p.sync,
        p.length_s,
        p.keep_days,
        p.keep_liked_days,
        p.max_gb,
        p.auto_search,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_grows_with_height() {
        assert!(kbps(360) < kbps(480));
        assert!(kbps(720) < kbps(1080));
    }
}
