//! Подготовка волны: начало подбирается под настройки до запуска, а не из того, что уже скачано.
//!
//! Новая сессия сразу подбирает очередь ([`QUEUE`] треков) по всему индексу. Если готовых на сервере меньше
//! [`NEED`], лучшие из неготовых качаются вне очереди (полоса «Сейчас»), а сессия — `preparing`: интерфейс
//! показывает «Готовим волну…» и спрашивает сессию снова. Трек, который скачать не вышло, заменяется
//! следующим подходящим. Готово нужное — `ready`, готовое — первым в очереди. За [`WAIT`] не успели (или
//! качать некому) — начало добирается лучшим по настройкам из того, что уже на сервере.
//!
//! Заготовки: раз в час под частые настройки слушателя (свои за месяц, сохранённые сочетания и «Моя волна»)
//! подбирается по [`POOL_SIZE`] треков и качается в полосе «Открытия» в пределах суточного предела
//! (`wave_pools`). Готовое из заготовок волна берёт первым — старт мгновенный.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{DateTime, Duration, Utc};
use dalywave_protocol::{
    catalog::AudioStatus,
    jobs::AcquireReason,
    wave::{Reason, WaveSettings},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::wave::{self, Env, Options, Pick, Session};
use crate::{
    acquire::{self, Enqueue, Request},
    error::AppResult,
    jobs::{
        JobRow, lanes,
        worker::{Context, JobError, JobResult},
    },
};

/// Сколько треков в начале волны.
pub const QUEUE: usize = 6;
/// Сколько из них должно быть на сервере, чтобы начать.
pub const NEED: usize = 3;
/// Сколько ждать загрузчика, прежде чем начать с готового.
pub const WAIT: Duration = Duration::seconds(90);
/// Заготовка под одни настройки.
pub const POOL_SIZE: usize = 5;
/// Под сколько настроек одного слушателя держать заготовки.
const POOL_COMBOS: usize = 8;
/// Сколько заготовок в сутки можно поставить на скачивание одному слушателю.
pub const POOL_DAILY_CAP: i64 = 40;
/// Заготовка старше — подбирается заново (вкус и индекс за это время сдвинулись).
const POOL_STALE: Duration = Duration::days(3);

/// Подготовка начала сессии (в `wave_sessions.state`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prep {
    /// Начало волны в порядке игры.
    pub queue: Vec<Picked>,
    /// Сколько готового нужно, чтобы начать.
    pub need: u32,
    /// Готово: можно играть.
    pub done: bool,
    /// Дольше не ждём — начинаем с того, что есть на сервере.
    pub deadline: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Picked {
    pub track_id: Uuid,
    pub reason: Reason,
}

/// Как идёт подготовка.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Готового хватает — начинать.
    Ready,
    /// Ждём загрузчика; `lost` — треки, которые скачать не вышло: их место займут другие.
    Wait { lost: Vec<Uuid> },
    /// Срок вышел — начинать с готового.
    Timeout,
}

/// Сколько готово и что делать дальше — по состояниям аудио треков очереди.
pub fn assess(
    prep: &Prep,
    status: &HashMap<Uuid, AudioStatus>,
    now: DateTime<Utc>,
) -> (u32, Verdict) {
    let ready = prep
        .queue
        .iter()
        .filter(|p| status.get(&p.track_id) == Some(&AudioStatus::Ready))
        .count() as u32;
    if ready >= prep.need {
        return (ready, Verdict::Ready);
    }
    if prep.deadline.is_some_and(|d| now >= d) {
        return (ready, Verdict::Timeout);
    }
    let lost = prep
        .queue
        .iter()
        .filter(|p| {
            matches!(
                status.get(&p.track_id),
                Some(AudioStatus::Unavailable | AudioStatus::Disputed | AudioStatus::Failed)
            )
        })
        .map(|p| p.track_id)
        .collect();
    (ready, Verdict::Wait { lost })
}

/// Готовое — первым, порядок внутри сохраняется: остальное успеет скачаться, пока играет начало.
pub fn ready_first(queue: Vec<Picked>, ready: &HashSet<Uuid>) -> Vec<Picked> {
    let (mut first, rest): (Vec<_>, Vec<_>) =
        queue.into_iter().partition(|p| ready.contains(&p.track_id));
    first.extend(rest);
    first
}

fn picked(out: &[Pick]) -> Vec<Picked> {
    out.iter()
        .map(|(s, r)| Picked {
            track_id: s.entry.cand.track_id,
            reason: r.clone(),
        })
        .collect()
}

fn ready_set(out: &[Pick]) -> HashSet<Uuid> {
    out.iter()
        .filter(|(s, _)| s.entry.cand.ready())
        .map(|(s, _)| s.entry.cand.track_id)
        .collect()
}

/// Подготовка новой сессии: очередь под настройки; готового мало — лучшее неготовое качается вне очереди.
pub async fn start(env: &Env<'_>, session: &mut Session, platform: Option<&str>) -> AppResult<()> {
    let user = session.user_id;
    let key = combo_key(&session.settings);
    let pooled: HashSet<Uuid> = if session.settings.seed.is_none() {
        sqlx::query_scalar!(
            "SELECT track_id FROM wave_pools WHERE user_id = $1 AND combo = $2",
            user,
            key,
        )
        .fetch_all(env.db)
        .await?
        .into_iter()
        .collect()
    } else {
        HashSet::new()
    };
    let opts = Options {
        only_ready: !env.can_acquire,
        boost: Some(&pooled),
        skip_heard: false,
    };
    let picks = wave::select(env, session, platform, QUEUE, opts).await?;
    let ready_ahead = picks
        .iter()
        .take(QUEUE)
        .filter(|(s, _)| s.entry.cand.ready())
        .count();
    // Запас в один трек: какой-нибудь может не скачаться
    let urgent = if env.can_acquire && ready_ahead < NEED {
        NEED - ready_ahead + 1
    } else {
        0
    };
    let out = wave::take(env, user, picks, QUEUE, urgent).await;
    let ready = ready_set(&out);
    let need = NEED.min(out.len()) as u32;
    // Качать некому — ждать нечего: начало уже из того, что на сервере
    let done = !env.can_acquire || ready.len() as u32 >= need;
    let queue = if done {
        ready_first(picked(&out), &ready)
    } else {
        picked(&out)
    };
    let mut state = session.state.clone();
    let first = state.step;
    for (s, _) in &out {
        state.push(&s.entry.cand);
    }
    state.prep = Some(Prep {
        queue,
        need,
        done,
        deadline: Some(session.started_at + WAIT),
    });
    wave::persist(env.db, session, &state, first, &out).await?;
    session.state = state;
    // Взятое из заготовок больше не заготовка — ни для этих настроек, ни для других
    let used: Vec<Uuid> = out.iter().map(|(s, _)| s.entry.cand.track_id).collect();
    sqlx::query!(
        "DELETE FROM wave_pools WHERE user_id = $1 AND track_id = ANY($2)",
        user,
        &used,
    )
    .execute(env.db)
    .await?;
    Ok(())
}

async fn statuses(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, AudioStatus>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT t.id, r.audio_status AS "audio_status?" FROM tracks t
           LEFT JOIN recordings r ON r.id = t.recording_id WHERE t.id = ANY($1)"#,
        ids,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| Some((r.id, r.audio_status?.parse().ok()?)))
        .collect())
}

/// Ход подготовки: готово — начинать; не скачалось — замена; срок вышел — начало из готового.
pub async fn refresh(
    env: &Env<'_>,
    session: &mut Session,
    platform: Option<&str>,
) -> AppResult<()> {
    let Some(mut prep) = session.state.prep.clone() else {
        return Ok(());
    };
    if prep.done {
        return Ok(());
    }
    let user = session.user_id;
    let ids: Vec<Uuid> = prep.queue.iter().map(|p| p.track_id).collect();
    let status = statuses(env.db, &ids).await?;
    let ready: HashSet<Uuid> = status
        .iter()
        .filter(|(_, s)| **s == AudioStatus::Ready)
        .map(|(id, _)| *id)
        .collect();
    let mut state = session.state.clone();
    let first = state.step;
    let mut added: Vec<Pick> = Vec::new();
    match assess(&prep, &status, Utc::now()).1 {
        Verdict::Ready => {
            prep.queue = ready_first(prep.queue, &ready);
            prep.done = true;
        }
        Verdict::Wait { lost } if lost.is_empty() => return Ok(()),
        Verdict::Wait { lost } => {
            prep.queue.retain(|p| !lost.contains(&p.track_id));
            let picks =
                wave::select(env, session, platform, lost.len(), Options::default()).await?;
            added = wave::take(env, user, picks, lost.len(), lost.len()).await;
            prep.queue.extend(picked(&added));
            prep.need = prep.need.min(prep.queue.len() as u32);
        }
        Verdict::Timeout => {
            // Начало — готовое из очереди, затем лучшее по настройкам из того, что уже на сервере;
            // неготовое остаётся в конце: докачается, пока играет начало
            prep.queue.retain(|p| {
                !matches!(
                    status.get(&p.track_id),
                    Some(AudioStatus::Unavailable | AudioStatus::Disputed | AudioStatus::Failed)
                )
            });
            let have = prep
                .queue
                .iter()
                .filter(|p| ready.contains(&p.track_id))
                .count();
            let want = QUEUE.saturating_sub(have).max(1);
            let opts = Options {
                only_ready: true,
                ..Default::default()
            };
            let picks = wave::select(env, session, platform, want, opts).await?;
            let picks: Vec<Pick> = picks
                .into_iter()
                .filter(|(s, _)| s.entry.cand.ready())
                .collect();
            added = wave::take(env, user, picks, want, 0).await;
            let (ready_part, pending): (Vec<_>, Vec<_>) = prep
                .queue
                .into_iter()
                .partition(|p| ready.contains(&p.track_id));
            let mut queue = ready_part;
            queue.extend(picked(&added));
            queue.extend(pending);
            prep.queue = queue;
            prep.done = true;
        }
    }
    for (s, _) in &added {
        state.push(&s.entry.cand);
    }
    state.prep = Some(prep);
    wave::persist(env.db, session, &state, first, &added).await?;
    session.state = state;
    Ok(())
}

// ---------- Заготовки ----------

/// Ключ настроек заготовки: занятие, характер, настроение, язык и темп бега.
pub fn combo_key(s: &WaveSettings) -> String {
    let mut key = format!(
        "{}:{}:{}:{}",
        s.activity.as_str(),
        s.character.as_str(),
        s.mood.as_str(),
        s.language.as_str()
    );
    if let Some(t) = s.tempo {
        key.push_str(&format!(":{t}"));
    }
    key
}

/// Под какие настройки держать заготовки: «Моя волна», свои за месяц (частые — первыми), сохранённые
/// сочетания слушателя (`wave_presets`). Радио (с затравкой) не заготавливается.
pub fn combos(recent: Vec<WaveSettings>, saved: Vec<WaveSettings>) -> Vec<WaveSettings> {
    let mut seen = HashSet::new();
    std::iter::once(WaveSettings::default())
        .chain(recent)
        .chain(saved)
        .filter(|s| s.seed.is_none())
        .filter(|s| seen.insert(combo_key(s)))
        .take(POOL_COMBOS)
        .collect()
}

/// Задача `wave_pool`: устаревшее и услышанное из заготовок уходит, недостающее подбирается заново,
/// неготовое качается в полосе «Открытия» — не больше [`POOL_DAILY_CAP`] в сутки на слушателя.
pub async fn pool_run(ctx: Arc<Context>, _job: JobRow) -> JobResult {
    let report = refill_pools(&ctx.db, &ctx.deezer, ctx.config.fetcher.server)
        .await
        .map_err(JobError::retry)?;
    Ok(report.into())
}

pub async fn refill_pools(
    db: &PgPool,
    deezer: &crate::deezer::Deezer,
    server_fetcher: bool,
) -> AppResult<serde_json::Value> {
    let env = Env::new(db, deezer, server_fetcher).await?;
    let stale_before = Utc::now() - POOL_STALE;
    let expired = sqlx::query!(
        "DELETE FROM wave_pools p
         WHERE p.picked_at < $1
            OR EXISTS (SELECT 1 FROM play_events e WHERE e.user_id = p.user_id AND e.track_id = p.track_id
                         AND e.started_at >= p.picked_at)
            OR EXISTS (SELECT 1 FROM tracks t JOIN recordings r ON r.id = t.recording_id
                       WHERE t.id = p.track_id AND r.audio_status IN ('unavailable', 'disputed', 'failed'))",
        stale_before,
    )
    .execute(db)
    .await?
    .rows_affected();
    let users = sqlx::query_scalar!(
        "SELECT u.id FROM users u
         WHERE EXISTS (SELECT 1 FROM wave_sessions w WHERE w.user_id = u.id AND w.started_at > now() - interval '30 days')
            OR EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = u.id AND p.started_at > now() - interval '30 days')
         ORDER BY u.id"
    )
    .fetch_all(db)
    .await?;
    let (mut picked_total, mut queued_total) = (0usize, 0i64);
    for user in &users {
        let (p, q) = refill_user(&env, *user).await?;
        picked_total += p;
        queued_total += q;
    }
    Ok(json!({
        "users": users.len(),
        "expired": expired,
        "picked": picked_total,
        "queued": queued_total,
    }))
}

async fn refill_user(env: &Env<'_>, user: Uuid) -> AppResult<(usize, i64)> {
    let recent: Vec<WaveSettings> = sqlx::query_scalar!(
        r#"SELECT settings AS "settings!" FROM wave_sessions
           WHERE user_id = $1 AND started_at > now() - interval '30 days' AND jsonb_typeof(settings -> 'seed') IS DISTINCT FROM 'object'
           GROUP BY settings ORDER BY count(*) DESC, max(started_at) DESC LIMIT 20"#,
        user,
    )
    .fetch_all(env.db)
    .await?
    .into_iter()
    .filter_map(|v| serde_json::from_value(v).ok())
    .collect();
    let used = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM acquisitions
           WHERE requested_by = $1 AND lane = 'discovery' AND reason = 'wave' AND created_at > now() - interval '24 hours'"#,
        user,
    )
    .fetch_one(env.db)
    .await?;
    let mut cap = (POOL_DAILY_CAP - used).max(0);
    let (mut picked_n, mut queued) = (0usize, 0i64);
    let saved = crate::routes::wave_presets::settings_of(env.db, user).await?;
    for settings in combos(recent, saved) {
        let key = combo_key(&settings);
        let have = sqlx::query!(
            r#"SELECT p.track_id, t.recording_id FROM wave_pools p JOIN tracks t ON t.id = p.track_id
               WHERE p.user_id = $1 AND p.combo = $2"#,
            user,
            key,
        )
        .fetch_all(env.db)
        .await?;
        let deficit = POOL_SIZE.saturating_sub(have.len());
        if deficit == 0 {
            continue;
        }
        let served = have
            .iter()
            .map(|r| r.recording_id.unwrap_or(r.track_id))
            .collect();
        let session = Session::transient(user, settings.clone(), served);
        let opts = Options {
            skip_heard: true,
            ..Default::default()
        };
        let picks = wave::select(env, &session, None, deficit, opts).await?;
        let settings_json = serde_json::to_value(&settings).map_err(anyhow::Error::from)?;
        let mut artists: HashSet<Uuid> = HashSet::new();
        let mut taken = 0usize;
        for (s, _) in picks {
            if taken >= deficit {
                break;
            }
            let c = &s.entry.cand;
            // Заготовка — разные артисты: начало волны их и так чередует
            if c.artist_ids.first().is_some_and(|a| !artists.insert(*a)) {
                continue;
            }
            sqlx::query!(
                "INSERT INTO wave_pools (user_id, combo, track_id, settings, score) VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (user_id, combo, track_id) DO UPDATE SET score = EXCLUDED.score, picked_at = now()",
                user,
                key,
                c.track_id,
                settings_json,
                s.score,
            )
            .execute(env.db)
            .await?;
            taken += 1;
            picked_n += 1;
            if !c.ready() && cap > 0 {
                let req = Request::new(c.track_id, AcquireReason::Wave, lanes::DISCOVERY).by(user);
                match acquire::request(env.db, env.deezer, req).await {
                    Ok(Enqueue::Queued { .. }) => {
                        cap -= 1;
                        queued += 1;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(track = %c.track_id, error = %e, "could not queue a wave pool track")
                    }
                }
            }
        }
    }
    Ok((picked_n, queued))
}

#[cfg(test)]
mod tests {
    use dalywave_protocol::wave::{
        ReasonKind, SeedKind, WaveActivity, WaveCharacter, WaveLanguage, WaveMood, WaveSeed,
    };

    use super::*;

    fn pick(n: u128) -> Picked {
        Picked {
            track_id: Uuid::from_u128(n),
            reason: Reason {
                kind: ReasonKind::NewToYou,
                text: "Новое для тебя".into(),
            },
        }
    }

    fn prep(n: u128, need: u32, deadline: DateTime<Utc>) -> Prep {
        Prep {
            queue: (1..=n).map(pick).collect(),
            need,
            done: false,
            deadline: Some(deadline),
        }
    }

    #[test]
    fn preparation_waits_replaces_and_times_out() {
        let now = Utc::now();
        let p = prep(6, 3, now + Duration::seconds(60));
        let mut status: HashMap<Uuid, AudioStatus> = (1..=6)
            .map(|n| (Uuid::from_u128(n), AudioStatus::Queued))
            .collect();
        status.insert(Uuid::from_u128(4), AudioStatus::Ready);
        assert_eq!(
            assess(&p, &status, now),
            (1, Verdict::Wait { lost: vec![] })
        );
        // Не скачалось — на замену
        status.insert(Uuid::from_u128(2), AudioStatus::Unavailable);
        status.insert(Uuid::from_u128(3), AudioStatus::Disputed);
        assert_eq!(
            assess(&p, &status, now),
            (
                1,
                Verdict::Wait {
                    lost: vec![Uuid::from_u128(2), Uuid::from_u128(3)]
                }
            )
        );
        // Готово нужное — начинать, даже если остальное ещё качается
        status.insert(Uuid::from_u128(5), AudioStatus::Ready);
        status.insert(Uuid::from_u128(6), AudioStatus::Ready);
        assert_eq!(assess(&p, &status, now).0, 3);
        assert_eq!(assess(&p, &status, now).1, Verdict::Ready);
        // Срок вышел
        status.insert(Uuid::from_u128(6), AudioStatus::Acquiring);
        assert_eq!(
            assess(&p, &status, now + Duration::seconds(61)).1,
            Verdict::Timeout
        );
        // Нечего ждать: пустая очередь сразу готова
        assert_eq!(
            assess(&prep(0, 0, now), &HashMap::new(), now).1,
            Verdict::Ready
        );
    }

    #[test]
    fn ready_tracks_go_first_in_order() {
        let queue: Vec<Picked> = (1..=5).map(pick).collect();
        let ready: HashSet<Uuid> = [4, 2].into_iter().map(Uuid::from_u128).collect();
        let order: Vec<u128> = ready_first(queue, &ready)
            .into_iter()
            .map(|p| p.track_id.as_u128())
            .collect();
        assert_eq!(order, vec![2, 4, 1, 3, 5]);
    }

    #[test]
    fn pools_cover_default_recent_and_saved_combos() {
        let work = WaveSettings {
            activity: WaveActivity::Work,
            mood: WaveMood::Calm,
            character: WaveCharacter::New,
            ..Default::default()
        };
        let radio = WaveSettings {
            seed: Some(WaveSeed {
                kind: SeedKind::Text,
                id: None,
                text: Some("дождь".into()),
            }),
            ..Default::default()
        };
        let run = WaveSettings {
            activity: WaveActivity::Sport,
            tempo: Some(170),
            ..Default::default()
        };
        let night = WaveSettings {
            activity: WaveActivity::Rest,
            character: WaveCharacter::Rare,
            mood: WaveMood::Sad,
            language: WaveLanguage::Any,
            ..Default::default()
        };
        let list = combos(
            vec![
                work.clone(),
                radio.clone(),
                work.clone(),
                WaveSettings::default(),
                run.clone(),
            ],
            vec![night, work.clone(), radio],
        );
        let keys: Vec<String> = list.iter().map(combo_key).collect();
        assert_eq!(keys[0], "any:mix:any:any", "«Моя волна» — всегда и первой");
        assert_eq!(keys[1], "work:new:calm:any");
        assert_eq!(keys[2], "sport:mix:any:any:170", "темп бега — часть ключа");
        assert_eq!(keys[3], "rest:rare:sad:any", "сохранённые — после своих");
        assert_eq!(
            keys.len(),
            4,
            "повторы и радио не заготавливаются: {keys:?}"
        );
        // Сочетаний не больше предела
        let many: Vec<WaveSettings> = [
            WaveActivity::Wake,
            WaveActivity::Road,
            WaveActivity::Work,
            WaveActivity::Sport,
            WaveActivity::Rest,
            WaveActivity::Sleep,
            WaveActivity::Party,
        ]
        .into_iter()
        .map(|activity| WaveSettings {
            activity,
            ..Default::default()
        })
        .collect();
        assert_eq!(combos(many, Vec::new()).len(), POOL_COMBOS);
    }
}
