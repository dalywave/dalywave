//! «Об артисте»: начало статьи Википедии (сначала русской, иначе английской) и факты из Wikidata —
//! годы на сцене, откуда, жанры.
//!
//! Статья ищется только по точным связям, чтобы не перепутать однофамильцев: элемент Wikidata с id
//! артиста Deezer (`P2722`), иначе — с id MusicBrainz (`P434`), который берётся из записи
//! MusicBrainz по ISRC трека этого артиста. По одному имени не ищем. Ответ хранится в `artist_about`:
//! найденное — 30 дней, «статьи нет» — 7.

use chrono::{Duration, Utc};
use dalywave_protocol::info::{ActiveYears, ArtistAbout};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    InfoError, Sources, loose, strip_stress,
    wiki::{Entity, Summary},
};

const FOUND_DAYS: i64 = 30;
const NOT_FOUND_DAYS: i64 = 7;
/// Начало статьи — не длиннее, по границе предложения.
const SUMMARY_MAX: usize = 640;
const GENRES: usize = 3;

/// Начало статьи до `max` символов, по границе предложения; без ударений.
pub fn trim_summary(text: &str, max: usize) -> String {
    // Википедия вырезает из начала статьи скобки с датами и произношением: «Adela ,» → «Adela,»
    let text = strip_stress(text.trim())
        .replace(" ,", ",")
        .replace(" .", ".")
        .replace("( )", "")
        .replace("  ", " ");
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    // Последняя точка, за которой пробел, — конец предложения (а не «т. е.» в середине — не идеально, но коротко)
    match cut.rfind(". ") {
        Some(i) if i > max / 3 => cut[..=i].trim_end().to_string(),
        _ => format!("{}…", cut.trim_end()),
    }
}

/// Какая статья: русская, иначе английская.
pub fn article(entity: &Entity) -> Option<(&'static str, String)> {
    [("ru", "ruwiki"), ("en", "enwiki")]
        .into_iter()
        .find_map(|(lang, site)| entity.sitelinks.get(site).map(|s| (lang, s.title.clone())))
}

/// Элементы, чьи подписи нужны фактам: место и жанры.
pub fn wanted_labels(entity: &Entity) -> Vec<String> {
    let place = if entity.is_human() { "P19" } else { "P740" };
    let mut ids: Vec<String> = entity.items(place).into_iter().take(1).collect();
    ids.extend(entity.items("P136").into_iter().take(GENRES));
    ids
}

/// Блок «Об артисте» из элемента, подписей и статьи.
pub fn compose(
    entity: &Entity,
    labels: &std::collections::HashMap<String, String>,
    lang: &str,
    summary: &Summary,
) -> Option<ArtistAbout> {
    if summary.kind == "disambiguation" || summary.extract.trim().is_empty() {
        return None;
    }
    let group = !entity.is_human();
    let active = match (entity.year("P2031"), group) {
        (Some(from), _) => Some(ActiveYears {
            from,
            to: entity.year("P2032"),
        }),
        (None, true) => entity.year("P571").map(|from| ActiveYears {
            from,
            to: entity.year("P576"),
        }),
        (None, false) => None,
    };
    let place = if group { "P740" } else { "P19" };
    let origin = entity
        .items(place)
        .first()
        .and_then(|q| labels.get(q))
        .cloned();
    let mut genres: Vec<String> = Vec::new();
    for g in entity.items("P136").iter().take(GENRES) {
        if let Some(l) = labels.get(g)
            && !genres.contains(l)
        {
            genres.push(l.clone());
        }
    }
    let url = summary.content_urls.as_ref()?.desktop.page.clone();
    Some(ArtistAbout {
        summary: trim_summary(&summary.extract, SUMMARY_MAX),
        url,
        lang: lang.into(),
        group,
        active,
        origin,
        genres,
    })
}

/// Элемент Wikidata артиста по точным связям. `None` — связи нет.
async fn wikidata_id(
    db: &PgPool,
    src: &Sources,
    artist_id: Uuid,
    name: &str,
    deezer_id: Option<i64>,
) -> Result<Option<String>, FetchError> {
    if let Some(d) = deezer_id
        && let Some(q) = src.wiki.find_by_claim("P2722", &d.to_string()).await?
    {
        return Ok(Some(q));
    }
    // MusicBrainz знает исполнителя записи по ISRC: среди её артистов — тот, чьё имя совпадает
    let isrcs = sqlx::query_scalar!(
        r#"SELECT t.isrc AS "isrc!" FROM track_artists ta
           JOIN tracks t ON t.id = ta.track_id
           WHERE ta.artist_id = $1 AND ta.role = 'main' AND t.isrc IS NOT NULL
           ORDER BY t.rank DESC NULLS LAST
           LIMIT 2"#,
        artist_id
    )
    .fetch_all(db)
    .await?;
    let want = loose(name);
    for isrc in isrcs {
        let recs = src.musicbrainz.recordings_by_isrc(&isrc).await?;
        let mbid = recs
            .iter()
            .flat_map(|r| &r.artist_credit)
            .find(|c| loose(&c.artist.name) == want || loose(&c.name) == want)
            .map(|c| c.artist.id);
        if let Some(mbid) = mbid {
            return Ok(src.wiki.find_by_claim("P434", &mbid.to_string()).await?);
        }
    }
    Ok(None)
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error(transparent)]
    Source(#[from] InfoError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Собирает блок заново. `(id Wikidata, блок)`; блок `None` — статьи нет.
async fn fetch(
    db: &PgPool,
    src: &Sources,
    artist_id: Uuid,
    name: &str,
    deezer_id: Option<i64>,
) -> Result<(Option<String>, Option<ArtistAbout>), FetchError> {
    let Some(qid) = wikidata_id(db, src, artist_id, name, deezer_id).await? else {
        return Ok((None, None));
    };
    let Some(entity) = src.wiki.entity(&qid).await? else {
        return Ok((Some(qid), None));
    };
    let Some((lang, title)) = article(&entity) else {
        return Ok((Some(qid), None));
    };
    let Some(summary) = src.wiki.summary(lang, &title).await? else {
        return Ok((Some(qid), None));
    };
    let labels = src.wiki.labels(&wanted_labels(&entity)).await?;
    Ok((Some(qid), compose(&entity, &labels, lang, &summary)))
}

/// Блок «Об артисте»: из `artist_about` или собранный заново. Сбой источников при сохранённом ответе —
/// сохранённый ответ.
pub async fn load(
    db: &PgPool,
    src: &Sources,
    artist_id: Uuid,
) -> Result<Option<ArtistAbout>, FetchError> {
    let artist = sqlx::query!(
        "SELECT name, deezer_id FROM artists WHERE id = $1",
        artist_id
    )
    .fetch_optional(db)
    .await?;
    let Some(artist) = artist else {
        return Ok(None);
    };
    let row = sqlx::query!(
        "SELECT about, fetched_at FROM artist_about WHERE artist_id = $1",
        artist_id
    )
    .fetch_optional(db)
    .await?;
    let stored: Option<Option<ArtistAbout>> = row
        .as_ref()
        .map(|r| r.about.clone().and_then(|v| serde_json::from_value(v).ok()));
    if let (Some(r), Some(s)) = (&row, &stored) {
        let ttl = Duration::days(if s.is_some() {
            FOUND_DAYS
        } else {
            NOT_FOUND_DAYS
        });
        if r.fetched_at > Utc::now() - ttl {
            return Ok(s.clone());
        }
    }
    let (qid, about) = match fetch(db, src, artist_id, &artist.name, artist.deezer_id).await {
        Ok(f) => f,
        Err(FetchError::Source(e)) => {
            return match stored {
                Some(s) => {
                    tracing::warn!(%artist_id, error = %e, "wiki failed, showing stored about");
                    Ok(s)
                }
                None => Err(e.into()),
            };
        }
        Err(e) => return Err(e),
    };
    sqlx::query!(
        "INSERT INTO artist_about (artist_id, wikidata_id, about, fetched_at) VALUES ($1, $2, $3, now())
         ON CONFLICT (artist_id) DO UPDATE SET wikidata_id = $2, about = $3, fetched_at = now()",
        artist_id,
        qid,
        about.as_ref().and_then(|a| serde_json::to_value(a).ok()),
    )
    .execute(db)
    .await?;
    Ok(about)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn entity() -> Entity {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/wiki/entity_billie.json"))
                .unwrap();
        serde_json::from_value(v["entities"]["Q29564107"].clone()).unwrap()
    }

    fn summary() -> Summary {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/wiki/summary_billie_ru.json"
        ))
        .unwrap()
    }

    #[test]
    fn facts_and_article_of_a_person() {
        let e = entity();
        assert!(e.is_human());
        assert_eq!(article(&e), Some(("ru", "Айлиш, Билли".to_string())));
        assert_eq!(
            wanted_labels(&e),
            vec!["Q65", "Q188450", "Q842324", "Q95469495"]
        );
        let labels: HashMap<String, String> = [
            ("Q65", "Лос-Анджелес"),
            ("Q188450", "электропоп"),
            ("Q842324", "инди-поп"),
            ("Q95469495", "альт-поп"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let about = compose(&e, &labels, "ru", &summary()).unwrap();
        assert!(!about.group);
        assert_eq!(
            about.active,
            Some(ActiveYears {
                from: 2015,
                to: None
            })
        );
        assert_eq!(about.origin.as_deref(), Some("Лос-Анджелес"));
        assert_eq!(about.genres, vec!["электропоп", "инди-поп", "альт-поп"]);
        assert!(
            about.summary.starts_with("Билли Айлиш"),
            "без ударений: {}",
            about.summary
        );
        assert!(about.summary.chars().count() <= SUMMARY_MAX);
        assert!(about.url.starts_with("https://ru.wikipedia.org/wiki/"));
    }

    #[test]
    fn disambiguation_pages_are_not_articles() {
        let mut s = summary();
        s.kind = "disambiguation".into();
        assert!(compose(&entity(), &HashMap::new(), "ru", &s).is_none());
    }

    #[test]
    fn summary_is_cut_at_a_sentence() {
        let text =
            "Первое предложение здесь. Второе тоже. Третье очень длинное и не влезает целиком.";
        assert_eq!(
            trim_summary(text, 45),
            "Первое предложение здесь. Второе тоже."
        );
        assert_eq!(trim_summary("Коротко.", 45), "Коротко.");
        assert_eq!(
            trim_summary("Tame Impala ,\u{a0}настоящее имя — Кевин Паркер.", 100),
            "Tame Impala,\u{a0}настоящее имя — Кевин Паркер."
        );
        assert_eq!(
            trim_summary("Одно длинное предложение без точек вовсе", 12),
            "Одно длинное…"
        );
    }
}
