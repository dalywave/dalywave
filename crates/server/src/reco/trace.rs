//! След подбора волны для «Как подобран»: что рассматривал подбор ([`super::wave::select`]), что и почему
//! отсеял, ~50 лучших вариантов с оценками и вероятностями сети, что выбрано и из каких слагаемых сложилась
//! оценка выбранного. Собирается по ходу подбора (числа уже посчитаны — сбор почти ничего не стоит),
//! пишется фоном вместе с показами ([`super::wave::persist`]): подбор, который не выдали (заготовки,
//! набор для устройства), следа не оставляет. Хранится 30 дней.
//!
//! Формат `wave_traces.data` (`v` = 1), числа округлены до тысячных:
//! ```text
//! { v, step, facets: [[id, label, доля]], sources: {facet|session|seed|query|favorite|forgotten|new_release: n},
//!   considered, drops: {rule|served|resting|unavailable|artistRest|heard|far|notReady|bounds: n},
//!   ranked, variants: [[track_id, откуда, оценка, сходство, [пропуск, дослушивание, лайк] | null, место | null]],
//!   dropped: [[track_id, откуда, причина]], picks: {track_id: {pos, parts: {sim, hit, miss, model, liked, mood, rest}}} }
//! ```
//! «Откуда» — `f<номер грани>` или вид источника ([`super::rank::Origin`]).

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    DayPart,
    rank::{Entry, Origin, RankContext, Scored, pull},
    taste::Facet,
    wave::Session,
};

/// Лучших вариантов в следе.
pub const VARIANTS: usize = 50;
/// Отсеянных — столько самых близких по звучанию: на карте они гаснут.
pub const DROPPED: usize = 30;
/// След хранится столько дней.
pub const KEEP_DAYS: i32 = 30;

/// Готовый след одного подбора: пишется вместе с показами выбранных из него треков.
#[derive(Debug)]
pub struct Draft {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub step: u32,
    pub data: Value,
}

fn r3(x: f32) -> f64 {
    (f64::from(x) * 1000.0).round() / 1000.0
}

/// Вид источника — как в показе (`reco::why::record`).
pub fn kind(o: &Origin) -> &'static str {
    match o {
        Origin::Facet { .. } => "facet",
        Origin::Session { .. } => "session",
        Origin::Seed { .. } => "seed",
        Origin::Query { .. } => "query",
        Origin::Favorite => "favorite",
        Origin::Forgotten => "forgotten",
        Origin::NewRelease => "new_release",
    }
}

/// Лёгкая строка варианта: всё, что нужно следу, без самого кандидата.
#[derive(Debug, Clone)]
struct Row {
    id: Uuid,
    origin: String,
    score: f32,
    sim: f32,
    probs: Option<[f32; 3]>,
}

/// Сборщик следа внутри подбора.
pub struct Collector<'a> {
    facets: &'a [Facet],
    shares: Vec<f32>,
    sources: BTreeMap<&'static str, usize>,
    considered: usize,
    drops: BTreeMap<&'static str, usize>,
    dropped: Vec<(Uuid, String, &'static str, f32)>,
    ranked: usize,
    variants: Vec<Row>,
}

impl<'a> Collector<'a> {
    /// `raw` — кандидаты до слияния: по ним считаются источники.
    pub fn new(
        facets: &'a [Facet],
        raw: &[(Uuid, Origin, f32)],
        part: DayPart,
        platform: Option<&str>,
    ) -> Self {
        let mut sources = BTreeMap::new();
        for (_, o, _) in raw {
            *sources.entry(kind(o)).or_insert(0) += 1;
        }
        let lifted: Vec<f32> = facets
            .iter()
            .map(|f| f.weight * f.lift(part, platform))
            .collect();
        let total: f32 = lifted.iter().sum::<f32>().max(1e-6);
        Self {
            facets,
            shares: lifted.iter().map(|w| w / total).collect(),
            sources,
            considered: 0,
            drops: BTreeMap::new(),
            dropped: Vec::new(),
            ranked: 0,
            variants: Vec::new(),
        }
    }

    /// Откуда кандидат: грань — её номером (по главным артистам, как её собирает `wave::pool`).
    pub fn origin(&self, o: &Origin) -> String {
        if let Origin::Facet { artists } = o
            && let Some(i) = self.facets.iter().position(|f| &f.artists == artists)
        {
            return format!("f{i}");
        }
        kind(o).to_string()
    }

    pub fn considered(&mut self, n: usize) {
        self.considered = n;
    }

    /// Кандидат отсеян по причине `reason`.
    pub fn drop(&mut self, e: &Entry, reason: &'static str) {
        *self.drops.entry(reason).or_insert(0) += 1;
        self.dropped
            .push((e.cand.track_id, self.origin(&e.origin), reason, e.sim));
    }

    /// Отсеяны без самих кандидатов под рукой (границы настроек — по лёгким строкам).
    fn drop_row(&mut self, r: &Row, reason: &'static str) {
        *self.drops.entry(reason).or_insert(0) += 1;
        self.dropped.push((r.id, r.origin.clone(), reason, r.sim));
    }

    /// Лёгкие строки оценённых — до границ настроек.
    pub fn rows(&self, scored: &[Scored]) -> Vec<(Uuid, String, f32)> {
        scored
            .iter()
            .map(|s| {
                (
                    s.entry.cand.track_id,
                    self.origin(&s.entry.origin),
                    s.entry.sim,
                )
            })
            .collect()
    }

    /// Прошедшие границы настроек: отсеянные ими — в отсев, лучшие — в варианты.
    pub fn bounded(&mut self, before: Vec<(Uuid, String, f32)>, bounded: &[Scored]) {
        let kept: std::collections::HashSet<Uuid> =
            bounded.iter().map(|s| s.entry.cand.track_id).collect();
        for (id, origin, sim) in before {
            if !kept.contains(&id) {
                let row = Row {
                    id,
                    origin,
                    score: 0.0,
                    sim,
                    probs: None,
                };
                self.drop_row(&row, "bounds");
            }
        }
        self.ranked = bounded.len();
        let mut rows: Vec<Row> = bounded
            .iter()
            .map(|s| Row {
                id: s.entry.cand.track_id,
                origin: self.origin(&s.entry.origin),
                score: s.score,
                sim: s.entry.sim,
                probs: s.entry.cand.model_probs,
            })
            .collect();
        rows.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
        self.variants = rows;
    }

    /// След готов: выбранные — с местом в выдаче и слагаемыми оценки.
    pub fn finish(mut self, session: &Session, ctx: &RankContext, picked: &[Scored]) -> Arc<Draft> {
        let pos: HashMap<Uuid, usize> = picked
            .iter()
            .enumerate()
            .map(|(i, s)| (s.entry.cand.track_id, i))
            .collect();
        // Лучшие и все выбранные (выбранный может стоять и ниже первых пятидесяти по оценке)
        let mut variants: Vec<&Row> = self.variants.iter().take(VARIANTS).collect();
        variants.extend(
            self.variants
                .iter()
                .skip(VARIANTS)
                .filter(|r| pos.contains_key(&r.id)),
        );
        let variants: Vec<Value> = variants
            .into_iter()
            .map(|r| {
                json!([
                    r.id,
                    r.origin,
                    r3(r.score),
                    r3(r.sim),
                    r.probs.map(|p| p.map(r3)),
                    pos.get(&r.id),
                ])
            })
            .collect();
        self.dropped
            .sort_by(|a, b| b.3.total_cmp(&a.3).then(a.0.cmp(&b.0)));
        let dropped: Vec<Value> = self
            .dropped
            .iter()
            .take(DROPPED)
            .map(|(id, o, why, _)| json!([id, o, why]))
            .collect();
        let picks: serde_json::Map<String, Value> = picked
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.entry.cand.track_id.to_string(),
                    json!({ "pos": i, "parts": parts(s, ctx) }),
                )
            })
            .collect();
        let facets: Vec<Value> = self
            .facets
            .iter()
            .zip(&self.shares)
            .map(|(f, w)| json!([f.id, f.label, r3(*w)]))
            .collect();
        let data = json!({
            "v": 1,
            "step": session.state.step,
            "facets": facets,
            "sources": self.sources,
            "considered": self.considered,
            "drops": self.drops,
            "ranked": self.ranked,
            "variants": variants,
            "dropped": dropped,
            "picks": picks,
        });
        Arc::new(Draft {
            id: Uuid::now_v7(),
            user_id: session.user_id,
            session_id: session.id,
            step: session.state.step,
            data,
        })
    }
}

/// Главные слагаемые оценки (`rank::score`): сходство с источником, «заходит» и «пропускается» сессии,
/// сеть вкуса, лайк, настроение; `rest` — всё остальное вместе со случайностью.
fn parts(s: &Scored, ctx: &RankContext) -> Value {
    let c = &s.entry.cand;
    let streak = if ctx.state.skip_streak >= 2 { 2.0 } else { 1.0 };
    let sim = 0.6 * s.entry.sim;
    let hit = c.hit_sim.map_or(0.0, |h| 0.3 * (h - 0.5));
    let miss = c
        .miss_sim
        .map_or(0.0, |m| -0.35 * streak * (m - 0.5).max(0.0));
    let model = c.model.unwrap_or(0.0);
    let liked = if c.liked { 0.08 } else { 0.0 };
    let mood = pull(ctx.settings, c);
    let rest = s.score - sim - hit - miss - model - liked - mood;
    json!({
        "sim": r3(sim), "hit": r3(hit), "miss": r3(miss), "model": r3(model),
        "liked": r3(liked), "mood": r3(mood), "rest": r3(rest),
    })
}

/// Следы выданного — в базу фоном: выдача их не ждёт. Сбой записи — только в журнал.
pub fn spawn_write(db: &PgPool, drafts: Vec<Arc<Draft>>) {
    if drafts.is_empty() {
        return;
    }
    let db = db.clone();
    tokio::spawn(async move {
        for d in drafts {
            if let Err(e) = write(&db, &d).await {
                tracing::warn!(error = %e, trace = %d.id, "wave trace was not saved");
            }
        }
    });
}

pub async fn write(db: &PgPool, d: &Draft) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO wave_traces (id, user_id, wave_session_id, step, data) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (id) DO NOTHING",
        d.id,
        d.user_id,
        d.session_id,
        d.step as i32,
        d.data,
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Чистка: следы старше [`KEEP_DAYS`].
pub async fn cleanup(db: &PgPool) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query!(
        "DELETE FROM wave_traces WHERE created_at < now() - make_interval(days => $1)",
        KEEP_DAYS,
    )
    .execute(db)
    .await?
    .rows_affected())
}
