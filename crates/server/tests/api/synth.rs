//! Синтетический каталог для тестов рекомендаций: артисты, записи, треки и анализ с заданными векторами
//! и признаками — прямо в базе, без Deezer и сервиса анализа. Векторы — «оси»: десять единиц подряд
//! от `axis` и одна 0,4 со сдвигом `jitter`; треки одной оси близки (косинус ≈ 0,98), разных — нет.

use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Feat {
    pub axis: usize,
    pub jitter: usize,
    pub energy: f32,
    pub valence: f32,
    pub bpm: f32,
    pub key: &'static str,
    pub voice: f32,
    pub dance: f32,
    pub rank: i32,
    /// `recordings.audio_status`
    pub status: &'static str,
    pub analyzed: bool,
}

impl Default for Feat {
    fn default() -> Self {
        Self {
            axis: 0,
            jitter: 0,
            energy: 0.6,
            valence: 0.5,
            bpm: 120.0,
            key: "8A",
            voice: 0.8,
            dance: 0.6,
            rank: 300_000,
            status: "none",
            analyzed: true,
        }
    }
}

pub fn vector(axis: usize, jitter: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; 1280];
    for i in 0..10 {
        v[(axis + i) % 1280] = 1.0;
    }
    v[(axis + 10 + jitter) % 1280] = 0.4;
    v
}

pub struct Synth {
    pub db: PgPool,
    next: AtomicI64,
}

impl Synth {
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            next: AtomicI64::new(9_000_000_000),
        }
    }

    fn id(&self) -> i64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn artist(&self, name: &str) -> Uuid {
        sqlx::query_scalar("INSERT INTO artists (deezer_id, name) VALUES ($1, $2) RETURNING id")
            .bind(self.id())
            .bind(name)
            .fetch_one(&self.db)
            .await
            .unwrap()
    }

    pub async fn track(&self, artist: Uuid, title: &str, f: Feat) -> Uuid {
        let n = self.id();
        let isrc = format!("QZ{:010}", n % 10_000_000_000);
        let recording: Uuid = sqlx::query_scalar(
            "INSERT INTO recordings (isrc, duration_ms, audio_status) VALUES ($1, 180000, $2) RETURNING id",
        )
        .bind(&isrc)
        .bind(f.status)
        .fetch_one(&self.db)
        .await
        .unwrap();
        let track: Uuid = sqlx::query_scalar(
            "INSERT INTO tracks (deezer_id, isrc, recording_id, title, duration_ms, rank, fetched_at)
             VALUES ($1, $2, $3, $4, 180000, $5, now()) RETURNING id",
        )
        .bind(n)
        .bind(&isrc)
        .bind(recording)
        .bind(title)
        .bind(f.rank)
        .fetch_one(&self.db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role, position) VALUES ($1, $2, 'main', 0)")
            .bind(track)
            .bind(artist)
            .execute(&self.db)
            .await
            .unwrap();
        if f.analyzed {
            sqlx::query(
                "INSERT INTO track_analysis (recording_id, source, models, sound, semantic, energy, valence, danceability,
                                             bpm, bpm_confidence, key_camelot, key_confidence, voice, moods, styles)
                 VALUES ($1, 'preview', '{}', $2::real[]::halfvec(1280), $3::real[]::halfvec(512), $4, $5, $6, $7, 0.9,
                         $8, 0.8, $9, '{\"relaxed\": 0.4, \"party\": 0.5}', '[{\"name\": \"Hip Hop---Trap\", \"p\": 0.5}]')",
            )
            .bind(recording)
            .bind(vector(f.axis, f.jitter))
            .bind(semantic(f.axis))
            .bind(f.energy)
            .bind(f.valence)
            .bind(f.dance)
            .bind(f.bpm)
            .bind(f.key)
            .bind(f.voice)
            .execute(&self.db)
            .await
            .unwrap();
        }
        track
    }

    pub async fn like(&self, user: Uuid, track: Uuid, days_ago: i64) {
        sqlx::query("INSERT INTO liked_tracks (user_id, track_id, created_at) VALUES ($1, $2, $3)")
            .bind(user)
            .bind(track)
            .bind(Utc::now() - Duration::days(days_ago))
            .execute(&self.db)
            .await
            .unwrap();
    }

    pub async fn play(&self, user: Uuid, track: Uuid, hours_ago: i64, completed: bool) {
        sqlx::query(
            "INSERT INTO play_events (client_event_id, user_id, track_id, started_at, played_ms, duration_ms, end_reason)
             VALUES ($1, $2, $3, $4, $5, 180000, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(user)
        .bind(track)
        .bind(Utc::now() - Duration::hours(hours_ago))
        .bind(if completed { 180_000 } else { 5_000 })
        .bind(if completed { "completed" } else { "skipped" })
        .execute(&self.db)
        .await
        .unwrap();
    }

    pub async fn block(&self, user: Uuid, kind: &str, id: Uuid) {
        sqlx::query("INSERT INTO blocked_items (user_id, kind, ref_id) VALUES ($1, $2, $3)")
            .bind(user)
            .bind(kind)
            .bind(id)
            .execute(&self.db)
            .await
            .unwrap();
    }

    pub async fn dislike(&self, user: Uuid, track: Uuid) {
        sqlx::query(
            "INSERT INTO feedback_events (client_event_id, user_id, track_id, kind, at) VALUES ($1, $2, $3, 'dislike', now())",
        )
        .bind(Uuid::now_v7())
        .bind(user)
        .bind(track)
        .execute(&self.db)
        .await
        .unwrap();
    }
}

/// «Смысл» (CLAP) по той же оси, в 512 измерениях.
pub fn semantic(axis: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; 512];
    v[axis % 512] = 1.0;
    v
}
