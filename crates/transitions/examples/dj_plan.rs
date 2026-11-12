//! Разбор «Диджея» без звука: что планировщик видит в анализе треков и какой переход выбирает
//! для каждой пары. Для проверки на реальных треках и для журнала «почему так».
//!
//! ```sh
//! cargo run -p dalywave-transitions --example dj_plan -- <каталог треков> [выразительность]
//! ```
//! В каталоге — `index.json` (`[{ "file", "analysis", "title" }]`) и ответы сервиса анализа.

use std::path::PathBuf;

use dalywave_transitions::{Analysis, PlanInput, plan};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().expect("каталог треков"));
    let x: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.6);
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).expect("index"))
            .expect("index.json");
    let tracks: Vec<(String, Analysis)> = index
        .as_array()
        .expect("массив")
        .iter()
        .map(|e| {
            let a: Analysis = serde_json::from_str(
                &std::fs::read_to_string(dir.join(e["analysis"].as_str().unwrap_or_default()))
                    .expect("analysis"),
            )
            .expect("analysis json");
            let name = e["file"]
                .as_str()
                .unwrap_or_default()
                .rsplit_once('.')
                .map_or("", |s| s.0)
                .to_string();
            (name, a)
        })
        .collect();
    for (name, a) in &tracks {
        let r = a.refined();
        let g = r.grid();
        let (first, last) = r.audible();
        let p = r.phrases();
        println!(
            "{name:16} {:6.1} с  {}  фраз {:2}  звук {:5.1}…{:5.1}  музыка до {:5.1}  вступл. {:5.1}  концовка {:5.1}  голос до {:5.1}",
            a.duration() / 1000.0,
            g.map_or("сетки нет".to_string(), |g| format!(
                "{:6.1} BPM ув. {:.2} ровн. {:.2} ({:.2} в концовке) {}/4",
                g.bpm,
                g.confidence,
                g.regularity,
                r.regularity_between(a.duration() - 60_000.0, a.duration()),
                g.beats_per_bar
            )),
            p.starts.len(),
            first / 1000.0,
            last / 1000.0,
            r.music_end() / 1000.0,
            a.intro_end_ms.unwrap_or(-1000.0) / 1000.0,
            a.outro_start_ms.unwrap_or(-1000.0) / 1000.0,
            a.vocal_end() / 1000.0,
        );
    }
    println!();
    let mut fallbacks = 0;
    let mut total = 0;
    for (an, a) in &tracks {
        for (bn, b) in &tracks {
            if an == bn {
                continue;
            }
            let p = plan(&PlanInput {
                out: a,
                inc: b,
                out_duration_ms: a.duration_ms,
                in_duration_ms: b.duration_ms,
                expressiveness: x,
                earliest_ms: 0.0,
            });
            total += 1;
            if p.technique == dalywave_transitions::Technique::Fade {
                fallbacks += 1;
            }
            println!(
                "{an:>16} → {bn:16} {:6}  конец перехода за {:5.1} с до конца  {}",
                p.technique.as_str(),
                (a.duration() - p.start_ms - p.length_ms.max(p.in_offset_ms)) / 1000.0,
                p.note
            );
        }
    }
    println!("\n«Умный» (страховка): {fallbacks} из {total}");
}
