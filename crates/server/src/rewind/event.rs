//! «Итоги года» как событие сервера: открыты ли истории сейчас, за какой год и в каком стиле.
//! Режим «Автоматически» открывает итоги 1 декабря (за этот год) и закрывает после 15 января
//! (в январе — итоги прошлого года); «Открыто сейчас» — запуск вручную за выбранный год.
//! Даты — по часовому поясу владельца (`reco_settings.utc_offset_min`), как и сам год итогов.

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use dalywave_protocol::rewind::{RewindMode, RewindStyle, RewindStyleSetting};
use sqlx::PgExecutor;
use uuid::Uuid;

use super::local_date;

/// Последний день января, когда итоги прошлого года ещё открыты сами.
pub const AUTO_CLOSE_DAY: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub style: RewindStyleSetting,
    pub mode: RewindMode,
    /// Год для «Открыто сейчас».
    pub year: Option<i32>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Event {
    fn default() -> Self {
        Self {
            style: RewindStyleSetting::Random,
            mode: RewindMode::Auto,
            year: None,
            updated_at: DateTime::<Utc>::UNIX_EPOCH,
        }
    }
}

/// Год, который режим «Автоматически» держит открытым в этот день, или `None`.
pub fn auto_year(today: NaiveDate) -> Option<i32> {
    match today.month() {
        12 => Some(today.year()),
        1 if today.day() <= AUTO_CLOSE_DAY => Some(today.year() - 1),
        _ => None,
    }
}

impl Event {
    /// Какой год открыт в этот день (по времени владельца); `None` — закрыто.
    pub fn open_year(&self, today: NaiveDate) -> Option<i32> {
        match self.mode {
            RewindMode::Auto => auto_year(today),
            RewindMode::Open => Some(self.year.unwrap_or(today.year()).min(today.year())),
            RewindMode::Closed => None,
        }
    }

    /// Ближайшее автоматическое открытие: 1 декабря в полночь. Только в режиме «Автоматически», пока закрыто.
    pub fn opens_at(&self, now: DateTime<Utc>, utc_offset_min: i16) -> Option<DateTime<Utc>> {
        let today = local_date(now, utc_offset_min);
        if self.mode != RewindMode::Auto || self.open_year(today).is_some() {
            return None;
        }
        local_midnight(
            NaiveDate::from_ymd_opt(today.year(), 12, 1)?,
            utc_offset_min,
        )
    }

    /// Когда закроется само: 16 января в полночь. Только в режиме «Автоматически», пока открыто.
    pub fn closes_at(&self, now: DateTime<Utc>, utc_offset_min: i16) -> Option<DateTime<Utc>> {
        if self.mode != RewindMode::Auto {
            return None;
        }
        let year = self.open_year(local_date(now, utc_offset_min))?;
        local_midnight(
            NaiveDate::from_ymd_opt(year + 1, 1, AUTO_CLOSE_DAY + 1)?,
            utc_offset_min,
        )
    }
}

fn local_midnight(day: NaiveDate, utc_offset_min: i16) -> Option<DateTime<Utc>> {
    Some(
        Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0)?)
            - Duration::minutes(i64::from(utc_offset_min)),
    )
}

/// Стиль историй слушателя. «Случайный» — свой у каждого, но постоянный в пределах года:
/// открыл истории второй раз — тот же стиль.
pub fn style_for(setting: RewindStyleSetting, user: Uuid, year: i32) -> RewindStyle {
    match setting {
        RewindStyleSetting::Wave => RewindStyle::Wave,
        RewindStyleSetting::Spread => RewindStyle::Spread,
        RewindStyleSetting::Record => RewindStyle::Record,
        RewindStyleSetting::Random => {
            // FNV-1a: стабильно между запусками и версиями Rust, в отличие от DefaultHasher
            let mut h: u32 = 2_166_136_261;
            for b in user.as_bytes().iter().chain(year.to_le_bytes().iter()) {
                h = (h ^ u32::from(*b)).wrapping_mul(16_777_619);
            }
            [RewindStyle::Wave, RewindStyle::Spread, RewindStyle::Record][(h % 3) as usize]
        }
    }
}

pub fn style_setting_str(s: RewindStyleSetting) -> &'static str {
    match s {
        RewindStyleSetting::Wave => "wave",
        RewindStyleSetting::Spread => "spread",
        RewindStyleSetting::Record => "record",
        RewindStyleSetting::Random => "random",
    }
}

pub fn mode_str(m: RewindMode) -> &'static str {
    match m {
        RewindMode::Auto => "auto",
        RewindMode::Open => "open",
        RewindMode::Closed => "closed",
    }
}

fn parse_style(s: &str) -> RewindStyleSetting {
    match s {
        "wave" => RewindStyleSetting::Wave,
        "spread" => RewindStyleSetting::Spread,
        "record" => RewindStyleSetting::Record,
        _ => RewindStyleSetting::Random,
    }
}

fn parse_mode(s: &str) -> RewindMode {
    match s {
        "open" => RewindMode::Open,
        "closed" => RewindMode::Closed,
        _ => RewindMode::Auto,
    }
}

pub async fn load(db: impl PgExecutor<'_>) -> Result<Event, sqlx::Error> {
    let row = sqlx::query!("SELECT style, mode, year, updated_at FROM rewind_event WHERE id")
        .fetch_optional(db)
        .await?;
    Ok(row.map_or_else(Event::default, |r| Event {
        style: parse_style(&r.style),
        mode: parse_mode(&r.mode),
        year: r.year,
        updated_at: r.updated_at,
    }))
}

/// Слушатель уже открывал истории этого года.
pub async fn seen(db: impl PgExecutor<'_>, user: Uuid, year: i32) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM rewind_seen WHERE user_id = $1 AND year = $2) AS "seen!""#,
        user,
        year
    )
    .fetch_one(db)
    .await
}

pub async fn mark_seen(db: impl PgExecutor<'_>, user: Uuid, year: i32) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO rewind_seen (user_id, year) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user,
        year
    )
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn auto_opens_in_december_and_closes_after_january_15() {
        assert_eq!(auto_year(day(2026, 11, 30)), None);
        assert_eq!(auto_year(day(2026, 12, 1)), Some(2026));
        assert_eq!(auto_year(day(2026, 12, 31)), Some(2026));
        assert_eq!(auto_year(day(2027, 1, 1)), Some(2026));
        assert_eq!(auto_year(day(2027, 1, 15)), Some(2026));
        assert_eq!(auto_year(day(2027, 1, 16)), None);
        assert_eq!(auto_year(day(2027, 6, 1)), None);
    }

    #[test]
    fn modes() {
        let ev = |mode, year| Event {
            mode,
            year,
            ..Event::default()
        };
        let nov = day(2026, 11, 12);
        assert_eq!(ev(RewindMode::Auto, None).open_year(nov), None);
        assert_eq!(ev(RewindMode::Open, Some(2024)).open_year(nov), Some(2024));
        // Год из будущего не открывается
        assert_eq!(ev(RewindMode::Open, Some(2030)).open_year(nov), Some(2026));
        assert_eq!(ev(RewindMode::Open, None).open_year(nov), Some(2026));
        assert_eq!(
            ev(RewindMode::Closed, None).open_year(day(2026, 12, 5)),
            None
        );
    }

    #[test]
    fn opening_and_closing_moments_follow_owner_time() {
        let auto = Event::default();
        // UTC+3: 1 декабря 00:00 по Москве — 30 ноября 21:00 UTC
        let now = Utc.with_ymd_and_hms(2026, 11, 12, 10, 0, 0).unwrap();
        assert_eq!(
            auto.opens_at(now, 180),
            Some(Utc.with_ymd_and_hms(2026, 11, 30, 21, 0, 0).unwrap())
        );
        assert_eq!(auto.closes_at(now, 180), None);
        let dec = Utc.with_ymd_and_hms(2026, 12, 10, 10, 0, 0).unwrap();
        assert_eq!(auto.opens_at(dec, 180), None);
        assert_eq!(
            auto.closes_at(dec, 180),
            Some(Utc.with_ymd_and_hms(2027, 1, 15, 21, 0, 0).unwrap())
        );
        let manual = Event {
            mode: RewindMode::Open,
            year: Some(2025),
            ..Event::default()
        };
        assert_eq!(manual.opens_at(now, 0), None);
        assert_eq!(manual.closes_at(now, 0), None);
    }

    #[test]
    fn random_style_is_stable_per_listener_and_year() {
        let a = Uuid::from_u128(1);
        let s = style_for(RewindStyleSetting::Random, a, 2026);
        assert_eq!(style_for(RewindStyleSetting::Random, a, 2026), s);
        // Разные слушатели получают разные стили
        let styles: std::collections::HashSet<_> = (0..60u128)
            .map(|i| style_for(RewindStyleSetting::Random, Uuid::from_u128(i * 7919), 2026))
            .collect();
        assert_eq!(styles.len(), 3);
        assert_eq!(
            style_for(RewindStyleSetting::Spread, a, 2026),
            RewindStyle::Spread
        );
    }
}
