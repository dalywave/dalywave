//! Переходы «Диджея» на реальных треках — в WAV, без колонок (проверки звука —
//! только рендером в файл). Для каждой пары: движок в тестовом режиме вывода сам выбирает переход,
//! готовит и растягивает входящий; общий звук пишется в `<out>/NN-<a>__<b>-<приём>.wav` (с 25 с до
//! перехода до 25 с после), для сведения по долям дорожки уходящего и входящего рендерятся ещё и
//! по отдельности, и их атаки на сильных долях сверяются (норма — расхождение до 10 мс).
//!
//! ```sh
//! cargo run --release --example dj_render -- <каталог треков> <каталог вывода> \
//!     01-devil:03-yashchiki@0.2 03-yashchiki:01-devil ...
//! ```
//! В каталоге треков — `index.json` (`[{ "file", "analysis", "title", "artist" }]`) и ответы
//! сервиса анализа `<slug>.analysis.json`. `@0.2` — выразительность пары (по умолчанию 0.5).
//! Отчёт — `<out>/report.txt`; код выхода 1, если сведение по долям где-то разошлось.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dalywave_audio::testkit::{downbeat_alignment, render_dj, write_wav};
use dalywave_audio::transitions::{PlanInput, plan};
use dalywave_audio::{Analysis, TrackSpec};

const RATE: u32 = 48_000;
const MARGIN_MS: f64 = 25_000.0;

struct Track {
    slug: String,
    title: String,
    spec: TrackSpec,
    analysis: Arc<Analysis>,
}

fn load(dir: &Path, slug: &str) -> Track {
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).expect("index.json"))
            .expect("index.json");
    let entry = index
        .as_array()
        .and_then(|a| {
            a.iter().find(|e| {
                e["file"]
                    .as_str()
                    .is_some_and(|f| f.starts_with(&format!("{slug}.")))
            })
        })
        .unwrap_or_else(|| panic!("нет трека {slug} в index.json"));
    let file = dir.join(entry["file"].as_str().unwrap_or_default());
    let analysis: Analysis = serde_json::from_str(
        &std::fs::read_to_string(dir.join(entry["analysis"].as_str().unwrap_or_default()))
            .expect("analysis"),
    )
    .expect("analysis json");
    let analysis = Arc::new(analysis);
    let mut spec = TrackSpec::file(slug, file);
    spec.title = entry["title"].as_str().unwrap_or(slug).to_string();
    spec.artist = entry["artist"].as_str().unwrap_or_default().to_string();
    spec.duration_ms = analysis.duration_ms.map(|d| d as u64);
    spec.analysis = Some(analysis.clone());
    Track {
        slug: slug.to_string(),
        title: format!("{} — {}", spec.artist, spec.title),
        spec,
        analysis,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("dj_render <каталог треков> <каталог вывода> a:b[@выразительность] ...");
        std::process::exit(2);
    }
    let tracks = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out).expect("out dir");
    let mut report = String::new();
    let mut failed = false;
    for (n, pair) in args[2..].iter().enumerate() {
        let (names, expr) = match pair.split_once('@') {
            Some((p, e)) => (p, e.parse::<f64>().unwrap_or(0.5)),
            None => (pair.as_str(), 0.5),
        };
        let Some((a, b)) = names.split_once(':') else {
            eprintln!("пара — «a:b»: {pair}");
            continue;
        };
        let (a, b) = (load(&tracks, a), load(&tracks, b));
        // Тот же выбор, что сделает движок: старт рендера за 25 с до перехода
        let expected = plan(&PlanInput {
            out: &a.analysis,
            inc: &b.analysis,
            out_duration_ms: a.analysis.duration_ms,
            in_duration_ms: b.analysis.duration_ms,
            expressiveness: expr,
            earliest_ms: 0.0,
        });
        let from_ms = (expected.start_ms - MARGIN_MS).max(0.0) as u64;
        let expressiveness = (expr.clamp(0.0, 1.0) * 100.0).round() as u8;
        let beatmatched = expected.is_beatmatched();
        let r = match render_dj(
            &a.spec,
            &b.spec,
            from_ms,
            RATE,
            expressiveness,
            beatmatched,
            Duration::from_secs(600),
        ) {
            Ok(r) => r,
            Err(e) => {
                let _ = writeln!(
                    report,
                    "{n:02} {} → {}: ошибка рендера: {e}",
                    a.slug, b.slug
                );
                failed = true;
                continue;
            }
        };
        let p = &r.plan;
        let end_ms = p.start_ms - from_ms as f64 + p.length_ms.max(p.in_offset_ms) + MARGIN_MS;
        let end = ((end_ms / 1000.0 * RATE as f64) as usize * 2).min(r.mix.len());
        let name = format!("{:02}-{}__{}-{}.wav", n + 1, a.slug, b.slug, p.technique);
        write_wav(&out.join(&name), RATE, &r.mix[..end]).expect("wav");
        let _ = writeln!(
            report,
            "{:02} {}\n   {} → {}\n   выразительность {expr:.1}: {}\n   переход в {:.1} с файла, {:.1} с от начала WAV",
            n + 1,
            name,
            a.title,
            b.title,
            p.note,
            p.start_ms / 1000.0,
            (p.start_ms - from_ms as f64) / 1000.0
        );
        if beatmatched {
            // Где по анализу (уточнённая сетка, как у движка) и карте темпа должны звучать доли
            let from = from_ms as f64;
            let out_beats: Vec<f64> = a
                .analysis
                .refined()
                .beats_ms
                .iter()
                .map(|t| t - from)
                .collect();
            let in_start = p.start_ms + p.in_offset_ms - from;
            let in_beats: Vec<f64> = b
                .analysis
                .refined()
                .beats_ms
                .iter()
                .map(|s| in_start + p.tempo.out_at(*s))
                .collect();
            let rep = downbeat_alignment(
                &r.outgoing,
                &r.incoming,
                RATE,
                &out_beats,
                &in_beats,
                in_start..p.start_ms + p.length_ms - from,
                10.0,
            );
            let ok = rep.checked > 0 && rep.share() >= 0.8 && rep.median_ms <= 10.0;
            failed |= !ok;
            let offsets: Vec<String> = rep.offsets.iter().map(|o| format!("{o:+.0}")).collect();
            let _ = writeln!(
                report,
                "   сильные доли: {} — {} (входящий − уходящий, мс: {})",
                rep.summary(),
                if ok { "ок" } else { "РАСХОДЯТСЯ" },
                offsets.join(" ")
            );
        }
        println!(
            "{}",
            report.lines().rev().take(4).collect::<Vec<_>>().join(" | ")
        );
    }
    std::fs::write(out.join("report.txt"), &report).expect("report");
    print!("{report}");
    if failed {
        std::process::exit(1);
    }
}
