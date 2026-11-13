//! Пересчитать карту звучания («Как подобран») вручную — как ночная задача `track_map`: UMAP сервисом
//! анализа (или PCA, если он недоступен), выравнивание к прошлой карте, грани вкуса.
//!
//! `DATABASE_URL=… DALYWAVE_ML_URL=http://localhost:8403 cargo run --release -p dalywave-server --example track_map`

use dalywave_server::{ml::Ml, reco::map};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let ml_url =
        std::env::var("DALYWAVE_ML_URL").unwrap_or_else(|_| "http://localhost:8403".into());
    let db = sqlx::PgPool::connect(&url).await?;
    let report = map::layout(&db, &Ml::new(&ml_url)?).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
