//! «Моя волна»: сессии, выдача, реакции; похожие треки и поиск словами по звучанию.
//!
//! Выдача: кандидаты — соседи граней вкуса (с поправкой на время суток и устройство), «что сейчас
//! заходит» минус «что пропускается», затравка радио, давно не слышанное любимое, новинки подписок
//! и открытия из индекса; отсев и расстановка — [`super::rank`]. Следующие треки, которых нет на сервере,
//! уходят на скачивание в полосу «Волна»; трек, который скачать нельзя, заменяется следующим. Каждый показ
//! пишется с причиной и оценкой, реакция — исходом показа.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    jobs::AcquireReason,
    realtime::{LibraryChange, ServerEvent},
    sync::SyncEntity,
    wave::{
        Reason, ReasonKind, SeedKind, SoundSearchResults, WaveFeedback, WaveFeedbackKind, WaveItem,
        WaveSession, WaveSettings,
    },
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    Cand, DayPart, MODEL_VERSION, Neighbour, Settings, analysis, candidates, nearest_semantic,
    nearest_sound,
    rank::{
        self, Entry, Origin, RankContext, Scored, SessionState, admissible, constraints, effect,
        merge, score, sequence, similar_threshold, violation, within_bounds,
    },
    taste, track_sound,
    vectors::{Rng, blend, weighted_mean},
    widen_search,
};
use crate::{
    acquire::{self, Enqueue, Request},
    auth::Principal,
    catalog::{self, views},
    downloaders,
    error::{AppError, AppResult, bad},
    jobs::lanes,
    ml::MlError,
    realtime::{self, Audience},
    state::AppState,
};

/// Сколько следующих треков, которых нет на сервере, ставить на скачивание.
const PREFETCH: usize = 8;
/// Первый из них — раньше остальных в полосе «Волна».
const PREFETCH_PRIORITY: i16 = 8;
pub const MAX_COUNT: usize = 30;
const TEXT_MAX: usize = 200;

/// Сессия волны из базы.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub settings: WaveSettings,
    pub state: SessionState,
    pub hits: Option<Vec<f32>>,
    pub misses: Option<Vec<f32>>,
    pub seed_sound: Option<Vec<f32>>,
    pub seed_text: Option<Vec<f32>>,
    pub started_at: DateTime<Utc>,
}

impl Session {
    /// Радио — сессия с затравкой.
    pub fn surface(&self) -> &'static str {
        if self.settings.seed.is_some() {
            "radio"
        } else {
            "wave"
        }
    }

    pub fn view(&self) -> WaveSession {
        WaveSession {
            id: self.id,
            settings: self.settings.clone(),
            title: self.state.title.clone(),
            served: self.state.step,
            started_at: self.started_at,
        }
    }
}

pub async fn load(db: &PgPool, user_id: Uuid, id: Uuid) -> AppResult<Session> {
    let row = sqlx::query!(
        r#"SELECT id, user_id, settings, state, hits::real[] AS hits, misses::real[] AS misses,
                  seed_sound::real[] AS seed_sound, seed_text::real[] AS seed_text, started_at
           FROM wave_sessions WHERE id = $1 AND user_id = $2"#,
        id,
        user_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Session {
        id: row.id,
        user_id: row.user_id,
        settings: serde_json::from_value(row.settings).unwrap_or_default(),
        state: serde_json::from_value(row.state).unwrap_or_default(),
        hits: row.hits,
        misses: row.misses,
        seed_sound: row.seed_sound,
        seed_text: row.seed_text,
        started_at: row.started_at,
    })
}

/// Затравка радио: векторы, подпись и артист.
#[derive(Debug, Default)]
struct Seeded {
    sound: Option<Vec<f32>>,
    text: Option<Vec<f32>>,
    label: String,
    title: String,
    artist: Option<Uuid>,
}

/// Средний звук треков: альбома, артиста, плейлиста.
async fn mean_sound(db: &PgPool, track_ids: &[Uuid]) -> Result<Option<Vec<f32>>, sqlx::Error> {
    let vectors = sqlx::query_scalar!(
        r#"SELECT DISTINCT ON (a.recording_id) a.sound::real[] AS "sound!"
           FROM tracks t JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = ANY($1) AND a.sound IS NOT NULL"#,
        track_ids,
    )
    .fetch_all(db)
    .await?;
    Ok(weighted_mean(vectors.iter().map(|v| (v.as_slice(), 1.0))))
}

/// Треки артиста — для его радио: проанализированные и популярное.
async fn artist_tracks(db: &PgPool, artist_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT t.id AS "id!" FROM track_artists ta JOIN tracks t ON t.id = ta.track_id
           WHERE ta.artist_id = $1 AND ta.role = 'main'
           ORDER BY (EXISTS (SELECT 1 FROM track_analysis a WHERE a.recording_id = t.recording_id)) DESC,
                    t.rank DESC NULLS LAST, t.id
           LIMIT 60"#,
        artist_id,
    )
    .fetch_all(db)
    .await
}

/// Ставит на анализ превью то, чего ждёт человек, — без ошибки, если не вышло.
async fn analyze_soon(db: &PgPool, track_ids: &[Uuid]) {
    let Ok(mut conn) = db.acquire().await else {
        return;
    };
    for id in track_ids {
        if let Err(e) =
            analysis::request_preview(&mut conn, *id, analysis::ON_DEMAND_PRIORITY).await
        {
            tracing::warn!(track = %id, error = %e, "could not queue a preview analysis");
        }
    }
}

fn ml_unavailable(e: MlError) -> AppError {
    AppError::Unavailable(format!(
        "поиск по звучанию сейчас недоступен, попробуйте позже ({e})"
    ))
}

async fn seed(
    st: &AppState,
    user_id: Uuid,
    seed: &dalywave_protocol::wave::WaveSeed,
) -> AppResult<Seeded> {
    let db = &st.db;
    let need_id = || {
        seed.id
            .ok_or_else(|| bad("для радио нужен id трека, артиста, альбома или плейлиста"))
    };
    match seed.kind {
        SeedKind::Track => {
            let id = need_id()?;
            let row = sqlx::query!(
                r#"SELECT t.title,
                          (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id
                           ORDER BY ta.position, ta.artist_id LIMIT 1) AS artist_id,
                          (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                           WHERE ta.track_id = t.id ORDER BY ta.position, ta.artist_id LIMIT 1) AS artist_name
                   FROM tracks t WHERE t.id = $1"#,
                id,
            )
            .fetch_optional(db)
            .await?
            .ok_or(AppError::NotFound)?;
            let mut sound = track_sound(db, id).await?;
            if sound.is_none() {
                // Трек ещё не разобран: превью — вне очереди, а пока радио держится его артиста
                analyze_soon(db, &[id]).await;
                if let Some(artist) = row.artist_id {
                    sound = mean_sound(db, &artist_tracks(db, artist).await?).await?;
                }
            }
            Ok(Seeded {
                sound,
                label: row.artist_name.clone().unwrap_or_else(|| row.title.clone()),
                title: format!("Радио: {}", row.title),
                artist: row.artist_id,
                ..Default::default()
            })
        }
        SeedKind::Artist => {
            let id = need_id()?;
            catalog::ensure_artist(db, &st.deezer, id).await?;
            let name = sqlx::query_scalar!("SELECT name FROM artists WHERE id = $1", id)
                .fetch_optional(db)
                .await?
                .ok_or(AppError::NotFound)?;
            let tracks = artist_tracks(db, id).await?;
            let sound = mean_sound(db, &tracks).await?;
            if sound.is_none() {
                analyze_soon(db, &tracks[..tracks.len().min(5)]).await;
            }
            Ok(Seeded {
                sound,
                label: name.clone(),
                title: format!("Радио: {name}"),
                artist: Some(id),
                ..Default::default()
            })
        }
        SeedKind::Album => {
            let id = need_id()?;
            catalog::ensure_album(db, &st.deezer, id).await?;
            let row = sqlx::query!(
                "SELECT al.title, al.artist_id, a.name FROM albums al JOIN artists a ON a.id = al.artist_id
                 WHERE al.id = $1",
                id,
            )
            .fetch_optional(db)
            .await?
            .ok_or(AppError::NotFound)?;
            let tracks = sqlx::query_scalar!("SELECT id FROM tracks WHERE album_id = $1", id)
                .fetch_all(db)
                .await?;
            let mut sound = mean_sound(db, &tracks).await?;
            if sound.is_none() {
                analyze_soon(db, &tracks[..tracks.len().min(5)]).await;
                sound = mean_sound(db, &artist_tracks(db, row.artist_id).await?).await?;
            }
            Ok(Seeded {
                sound,
                label: row.name,
                title: format!("Радио: {}", row.title),
                artist: Some(row.artist_id),
                ..Default::default()
            })
        }
        SeedKind::Playlist => {
            let id = need_id()?;
            let title = sqlx::query_scalar!(
                "SELECT title FROM playlists WHERE id = $1 AND user_id = $2",
                id,
                user_id
            )
            .fetch_optional(db)
            .await?
            .ok_or(AppError::NotFound)?;
            let tracks = sqlx::query_scalar!(
                "SELECT track_id FROM playlist_items WHERE playlist_id = $1",
                id
            )
            .fetch_all(db)
            .await?;
            let sound = mean_sound(db, &tracks).await?;
            if sound.is_none() {
                analyze_soon(db, &tracks[..tracks.len().min(5)]).await;
            }
            Ok(Seeded {
                sound,
                label: format!("«{title}»"),
                title: format!("Радио: {title}"),
                ..Default::default()
            })
        }
        SeedKind::Text => {
            let text = seed
                .text
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or_else(|| bad("для волны по описанию нужен текст"))?;
            if text.chars().count() > TEXT_MAX {
                return Err(bad("описание — не длиннее 200 символов"));
            }
            let tv = st.ml.embed_text(text).await.map_err(ml_unavailable)?;
            let v = tv.vector().map_err(ml_unavailable)?;
            Ok(Seeded {
                text: Some(v),
                label: text.to_string(),
                title: format!("По запросу «{text}»"),
                ..Default::default()
            })
        }
    }
}

/// Новая сессия волны или радио.
pub async fn create(
    st: &AppState,
    principal: &Principal,
    settings: WaveSettings,
) -> AppResult<Session> {
    let user_id = principal.user.id;
    if let Some(t) = settings.tempo
        && !(100..=220).contains(&t)
    {
        return Err(bad("темп бега — от 100 до 220 шагов в минуту"));
    }
    let seeded = match &settings.seed {
        Some(s) => seed(st, user_id, s).await?,
        None => Seeded {
            title: "Моя волна".into(),
            ..Default::default()
        },
    };
    let state = SessionState {
        title: seeded.title.clone(),
        seed_label: (!seeded.label.is_empty()).then(|| seeded.label.clone()),
        seed_artist: seeded.artist,
        ..Default::default()
    };
    let settings_json = serde_json::to_value(&settings).map_err(anyhow::Error::from)?;
    let state_json = serde_json::to_value(&state).map_err(anyhow::Error::from)?;
    let row = sqlx::query!(
        "INSERT INTO wave_sessions (user_id, device_id, settings, state, seed_sound, seed_text)
         VALUES ($1, $2, $3, $4, $5::real[]::halfvec(1280), $6::real[]::halfvec(512))
         RETURNING id, started_at",
        user_id,
        principal.device_id,
        settings_json,
        state_json,
        seeded.sound.as_deref(),
        seeded.text.as_deref(),
    )
    .fetch_one(&st.db)
    .await?;
    Ok(Session {
        id: row.id,
        user_id,
        settings,
        state,
        hits: None,
        misses: None,
        seed_sound: seeded.sound,
        seed_text: seeded.text,
        started_at: row.started_at,
    })
}

/// Источники кандидатов: (трек, откуда, сходство).
type Raw = (Uuid, Origin, f32);

fn push_neighbours(out: &mut Vec<Raw>, near: Vec<Neighbour>, origin: &Origin) {
    out.extend(
        near.into_iter()
            .map(|n| (n.track_id, origin.clone(), n.similarity)),
    );
}

/// Давно не слышанное любимое: лайкнутое или дослушанное трижды, что не играло месяц.
pub async fn forgotten(db: &PgPool, user_id: Uuid, limit: i64) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"WITH stats AS (
               SELECT p.track_id,
                      count(*) FILTER (WHERE p.end_reason = 'completed' OR p.played_ms >= 0.8 * p.duration_ms) AS completed,
                      max(p.started_at) AS last
               FROM play_events p WHERE p.user_id = $1 GROUP BY p.track_id
           )
           SELECT t.id AS "id!" FROM tracks t
           LEFT JOIN stats s ON s.track_id = t.id
           LEFT JOIN liked_tracks l ON l.track_id = t.id AND l.user_id = $1
           WHERE (l.track_id IS NOT NULL OR s.completed >= 3)
             AND COALESCE(s.last, l.created_at) < now() - interval '30 days'
             AND NOT EXISTS (SELECT 1 FROM blocked_items b WHERE b.user_id = $1 AND b.kind = 'track' AND b.ref_id = t.id)
           ORDER BY COALESCE(s.completed, 0) + CASE WHEN l.track_id IS NOT NULL THEN 3 ELSE 0 END DESC,
                    COALESCE(s.last, l.created_at), t.id
           LIMIT $2"#,
        user_id,
        limit,
    )
    .fetch_all(db)
    .await
}

/// Кандидаты выдачи: соседи затравки, сессии и граней, любимое, забытое, новинки подписок.
async fn pool(
    st: &AppState,
    session: &Session,
    part: DayPart,
    platform: Option<&str>,
) -> AppResult<Vec<Raw>> {
    let db = &st.db;
    let user = session.user_id;
    let seeded = session.settings.seed.is_some();
    let mut raw: Vec<Raw> = Vec::new();
    let facets = taste::facets(db, user).await?;
    let mut tx = db.begin().await?;
    widen_search(&mut tx, 300).await?;
    if let Some(v) = &session.seed_sound {
        let origin = Origin::Seed {
            label: session.state.seed_label.clone().unwrap_or_default(),
            artist: session.state.seed_artist,
        };
        push_neighbours(
            &mut raw,
            nearest_sound(&mut tx, v, 250, user).await?,
            &origin,
        );
    }
    if let Some(v) = &session.seed_text {
        let origin = Origin::Query {
            text: session.state.seed_label.clone().unwrap_or_default(),
        };
        push_neighbours(
            &mut raw,
            nearest_semantic(&mut tx, v, 250, user).await?,
            &origin,
        );
    }
    if let Some(h) = &session.hits {
        // «Заходит» минус «пропускается»: волна уходит от того, что пропускают
        let query = match &session.misses {
            Some(m) => blend(Some(h), 1.0, &m.iter().map(|x| -x).collect::<Vec<_>>(), 0.4),
            None => h.clone(),
        };
        let origin = Origin::Session {
            artist: session
                .state
                .hit_artist
                .as_ref()
                .map(|a| (a.id, a.name.clone())),
        };
        push_neighbours(
            &mut raw,
            nearest_sound(&mut tx, &query, 150, user).await?,
            &origin,
        );
    }
    // Радио держится затравки; волна — вкуса, с поправкой граней на время суток и устройство
    let budget = if seeded { 60.0 } else { 400.0 };
    let total: f32 = facets
        .iter()
        .map(|f| f.weight * f.lift(part, platform))
        .sum();
    for f in &facets {
        let share = f.weight * f.lift(part, platform) / total.max(1e-6);
        let limit = (budget * share).round().clamp(30.0, 220.0) as i64;
        let origin = Origin::Facet {
            artists: f.artists.clone(),
        };
        push_neighbours(
            &mut raw,
            nearest_sound(&mut tx, &f.centroid, limit, user).await?,
            &origin,
        );
    }
    tx.commit().await?;

    if let Some(artist) = session.state.seed_artist {
        let origin = Origin::Seed {
            label: session.state.seed_label.clone().unwrap_or_default(),
            artist: Some(artist),
        };
        for id in artist_tracks(db, artist).await? {
            raw.push((id, origin.clone(), 0.75));
        }
    }
    if !seeded || session.seed_sound.is_none() {
        // Любимое — в псевдослучайном порядке сессии: каждый раз свой срез
        let salt = format!("{}:{}", session.id, session.state.step / 20);
        let favorites = sqlx::query_scalar!(
            "SELECT track_id FROM liked_tracks WHERE user_id = $1 ORDER BY md5(track_id::text || $2) LIMIT 80",
            user,
            salt,
        )
        .fetch_all(db)
        .await?;
        raw.extend(favorites.into_iter().map(|id| (id, Origin::Favorite, 0.55)));
        raw.extend(
            forgotten(db, user, 30)
                .await?
                .into_iter()
                .map(|id| (id, Origin::Forgotten, 0.6)),
        );
        let fresh = sqlx::query_scalar!(
            r#"SELECT t.id AS "id!" FROM followed_artists f
               JOIN artist_releases r ON r.artist_id = f.artist_id
               JOIN albums al ON al.id = r.album_id
               JOIN tracks t ON t.album_id = al.id
               WHERE f.user_id = $1 AND al.release_date >= current_date - 45 AND al.release_date <= current_date
                 AND NOT EXISTS (SELECT 1 FROM play_events p WHERE p.user_id = $1 AND p.track_id = t.id)
               ORDER BY al.release_date DESC, t.track_position NULLS LAST, t.id
               LIMIT 30"#,
            user,
        )
        .fetch_all(db)
        .await?;
        raw.extend(fresh.into_iter().map(|id| (id, Origin::NewRelease, 0.6)));
    }
    if raw.is_empty() {
        // Холодный старт: ни анализа, ни лайков — популярное артистов из подписок
        let top = sqlx::query_scalar!(
            "SELECT tt.track_id FROM followed_artists f JOIN artist_top_tracks tt ON tt.artist_id = f.artist_id
             WHERE f.user_id = $1 ORDER BY tt.position, tt.track_id LIMIT 60",
            user,
        )
        .fetch_all(db)
        .await?;
        raw.extend(top.into_iter().map(|id| (id, Origin::Favorite, 0.5)));
    }
    Ok(raw)
}

async fn device_platform(db: &PgPool, device: Option<Uuid>) -> Result<String, sqlx::Error> {
    let Some(device) = device else {
        return Ok("web".into());
    };
    Ok(
        sqlx::query_scalar!("SELECT platform FROM devices WHERE id = $1", device)
            .fetch_optional(db)
            .await?
            .unwrap_or_else(|| "web".into()),
    )
}

/// Следующие треки сессии.
pub async fn next(
    st: &AppState,
    principal: &Principal,
    id: Uuid,
    count: usize,
) -> AppResult<Vec<WaveItem>> {
    let user = principal.user.id;
    let count = count.clamp(1, MAX_COUNT);
    let session = load(&st.db, user, id).await?;
    let settings = Settings::load(&st.db).await?;
    let now = Utc::now();
    let part = DayPart::of_hour(settings.local_hour(now));
    let platform = device_platform(&st.db, principal.device_id).await?;
    let can_acquire = st.config.fetcher.server || downloaders::online(&st.db).await? > 0;

    let raw = pool(st, &session, part, Some(&platform)).await?;
    let mut ids: Vec<Uuid> = Vec::with_capacity(raw.len());
    let mut seen = HashSet::new();
    for (track, _, _) in &raw {
        if seen.insert(*track) {
            ids.push(*track);
        }
    }
    let cands: HashMap<Uuid, Cand> = candidates(
        &st.db,
        &ids,
        user,
        session.hits.as_deref(),
        session.misses.as_deref(),
    )
    .await?
    .into_iter()
    .map(|c| (c.track_id, c))
    .collect();
    let entries = merge(
        raw.into_iter()
            .filter_map(|(track, origin, sim)| {
                Some(Entry {
                    cand: cands.get(&track)?.clone(),
                    origin,
                    sim,
                })
            })
            .collect(),
    );

    let ctx = RankContext {
        settings: &session.settings,
        state: &session.state,
        can_acquire,
        now,
        similar_threshold: similar_threshold(&entries),
    };
    let exclude = session.state.served_set();
    let bounds = constraints(&session.settings);
    let mut rng = Rng::from_uuid(session.id, u64::from(session.state.step));
    let scored: Vec<Scored> = entries
        .into_iter()
        .filter(|e| admissible(e, &ctx, &exclude))
        .map(|e| {
            let v = violation(&e.cand, &bounds);
            let s = score(&e, &ctx, &mut rng) - 1.5 * v.min(1.0);
            Scored {
                entry: e,
                score: s,
                violation: v,
            }
        })
        .collect();
    let picked = sequence(within_bounds(scored, count, &bounds), count + 5, &ctx);

    // Следующие треки, которых нет на сервере, — на скачивание; тот, что скачать нельзя, — мимо
    let mut out: Vec<(Scored, Reason)> = Vec::with_capacity(count);
    let mut prefetched = 0usize;
    for s in picked {
        if out.len() >= count {
            break;
        }
        let c = &s.entry.cand;
        if let Some((last, _)) = out.last()
            && last
                .entry
                .cand
                .artist_ids
                .iter()
                .any(|a| c.artist_ids.contains(a))
        {
            continue;
        }
        if !c.ready() && prefetched < PREFETCH {
            let req = Request::new(c.track_id, AcquireReason::Wave, lanes::WAVE)
                .by(user)
                .priority(PREFETCH_PRIORITY - prefetched as i16);
            match acquire::request(&st.db, &st.deezer, req).await {
                Ok(Enqueue::Skipped(why)) => {
                    tracing::debug!(track = %c.track_id, why, "wave track cannot be acquired, skipped");
                    continue;
                }
                Ok(_) => prefetched += 1,
                Err(e) => {
                    tracing::warn!(track = %c.track_id, error = %e, "could not prefetch a wave track")
                }
            }
        }
        let reason = rank::reason(&s.entry, &ctx);
        out.push((s, reason));
    }

    // Состояние сессии и показы — одной транзакцией
    let mut state = session.state.clone();
    let first_position = state.step;
    for (s, _) in &out {
        state.push(&s.entry.cand);
    }
    let track_ids: Vec<Uuid> = out.iter().map(|(s, _)| s.entry.cand.track_id).collect();
    let scores: Vec<f32> = out.iter().map(|(s, _)| s.score).collect();
    let reasons: Vec<String> = out
        .iter()
        .map(|(s, r)| {
            json!({ "kind": r.kind, "text": r.text, "sim": (s.entry.sim * 1000.0).round() / 1000.0 })
                .to_string()
        })
        .collect();
    let positions: Vec<i32> = (0..out.len())
        .map(|i| (first_position as usize + i) as i32)
        .collect();
    let state_json = serde_json::to_value(&state).map_err(anyhow::Error::from)?;
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "UPDATE wave_sessions SET state = $2, last_active_at = now() WHERE id = $1",
        session.id,
        state_json,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO recommendation_impressions (user_id, track_id, surface, wave_session_id, model_version, score,
                                                 reason, position)
         SELECT $1, t, $2, $3, $4, s, r::jsonb, p
         FROM unnest($5::uuid[], $6::real[], $7::text[], $8::int[]) AS x(t, s, r, p)",
        user,
        session.surface(),
        session.id,
        MODEL_VERSION,
        &track_ids,
        &scores,
        &reasons,
        &positions,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    items(
        &st.db,
        user,
        out.into_iter().map(|(s, r)| (s.entry.cand, r)).collect(),
    )
    .await
}

/// Выдача в виде API: трек как везде, подпись, темп, тональность и энергия.
async fn items(
    db: &PgPool,
    user_id: Uuid,
    picked: Vec<(Cand, Reason)>,
) -> AppResult<Vec<WaveItem>> {
    let ids: Vec<Uuid> = picked.iter().map(|(c, _)| c.track_id).collect();
    let mut tracks: HashMap<Uuid, _> = views::tracks(db, &ids, user_id)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    Ok(picked
        .into_iter()
        .filter_map(|(c, reason)| {
            Some(WaveItem {
                track: tracks.remove(&c.track_id)?,
                reason,
                bpm: c.bpm,
                key_camelot: c.key_camelot,
                energy: c.energy,
            })
        })
        .collect())
}

fn feedback_event(kind: WaveFeedbackKind) -> Option<&'static str> {
    match kind {
        WaveFeedbackKind::Like => Some("like"),
        WaveFeedbackKind::Dislike => Some("dislike"),
        WaveFeedbackKind::NotNow => Some("not_now"),
        WaveFeedbackKind::MoreLikeThis => Some("more_like_this"),
        // Пропуск и дослушивание приходят событиями прослушивания
        WaveFeedbackKind::Skip | WaveFeedbackKind::Completed => None,
    }
}

/// Реакция на трек сессии: векторы «заходит» и «пропускается», серия пропусков, отдых артиста, исход
/// показа; лайк — ещё и в медиатеку, «не нравится» — трек больше не рекомендуется.
pub async fn feedback(
    st: &AppState,
    principal: &Principal,
    id: Uuid,
    fb: WaveFeedback,
) -> AppResult<()> {
    let user = principal.user.id;
    let session = load(&st.db, user, id).await?;
    let cand = candidates(&st.db, &[fb.track_id], user, None, None)
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)?;
    let sound = track_sound(&st.db, fb.track_id).await?;
    let fx = effect(fb.kind, fb.position_ms);
    let mut state = session.state.clone();
    state.apply(&fx, &cand);
    let mut hits = session.hits.clone();
    let mut misses = session.misses.clone();
    if let Some(v) = &sound {
        if fx.hit > 0.0 {
            hits = Some(blend(hits.as_deref(), state.hits_weight, v, fx.hit));
            state.hits_weight += fx.hit;
        }
        if fx.miss > 0.0 {
            misses = Some(blend(misses.as_deref(), state.misses_weight, v, fx.miss));
            state.misses_weight += fx.miss;
        }
    }
    let state_json = serde_json::to_value(&state).map_err(anyhow::Error::from)?;
    let mut tx = st.db.begin().await?;
    sqlx::query!(
        "UPDATE wave_sessions SET state = $2, hits = $3::real[]::halfvec(1280), misses = $4::real[]::halfvec(1280),
                last_active_at = now()
         WHERE id = $1",
        session.id,
        state_json,
        hits.as_deref(),
        misses.as_deref(),
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE recommendation_impressions SET outcome = $3, outcome_at = now()
         WHERE id = (SELECT id FROM recommendation_impressions WHERE wave_session_id = $1 AND track_id = $2
                     ORDER BY shown_at DESC LIMIT 1)",
        session.id,
        fb.track_id,
        fb.kind.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    if let Some(kind) = feedback_event(fb.kind) {
        sqlx::query!(
            "INSERT INTO feedback_events (client_event_id, user_id, track_id, kind, context_type, context_id,
                                          wave_session_id, at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, now())",
            Uuid::now_v7(),
            user,
            fb.track_id,
            kind,
            session.surface(),
            session.id.to_string(),
            session.id,
        )
        .execute(&mut *tx)
        .await?;
    }
    let liked = if fb.kind == WaveFeedbackKind::Like {
        let inserted = sqlx::query!(
            "INSERT INTO liked_tracks (user_id, track_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            user,
            fb.track_id,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if inserted {
            realtime::publish(
                &mut *tx,
                Audience::User(user),
                ServerEvent::LibraryChanged(LibraryChange {
                    entities: vec![SyncEntity::LikedTracks],
                }),
            )
            .await;
        }
        inserted
    } else {
        false
    };
    tx.commit().await?;
    if liked {
        // Лайк из волны — как лайк в медиатеке: трек качается на сервер, ближайшая грань сдвигается
        let req = Request::new(fb.track_id, AcquireReason::Like, lanes::LIBRARY).by(user);
        if let Err(e) = acquire::request(&st.db, &st.deezer, req).await {
            tracing::warn!(track = %fb.track_id, error = %e, "could not queue a liked wave track");
        }
        if let Err(e) = taste::on_like(&st.db, user, fb.track_id).await {
            tracing::warn!(track = %fb.track_id, error = %e, "could not move the taste facet");
        }
    }
    Ok(())
}

/// Похожие на трек: соседи по звучанию, не больше двух у артиста. Трек ещё не разобран — его превью
/// ставится на анализ вне очереди, а пока — популярное похожих артистов.
pub async fn similar(
    st: &AppState,
    user: Uuid,
    track_id: Uuid,
    limit: usize,
) -> AppResult<Vec<WaveItem>> {
    let base = sqlx::query!(
        r#"SELECT t.title, t.recording_id,
                  (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id
                   ORDER BY ta.position, ta.artist_id LIMIT 1) AS artist_id
           FROM tracks t WHERE t.id = $1"#,
        track_id,
    )
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let now = Utc::now();
    let mut picked: Vec<(Cand, Reason)> = Vec::new();
    let mut per_artist: HashMap<Uuid, usize> = HashMap::new();
    let mut take = |c: Cand, reason: Reason, picked: &mut Vec<(Cand, Reason)>| {
        if picked.len() >= limit
            || c.excluded
            || !rank::acquirable(&c)
            || c.track_id == track_id
            || (c.recording_id.is_some() && c.recording_id == base.recording_id)
            || picked
                .iter()
                .any(|(p, _)| p.recording_id.is_some() && p.recording_id == c.recording_id)
            || rank::resting(&c, dalywave_protocol::wave::WaveCharacter::Mix, now)
        {
            return;
        }
        let artist = c.artist_ids.first().copied().unwrap_or(c.track_id);
        let n = per_artist.entry(artist).or_default();
        if *n >= 2 {
            return;
        }
        *n += 1;
        picked.push((c, reason));
    };
    match track_sound(&st.db, track_id).await? {
        Some(v) => {
            let mut tx = st.db.begin().await?;
            let want = (limit * 4 + 10) as i64;
            widen_search(&mut tx, want).await?;
            let near = nearest_sound(&mut tx, &v, want, user).await?;
            tx.commit().await?;
            let ids: Vec<Uuid> = near.iter().map(|n| n.track_id).collect();
            for c in candidates(&st.db, &ids, user, None, None).await? {
                let reason = Reason {
                    kind: ReasonKind::Similar,
                    text: format!("Похоже на «{}»", base.title),
                };
                take(c, reason, &mut picked);
            }
        }
        None => {
            analyze_soon(&st.db, &[track_id]).await;
            if let Some(artist) = base.artist_id {
                catalog::ensure_artist(&st.db, &st.deezer, artist).await?;
                let rows = sqlx::query!(
                    "SELECT tt.track_id, a.name FROM artist_related r
                     JOIN artist_top_tracks tt ON tt.artist_id = r.related_id
                     JOIN artists a ON a.id = r.related_id
                     WHERE r.artist_id = $1 ORDER BY tt.position, r.position LIMIT $2",
                    artist,
                    (limit * 3) as i64,
                )
                .fetch_all(&st.db)
                .await?;
                let names: HashMap<Uuid, String> =
                    rows.iter().map(|r| (r.track_id, r.name.clone())).collect();
                let ids: Vec<Uuid> = rows.iter().map(|r| r.track_id).collect();
                for c in candidates(&st.db, &ids, user, None, None).await? {
                    let name = names.get(&c.track_id).cloned().unwrap_or_default();
                    let reason = Reason {
                        kind: ReasonKind::Similar,
                        text: format!("Похожий артист: {name}"),
                    };
                    take(c, reason, &mut picked);
                }
            }
        }
    }
    let track_ids: Vec<Uuid> = picked.iter().map(|(c, _)| c.track_id).collect();
    let reasons: Vec<String> = picked
        .iter()
        .map(|(_, r)| json!({ "kind": r.kind, "text": r.text }).to_string())
        .collect();
    let positions: Vec<i32> = (0..picked.len() as i32).collect();
    sqlx::query!(
        "INSERT INTO recommendation_impressions (user_id, track_id, surface, model_version, reason, position)
         SELECT $1, t, 'similar', $2, r::jsonb, p FROM unnest($3::uuid[], $4::text[], $5::int[]) AS x(t, r, p)",
        user,
        MODEL_VERSION,
        &track_ids,
        &reasons,
        &positions,
    )
    .execute(&st.db)
    .await?;
    items(&st.db, user, picked).await
}

/// Поиск словами по звучанию: вектор запроса в пространстве CLAP против «смысла» треков — отдельно
/// медиатека (лайки, прослушанное, скачанное) и индекс каталога.
pub async fn sound_search(
    st: &AppState,
    user: Uuid,
    text: &str,
    limit: usize,
) -> AppResult<SoundSearchResults> {
    let text = text.trim();
    if text.is_empty() {
        return Err(bad("пустой запрос"));
    }
    if text.chars().count() > TEXT_MAX {
        return Err(bad("слишком длинный запрос"));
    }
    let tv = st.ml.embed_text(text).await.map_err(ml_unavailable)?;
    let v = tv.vector().map_err(ml_unavailable)?;
    let mut tx = st.db.begin().await?;
    widen_search(&mut tx, 400).await?;
    let near = nearest_semantic(&mut tx, &v, 400, user).await?;
    tx.commit().await?;
    let ids: Vec<Uuid> = near.iter().map(|n| n.track_id).collect();
    let mut library = Vec::new();
    let mut catalog = Vec::new();
    for c in candidates(&st.db, &ids, user, None, None).await? {
        if c.excluded {
            continue;
        }
        let mine = c.liked || c.completed > 0 || c.plays_30d > 0 || c.ready();
        let list = if mine { &mut library } else { &mut catalog };
        if list.len() < limit {
            list.push(c.track_id);
        }
    }
    Ok(SoundSearchResults {
        query: tv.query,
        translated: tv.translated,
        unknown_words: tv.unknown_words,
        library: views::tracks(&st.db, &library, user).await?,
        catalog: views::tracks(&st.db, &catalog, user).await?,
    })
}
