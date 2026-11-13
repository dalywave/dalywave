//! Место припева в треке: самое энергичное окно по кривой энергии полного анализа, без него — 30 %
//! трека. Нужно музыке «Итогов года» (25 секунд на слайд) и отрывку «послушать припев» перед
//! включением (15 секунд).

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

/// Окно, в котором ищется припев, секунд.
const WINDOW_S: usize = 20;
/// Без анализа — с этой доли трека.
const FALLBACK_SHARE: f64 = 0.3;
/// Окно припева должно быть заметно громче самого тихого окна, иначе «припев» — случайность.
const MIN_CONTRAST: i64 = 60;

/// Что известно о форме трека из полного анализа.
#[derive(Debug, Clone, Default)]
pub struct Shape {
    /// Энергия по секунде, 0..1000.
    pub curve: Vec<i16>,
    pub downbeats_ms: Vec<i32>,
    pub intro_end_ms: Option<i32>,
    pub outro_start_ms: Option<i32>,
}

/// Без анализа: 30 % трека, но так, чтобы после начала оставалось `tail_ms` звука.
pub fn fallback_start(duration_ms: i64, tail_ms: i64) -> i64 {
    let at = (duration_ms as f64 * FALLBACK_SHARE) as i64;
    at.min(duration_ms - tail_ms).max(0)
}

/// Начало припева: самое энергичное окно в середине трека (из почти равных — первое: первый припев
/// узнаётся лучше последнего), уточнённое по подъёму энергии и сдвинутое на ближайшую сильную долю.
/// После начала остаётся не меньше `tail_ms` звука. `None` — кривая не годится (только превью или
/// трек ровный).
pub fn chorus_start(duration_ms: i64, shape: &Shape, tail_ms: i64) -> Option<i64> {
    let curve = &shape.curve;
    let n = curve.len();
    let dur_s = usize::try_from(duration_ms / 1000).ok()?;
    // Кривая 30-секундного превью о полном треке ничего не говорит
    if n < 60 || dur_s == 0 || n * 10 < dur_s * 8 {
        return None;
    }
    let secs = |ms: Option<i32>| ms.map(|m| usize::try_from(m.max(0) / 1000).unwrap_or(0));
    let lo = secs(shape.intro_end_ms)
        .unwrap_or(0)
        .max(n * 12 / 100)
        .max(8);
    let end = secs(shape.outro_start_ms)
        .filter(|&o| o > lo)
        .unwrap_or(n)
        .min(n * 88 / 100);
    if end < lo + WINDOW_S {
        return None;
    }
    let mut pre = vec![0i64; n + 1];
    for (i, v) in curve.iter().enumerate() {
        pre[i + 1] = pre[i] + i64::from((*v).clamp(0, 1000));
    }
    let sum = |s: usize| pre[s + WINDOW_S] - pre[s];
    let starts = lo..=end - WINDOW_S;
    let best = starts.clone().map(sum).max()?;
    let worst = starts.clone().map(sum).min()?;
    if best - worst < MIN_CONTRAST * WINDOW_S as i64 {
        return None;
    }
    let s = starts.clone().find(|&s| sum(s) * 100 >= best * 92)?;
    // Окно могло захватить конец куплета: начало — там, где энергия поднимается сильнее всего
    let mean = |a: usize, b: usize| (pre[b] - pre[a]) / (b - a) as i64;
    let rise = (s.max(4)..=(s + WINDOW_S / 2).min(n - 4))
        .map(|i| (mean(i, i + 4) - mean(i - 4, i), i))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .filter(|(d, _)| *d > 0)
        .map_or(s, |(_, i)| i);
    let at = rise as i64 * 1000;
    // Кривая — по секунде, доли точнее: ближайшая сильная доля, лучше чуть раньше, чем позже
    let snapped = shape
        .downbeats_ms
        .iter()
        .map(|&d| i64::from(d))
        .filter(|&d| d >= at - 2000 && d <= at + 1000)
        .min_by_key(|&d| if d <= at { (at - d) * 2 } else { (d - at) * 3 })
        .unwrap_or(at);
    Some(snapped.min(duration_ms - tail_ms).max(0))
}

/// Место начала для каждого трека: по полному анализу (`true`) или 30 % трека (`false`).
pub async fn starts(
    db: &PgPool,
    ids: &[Uuid],
    tail_ms: i64,
) -> Result<HashMap<Uuid, (i64, bool)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT t.id AS "id!", COALESCE(t.duration_ms, 0) AS "duration_ms!", a.source AS "source?",
                  a.energy_curve AS "curve?", a.downbeats_ms AS "downbeats?",
                  a.intro_end_ms AS "intro_end_ms?", a.outro_start_ms AS "outro_start_ms?"
           FROM tracks t LEFT JOIN track_analysis a ON a.recording_id = t.recording_id
           WHERE t.id = ANY($1)"#,
        ids,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let dur = i64::from(r.duration_ms);
            let analysed = (r.source.as_deref() == Some("full"))
                .then(|| Shape {
                    curve: r.curve.unwrap_or_default(),
                    downbeats_ms: r.downbeats.unwrap_or_default(),
                    intro_end_ms: r.intro_end_ms,
                    outro_start_ms: r.outro_start_ms,
                })
                .and_then(|shape| chorus_start(dur, &shape, tail_ms));
            let start = match analysed {
                Some(at) => (at, true),
                None => (fallback_start(dur, tail_ms), false),
            };
            (r.id, start)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAIL: i64 = 25_000;

    /// Куплет 300, припев 800 (50–70 с), куплет, второй припев 820 (110–130 с), бридж, тихий финал.
    fn song() -> Vec<i16> {
        let mut c = vec![300i16; 200];
        c[50..70].fill(800);
        c[110..130].fill(820);
        c[130..160].fill(400);
        c[160..].fill(200);
        c
    }

    #[test]
    fn first_chorus_wins_over_a_slightly_louder_last_one() {
        let shape = Shape {
            curve: song(),
            downbeats_ms: (0..100).map(|i| i * 2000).collect(),
            ..Default::default()
        };
        assert_eq!(chorus_start(200_000, &shape, TAIL), Some(50_000));
    }

    #[test]
    fn start_snaps_to_the_nearest_downbeat_preferring_earlier() {
        // Доли сдвинуты на 0,4 с: 48,4 и 50,4 — ближе и не позже чем на секунду — 50,4
        let shape = Shape {
            curve: song(),
            downbeats_ms: (0..100).map(|i| 400 + i * 2000).collect(),
            ..Default::default()
        };
        assert_eq!(chorus_start(200_000, &shape, TAIL), Some(50_400));
        // Без сетки долей — по секунде кривой
        let shape = Shape {
            curve: song(),
            ..Default::default()
        };
        assert_eq!(chorus_start(200_000, &shape, TAIL), Some(50_000));
    }

    #[test]
    fn loud_intro_and_outro_are_skipped() {
        let mut curve = song();
        curve[..20].fill(1000);
        curve[175..].fill(1000);
        let shape = Shape {
            curve,
            intro_end_ms: Some(20_000),
            outro_start_ms: Some(175_000),
            ..Default::default()
        };
        assert_eq!(chorus_start(200_000, &shape, TAIL), Some(50_000));
    }

    #[test]
    fn preview_or_flat_curves_fall_back() {
        let preview = Shape {
            curve: vec![500; 30],
            ..Default::default()
        };
        assert_eq!(chorus_start(200_000, &preview, TAIL), None);
        let flat = Shape {
            curve: vec![500; 200],
            ..Default::default()
        };
        assert_eq!(chorus_start(200_000, &flat, TAIL), None);
        assert_eq!(fallback_start(200_000, TAIL), 60_000);
        // Короткий трек: слайду нужно 25 секунд звука
        assert_eq!(fallback_start(30_000, TAIL), 5_000);
        assert_eq!(fallback_start(10_000, TAIL), 0);
        // Отрывку перед включением хватает 15 секунд
        assert_eq!(fallback_start(30_000, 15_000), 9_000);
    }

    #[test]
    fn chorus_leaves_the_asked_tail() {
        let mut curve = vec![200i16; 100];
        curve[70..90].fill(900);
        let shape = Shape {
            curve,
            ..Default::default()
        };
        // Припев на 70-й секунде из 100, но слайду нужно 25 секунд — не позже 75
        assert_eq!(chorus_start(100_000, &shape, TAIL), Some(70_000));
        // Отрывку — 15 секунд: тоже 70, а с хвостом в 40 секунд — уже 60
        assert_eq!(chorus_start(100_000, &shape, 15_000), Some(70_000));
        assert_eq!(chorus_start(100_000, &shape, 40_000), Some(60_000));
    }
}
