//! Разбор сети вкуса на копии базы: набор по рецепту, распределения меток, заполненность признаков,
//! AUC отдельных признаков, обучение и проверка на отложенной части — как у ночного обучения.
//!
//! `DATABASE_URL=… cargo run --release -p dalywave-server --example taste_lab -- [рецепты…] [--dump-legacy файл]`
//! Рецепты: `legacy`, `default`, `no-weights`, `no-import-skips`, `no-library-likes`, `live-facets`, `year`.
//!
//! `-- --device http://localhost:8434 dwd_…` — как приложение на компьютере: задание, обучение, веса,
//! решение сервера (тот же путь, что у apps/shell/src/trainer.rs).

use std::{collections::HashMap, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD};
use dalywave_server::reco::model::{self, Dataset, Loaded, Recipe, SCALARS};
use dalywave_taste::{Spec, f16, metrics, onnx};
use serde_json::json;
use uuid::Uuid;

fn recipe(name: &str) -> Recipe {
    let d = Recipe::default();
    match name {
        "legacy" => Recipe::legacy(),
        "no-weights" => Recipe {
            half_life_days: None,
            ..d
        },
        "no-import-skips" => Recipe {
            import_stops_skip: false,
            ..d
        },
        "no-library-likes" => Recipe {
            library_likes: false,
            ..d
        },
        "live-facets" => Recipe {
            cut_facets: false,
            ..d
        },
        "year" => Recipe {
            since_days: Some(365),
            ..d
        },
        _ => d,
    }
}

fn col(data: &Dataset, name: &str) -> Vec<f32> {
    let k = SCALARS.iter().position(|s| *s == name).unwrap();
    let cd = data.set.scalar_dim;
    (0..data.rows())
        .map(|i| data.set.scalars[i * cd + k])
        .collect()
}

fn stats(data: &Dataset) {
    let n = data.rows();
    let h = data.holdout_from;
    println!(
        "  строк {n}, обучение {h}, проверка {} ({}…{}), треков {}, слушателей {}",
        n - h,
        data.times
            .get(h)
            .map_or("—".into(), |t| t.date_naive().to_string()),
        data.times
            .last()
            .map_or("—".into(), |t| t.date_naive().to_string()),
        data.set.vector_rows(),
        data.users
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
    );
    for (k, head) in model::HEADS.iter().enumerate() {
        let pos = |r: std::ops::Range<usize>| {
            r.clone()
                .filter(|i| data.set.labels[i * 3 + k] > 0.5)
                .count()
        };
        println!(
            "  {head:8} обучение {:5} ({:.1} %), проверка {:4} ({:.1} %)",
            pos(0..h),
            100.0 * pos(0..h) as f64 / h.max(1) as f64,
            pos(h..n),
            100.0 * pos(h..n) as f64 / (n - h).max(1) as f64
        );
    }
    let w = &data.set.weights;
    println!(
        "  вес строки: мин {:.2}, средний {:.2}, сумма {:.0}",
        w.iter().copied().fold(f32::INFINITY, f32::min),
        w.iter().sum::<f32>() / n.max(1) as f32,
        w.iter().sum::<f32>()
    );
    let fill = |name: &str| {
        let c = col(data, name);
        100.0 * c.iter().filter(|x| **x != 0.0).count() as f64 / n.max(1) as f64
    };
    println!(
        "  заполнено: смысл {:.0} %, темп {:.0} %, тональность {:.0} %, грани {:.0} %, в волне {:.1} %, мобильное {:.0} %, \
         компьютер {:.0} %, слышал раньше {:.0} %, лайк до {:.1} %, «заходит» {:.0} %, «пропускается» {:.0} %",
        fill("has_semantic"),
        fill("bpm_known"),
        fill("key_known"),
        fill("has_facets"),
        fill("in_wave"),
        fill("device_mobile"),
        fill("device_desktop"),
        fill("heard_before"),
        fill("liked"),
        fill("has_hits"),
        fill("has_misses"),
    );
    // AUC отдельных признаков на проверке: пропуск (выше — реже пропускают) и дослушивание
    let yh = |k: usize| -> Vec<f32> { (h..n).map(|i| data.set.labels[i * 3 + k]).collect() };
    let mut line = String::from("  AUC признака на проверке (пропуск↓ / дослушал / лайк):");
    for name in [
        "taste_sim",
        "completed",
        "track_skips",
        "days_since_play",
        "heard_before",
        "artist_familiar",
        "hit_sim",
        "miss_sim",
        "skip_streak",
        "session_pos",
        "popularity",
        "energy",
    ] {
        let c: Vec<f32> = col(data, name)[h..].to_vec();
        let neg: Vec<f32> = c.iter().map(|x| -x).collect();
        let f = |x: Option<f64>| x.map_or("—".into(), |v| format!("{v:.3}"));
        line += &format!(
            "\n    {name:16} {} / {} / {}",
            f(metrics::auc(&yh(0), &neg)),
            f(metrics::auc(&yh(1), &c)),
            f(metrics::auc(&yh(2), &c))
        );
    }
    let b = &data.baseline[h..];
    let nb: Vec<f32> = b.iter().map(|x| -x).collect();
    line += &format!(
        "\n    {:16} {:.3} / {:.3} / {}",
        "правила",
        metrics::auc(&yh(0), &nb).unwrap_or(f64::NAN),
        metrics::auc(&yh(1), b).unwrap_or(f64::NAN),
        metrics::auc(&yh(2), b).map_or("—".into(), |v| format!("{v:.3}"))
    );
    println!("{line}");
}

fn legacy_spec() -> Spec {
    Spec {
        hidden: vec![384, 256, 64],
        sound_k: 32,
        semantic_k: 16,
        max_epochs: 60,
        patience: 6,
        batch: 256,
        lr: 1e-3,
        weight_decay: 1e-4,
        clip: 100.0,
        ..Spec::default()
    }
}

fn specs() -> Vec<(&'static str, Spec)> {
    let d = model::spec();
    vec![
        ("сервер", d.clone()),
        ("старая архитектура", legacy_spec()),
        (
            "логистическая",
            Spec {
                hidden: vec![],
                ..d.clone()
            },
        ),
        (
            "64-32",
            Spec {
                hidden: vec![64, 32],
                ..d.clone()
            },
        ),
        (
            "128-64",
            Spec {
                hidden: vec![128, 64],
                ..d.clone()
            },
        ),
        (
            "k32/16",
            Spec {
                sound_k: 32,
                semantic_k: 16,
                ..d.clone()
            },
        ),
        (
            "k8/4",
            Spec {
                sound_k: 8,
                semantic_k: 4,
                ..d.clone()
            },
        ),
        (
            "L2 1e-3",
            Spec {
                weight_decay: 1e-3,
                ..d
            },
        ),
    ]
}

fn run(data: Arc<Dataset>, label: &str, spec: &Spec) {
    run_on(data.clone(), data.train_part(), label, spec);
}

/// Обучающая часть — только самые поздние `n` строк (кривая обучения: помогает ли больше данных).
fn recent(data: &Dataset, n: usize) -> dalywave_taste::TrainSet {
    let h = data.holdout_from;
    let from = h.saturating_sub(n);
    let s = &data.set;
    let cd = s.scalar_dim;
    dalywave_taste::TrainSet {
        sound_dim: s.sound_dim,
        semantic_dim: s.semantic_dim,
        scalar_dim: cd,
        vectors: s.vectors.clone(),
        row_vector: s.row_vector[from..h].to_vec(),
        scalars: s.scalars[from * cd..h * cd].to_vec(),
        labels: s.labels[from * 3..h * 3].to_vec(),
        weights: s.weights[from..h].to_vec(),
        groups: s.groups[from..h].to_vec(),
    }
}

fn run_on(data: Arc<Dataset>, train: dalywave_taste::TrainSet, label: &str, spec: &Spec) {
    let (p, report) = match dalywave_taste::train(&train, spec) {
        Ok(x) => x,
        Err(e) => {
            println!("    {label}: не обучилась: {e}");
            return;
        }
    };
    let bytes = onnx::export(&p);
    let net = Loaded::build(Uuid::nil(), label, &bytes).expect("ONNX загружается");
    let e = model::evaluate(&data, &net, None).expect("проверка");
    let v = model::judge(&e);
    let f = |x: Option<f64>| x.map_or("—".into(), |v| format!("{v:.3}"));
    let heads: Vec<String> = e
        .net
        .heads
        .iter()
        .zip(&e.rules.heads)
        .map(|(a, b)| format!("{}/{}", f(a.auc), f(b.auc)))
        .collect();
    println!(
        "    {label:20} AUC {} vs {} (головы {}), logloss {} vs {}, в сессии {} vs {}, NDCG {} vs {}, уверенность {}, \
         эпох {}/{}, val {:.4}→{:.4}, fit {:.4}→{:.4}, {} пар., {} мс → {}",
        f(e.net.auc),
        f(e.rules.auc),
        heads.join(" "),
        f(e.net.logloss),
        f(e.rules.logloss),
        f(e.net.session_auc),
        f(e.rules.session_auc),
        f(e.net.ndcg),
        f(e.rules.ndcg),
        f(e.vs_rules.p_better),
        report.best_epoch,
        report.epochs,
        report.val_loss.first().copied().unwrap_or(0.0),
        report
            .val_loss
            .get(report.best_epoch.saturating_sub(1))
            .copied()
            .unwrap_or(0.0),
        report.fit_loss.first().copied().unwrap_or(0.0),
        report.fit_loss.last().copied().unwrap_or(0.0),
        p.trainable(),
        report.took_ms,
        if v.activate {
            "включилась"
        } else {
            "не включилась"
        },
    );
}

/// Набор в формате старого `POST /v1/taste/train` (36 первых чисел) — для старого обучения на Python.
fn dump_legacy(data: &Dataset, path: &str) {
    let old = 36;
    let cd = data.set.scalar_dim;
    let n = data.rows();
    let scalars: Vec<f32> = (0..n)
        .flat_map(|i| data.set.scalars[i * cd..i * cd + old].to_vec())
        .collect();
    let f32s =
        |v: &[f32]| STANDARD.encode(v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>());
    let body = json!({
        "vectors": STANDARD.encode(data.set.vectors.iter().flat_map(|x| f16::to_bits(*x).to_le_bytes()).collect::<Vec<u8>>()),
        "vectorRows": data.set.vector_rows(),
        "soundDim": data.set.sound_dim,
        "semanticDim": data.set.semantic_dim,
        "rowVector": STANDARD.encode(data.set.row_vector.iter().flat_map(|x| (*x as i32).to_le_bytes()).collect::<Vec<u8>>()),
        "scalars": f32s(&scalars),
        "scalarDim": old,
        "rows": n,
        "labels": f32s(&data.set.labels),
        "baseline": f32s(&data.baseline),
        "holdoutFrom": data.holdout_from,
        "featureNames": SCALARS[..old].to_vec(),
        "groups": data.set.groups,
    });
    std::fs::write(path, body.to_string()).expect("запись");
    println!("  старый набор → {path}");
}

#[tokio::main]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("--device") {
        device(
            &argv[1],
            &argv[2],
            argv.get(3).map_or("auto", String::as_str),
        )
        .await;
        return;
    }
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let db = sqlx::PgPool::connect(&url).await.expect("база");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dump = None;
    let mut names = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--dump-legacy" {
            dump = it.next().cloned();
        } else {
            names.push(a.clone());
        }
    }
    if names.is_empty() {
        names = vec!["legacy".into(), "default".into()];
    }
    let mut cache: HashMap<String, Arc<Dataset>> = HashMap::new();
    for name in &names {
        println!("\n== рецепт {name}");
        let data = Arc::new(
            model::dataset(&db, &recipe(name), None, None)
                .await
                .expect("набор"),
        );
        stats(&data);
        if name == "legacy"
            && let Some(path) = &dump
        {
            dump_legacy(&data, path);
        }
        if !data.enough() {
            println!("  мало данных");
            continue;
        }
        if name == "default" {
            println!("  кривая обучения (самые поздние строки обучающей части):");
            for n in [500, 1000, 2000, 4000, usize::MAX] {
                let n = n.min(data.holdout_from);
                for seed in 0..2 {
                    let spec = Spec {
                        seed,
                        ..model::spec()
                    };
                    run_on(
                        data.clone(),
                        recent(&data, n),
                        &format!("{n} строк #{seed}"),
                        &spec,
                    );
                }
            }
        }
        let only_server = name != "default";
        for (label, spec) in specs() {
            if only_server
                && label != "сервер"
                && !(name == "legacy" && label == "старая архитектура")
            {
                continue;
            }
            for seed in 0..if only_server { 1 } else { 2 } {
                run(
                    data.clone(),
                    &format!("{label} #{seed}"),
                    &Spec {
                        seed,
                        ..spec.clone()
                    },
                );
            }
        }
        cache.insert(name.clone(), data);
    }
}

/// Как приложение на компьютере: задание → обучение → веса → решение сервера.
async fn device(server: &str, token: &str, scope: &str) {
    let http = reqwest::Client::new();
    let res = http
        .post(format!("{server}/api/taste/training/claim"))
        .bearer_auth(token)
        .json(&json!({ "scope": scope }))
        .send()
        .await
        .expect("сервер");
    println!("задание: HTTP {}", res.status());
    if res.status() != 200 {
        println!("{}", res.text().await.unwrap_or_default());
        return;
    }
    let claim: serde_json::Value = res.json().await.expect("JSON");
    let pkg: dalywave_taste::wire::Package =
        serde_json::from_value(claim["package"].clone()).expect("пакет");
    let (spec, set) = pkg.decode().expect("набор");
    println!(
        "  {} строк обучения, сеть {:?}, k {}/{}",
        set.rows(),
        spec.hidden,
        spec.sound_k,
        spec.semantic_k
    );
    let (params, report) = dalywave_taste::train(&set, &spec).expect("обучение");
    println!(
        "  обучено за {} мс, лучшая эпоха {} из {}",
        report.took_ms, report.best_epoch, report.epochs
    );
    let res = http
        .post(format!(
            "{server}/api/taste/training/{}/result",
            claim["taskId"].as_str().unwrap()
        ))
        .bearer_auth(token)
        .json(
            &json!({ "params": dalywave_taste::wire::WireParams::new(&params), "report": report }),
        )
        .send()
        .await
        .expect("сервер");
    println!(
        "решение: HTTP {} {}",
        res.status(),
        res.text().await.unwrap_or_default()
    );
}
