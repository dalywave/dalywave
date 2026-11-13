//! Оверлей «Сейчас играет» для стрима: секретная ссылка `…/overlay?k=dwo_…` (OBS Browser Source),
//! данные — потоком SSE `/api/overlay/stream` из снимков активного устройства ([`crate::connect`]).
//!
//! Токен в базе не хранится: он — хеш секрета сервера и случайного `nonce` строки (так ссылку можно
//! скопировать снова на любом устройстве), а ищется по своему SHA-256. «Сменить ссылку» — новый nonce,
//! прежняя ссылка перестаёт работать (открытый поток узнаёт об этом при перепроверке и закрывается).
//! Ссылка даёт одно: что играет у владельца. Пауза, приватная сессия, пропавшее устройство — плашка
//! прячется (`visible: false`).

use chrono::{DateTime, Utc};
use dalywave_protocol::{
    connect::PlaybackState,
    overlay::{OverlayLink, OverlayNow, OverlayTrack, TOKEN_PREFIX},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit,
    config::Config,
    crypto::{b64, random_bytes, sha256},
    error::AppResult,
    state::AppState,
};

/// Снимок играющего устройства старше этого — оно пропало (активное шлёт позицию раз в 5 с).
pub const STALE_SECS: i64 = 30;
/// Позиция разошлась с ожидаемой больше чем на столько — была перемотка: плашке нужен новый снимок.
pub const JUMP_MS: i64 = 2500;

/// Токен ссылки из секрета сервера и nonce строки.
pub fn token(secret: &[u8; 32], nonce: &[u8]) -> String {
    let mut data = Vec::with_capacity(16 + secret.len() + nonce.len());
    data.extend_from_slice(b"dalywave overlay");
    data.extend_from_slice(secret);
    data.extend_from_slice(nonce);
    format!("{TOKEN_PREFIX}{}", b64(sha256(data)))
}

fn url(config: &Config, token: &str) -> String {
    format!(
        "{}/overlay?k={token}",
        config.public_url.trim_end_matches('/')
    )
}

/// Ссылка человека; `None` — он её ещё не создавал (или выключил).
pub async fn link(st: &AppState, user: Uuid) -> AppResult<Option<OverlayLink>> {
    let row = sqlx::query!(
        "SELECT nonce, hide_private, created_at FROM overlay_links WHERE user_id = $1",
        user,
    )
    .fetch_optional(&st.db)
    .await?;
    Ok(row.map(|r| OverlayLink {
        url: url(&st.config, &token(&st.config.secret_key, &r.nonce)),
        created_at: r.created_at,
        hide_private: r.hide_private,
    }))
}

/// Новая ссылка (прежняя перестаёт работать). Настройки плашки сохраняются.
pub async fn rotate(st: &AppState, user: Uuid) -> AppResult<OverlayLink> {
    let nonce = random_bytes::<32>()?;
    let token = token(&st.config.secret_key, &nonce);
    let hash = sha256(&token);
    let replaced = sqlx::query_scalar!(
        r#"WITH prev AS (SELECT 1 FROM overlay_links WHERE user_id = $1)
           INSERT INTO overlay_links (user_id, nonce, token_hash) VALUES ($1, $2, $3)
           ON CONFLICT (user_id) DO UPDATE SET nonce = EXCLUDED.nonce, token_hash = EXCLUDED.token_hash,
               created_at = now()
           RETURNING EXISTS (SELECT 1 FROM prev) AS "replaced!""#,
        user,
        &nonce[..],
        &hash[..],
    )
    .fetch_one(&st.db)
    .await?;
    if replaced {
        audit::record(
            &st.db,
            Some(user),
            audit::OVERLAY_LINK,
            None,
            serde_json::json!({ "action": "changed" }),
        )
        .await?;
    }
    link(st, user)
        .await?
        .ok_or(crate::error::AppError::NotFound)
}

/// Выключить оверлей: ссылка перестаёт работать.
pub async fn disable(st: &AppState, user: Uuid) -> AppResult<()> {
    let gone = sqlx::query!("DELETE FROM overlay_links WHERE user_id = $1", user)
        .execute(&st.db)
        .await?
        .rows_affected();
    if gone > 0 {
        audit::record(
            &st.db,
            Some(user),
            audit::OVERLAY_LINK,
            None,
            serde_json::json!({ "action": "disabled" }),
        )
        .await?;
    }
    Ok(())
}

pub async fn set_hide_private(st: &AppState, user: Uuid, hide: bool) -> AppResult<bool> {
    let n = sqlx::query!(
        "UPDATE overlay_links SET hide_private = $2 WHERE user_id = $1",
        user,
        hide,
    )
    .execute(&st.db)
    .await?
    .rows_affected();
    Ok(n > 0)
}

/// Владелец ссылки: человек с доступом к сервису и его «прятать в приватной сессии».
pub async fn owner(db: &PgPool, token: &str) -> AppResult<Option<(Uuid, bool)>> {
    if !token.starts_with(TOKEN_PREFIX) || token.len() > 100 {
        return Ok(None);
    }
    let hash = sha256(token);
    let row = sqlx::query!(
        "SELECT o.user_id, o.hide_private FROM overlay_links o JOIN users u ON u.id = o.user_id
         WHERE o.token_hash = $1 AND NOT u.disabled AND cardinality(u.roles) > 0",
        &hash[..],
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| (r.user_id, r.hide_private)))
}

/// Идёт приватная сессия (и её надо учитывать)?
pub async fn private(db: &PgPool, user: Uuid, hide_private: bool) -> AppResult<bool> {
    if !hide_private {
        return Ok(false);
    }
    Ok(
        sqlx::query_scalar!(r#"SELECT played_privately($1, now()) AS "private!""#, user)
            .fetch_one(db)
            .await?,
    )
}

/// Что показать на плашке по снимку активного устройства.
pub fn view(state: Option<&PlaybackState>, private: bool, now: DateTime<Utc>) -> OverlayNow {
    let hidden = OverlayNow {
        visible: false,
        track: None,
        playing: false,
        position_ms: 0,
        duration_ms: 0,
        at: now,
    };
    let Some((s, item)) = state.and_then(|s| s.current.as_ref().map(|c| (s, c))) else {
        return hidden;
    };
    if private {
        return hidden;
    }
    // Играющее устройство пропало (закрыли приложение) — его последний снимок «играет» вечно
    let fresh =
        s.at.is_some_and(|at| (now - at).num_seconds() <= STALE_SECS);
    let playing = s.playing && s.pending.is_none() && fresh;
    let t = &item.track;
    let names: Vec<&str> = t.artists.iter().map(|a| a.name.as_str()).collect();
    let duration = if s.duration_ms > 0 {
        s.duration_ms
    } else {
        u32::try_from(t.duration_ms).unwrap_or(0)
    };
    OverlayNow {
        visible: playing,
        track: Some(OverlayTrack {
            id: t.id,
            title: t.title.clone(),
            artist: names.join(", "),
            cover: t
                .album
                .as_ref()
                .and_then(|a| a.image.as_ref())
                .map(|i| i.small.clone()),
        }),
        playing,
        position_ms: s.position_at(now),
        duration_ms: duration,
        at: now,
    }
}

/// Плашке нужен новый снимок: появилась или спряталась, другой трек, пауза, перемотка.
pub fn changed(prev: &OverlayNow, next: &OverlayNow) -> bool {
    if prev.visible != next.visible
        || prev.playing != next.playing
        || prev.track.as_ref().map(|t| t.id) != next.track.as_ref().map(|t| t.id)
    {
        return true;
    }
    if !next.playing {
        return false;
    }
    let expected = i64::from(prev.position_ms) + (next.at - prev.at).num_milliseconds();
    (i64::from(next.position_ms) - expected).abs() > JUMP_MS
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use dalywave_protocol::catalog::{
        AlbumRef, ArtistRole, AudioStatus, ImageSet, Track, TrackArtist,
    };
    use dalywave_protocol::connect::{QueueItem, QueueSource};

    use super::*;

    fn state(playing: bool, at: DateTime<Utc>) -> PlaybackState {
        let mut s: PlaybackState = serde_json::from_str("{}").unwrap();
        s.current = Some(QueueItem {
            uid: "u1".into(),
            track: Track {
                id: Uuid::nil(),
                deezer_id: None,
                title: "Float".into(),
                version: String::new(),
                duration_ms: 169_000,
                explicit: false,
                disc_number: 1,
                track_position: Some(1),
                artists: vec![
                    TrackArtist {
                        id: Uuid::nil(),
                        name: "Call Me Karizma".into(),
                        role: ArtistRole::Main,
                    },
                    TrackArtist {
                        id: Uuid::nil(),
                        name: "Kid".into(),
                        role: ArtistRole::Main,
                    },
                ],
                album: Some(AlbumRef {
                    id: Uuid::nil(),
                    title: "Float".into(),
                    image: Some(ImageSet {
                        small: "/api/images/albums/x/250.jpg".into(),
                        medium: String::new(),
                        large: String::new(),
                    }),
                }),
                recording_id: None,
                isrc: None,
                audio_status: AudioStatus::Ready,
                liked: false,
                clean_id: None,
            },
            source: QueueSource::Context,
            reason: None,
        });
        s.playing = playing;
        s.position_ms = 60_000;
        s.at = Some(at);
        s
    }

    #[test]
    fn token_depends_on_secret_and_nonce() {
        let a = token(&[1; 32], b"n1");
        assert!(a.starts_with("dwo_"));
        assert_eq!(a, token(&[1; 32], b"n1"));
        assert_ne!(a, token(&[2; 32], b"n1"));
        assert_ne!(a, token(&[1; 32], b"n2"));
    }

    #[test]
    fn plate_shows_only_what_is_playing() {
        let at = Utc::now();
        let v = view(Some(&state(true, at)), false, at + Duration::seconds(2));
        assert!(v.visible);
        let t = v.track.unwrap();
        assert_eq!(t.artist, "Call Me Karizma, Kid");
        assert_eq!(t.cover.as_deref(), Some("/api/images/albums/x/250.jpg"));
        assert_eq!(v.position_ms, 62_000);
        assert_eq!(v.duration_ms, 169_000);
        // Пауза — спрятать, но трек помнить
        let v = view(Some(&state(false, at)), false, at);
        assert!(!v.visible && v.track.is_some());
        // Приватная сессия — ни трека, ни плашки
        let v = view(Some(&state(true, at)), true, at);
        assert!(!v.visible && v.track.is_none());
        // Устройство пропало: снимок «играет» давно
        let v = view(Some(&state(true, at)), false, at + Duration::seconds(45));
        assert!(!v.visible);
        assert!(!view(None, false, at).visible);
    }

    #[test]
    fn only_meaningful_changes_reach_the_plate() {
        let at = Utc::now();
        let a = view(Some(&state(true, at)), false, at);
        // Тот же трек, позиция идёт как ожидалось
        let mut s = state(true, at + Duration::seconds(5));
        s.position_ms = 65_100;
        let b = view(Some(&s), false, at + Duration::seconds(5));
        assert!(!changed(&a, &b));
        // Перемотка
        s.position_ms = 120_000;
        let c = view(Some(&s), false, at + Duration::seconds(5));
        assert!(changed(&a, &c));
        // Пауза
        let d = view(Some(&state(false, at)), false, at);
        assert!(changed(&a, &d));
    }
}
