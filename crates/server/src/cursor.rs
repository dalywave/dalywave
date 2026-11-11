//! Курсоры списков: `nextCursor` — непрозрачная строка из времени (мкс) и id последней строки.
//! Постгрес хранит время с точностью до микросекунды, поэтому граница страницы точная.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::{AppError, AppResult, bad};

pub fn encode(at: DateTime<Utc>, id: Uuid) -> String {
    format!("{}_{}", at.timestamp_micros(), id)
}

pub fn decode(cursor: &str) -> AppResult<(DateTime<Utc>, Uuid)> {
    let invalid = || bad("неверный cursor");
    let (micros, id) = cursor.split_once('_').ok_or_else(invalid)?;
    let micros: i64 = micros.parse().map_err(|_| invalid())?;
    let at = DateTime::from_timestamp_micros(micros).ok_or_else(invalid)?;
    let id = id.parse().map_err(|_| invalid())?;
    Ok((at, id))
}

pub fn decode_opt(cursor: Option<&str>) -> AppResult<Option<(DateTime<Utc>, Uuid)>> {
    cursor.filter(|c| !c.is_empty()).map(decode).transpose()
}

/// `limit` из запроса: по умолчанию `default`, допустимо 1..=`max`.
pub fn limit(value: Option<u32>, default: u32, max: u32) -> AppResult<i64> {
    let limit = value.unwrap_or(default);
    if !(1..=max).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit должен быть от 1 до {max}"
        )));
    }
    Ok(i64::from(limit))
}

/// Обрезает лишнюю строку, запрошенную сверх `limit`, и возвращает курсор следующей страницы.
pub fn page<T>(
    rows: &mut Vec<T>,
    limit: i64,
    key: impl Fn(&T) -> (DateTime<Utc>, Uuid),
) -> Option<String> {
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    if rows.len() > limit {
        rows.truncate(limit);
        rows.last().map(|r| {
            let (at, id) = key(r);
            encode(at, id)
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let at = DateTime::from_timestamp_micros(1_794_358_445_123_456).unwrap();
        let id = Uuid::now_v7();
        assert_eq!(decode(&encode(at, id)).unwrap(), (at, id));
        assert!(decode("nope").is_err());
        assert!(decode("12_notuuid").is_err());
        assert!(decode_opt(None).unwrap().is_none());
        assert!(decode_opt(Some("")).unwrap().is_none());
        assert_eq!(limit(None, 50, 200).unwrap(), 50);
        assert!(limit(Some(0), 50, 200).is_err());
        assert!(limit(Some(201), 50, 200).is_err());
    }

    #[test]
    fn page_cuts_extra_row() {
        let at = Utc::now();
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::now_v7()).collect();
        let mut rows = ids.clone();
        let next = page(&mut rows, 2, |id| (at, *id));
        assert_eq!(rows.len(), 2);
        assert_eq!(next, Some(encode(at, ids[1])));
        let mut rows = ids.clone();
        assert_eq!(page(&mut rows, 3, |id| (at, *id)), None);
    }
}
