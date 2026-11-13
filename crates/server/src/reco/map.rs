//! Карта звучания для «Как подобран»: 2D-координаты всех треков индекса и граней вкуса.
//!
//! Раз в сутки, ночью (задача `track_map`), векторы звучания (Discogs-EffNet) всего индекса уходят
//! в сервис анализа: там UMAP, начатый с прошлой карты. Получившаяся карта выравнивается к прошлой
//! (Procrustes: поворот, масштаб, сдвиг, при нужде — отражение), чтобы она не «прыгала» каждый день;
//! самая первая — укладывается в квадрат 0..1. Сервис недоступен — запас: две главные компоненты
//! (PCA) здесь же, выровненные так же. Новые треки между пересчётами получают среднее десяти ближайших
//! по звучанию соседей ([`fill`], каждый час). Грань вкуса на карте — среднее ближайших к её центру
//! треков, радиус облака — их разброс ([`facets`]).

use std::{sync::Arc, time::Instant};

use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::{Settings, vectors::SOUND_DIM, widen_search};
use crate::{
    jobs::{
        self, JobRow, NewJob, lanes,
        worker::{Context, JobResult},
    },
    ml::Ml,
};

/// Меньше треков с анализом — карты нет: не из чего.
pub const MIN_POINTS: usize = 20;
/// Новый трек и грань — по стольким ближайшим соседям с координатами.
pub const NEIGHBOURS: i64 = 10;
/// Облако грани — по стольким ближайшим к её центру трекам.
const FACET_CLOUD: i64 = 30;
/// Строк в одном обновлении координат.
const CHUNK: usize = 2000;

pub const KIND: &str = dalywave_protocol::jobs::kind::TRACK_MAP;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapReport {
    pub points: usize,
    /// `umap` — сервисом анализа, `pca` — запасной путь здесь, `skipped` — мало треков.
    pub method: &'static str,
    /// Сколько треков было на прошлой карте: по ним выравнивается новая.
    pub aligned: usize,
    /// Средний сдвиг выровненных треков относительно прошлой карты (в долях её размаха).
    pub shift: Option<f32>,
    pub facets: u64,
    pub took_ms: u64,
    pub note: Option<String>,
}

// ───────────── Выравнивание ─────────────

/// Преобразование подобия на плоскости: `b ≈ s·R·(a − ca) + cb`, `R` — поворот, при `flip` — с отражением.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Similarity {
    pub scale: f32,
    pub cos: f32,
    pub sin: f32,
    pub flip: bool,
    pub from: [f32; 2],
    pub to: [f32; 2],
}

impl Similarity {
    pub fn apply(&self, p: [f32; 2]) -> [f32; 2] {
        let x = p[0] - self.from[0];
        let y = (p[1] - self.from[1]) * if self.flip { -1.0 } else { 1.0 };
        [
            self.scale * (self.cos * x - self.sin * y) + self.to[0],
            self.scale * (self.sin * x + self.cos * y) + self.to[1],
        ]
    }
}

fn centroid(points: &[[f32; 2]]) -> [f32; 2] {
    let n = points.len().max(1) as f64;
    let (x, y) = points.iter().fold((0.0f64, 0.0f64), |(x, y), p| {
        (x + f64::from(p[0]), y + f64::from(p[1]))
    });
    [(x / n) as f32, (y / n) as f32]
}

/// Procrustes для пар «новая точка → прошлая»: лучшее подобие по сумме квадратов (с отражением,
/// если так ближе — UMAP и PCA не помнят, где у карты «верх»).
pub fn procrustes(new: &[[f32; 2]], old: &[[f32; 2]]) -> Option<Similarity> {
    if new.len() < 3 || new.len() != old.len() {
        return None;
    }
    let ca = centroid(new);
    let cb = centroid(old);
    let (mut sxx, mut sxy, mut syx, mut syy, mut norm) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (a, b) in new.iter().zip(old) {
        let (ax, ay) = (f64::from(a[0] - ca[0]), f64::from(a[1] - ca[1]));
        let (bx, by) = (f64::from(b[0] - cb[0]), f64::from(b[1] - cb[1]));
        sxx += ax * bx;
        sxy += ax * by;
        syx += ay * bx;
        syy += ay * by;
        norm += ax * ax + ay * ay;
    }
    if norm < 1e-12 {
        return None;
    }
    // Без отражения: максимум Σ b·R a — при угле atan2(Sxy − Syx, Sxx + Syy)
    let (c0, s0) = (sxx + syy, sxy - syx);
    // С отражением (y → −y): то же для отражённых точек
    let (c1, s1) = (sxx - syy, sxy + syx);
    let (d0, d1) = (c0.hypot(s0), c1.hypot(s1));
    let (flip, c, s, d) = if d1 > d0 {
        (true, c1, s1, d1)
    } else {
        (false, c0, s0, d0)
    };
    let angle = s.atan2(c);
    Some(Similarity {
        scale: (d / norm) as f32,
        cos: angle.cos() as f32,
        sin: angle.sin() as f32,
        flip,
        from: ca,
        to: cb,
    })
}

/// Первая карта: в квадрат 0..1 по 1-му и 99-му процентилям (выбросы — чуть за край), пропорции
/// сохраняются.
pub fn normalize(points: &mut [[f32; 2]]) {
    if points.is_empty() {
        return;
    }
    let pct = |axis: usize, q: f32| {
        let mut v: Vec<f32> = points.iter().map(|p| p[axis]).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v[((v.len() - 1) as f32 * q).round() as usize]
    };
    let (x0, x1, y0, y1) = (pct(0, 0.01), pct(0, 0.99), pct(1, 0.01), pct(1, 0.99));
    let span = (x1 - x0).max(y1 - y0).max(1e-6);
    let (ox, oy) = (
        (1.0 - (x1 - x0) / span) / 2.0,
        (1.0 - (y1 - y0) / span) / 2.0,
    );
    for p in points.iter_mut() {
        p[0] = ox + (p[0] - x0) / span;
        p[1] = oy + (p[1] - y0) / span;
    }
}

/// Новая карта в системе прошлой. Общих точек мало — первая карта ([`normalize`]). Отдаёт число
/// общих точек и средний сдвиг (в долях размаха прошлой карты).
pub fn align(points: &mut [[f32; 2]], prev: &[Option<[f32; 2]>]) -> (usize, Option<f32>) {
    let pairs: Vec<([f32; 2], [f32; 2])> = points
        .iter()
        .zip(prev)
        .filter_map(|(a, b)| b.map(|b| (*a, b)))
        .collect();
    let fit = (pairs.len() >= MIN_POINTS)
        .then(|| {
            let (a, b): (Vec<_>, Vec<_>) = pairs.iter().copied().unzip();
            procrustes(&a, &b)
        })
        .flatten();
    let Some(t) = fit else {
        normalize(points);
        return (0, None);
    };
    for p in points.iter_mut() {
        *p = t.apply(*p);
    }
    let old: Vec<[f32; 2]> = pairs.iter().map(|p| p.1).collect();
    let c = centroid(&old);
    let spread = (old
        .iter()
        .map(|p| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2))
        .sum::<f32>()
        / old.len() as f32)
        .sqrt()
        .max(1e-6);
    let moved = pairs
        .iter()
        .map(|(a, b)| {
            let p = t.apply(*a);
            ((p[0] - b[0]).powi(2) + (p[1] - b[1]).powi(2)).sqrt()
        })
        .sum::<f32>()
        / pairs.len() as f32;
    (pairs.len(), Some(moved / spread))
}

// ───────────── Запас: PCA ─────────────

/// Две главные компоненты нормированных векторов (степенной метод с ортогонализацией): запас, когда
/// сервис анализа недоступен. Детерминированно.
pub fn pca2(data: &[f32], rows: usize, dim: usize) -> Vec<[f32; 2]> {
    let unit: Vec<f64> = (0..rows)
        .flat_map(|r| {
            let v = &data[r * dim..(r + 1) * dim];
            let n = v
                .iter()
                .map(|x| f64::from(*x).powi(2))
                .sum::<f64>()
                .sqrt()
                .max(1e-12);
            v.iter().map(move |x| f64::from(*x) / n)
        })
        .collect();
    let mut mean = vec![0.0f64; dim];
    for r in 0..rows {
        for (m, x) in mean.iter_mut().zip(&unit[r * dim..(r + 1) * dim]) {
            *m += x / rows.max(1) as f64;
        }
    }
    // Xᵀ X v без матрицы ковариации: проход по строкам
    let cov_mul = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0f64; dim];
        for r in 0..rows {
            let row = &unit[r * dim..(r + 1) * dim];
            let dot: f64 = row
                .iter()
                .zip(&mean)
                .zip(v)
                .map(|((x, m), v)| (x - m) * v)
                .sum();
            for ((o, x), m) in out.iter_mut().zip(row).zip(&mean) {
                *o += dot * (x - m);
            }
        }
        out
    };
    let unit_len = |v: &mut Vec<f64>| {
        let n = v.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-12);
        v.iter_mut().for_each(|x| *x /= n);
    };
    let mut comps: Vec<Vec<f64>> = Vec::new();
    for c in 0..2 {
        let mut v: Vec<f64> = (0..dim)
            .map(|i| (((i * 7919 + c * 104_729) % 1000) as f64 / 1000.0) - 0.5)
            .collect();
        for _ in 0..60 {
            v = cov_mul(&v);
            for p in &comps {
                let d: f64 = v.iter().zip(p).map(|(a, b)| a * b).sum();
                v.iter_mut().zip(p).for_each(|(a, b)| *a -= d * b);
            }
            unit_len(&mut v);
        }
        comps.push(v);
    }
    (0..rows)
        .map(|r| {
            let row = &unit[r * dim..(r + 1) * dim];
            let proj = |p: &[f64]| -> f32 {
                row.iter()
                    .zip(&mean)
                    .zip(p)
                    .map(|((x, m), p)| (x - m) * p)
                    .sum::<f64>() as f32
            };
            [proj(&comps[0]), proj(&comps[1])]
        })
        .collect()
}

// ───────────── Пересчёт ─────────────

/// Пересчёт карты всего индекса: UMAP сервисом анализа (или PCA здесь), выравнивание к прошлой карте,
/// запись, затем грани вкуса всех слушателей.
pub async fn layout(db: &PgPool, ml: &Ml) -> Result<MapReport, sqlx::Error> {
    let started = Instant::now();
    let rows = sqlx::query!(
        r#"SELECT recording_id, sound::real[] AS "sound!", map_x, map_y FROM track_analysis
           WHERE sound IS NOT NULL ORDER BY recording_id"#
    )
    .fetch_all(db)
    .await?;
    let n = rows.len();
    let mut report = MapReport {
        points: n,
        method: "skipped",
        ..Default::default()
    };
    if n < MIN_POINTS {
        return Ok(report);
    }
    let mut ids = Vec::with_capacity(n);
    let mut prev = Vec::with_capacity(n);
    let mut data: Vec<f32> = Vec::with_capacity(n * SOUND_DIM);
    for r in rows {
        if r.sound.len() != SOUND_DIM {
            continue;
        }
        ids.push(r.recording_id);
        prev.push(r.map_x.zip(r.map_y).map(|(x, y)| [x, y]));
        data.extend(r.sound);
    }
    let n = ids.len();
    report.points = n;
    let mut points = match ml.layout(&data, n, SOUND_DIM, &prev).await {
        Ok(p) if p.len() == n => {
            report.method = "umap";
            p
        }
        other => {
            let why = match other {
                Ok(p) => format!("сервис вернул {} точек из {n}", p.len()),
                Err(e) => e.to_string(),
            };
            tracing::warn!(error = %why, "track map: ml layout unavailable, falling back to PCA");
            report.note = Some(why);
            report.method = "pca";
            let rows = n;
            tokio::task::spawn_blocking(move || pca2(&data, rows, SOUND_DIM))
                .await
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?
        }
    };
    let (aligned, shift) = align(&mut points, &prev);
    report.aligned = aligned;
    report.shift = shift;
    for (ids, pts) in ids.chunks(CHUNK).zip(points.chunks(CHUNK)) {
        let xs: Vec<f32> = pts.iter().map(|p| p[0]).collect();
        let ys: Vec<f32> = pts.iter().map(|p| p[1]).collect();
        sqlx::query!(
            "UPDATE track_analysis a SET map_x = x.x, map_y = x.y
             FROM unnest($1::uuid[], $2::real[], $3::real[]) AS x(id, x, y) WHERE a.recording_id = x.id",
            ids,
            &xs,
            &ys,
        )
        .execute(db)
        .await?;
    }
    report.facets = facets(db, None, true).await?;
    report.took_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

/// Новые треки между пересчётами: среднее [`NEIGHBOURS`] ближайших по звучанию соседей с координатами.
/// `only` — только эти записи (объяснение подбора), иначе — до `limit` любых без координат.
pub async fn fill(db: &PgPool, only: Option<&[Uuid]>, limit: i64) -> Result<u64, sqlx::Error> {
    let mut tx = db.begin().await?;
    widen_search(&mut tx, NEIGHBOURS * 4).await?;
    let done = sqlx::query!(
        r#"UPDATE track_analysis a SET map_x = n.x, map_y = n.y
           FROM (SELECT t.recording_id, avg(k.map_x)::real AS x, avg(k.map_y)::real AS y
                 FROM (SELECT recording_id, sound FROM track_analysis
                       WHERE sound IS NOT NULL AND map_x IS NULL
                         AND ($1::uuid[] IS NULL OR recording_id = ANY($1))
                       LIMIT $2) t
                 CROSS JOIN LATERAL (
                     SELECT c.map_x, c.map_y FROM (
                         SELECT b.map_x, b.map_y, b.sound <=> t.sound AS d FROM track_analysis b
                         WHERE b.sound IS NOT NULL ORDER BY b.sound <=> t.sound LIMIT $3) c
                     WHERE c.map_x IS NOT NULL ORDER BY c.d LIMIT $4) k
                 GROUP BY t.recording_id) n
           WHERE a.recording_id = n.recording_id AND n.x IS NOT NULL"#,
        only,
        limit,
        NEIGHBOURS * 4,
        NEIGHBOURS,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(done)
}

/// Грани вкуса на карте: центр — среднее [`NEIGHBOURS`] ближайших к центру грани треков с координатами,
/// радиус — средний разброс [`FACET_CLOUD`] ближайших. `user` — только его грани; `all` — и те, у кого
/// координаты уже есть (после пересчёта карты).
pub async fn facets(db: &PgPool, user: Option<Uuid>, all: bool) -> Result<u64, sqlx::Error> {
    let mut tx = db.begin().await?;
    widen_search(&mut tx, FACET_CLOUD * 4).await?;
    let done = sqlx::query!(
        r#"UPDATE taste_facets f SET map_x = n.x, map_y = n.y, map_r = n.r
           FROM (SELECT g.id, c.x, c.y,
                        (SELECT avg(sqrt(power(k.map_x - c.x, 2) + power(k.map_y - c.y, 2)))
                         FROM (SELECT b.map_x, b.map_y FROM track_analysis b
                               WHERE b.sound IS NOT NULL AND b.map_x IS NOT NULL
                               ORDER BY b.sound <=> g.centroid LIMIT $3) k)::real AS r
                 FROM taste_facets g
                 CROSS JOIN LATERAL (
                     SELECT avg(k.map_x)::real AS x, avg(k.map_y)::real AS y
                     FROM (SELECT b.map_x, b.map_y FROM track_analysis b
                           WHERE b.sound IS NOT NULL AND b.map_x IS NOT NULL
                           ORDER BY b.sound <=> g.centroid LIMIT $4) k) c
                 WHERE ($1::uuid IS NULL OR g.user_id = $1) AND ($2 OR g.map_x IS NULL)) n
           WHERE f.id = n.id AND n.x IS NOT NULL"#,
        user,
        all,
        FACET_CLOUD,
        NEIGHBOURS,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(done)
}

// ───────────── Расписание ─────────────

/// Пересчёт карты — раз в сутки ночью (3–6 по времени слушателя; сервер спал — через полтора дня), если
/// треков с анализом хватает. Каждый час — координаты новым трекам и граням без них.
pub async fn schedule(db: &PgPool, settings: &Settings) -> Result<bool, sqlx::Error> {
    if let Err(e) = fill(db, None, 500).await {
        tracing::warn!(error = %e, "track map: filling new tracks failed");
    }
    if let Err(e) = facets(db, None, false).await {
        tracing::warn!(error = %e, "track map: facet points failed");
    }
    let hour = settings.local_hour(Utc::now());
    let hours: i32 = if (3..6).contains(&hour) { 20 } else { 36 };
    let due = sqlx::query_scalar!(
        r#"SELECT NOT EXISTS (SELECT 1 FROM jobs WHERE kind = $1
                                AND (status IN ('queued', 'running')
                                     OR (status IN ('done', 'failed') AND finished_at > now() - make_interval(hours => $2))))
                  AND (SELECT count(*) FROM track_analysis WHERE sound IS NOT NULL) >= $3
                  AS "due!""#,
        KIND,
        hours,
        MIN_POINTS as i64,
    )
    .fetch_one(db)
    .await?;
    if !due {
        return Ok(false);
    }
    let mut conn = db.acquire().await?;
    Ok(jobs::enqueue(
        &mut conn,
        NewJob::new(KIND, lanes::MAINTENANCE, serde_json::json!({})).dedupe(KIND),
    )
    .await?
    .created)
}

/// Задача `track_map`.
pub async fn run(ctx: Arc<Context>, _job: JobRow) -> JobResult {
    let report = layout(&ctx.db, &ctx.ml).await?;
    tracing::info!(
        points = report.points,
        method = report.method,
        aligned = report.aligned,
        took_ms = report.took_ms,
        "track map rebuilt"
    );
    Ok(serde_json::to_value(report)?.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }

    #[test]
    fn procrustes_recovers_rotation_scale_and_flip() {
        let old: Vec<[f32; 2]> = (0..30)
            .map(|i| {
                let t = i as f32 * 0.37;
                [t.cos() * (1.0 + 0.1 * i as f32), (2.0 * t).sin()]
            })
            .collect();
        for flip in [false, true] {
            // Новая карта — та же, но повёрнута, растянута, сдвинута (и отражена)
            let (a, s) = (1.1f32, 3.0f32);
            let new: Vec<[f32; 2]> = old
                .iter()
                .map(|p| {
                    let y = if flip { -p[1] } else { p[1] };
                    [
                        s * (a.cos() * p[0] - a.sin() * y) + 5.0,
                        s * (a.sin() * p[0] + a.cos() * y) - 2.0,
                    ]
                })
                .collect();
            let t = procrustes(&new, &old).unwrap();
            assert_eq!(t.flip, flip);
            for (n, o) in new.iter().zip(&old) {
                assert!(
                    close(t.apply(*n), *o),
                    "{:?} → {:?} vs {o:?}",
                    n,
                    t.apply(*n)
                );
            }
        }
    }

    #[test]
    fn align_keeps_old_frame_and_normalizes_first_map() {
        let mut first: Vec<[f32; 2]> = (0..40).map(|i| [i as f32 * 10.0, (i % 7) as f32]).collect();
        let none = vec![None; first.len()];
        assert_eq!(align(&mut first, &none), (0, None));
        assert!(
            first
                .iter()
                .all(|p| (-0.1..=1.1).contains(&p[0]) && (-0.1..=1.1).contains(&p[1]))
        );
        // Та же карта, повёрнутая на 90° — после выравнивания на старых местах, сдвиг ≈ 0
        let prev: Vec<Option<[f32; 2]>> = first.iter().map(|p| Some(*p)).collect();
        let mut turned: Vec<[f32; 2]> = first.iter().map(|p| [-p[1] * 4.0, p[0] * 4.0]).collect();
        let (n, shift) = align(&mut turned, &prev);
        assert_eq!(n, 40);
        assert!(shift.unwrap() < 1e-3);
        for (a, b) in turned.iter().zip(&first) {
            assert!(close(*a, *b));
        }
    }

    #[test]
    fn pca_separates_two_groups() {
        // Две группы векторов: по первой компоненте они по разные стороны
        let dim = 16;
        let mut data = Vec::new();
        for i in 0..20 {
            for d in 0..dim {
                let base = if i < 10 {
                    (d < 8) as i32 as f32
                } else {
                    (d >= 8) as i32 as f32
                };
                data.push(base + 0.01 * ((i * 31 + d * 7) % 10) as f32);
            }
        }
        let p = pca2(&data, 20, dim);
        let left = p[..10].iter().all(|q| q[0] < 0.0);
        let right = p[10..].iter().all(|q| q[0] > 0.0);
        assert!(
            left && right
                || p[..10].iter().all(|q| q[0] > 0.0) && p[10..].iter().all(|q| q[0] < 0.0)
        );
    }
}
