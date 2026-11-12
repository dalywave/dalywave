//! Переходы «Диджея» на реальных треках — в WAV, без колонок (проверки звука —
//! только рендером в файл). Для каждой пары движок в тестовом режиме вывода сам выбирает переход,
//! готовит и растягивает входящий; общий звук пишется в `<out>/NN-<a>__<b>-<приём>.wav` (с 20 с
//! до перехода до 20 с после). Дорожки уходящего и входящего рендерятся ещё и по отдельности, и
//! по ним проверяется:
//!
//! - доли совпадают: при сведении по долям — атаки обеих дорожек в окне наложения (норма —
//!   расхождение до 10 мс); при эхо-выходе и резкой смене — последняя сильная доля уходящего и
//!   первая входящего ложатся на одну сетку;
//! - переход начинается на границе фразы уходящего (или на середине фразы — 4 такта);
//! - нет клиппинга (пик общего звука ниже 0 дБFS);
//! - темп возвращается: по карте темпа скорость после возврата — единица, а атаки входящего и
//!   после возврата лежат там, где их обещает карта (сдвиг не уплывает больше чем на 10 мс).
//!
//! ```sh
//! cargo run --release --example dj_render -- <каталог треков> <каталог вывода> \
//!     devil:yashchiki@0.6 gde-papa:reckless ...
//! ```
//! В каталоге треков — `index.json` (`[{ "file", "analysis", "title", "artist" }]`) и ответы
//! сервиса анализа. `@0.6` — выразительность пары (по умолчанию 0.6, как в приложении).
//! Отчёт — `<out>/report.txt` и `<out>/README.md`; код выхода 1, если какая-то проверка не прошла.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dalywave_audio::testkit::{
    Decoded, decode_all, downbeat_alignment, grid_lags, kick_lags, render_dj, resample_all,
    write_wav,
};
use dalywave_audio::transitions::{PlanInput, plan};
use dalywave_audio::{Analysis, TrackSpec};

const RATE: u32 = 48_000;
const MARGIN_MS: f64 = 20_000.0;
/// Доли в наложении — до 10 мс (флэм на слух начинается примерно с 10–20 мс).
const TOLERANCE_MS: f64 = 10.0;

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

/// Файл целиком на частоте рендера: детектор атак считает по миллисекундам целым числом
/// сэмплов, и на 44,1 кГц его шкала уплывала бы на 0,2 %.
fn decode(spec: &TrackSpec) -> Decoded {
    let mut d = decode_all(&spec.source).expect("decode");
    if d.rate != RATE {
        d.samples = resample_all(&d.samples, d.rate, RATE).expect("resample");
        d.rate = RATE;
    }
    d
}

fn mmss(ms: f64) -> String {
    let s = (ms / 1000.0).max(0.0);
    format!("{}:{:04.1}", (s / 60.0).floor() as u32, s % 60.0)
}

fn technique_ru(t: &str) -> &'static str {
    match t {
        "blend" => "сведение по долям со сменой баса",
        "filter" => "сведение по долям через фильтр",
        "echo" => "эхо-выход на сильную долю",
        "cut" => "резкая смена на сильную долю",
        _ => "страховка «Умный» (наложение)",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("dj_render <каталог треков> <каталог вывода> a:b[@выразительность] ...");
        std::process::exit(2);
    }
    let tracks = PathBuf::from(&args[0]);
    if args[1] == "--grid" {
        // Насколько сетка анализа (после починки и уточнения) лежит на атаках самого трека:
        // сдвиг окнами по 16 долей от начала до конца
        for slug in &args[2..] {
            let t = load(&tracks, slug);
            let d = decode(&t.spec);
            let beats = t.analysis.refined().beats_ms;
            for (what, lags) in [
                (
                    "атаки",
                    grid_lags(&d.samples, d.rate, &beats, 0.0..f64::MAX),
                ),
                (
                    "бочка",
                    kick_lags(&d.samples, d.rate, &beats, 0.0..f64::MAX),
                ),
            ] {
                let text: Vec<String> = lags
                    .iter()
                    .map(|l| format!("{} {:+.0}", mmss(l.0), l.1))
                    .collect();
                println!("{slug} {what}: {}", text.join(", "));
            }
        }
        return;
    }
    let out = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out).expect("out dir");
    let mut report = String::new();
    let mut readme = String::from(
        "# «Диджей»: переходы на реальных треках\n\n\
         Каждый файл — от 20 с до перехода до 20 с после (WAV, 48 кГц). Переход выбирал сам \
         движок, как в приложении; время перехода — от начала файла.\n\n\
         | Файл | Пара | Приём | Где в файле | Такт уходящего |\n|---|---|---|---|---|\n",
    );
    let mut notes = String::new();
    let mut failed = false;
    for (n, pair) in args[2..].iter().enumerate() {
        let (names, expr) = match pair.split_once('@') {
            Some((p, e)) => (p, e.parse::<f64>().unwrap_or(0.6)),
            None => (pair.as_str(), 0.6),
        };
        let Some((a, b)) = names.split_once(':') else {
            eprintln!("пара — «a:b»: {pair}");
            continue;
        };
        let (a, b) = (load(&tracks, a), load(&tracks, b));
        // Тот же выбор, что сделает движок: старт рендера за 20 с до перехода
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
        let r = match render_dj(
            &a.spec,
            &b.spec,
            from_ms,
            RATE,
            expressiveness,
            true,
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
        let from = from_ms as f64;
        let beatmatched = matches!(p.technique.as_str(), "blend" | "filter");
        let in_start = p.start_ms + p.in_offset_ms - from;
        let mix_end = p.start_ms - from + p.length_ms.max(p.in_offset_ms);
        let end_ms = mix_end
            + MARGIN_MS.max(if beatmatched {
                // Сведение по долям — до конца возврата темпа
                in_start + p.tempo.settled_at() - mix_end + 4000.0
            } else {
                0.0
            });
        let end = ((end_ms / 1000.0 * RATE as f64) as usize * 2).min(r.mix.len());
        let name = format!("{:02}-{}__{}-{}.wav", n + 1, a.slug, b.slug, p.technique);
        write_wav(&out.join(&name), RATE, &r.mix[..end]).expect("wav");

        // Где по анализу (уточнённая сетка, как у движка) и карте темпа звучат доли
        let ra = a.analysis.refined();
        let rb = b.analysis.refined();
        let out_beats: Vec<f64> = ra.beats_ms.iter().map(|t| t - from).collect();
        let in_beats: Vec<f64> = rb
            .beats_ms
            .iter()
            .map(|s| in_start + p.tempo.out_at(*s))
            .collect();
        let mut checks: Vec<(bool, String)> = Vec::new();

        // 1. Доли. Главная проверка — где движок ставит удары: атаки каждой дорожки рендера
        // против атак того же места её файла без обработки (один и тот же детектор, окнами по 8
        // долей), разница уходящего и входящего — насколько разошлись их доли из-за движка
        // (сетка анализа считается верной). Справочно — сверка по атакам верха напрямую: в треках,
        // где верх занят хэтами между долями (фонк, фанк), она путается.
        let file_out = decode(&a.spec);
        let file_in = decode(&b.spec);
        // Ошибка движка в окне из 8 долей с номера k: рендер против файла
        let placement = |stem: &[f32],
                         render_beats: &[f64],
                         file: &Decoded,
                         file_beats: &[f64],
                         k: usize,
                         n: usize|
         -> Option<f64> {
            let w = k..(k + n).min(render_beats.len()).min(file_beats.len());
            if w.len() < 4 {
                return None;
            }
            let span = |v: &[f64]| v[w.start] - 1.0..v[w.end - 1] + 1.0;
            let st = grid_lags(stem, RATE, render_beats, span(render_beats));
            let fl = grid_lags(&file.samples, file.rate, file_beats, span(file_beats));
            Some(st.first()?.1 - fl.first()?.1)
        };
        let beat_index = |beats: &[f64], t: f64| beats.partition_point(|x| *x < t - 1.0);
        if beatmatched {
            let mut diffs: Vec<(f64, f64)> = Vec::new();
            let mut k = beat_index(&out_beats, in_start);
            while k + 4 <= out_beats.len() && out_beats[k] < p.start_ms + p.length_ms - from {
                let j = beat_index(&in_beats, out_beats[k]);
                if let (Some(o), Some(i)) = (
                    placement(&r.outgoing, &out_beats, &file_out, &ra.beats_ms, k, 8),
                    placement(&r.incoming, &in_beats, &file_in, &rb.beats_ms, j, 8),
                ) {
                    diffs.push((out_beats[k], i - o));
                }
                k += 8;
            }
            let worst = diffs.iter().map(|d| d.1.abs()).fold(0.0, f64::max);
            checks.push((
                worst <= TOLERANCE_MS,
                if diffs.is_empty() {
                    "доли в наложении (движок): не измерить — во вступлении входящего нет ударов"
                        .to_string()
                } else {
                    format!(
                    "доли в наложении (движок): {} окон по 8 долей, худшее расхождение {worst:.1} мс \
                     ({})",
                    diffs.len(),
                    diffs
                        .iter()
                        .map(|d| format!("{}: {:+.1}", mmss(d.0), d.1))
                        .collect::<Vec<_>>()
                        .join(", ")
                    )
                },
            ));
            let rep = downbeat_alignment(
                &r.outgoing,
                &r.incoming,
                RATE,
                &out_beats,
                &in_beats,
                in_start..p.start_ms + p.length_ms - from,
                TOLERANCE_MS,
            );
            let offsets: Vec<String> = rep.offsets.iter().map(|o| format!("{o:+.0}")).collect();
            checks.push((
                true,
                format!(
                    "справочно, по атакам верха: {} (входящий − уходящий, мс: {})",
                    rep.summary(),
                    offsets.join(" ")
                ),
            ));
        } else if p.technique != "fade" {
            // Последнее окно уходящего перед срезом и первое входящего после него
            let ko = beat_index(&out_beats, in_start).saturating_sub(16);
            let ki = beat_index(&in_beats, in_start);
            let (o, i) = (
                placement(&r.outgoing, &out_beats, &file_out, &ra.beats_ms, ko, 16),
                placement(&r.incoming, &in_beats, &file_in, &rb.beats_ms, ki, 16),
            );
            match (o, i) {
                (Some(o), Some(i)) => checks.push((
                    (i - o).abs() <= TOLERANCE_MS,
                    format!(
                        "сильная доля на сильную долю (движок): последние доли уходящего и первые \
                         входящего разошлись на {:+.1} мс",
                        i - o
                    ),
                )),
                _ => checks.push((
                    true,
                    format!(
                        "сильная доля на сильную долю (движок): не измерить — нет ударов {}",
                        if o.is_none() {
                            "у уходящего перед срезом"
                        } else {
                            "в начале входящего"
                        }
                    ),
                )),
            }
        }

        // 2. Граница фразы
        let phrases = ra.phrases();
        let on_phrase = |t: f64| phrases.starts.iter().any(|q| (q - t).abs() < 1.0);
        let on_half = |t: f64| {
            phrases.starts.iter().any(|q| {
                let i = ra.downbeats_ms.partition_point(|d| *d < q - 1.0);
                ra.downbeats_ms
                    .get(i + 4)
                    .is_some_and(|d| (d - t).abs() < 1.0)
            })
        };
        // Эхо-выход начинается за такт до среза: граница фразы — сам срез
        let edge = if beatmatched {
            p.start_ms
        } else {
            p.start_ms + p.in_offset_ms
        };
        let bar_no = ra.downbeats_ms.partition_point(|d| *d < edge - 1.0) + 1;
        if p.technique != "fade" {
            checks.push((
                on_phrase(edge) || on_half(edge),
                format!(
                    "граница фразы: {} — такт {bar_no} уходящего ({})",
                    mmss(edge),
                    if on_phrase(edge) {
                        "начало фразы из 8 тактов"
                    } else if on_half(edge) {
                        "середина фразы, 4 такта"
                    } else {
                        "НЕ на границе фразы"
                    }
                ),
            ));
        }

        // 3. Клиппинг
        let peak = r.mix.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let over = r.mix.iter().filter(|x| x.abs() >= 0.999).count();
        checks.push((
            over == 0,
            format!(
                "клиппинг: пик {:.1} дБFS, сэмплов у 0 дБFS — {over}",
                20.0 * f64::from(peak.max(1e-9)).log10()
            ),
        ));

        // 4. Темп: атаки входящего в рендере (растяжение и возврат) против атак того же места
        // файла без обработки — окнами по 8 долей от старта до конца возврата и немного после.
        // Разница — ошибка самого растяжения, без погрешности сетки анализа.
        if beatmatched {
            let settled = p.tempo.settled_at();
            let rate_after = p.tempo.rate_at(settled + 1.0);
            let file = &file_in;
            let first = rb.beats_ms.partition_point(|s| *s < p.tempo.start() - 1.0);
            let mut diffs: Vec<(f64, f64)> = Vec::new();
            let mut k = first;
            while k + 8 < rb.beats_ms.len()
                && in_beats[k] < (in_start + settled + 12_000.0).min(end_ms)
            {
                let w = k..k + 8;
                let span = |v: &[f64]| v[w.start] - 1.0..v[w.end - 1] + 1.0;
                let stem = grid_lags(&r.incoming, RATE, &in_beats, span(&in_beats));
                let own = grid_lags(&file.samples, file.rate, &rb.beats_ms, span(&rb.beats_ms));
                if let (Some(a), Some(o)) = (stem.first(), own.first()) {
                    diffs.push((in_beats[k], a.1 - o.1));
                }
                k += 8;
            }
            // Одиночное окно может спутать удар с соседним — норма: 80 % окон в пределах 10 мс
            let within = diffs.iter().filter(|d| d.1.abs() <= TOLERANCE_MS).count();
            checks.push((
                (rate_after - 1.0).abs() < 1e-9
                    && !diffs.is_empty()
                    && within * 5 >= diffs.len() * 4,
                format!(
                    "темп: входящий ×{:.3} в наложении, к своему темпу возвращается к {} файла \
                     (×{rate_after:.3}); атаки входящего против файла без растяжения: {within} из \
                     {} окон в пределах 10 мс ({})",
                    p.tempo.rate_at(1.0),
                    mmss(in_start + settled),
                    diffs.len(),
                    diffs
                        .iter()
                        .map(|l| format!("{}: {:+.0}", mmss(l.0), l.1))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }

        let ok = checks.iter().all(|c| c.0);
        failed |= !ok;
        let _ = writeln!(
            report,
            "{:02} {name}\n   {} → {}\n   выразительность {expr:.1}: {}\n   переход в {} файла уходящего, {} от начала WAV",
            n + 1,
            a.title,
            b.title,
            p.note,
            mmss(p.start_ms),
            mmss(p.start_ms - from),
        );
        for (c_ok, text) in &checks {
            let _ = writeln!(report, "   [{}] {text}", if *c_ok { "ок" } else { "НЕТ" });
        }
        let _ = writeln!(
            readme,
            "| `{name}` | {} → {} | {}{} | {} | {bar_no} |",
            a.title,
            b.title,
            technique_ru(&p.technique),
            if beatmatched {
                format!(
                    ", {} {}, входящий ×{:.3}",
                    expected.bars,
                    if expected.bars < 5 {
                        "такта"
                    } else {
                        "тактов"
                    },
                    p.tempo.rate_at(1.0)
                )
            } else {
                String::new()
            },
            mmss(p.start_ms - from),
        );
        let _ = writeln!(notes, "- `{name}` — выразительность {expr:.1}. {}", p.note);
        println!(
            "{}",
            report.lines().rev().take(6).collect::<Vec<_>>().join(" | ")
        );
    }
    let _ = write!(
        readme,
        "\nПочему так (запись движка):\n\n{notes}\nПроверки (доли, граница фразы, клиппинг, возврат \
         темпа) — в `report.txt`.\n"
    );
    std::fs::write(out.join("report.txt"), &report).expect("report");
    std::fs::write(out.join("README.md"), &readme).expect("readme");
    print!("{report}");
    if failed {
        std::process::exit(1);
    }
}
