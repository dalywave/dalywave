//! Режим вечеринки: сцена на большом экране и настройки, которые меняют волну хозяина, пока вечеринка
//! идёт (`parties.active_until` в будущем). «Только энергичное» — настроение «Бодрое» у новых сессий
//! волны и автопродолжения; «без повторов артистов» — в выдаче волны сначала артисты, которых на этой
//! вечеринке ещё не было. Маршруты — [`crate::routes::tv`].

use std::collections::HashSet;

use dalywave_protocol::{
    tv::{Party, PartySettings},
    wave::{WaveActivity, WaveMood, WaveSettings},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;

/// Вечеринка человека (выключенная — с сохранёнными настройками).
pub async fn of(db: &PgPool, user: Uuid) -> AppResult<Party> {
    let row = sqlx::query!(
        r#"SELECT active_until, energetic, no_artist_repeat, dj, coalesce(active_until > now(), false) AS "active!"
           FROM parties WHERE user_id = $1"#,
        user,
    )
    .fetch_optional(db)
    .await?;
    Ok(match row {
        Some(r) => Party {
            active: r.active,
            settings: PartySettings {
                energetic: r.energetic,
                no_artist_repeat: r.no_artist_repeat,
                dj: r.dj,
            },
            until: r.active_until.filter(|_| r.active),
        },
        None => Party {
            active: false,
            settings: PartySettings::default(),
            until: None,
        },
    })
}

/// Настройки идущей вечеринки; `None` — вечеринки нет.
pub async fn active(db: &PgPool, user: Uuid) -> AppResult<Option<PartySettings>> {
    let party = of(db, user).await?;
    Ok(party.active.then_some(party.settings))
}

/// Настройки волны на вечеринке: «только энергичное» — бодрое настроение и, если занятие не выбрано,
/// «Вечеринка»; спокойные занятия уступают место вечеринке.
pub fn tune(settings: &mut WaveSettings, party: &PartySettings) {
    if !party.energetic {
        return;
    }
    settings.mood = WaveMood::Energetic;
    if matches!(
        settings.activity,
        WaveActivity::Any | WaveActivity::Rest | WaveActivity::Sleep
    ) {
        settings.activity = WaveActivity::Party;
    }
}

/// Настройки новой сессии волны с поправкой на вечеринку.
pub async fn tuned(db: &PgPool, user: Uuid, mut settings: WaveSettings) -> AppResult<WaveSettings> {
    if let Some(party) = active(db, user).await? {
        tune(&mut settings, &party);
    }
    Ok(settings)
}

/// Артисты уже выданного в сессии (`served` — id записей или треков без записи).
pub async fn heard_artists(db: &PgPool, served: &[Uuid]) -> AppResult<HashSet<Uuid>> {
    if served.is_empty() {
        return Ok(HashSet::new());
    }
    let ids = sqlx::query_scalar!(
        r#"SELECT DISTINCT ta.artist_id AS "id!" FROM tracks t JOIN track_artists ta ON ta.track_id = t.id
           WHERE t.recording_id = ANY($1) OR t.id = ANY($1)"#,
        served,
    )
    .fetch_all(db)
    .await?;
    Ok(ids.into_iter().collect())
}

/// «Без повторов артистов»: сначала треки артистов, которых ещё не было (и не два одного артиста
/// в одной выдаче), потом — остальные в прежнем порядке, чтобы волна не иссякла.
pub fn spread<T>(picks: Vec<T>, artists: impl Fn(&T) -> &[Uuid], heard: &HashSet<Uuid>) -> Vec<T> {
    let mut seen = heard.clone();
    let mut fresh = Vec::with_capacity(picks.len());
    let mut rest = Vec::new();
    for p in picks {
        let a = artists(&p);
        if a.iter().any(|id| seen.contains(id)) {
            rest.push(p);
        } else {
            seen.extend(a.iter().copied());
            fresh.push(p);
        }
    }
    fresh.extend(rest);
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energetic_party_overrides_calm_settings() {
        let party = PartySettings {
            energetic: true,
            ..PartySettings::default()
        };
        let mut s = WaveSettings {
            mood: WaveMood::Calm,
            activity: WaveActivity::Sleep,
            ..WaveSettings::default()
        };
        tune(&mut s, &party);
        assert_eq!(
            (s.mood, s.activity),
            (WaveMood::Energetic, WaveActivity::Party)
        );
        let mut road = WaveSettings {
            activity: WaveActivity::Road,
            ..WaveSettings::default()
        };
        tune(&mut road, &party);
        assert_eq!(road.activity, WaveActivity::Road, "занятие остаётся");
        let mut calm = WaveSettings {
            mood: WaveMood::Calm,
            ..WaveSettings::default()
        };
        tune(&mut calm, &PartySettings::default());
        assert_eq!(
            calm.mood,
            WaveMood::Calm,
            "без «только энергичного» не трогаем"
        );
    }

    #[test]
    fn spread_puts_new_artists_first() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let c = Uuid::from_u128(3);
        let picks = vec![(1, vec![a]), (2, vec![b]), (3, vec![b]), (4, vec![c])];
        let heard: HashSet<Uuid> = [a].into();
        let out = spread(picks, |p| &p.1, &heard);
        assert_eq!(
            out.iter().map(|p| p.0).collect::<Vec<_>>(),
            vec![2, 4, 1, 3]
        );
    }
}
