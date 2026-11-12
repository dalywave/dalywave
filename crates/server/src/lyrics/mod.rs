//! Тексты песен: LRCLIB → кеш в `track_lyrics` → `GET /api/tracks/{id}/lyrics`.
//!
//! Текст берётся при первом запросе или заранее, задачей полосы «Тексты» для ближайших треков очереди
//! (их присылает активное устройство снимком). Запись LRCLIB принимается, только если её длительность
//! расходится с треком не больше чем на 3 с, а название и артист похожи: иначе под песню попадёт
//! текст ремикса или кавера. Синхронный текст (LRC) лучше простого. «Не нашли» живёт 30 дней.

pub mod lrc;
pub mod lrclib;

use std::sync::Arc;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dalywave_protocol::{
    jobs::{LyricsPayload, kind},
    lyrics::{LyricsKind, LyricsLine, TrackLyrics},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use self::lrclib::{Lrclib, LrclibError, Record, Wanted};
use crate::jobs::{
    self, JobRow, NewJob, lanes,
    worker::{Context, JobError, JobResult},
};

/// Допуск по длительности.
const DURATION_SLACK_MS: f64 = 3000.0;
/// «Не нашли» — столько не спрашиваем снова.
const NOT_FOUND_DAYS: i64 = 30;
/// Сбой LRCLIB после всех попыток задачи — пауза поменьше.
const FAILED_HOURS: i64 = 6;
/// Сколько ближайших треков очереди готовить заранее.
pub const PREFETCH: usize = 4;

/// Строка `lines` в базе.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredLine {
    t_ms: u32,
    text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("нет такого трека")]
    NoTrack,
    #[error(transparent)]
    Source(#[from] LrclibError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Нормализация для сравнения названий: без регистра и диакритики, «ё» = «е», без скобок
/// («(feat. …)», «(Remastered)»), без хвоста « - …» и знаков.
pub fn norm(s: &str) -> String {
    let mut depth = 0u32;
    let mut cut = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => cut.push(c),
            _ => {}
        }
    }
    let mut cut = cut.split(" - ").next().unwrap_or_default().to_lowercase();
    if let Some(i) = [" feat.", " feat ", " ft.", " featuring "]
        .iter()
        .filter_map(|p| cut.find(p))
        .min()
    {
        cut.truncate(i);
    }
    loose(&cut)
}

/// Без регистра, диакритики и знаков; «ё» = «е». Для строк текста и запроса поиска.
pub fn loose(s: &str) -> String {
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn similar(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || (a.len().min(b.len()) >= 3 && (a.contains(b) || b.contains(a)))
}

/// Подходит ли запись: длительность ±3 с, название и хотя бы один артист похожи.
fn fits(r: &Record, w: &Wanted) -> bool {
    let Some(d) = r.duration else {
        return false;
    };
    if (d * 1000.0 - f64::from(w.duration_ms)).abs() > DURATION_SLACK_MS {
        return false;
    }
    if !similar(&norm(&r.track_name), &norm(&w.title)) {
        return false;
    }
    let artist = loose(&r.artist_name);
    w.artists.iter().any(|a| similar(&artist, &loose(a)))
}

/// Синхронный текст лучше простого, простой — лучше пометки «инструментал».
fn rank(r: &Record) -> u8 {
    if r.synced().is_some() {
        3
    } else if r.plain().is_some() {
        2
    } else if r.instrumental {
        1
    } else {
        0
    }
}

/// Лучшая подходящая запись: по виду текста, затем по близости длительности.
pub fn pick(records: Vec<Record>, w: &Wanted) -> Option<Record> {
    let target = f64::from(w.duration_ms) / 1000.0;
    records
        .into_iter()
        .filter(|r| rank(r) > 0 && fits(r, w))
        .min_by(|a, b| {
            rank(b).cmp(&rank(a)).then_with(|| {
                let da = (a.duration.unwrap_or(0.0) - target).abs();
                let db = (b.duration.unwrap_or(0.0) - target).abs();
                da.total_cmp(&db)
            })
        })
}

/// Что искать для трека: название, основные артисты, альбом, длительность.
async fn wanted(db: &PgPool, track_id: Uuid) -> Result<Option<Wanted>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT t.title, t.duration_ms, al.title AS "album?",
                  COALESCE(ARRAY(SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                                  WHERE ta.track_id = t.id
                                  ORDER BY (ta.role = 'main') DESC, ta.position), '{}') AS "artists!"
             FROM tracks t
             LEFT JOIN albums al ON al.id = t.album_id
            WHERE t.id = $1"#,
        track_id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| Wanted {
        title: r.title,
        artists: r.artists,
        album: r.album,
        duration_ms: r.duration_ms,
    }))
}

/// Спрашивает LRCLIB: сначала точное совпадение, затем поиск.
async fn fetch(client: &Lrclib, w: &Wanted) -> Result<Option<Record>, LrclibError> {
    if w.artists.is_empty() {
        return Ok(None);
    }
    if let Some(r) = client.get(w).await?
        && rank(&r) > 0
        && fits(&r, w)
    {
        return Ok(Some(r));
    }
    Ok(pick(client.search(w).await?, w))
}

/// Кешированная строка, если она ещё в силе.
struct Cached {
    synced: bool,
    instrumental: bool,
    lines: Option<serde_json::Value>,
    plain: Option<String>,
    source: String,
    not_found_until: Option<DateTime<Utc>>,
}

impl Cached {
    fn fresh(&self, now: DateTime<Utc>) -> bool {
        self.not_found_until.is_none_or(|t| t > now)
    }

    fn view(self, track_id: Uuid) -> TrackLyrics {
        let lines: Vec<LyricsLine> = if self.synced {
            self.lines
                .and_then(|v| serde_json::from_value::<Vec<StoredLine>>(v).ok())
                .unwrap_or_default()
                .into_iter()
                .map(|l| LyricsLine {
                    t_ms: l.t_ms,
                    text: l.text,
                })
                .collect()
        } else {
            Vec::new()
        };
        let kind = if self.instrumental {
            LyricsKind::Instrumental
        } else if !lines.is_empty() {
            LyricsKind::Synced
        } else if self.plain.is_some() {
            LyricsKind::Plain
        } else {
            LyricsKind::None
        };
        let found = !matches!(kind, LyricsKind::None);
        TrackLyrics {
            track_id,
            kind,
            lines,
            plain: if self.instrumental { None } else { self.plain },
            source: found.then_some(self.source),
        }
    }
}

async fn cached(db: &PgPool, track_id: Uuid) -> Result<Option<Cached>, sqlx::Error> {
    sqlx::query_as!(
        Cached,
        "SELECT synced, instrumental, lines, plain, source, not_found_until
           FROM track_lyrics WHERE track_id = $1",
        track_id
    )
    .fetch_optional(db)
    .await
}

/// Записывает найденное (или «не нашли» до `retry_at`).
async fn store(
    db: &PgPool,
    track_id: Uuid,
    record: Option<&Record>,
    retry_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let (synced, instrumental, lines, plain, source_id, not_found_until) = match record {
        Some(r) => {
            let parsed = r.synced().map(lrc::parse).unwrap_or_default();
            let plain = r
                .plain()
                .map(|p| p.replace("\r\n", "\n").trim().to_string());
            // Текст без слов: строки не храним (бывает, что у инструментала лежит «♪»)
            let instrumental = r.instrumental && parsed.is_empty() && plain.is_none();
            let plain = plain.or_else(|| {
                (!parsed.is_empty()).then(|| {
                    parsed
                        .iter()
                        .filter(|l| !l.text.is_empty())
                        .map(|l| l.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            });
            let synced = !parsed.is_empty();
            let lines = synced.then(|| {
                json!(
                    parsed
                        .iter()
                        .map(|l| StoredLine {
                            t_ms: l.t_ms,
                            text: l.text.clone(),
                        })
                        .collect::<Vec<_>>()
                )
            });
            let empty = !synced && plain.is_none() && !instrumental;
            (
                synced,
                instrumental,
                lines,
                if instrumental { None } else { plain },
                Some(r.id),
                empty.then_some(retry_at),
            )
        }
        None => (false, false, None, None, None, Some(retry_at)),
    };
    sqlx::query!(
        "INSERT INTO track_lyrics (track_id, synced, instrumental, lines, plain, source, source_id, fetched_at, not_found_until)
         VALUES ($1, $2, $3, $4, $5, 'lrclib', $6, now(), $7)
         ON CONFLICT (track_id) DO UPDATE SET synced = EXCLUDED.synced, instrumental = EXCLUDED.instrumental,
             lines = EXCLUDED.lines, plain = EXCLUDED.plain, source = EXCLUDED.source,
             source_id = EXCLUDED.source_id, fetched_at = now(), not_found_until = EXCLUDED.not_found_until",
        track_id,
        synced,
        instrumental,
        lines,
        plain,
        source_id,
        not_found_until,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Текст трека: из кеша, а если его нет или «не нашли» устарело — у LRCLIB.
pub async fn resolve(
    db: &PgPool,
    client: &Lrclib,
    track_id: Uuid,
) -> Result<TrackLyrics, LyricsError> {
    let now = Utc::now();
    if let Some(c) = cached(db, track_id).await?
        && c.fresh(now)
    {
        return Ok(c.view(track_id));
    }
    let w = wanted(db, track_id).await?.ok_or(LyricsError::NoTrack)?;
    let record = fetch(client, &w).await?;
    store(
        db,
        track_id,
        record.as_ref(),
        now + ChronoDuration::days(NOT_FOUND_DAYS),
    )
    .await?;
    let c = cached(db, track_id).await?.ok_or(LyricsError::NoTrack)?;
    Ok(c.view(track_id))
}

/// Ставит в полосу «Тексты» треки без текста в кеше (или с устаревшим «не нашли»). Порядок — порядок очереди.
pub async fn prefetch(db: &PgPool, track_ids: &[Uuid]) -> Result<usize, sqlx::Error> {
    if track_ids.is_empty() {
        return Ok(0);
    }
    let ids: Vec<Uuid> = sqlx::query_scalar!(
        r#"SELECT t.id AS "id!"
             FROM unnest($1::uuid[]) WITH ORDINALITY AS q(id, n)
             JOIN tracks t ON t.id = q.id
             LEFT JOIN track_lyrics l ON l.track_id = t.id
            WHERE l.track_id IS NULL OR l.not_found_until < now()
            ORDER BY q.n"#,
        track_ids,
    )
    .fetch_all(db)
    .await?;
    if ids.is_empty() {
        return Ok(0);
    }
    let mut conn = db.acquire().await?;
    let mut created = 0;
    for (i, id) in ids.iter().enumerate() {
        let job = NewJob::new(kind::LYRICS, lanes::LYRICS, LyricsPayload { track_id: *id })
            .dedupe(format!("lyrics:{id}"))
            // Ближайший трек — первым
            .priority(-(i as i16))
            .max_attempts(3);
        if jobs::enqueue(&mut conn, job).await?.created {
            created += 1;
        }
    }
    Ok(created)
}

/// Задача `lyrics`: текст трека заранее.
pub async fn run(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: LyricsPayload = job.payload()?;
    match resolve(&ctx.db, &ctx.lrclib, p.track_id).await {
        Ok(l) => Ok(json!({ "kind": l.kind.as_str(), "lines": l.lines.len() }).into()),
        Err(LyricsError::NoTrack) => Err(JobError::permanent("нет такого трека")),
        Err(LyricsError::Db(e)) => Err(e.into()),
        Err(LyricsError::Source(e)) => {
            if let LrclibError::RateLimited(wait) = &e {
                ctx.cooldowns.pause(kind::LYRICS, *wait);
            }
            // Последняя попытка: «не нашли» на несколько часов, чтобы следующий снимок очереди не ставил трек снова
            if job.attempts >= job.max_attempts {
                store(
                    &ctx.db,
                    p.track_id,
                    None,
                    Utc::now() + ChronoDuration::hours(FAILED_HOURS),
                )
                .await?;
            }
            Err(JobError::retry(e))
        }
    }
}

/// Строка текста, где нашлась строчка; иначе — первая.
pub fn matching_line(plain: &str, q: &str) -> String {
    let needle = loose(q);
    let lines = plain.lines().map(str::trim).filter(|l| !l.is_empty());
    lines
        .clone()
        .find(|l| loose(l).contains(&needle))
        .or_else(|| lines.clone().next())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(title: &str, artist: &str, dur: f64, synced: bool, plain: bool) -> Record {
        Record {
            id: 1,
            track_name: title.into(),
            artist_name: artist.into(),
            album_name: None,
            duration: Some(dur),
            instrumental: false,
            plain_lyrics: plain.then(|| "la".into()),
            synced_lyrics: synced.then(|| "[00:01.00]la".into()),
        }
    }

    fn want() -> Wanted {
        Wanted {
            title: "Light Me Up".into(),
            artists: vec!["Lowborn".into()],
            album: None,
            duration_ms: 193_000,
        }
    }

    #[test]
    fn normalizes_titles() {
        assert_eq!(norm("Light Me Up (feat. VISTA)"), "light me up");
        assert_eq!(norm("Ёлка — Remastered 2011"), "елка remastered 2011");
        assert_eq!(norm("Song - Radio Edit"), "song");
        assert_eq!(norm("Beyoncé feat. Jay-Z"), "beyonce");
        assert_eq!(loose("Ça va, ÉTÉ!"), "ca va ete");
    }

    #[test]
    fn picks_synced_within_three_seconds() {
        let w = want();
        let got = pick(
            vec![
                rec("Light Me Up", "Lowborn", 193.0, false, true),
                rec("Light Me Up (feat. VISTA)", "Lowborn", 195.0, true, true),
                rec("Light Me Up", "Lowborn", 240.0, true, true),
                rec("Light Me Up", "Someone Else", 193.0, true, true),
                rec("Other", "Lowborn", 193.0, true, true),
            ],
            &w,
        )
        .unwrap();
        assert_eq!(got.track_name, "Light Me Up (feat. VISTA)");
        assert!(pick(vec![rec("Light Me Up", "Lowborn", 196.5, true, true)], &w).is_none());
        assert!(pick(vec![rec("Light Me Up", "Lowborn", 193.0, false, false)], &w).is_none());
    }

    #[test]
    fn finds_matching_line() {
        let plain = "Первая строка\n\nЁжик в тумане идёт\nпоследняя";
        assert_eq!(matching_line(plain, "ежик в ТУМАНЕ"), "Ёжик в тумане идёт");
        assert_eq!(matching_line(plain, "нет такого"), "Первая строка");
    }
}
