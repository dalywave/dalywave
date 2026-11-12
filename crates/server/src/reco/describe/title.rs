//! Название и описание плейлиста шаблонами из понятого: «Фонк для зала · 40 мин», «Спокойный русский
//! инди под дождь», «Как Скриптонит, но редкое»; в описании — то, что в название не вошло:
//! «Без медляков · только русское».

use dalywave_protocol::{
    describe::DescribeBrief,
    wave::{WaveCharacter, WaveLanguage, WaveMood},
};

use super::genres::{self, Gender};

/// Окончание прилагательного по роду: «спокойн» + «ый/ая/ое/ые».
fn adjective(stem: &str, gender: Gender, soft: bool) -> String {
    let end = match (gender, soft) {
        (Gender::M, false) => "ый",
        (Gender::M, true) => "ий",
        (Gender::F, _) => "ая",
        (Gender::N, _) => "ое",
        (Gender::Plural, false) => "ые",
        (Gender::Plural, true) => "ие",
    };
    format!("{stem}{end}")
}

fn mood_adjective(mood: WaveMood, gender: Gender) -> Option<String> {
    let stem = match mood {
        WaveMood::Any => return None,
        WaveMood::Energetic => "бодр",
        WaveMood::Calm => "спокойн",
        WaveMood::Happy => "весёл",
        WaveMood::Sad => "грустн",
    };
    Some(adjective(stem, gender, false))
}

fn language_adjective(language: WaveLanguage, gender: Gender) -> Option<String> {
    match language {
        WaveLanguage::Any => None,
        // «русский», но «русская»
        WaveLanguage::Ru => Some(adjective("русск", gender, true)),
        WaveLanguage::Foreign => Some(adjective("зарубежн", gender, false)),
        WaveLanguage::None => Some(adjective("инструментальн", gender, false)),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Название жанра внутри фразы: «инди», но «R&B» и «K-pop» — как есть.
fn genre_word(label: &str) -> String {
    if label.chars().next().is_some_and(|c| c.is_ascii()) {
        label.to_string()
    } else {
        label.to_lowercase()
    }
}

/// «40 мин», «1 ч», «1 ч 30 мин».
pub fn duration(minutes: u32) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} мин"),
        (h, 0) => format!("{h} ч"),
        (h, m) => format!("{h} ч {m} мин"),
    }
}

fn names(list: &[String]) -> String {
    match list {
        [] => String::new(),
        [a] => a.clone(),
        [a, b, ..] => format!("{a} и {b}"),
    }
}

/// Что вошло в название — в описании не повторяется.
#[derive(Default)]
struct Used {
    mood: bool,
    language: bool,
    character: bool,
}

/// Название и описание. `phrase` — описание без служебного (из разбора): годится в название, когда
/// в нём нет жанра, ориентира и обстановки.
pub fn compose(brief: &DescribeBrief, phrase: &str) -> (String, String) {
    let mut used = Used::default();
    let genres: Vec<&genres::Genre> = brief
        .genres
        .iter()
        .filter_map(|k| genres::by_key(k))
        .take(2)
        .collect();
    let like: Vec<String> = brief.like.iter().map(|a| a.name.clone()).take(2).collect();
    let genre_phrase = match genres.as_slice() {
        [] => None,
        [g] => Some((genre_word(g.label), g.gender)),
        [a, b, ..] => Some((
            format!("{} и {}", genre_word(a.label), genre_word(b.label)),
            a.gender,
        )),
    };

    let base = if !like.is_empty() {
        let head = match &genre_phrase {
            Some((g, _)) => format!("{g} как {}", names(&like)),
            None => format!("как {}", names(&like)),
        };
        let tail = match brief.character {
            WaveCharacter::Rare => ", но редкое",
            WaveCharacter::New => ", но незнакомое",
            _ => "",
        };
        used.character = !tail.is_empty();
        format!("{head}{tail}")
    } else if genre_phrase.is_some() || brief.scene.is_some() || phrase.is_empty() {
        let (noun, gender) = genre_phrase
            .clone()
            .unwrap_or_else(|| ("музыка".to_string(), Gender::F));
        let mut words: Vec<String> = Vec::new();
        if let Some(m) = mood_adjective(brief.mood, gender) {
            words.push(m);
            used.mood = true;
        }
        if let Some(l) = language_adjective(brief.language, gender) {
            words.push(l);
            used.language = true;
        }
        let bare = words.is_empty() && genre_phrase.is_none() && brief.scene.is_none();
        words.push(noun);
        if let Some(s) = &brief.scene {
            words.push(s.clone());
        }
        if bare {
            // Ни жанра, ни настроения, ни обстановки: название — по характеру
            used.character = true;
            match brief.character {
                WaveCharacter::Rare => "редкие находки".to_string(),
                WaveCharacter::New => "новое для тебя".to_string(),
                WaveCharacter::Pop => "хиты".to_string(),
                WaveCharacter::Fav => "любимое".to_string(),
                WaveCharacter::Mix => {
                    used.character = false;
                    "плейлист по описанию".to_string()
                }
            }
        } else {
            words.join(" ")
        }
    } else {
        // «Спокойный вечер»: своими словами человека — живее шаблона
        let words: Vec<&str> = phrase.split_whitespace().collect();
        let short = words.iter().take(6).copied().collect::<Vec<_>>().join(" ");
        used.mood = brief.mood != WaveMood::Any;
        short
    };
    let mut title = capitalize(&base);
    if let Some(m) = brief.minutes {
        title = format!("{title} · {}", duration(m));
    }

    let mut parts: Vec<String> = Vec::new();
    if !used.mood
        && let Some(m) = match brief.mood {
            WaveMood::Any => None,
            WaveMood::Energetic => Some("бодрое"),
            WaveMood::Calm => Some("спокойное"),
            WaveMood::Happy => Some("весёлое"),
            WaveMood::Sad => Some("грустное"),
        }
    {
        parts.push(m.into());
    }
    if !used.language
        && let Some(l) = match brief.language {
            WaveLanguage::Any => None,
            WaveLanguage::Ru => Some("только русское"),
            WaveLanguage::Foreign => Some("только зарубежное"),
            WaveLanguage::None => Some("без слов"),
        }
    {
        parts.push(l.into());
    }
    if brief.no_slow {
        parts.push("без медляков".into());
    }
    if brief.no_explicit {
        parts.push("без нецензурного".into());
    }
    let avoided: Vec<&str> = brief
        .avoid_genres
        .iter()
        .filter_map(|k| genres::by_key(k))
        .map(|g| g.genitive)
        .collect();
    if !avoided.is_empty() {
        parts.push(format!("без {}", avoided.join(" и ")));
    }
    for a in &brief.without {
        parts.push(format!("без {}", capitalize(&a.said)));
    }
    if !used.character
        && let Some(c) = match brief.character {
            WaveCharacter::Mix => None,
            WaveCharacter::New => Some("только незнакомое"),
            WaveCharacter::Rare => Some("редкое"),
            WaveCharacter::Pop => Some("популярное"),
            WaveCharacter::Fav => Some("знакомое"),
        }
    {
        parts.push(c.into());
    }
    let description = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                capitalize(p)
            } else {
                lower_first(p)
            }
        })
        .collect::<Vec<_>>()
        .join(" · ");
    (title, description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reco::describe::parse::parse;

    fn titled(text: &str) -> (String, String) {
        let p = parse(text);
        compose(&p.brief, &p.phrase)
    }

    #[test]
    fn titles_read_like_people_wrote_them() {
        assert_eq!(
            titled("40 минут фонка для зала, без медляков"),
            ("Фонк для зала · 40 мин".into(), "Без медляков".into())
        );
        assert_eq!(
            titled("спокойный инди под дождь, только русское"),
            ("Спокойный русский инди под дождь".into(), String::new())
        );
        assert_eq!(
            titled("как Скриптонит, но менее известное"),
            ("Как Скриптонит, но редкое".into(), String::new())
        );
        assert_eq!(
            titled("40 минут для зала"),
            ("Музыка для зала · 40 мин".into(), String::new())
        );
        assert_eq!(
            titled("спокойный вечер"),
            ("Спокойный вечер".into(), String::new())
        );
        assert_eq!(
            titled("бодрая электроника на час без мата"),
            ("Бодрая электроника · 1 ч".into(), "Без нецензурного".into())
        );
        assert_eq!(
            titled("20 треков русского рэпа без Моргенштерна"),
            ("Русский рэп".into(), "Без Моргенштерна".into())
        );
        assert_eq!(titled("что-нибудь редкое").0, "Редкие находки");
        assert_eq!(titled("только русское").0, "Русская музыка");
        assert_eq!(titled("грустное техно").0, "Грустное техно", "средний род");
        assert_eq!(titled("r&b для вечеринки").0, "R&B для вечеринки");
        assert_eq!(titled("полтора часа джаза").0, "Джаз · 1 ч 30 мин");
    }

    #[test]
    fn numbers_and_words() {
        assert_eq!(duration(45), "45 мин");
        assert_eq!(duration(120), "2 ч");
    }
}
