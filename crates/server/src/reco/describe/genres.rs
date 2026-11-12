//! Жанры, которые узнаёт разбор описания: как их пишут (основы слов, по-русски и латиницей), как
//! назвать в заголовке и как найти в индексе — по стилям Discogs из анализа звука (`track_analysis.styles`)
//! и жанрам альбома у Deezer. Стилей «фонк» или «гиперпоп» у Discogs нет: такие жанры ищутся по
//! звучанию (CLAP), а стили только подсказывают.

/// Род слова — для согласования прилагательных: «спокойный инди», «спокойная электроника».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    M,
    F,
    N,
    Plural,
}

#[derive(Debug)]
pub struct Genre {
    pub key: &'static str,
    /// В заголовке: «Фонк».
    pub label: &'static str,
    /// «Без фонка».
    pub genitive: &'static str,
    pub gender: Gender,
    /// Основы слов (после нижнего регистра и ё → е): слово начинается с основы и длиннее её не больше
    /// чем на четыре буквы.
    pub stems: &'static [&'static str],
    /// Слова целиком: короткие основы («поп», «эмо») ловили бы чужие слова.
    pub words: &'static [&'static str],
    /// Шаблоны `LIKE` по стилям Discogs.
    pub styles: &'static [&'static str],
    /// Подстроки жанров альбома у Deezer.
    pub deezer: &'static [&'static str],
}

impl Genre {
    pub fn matches(&self, word: &str) -> bool {
        self.words.contains(&word) || self.stems.iter().any(|s| stem_match(word, s, 4))
    }
}

/// Слово начинается с основы и длиннее её не больше чем на `extra` букв.
pub fn stem_match(word: &str, stem: &str, extra: usize) -> bool {
    word.starts_with(stem) && word.chars().count() <= stem.chars().count() + extra
}

pub const GENRES: &[Genre] = &[
    Genre {
        key: "phonk",
        label: "Фонк",
        genitive: "фонка",
        gender: Gender::M,
        stems: &["фонк", "phonk"],
        words: &[],
        styles: &[
            "Hip Hop---Trap",
            "Hip Hop---Bass Music",
            "Hip Hop---Screw",
            "Hip Hop---Miami Bass",
            "Electronic---Ghetto House",
        ],
        deezer: &[],
    },
    Genre {
        key: "trap",
        label: "Трэп",
        genitive: "трэпа",
        gender: Gender::M,
        stems: &["трэп", "trap"],
        words: &["треп", "трепа", "трепчик"],
        styles: &["Hip Hop---Trap"],
        deezer: &["Rap"],
    },
    Genre {
        key: "drill",
        label: "Дрилл",
        genitive: "дрилла",
        gender: Gender::M,
        stems: &["дрилл", "drill"],
        words: &["дрил", "дрила"],
        styles: &["Hip Hop---Trap", "Hip Hop---Grime", "Electronic---Grime"],
        deezer: &["Rap"],
    },
    Genre {
        key: "cloud",
        label: "Клауд-рэп",
        genitive: "клауд-рэпа",
        gender: Gender::M,
        stems: &["клауд", "cloud"],
        words: &[],
        styles: &["Hip Hop---Cloud Rap"],
        deezer: &["Rap"],
    },
    Genre {
        key: "rap",
        label: "Рэп",
        genitive: "рэпа",
        gender: Gender::M,
        stems: &["рэп", "хип-хоп", "хипхоп", "hip-hop", "hiphop"],
        words: &[
            "реп",
            "репа",
            "репчик",
            "репчика",
            "рэпчик",
            "рэпчика",
            "rap",
            "репак",
            "рэпак",
        ],
        styles: &["Hip Hop---%"],
        deezer: &["Rap", "Hip Hop"],
    },
    Genre {
        key: "postpunk",
        label: "Пост-панк",
        genitive: "пост-панка",
        gender: Gender::M,
        stems: &["пост-панк", "постпанк", "post-punk", "postpunk"],
        words: &[],
        styles: &[
            "Rock---Post-Punk",
            "Rock---Coldwave",
            "Electronic---Darkwave",
            "Rock---New Wave",
        ],
        deezer: &[],
    },
    Genre {
        key: "punk",
        label: "Панк",
        genitive: "панка",
        gender: Gender::M,
        stems: &["панк", "punk"],
        words: &[],
        styles: &["Rock---Punk", "Rock---Pop Punk", "Rock---Hardcore"],
        deezer: &[],
    },
    Genre {
        key: "metal",
        label: "Метал",
        genitive: "метала",
        gender: Gender::M,
        stems: &["метал", "metal"],
        words: &[],
        styles: &["%Metal%", "Rock---Metalcore", "Rock---Deathcore"],
        deezer: &["Metal"],
    },
    Genre {
        key: "shoegaze",
        label: "Шугейз",
        genitive: "шугейза",
        gender: Gender::M,
        stems: &["шугейз", "shoegaze", "дрим-поп", "dream-pop", "dreampop"],
        words: &[],
        styles: &["Rock---Shoegaze", "Rock---Dream Pop"],
        deezer: &[],
    },
    Genre {
        key: "grunge",
        label: "Гранж",
        genitive: "гранжа",
        gender: Gender::M,
        stems: &["гранж", "grunge"],
        words: &[],
        styles: &["Rock---Grunge"],
        deezer: &[],
    },
    Genre {
        key: "emo",
        label: "Эмо",
        genitive: "эмо",
        gender: Gender::N,
        stems: &[],
        words: &["эмо", "emo"],
        styles: &["Rock---Emo", "Rock---Pop Punk"],
        deezer: &[],
    },
    Genre {
        key: "indie",
        label: "Инди",
        genitive: "инди",
        gender: Gender::M,
        stems: &["инди", "indie"],
        words: &[],
        styles: &[
            "Rock---Indie Rock",
            "Pop---Indie Pop",
            "Rock---Lo-Fi",
            "Rock---Dream Pop",
            "Rock---Shoegaze",
            "Rock---Alternative Rock",
        ],
        deezer: &["Alternative", "Indie"],
    },
    Genre {
        key: "alternative",
        label: "Альтернатива",
        genitive: "альтернативы",
        gender: Gender::F,
        stems: &["альтернатив", "alternative"],
        words: &[],
        styles: &[
            "Rock---Alternative Rock",
            "Rock---Indie Rock",
            "Rock---Nu Metal",
        ],
        deezer: &["Alternative"],
    },
    Genre {
        key: "rock",
        label: "Рок",
        genitive: "рока",
        gender: Gender::M,
        stems: &["rock"],
        words: &[
            "рок",
            "рока",
            "року",
            "роком",
            "роке",
            "рокешник",
            "рокешника",
            "рокец",
        ],
        styles: &["Rock---%"],
        deezer: &["Rock"],
    },
    Genre {
        key: "hyperpop",
        label: "Гиперпоп",
        genitive: "гиперпопа",
        gender: Gender::M,
        stems: &["гиперпоп", "hyperpop"],
        words: &[],
        styles: &[],
        deezer: &[],
    },
    Genre {
        key: "kpop",
        label: "K-pop",
        genitive: "k-pop",
        gender: Gender::M,
        stems: &["кпоп", "k-pop", "kpop", "кей-поп"],
        words: &[],
        styles: &["Pop---K-pop"],
        deezer: &[],
    },
    Genre {
        key: "pop",
        label: "Поп",
        genitive: "попсы",
        gender: Gender::M,
        stems: &["попс"],
        words: &["поп", "pop", "попа"],
        styles: &[
            "Pop---%",
            "Electronic---Dance-pop",
            "Rock---Pop Rock",
            "Hip Hop---Pop Rap",
            "Electronic---Synth-pop",
        ],
        deezer: &["Pop"],
    },
    Genre {
        key: "synthwave",
        label: "Синтвейв",
        genitive: "синтвейва",
        gender: Gender::M,
        stems: &[
            "синтвейв",
            "synthwave",
            "ретровейв",
            "retrowave",
            "вейпорвейв",
            "vaporwave",
            "синтипоп",
            "synthpop",
        ],
        words: &[],
        styles: &[
            "Electronic---Synthwave",
            "Electronic---Synth-pop",
            "Electronic---Vaporwave",
            "Electronic---Italo-Disco",
        ],
        deezer: &[],
    },
    Genre {
        key: "techno",
        label: "Техно",
        genitive: "техно",
        gender: Gender::N,
        stems: &["техно", "techno"],
        words: &[],
        styles: &["%Techno%", "Electronic---Minimal"],
        deezer: &["Techno"],
    },
    Genre {
        key: "house",
        label: "Хаус",
        genitive: "хауса",
        gender: Gender::M,
        stems: &["хаус", "house"],
        words: &[],
        styles: &["%House%"],
        deezer: &["House"],
    },
    Genre {
        key: "trance",
        label: "Транс",
        genitive: "транса",
        gender: Gender::M,
        stems: &["trance"],
        words: &["транс", "транса", "трансом", "трансе"],
        styles: &["%Trance%"],
        deezer: &["Trance"],
    },
    Genre {
        key: "dnb",
        label: "Драм-н-бейс",
        genitive: "драм-н-бейса",
        gender: Gender::M,
        stems: &["драмнбейс", "драм-н-бейс", "dnb", "днб", "джангл", "jungle"],
        words: &[],
        styles: &["Electronic---Drum n Bass", "Electronic---Jungle"],
        deezer: &[],
    },
    Genre {
        key: "dubstep",
        label: "Дабстеп",
        genitive: "дабстепа",
        gender: Gender::M,
        stems: &["дабстеп", "dubstep"],
        words: &[],
        styles: &["Electronic---Dubstep", "Electronic---Halftime"],
        deezer: &[],
    },
    Genre {
        key: "hardstyle",
        label: "Хардстайл",
        genitive: "хардстайла",
        gender: Gender::M,
        stems: &["хардстайл", "hardstyle", "хардбас", "hardbass"],
        words: &[],
        styles: &[
            "Electronic---Hardstyle",
            "Electronic---Jumpstyle",
            "Electronic---Hardcore",
        ],
        deezer: &[],
    },
    Genre {
        key: "ambient",
        label: "Эмбиент",
        genitive: "эмбиента",
        gender: Gender::M,
        stems: &["эмбиент", "амбиент", "ambient"],
        words: &[],
        styles: &["%Ambient%", "Electronic---Drone", "Electronic---New Age"],
        deezer: &[],
    },
    Genre {
        key: "lofi",
        label: "Лоу-фай",
        genitive: "лоу-фая",
        gender: Gender::M,
        stems: &["лофай", "лоуфай", "lofi", "lo-fi"],
        words: &[],
        styles: &[
            "Hip Hop---Jazzy Hip-Hop",
            "Hip Hop---Instrumental",
            "Electronic---Downtempo",
            "Rock---Lo-Fi",
        ],
        deezer: &[],
    },
    Genre {
        key: "triphop",
        label: "Трип-хоп",
        genitive: "трип-хопа",
        gender: Gender::M,
        stems: &[
            "трип-хоп",
            "трипхоп",
            "trip-hop",
            "triphop",
            "даунтемпо",
            "downtempo",
        ],
        words: &[],
        styles: &[
            "Electronic---Trip Hop",
            "Hip Hop---Trip Hop",
            "Electronic---Downtempo",
        ],
        deezer: &[],
    },
    Genre {
        key: "electronic",
        label: "Электроника",
        genitive: "электроники",
        gender: Gender::F,
        stems: &["электрон", "electronic", "электро"],
        words: &["edm"],
        styles: &["Electronic---%"],
        deezer: &["Electro", "Dance"],
    },
    Genre {
        key: "disco",
        label: "Диско",
        genitive: "диско",
        gender: Gender::N,
        stems: &["диско", "disco"],
        words: &[],
        styles: &["%Disco%"],
        deezer: &[],
    },
    Genre {
        key: "jazz",
        label: "Джаз",
        genitive: "джаза",
        gender: Gender::M,
        stems: &["джаз", "jazz"],
        words: &[],
        styles: &["Jazz---%"],
        deezer: &["Jazz"],
    },
    Genre {
        key: "blues",
        label: "Блюз",
        genitive: "блюза",
        gender: Gender::M,
        stems: &["блюз", "blues"],
        words: &[],
        styles: &["Blues---%"],
        deezer: &["Blues"],
    },
    Genre {
        key: "soul",
        label: "Соул",
        genitive: "соула",
        gender: Gender::M,
        stems: &["соул", "soul"],
        words: &[],
        styles: &["Funk / Soul---Soul", "Funk / Soul---Neo Soul"],
        deezer: &["Soul"],
    },
    Genre {
        key: "funk",
        label: "Фанк",
        genitive: "фанка",
        gender: Gender::M,
        stems: &["фанк", "funk"],
        words: &[],
        styles: &[
            "Funk / Soul---Funk",
            "Funk / Soul---P.Funk",
            "Jazz---Jazz-Funk",
        ],
        deezer: &["Funk"],
    },
    Genre {
        key: "rnb",
        label: "R&B",
        genitive: "R&B",
        gender: Gender::M,
        stems: &["рнб", "rnb", "r&b", "эрэнби"],
        words: &[],
        styles: &[
            "Funk / Soul---Contemporary R&B",
            "Hip Hop---RnB/Swing",
            "Funk / Soul---Neo Soul",
        ],
        deezer: &["R&B"],
    },
    Genre {
        key: "classical",
        label: "Классика",
        genitive: "классики",
        gender: Gender::F,
        stems: &[
            "классик",
            "классическ",
            "classical",
            "оркестр",
            "orchestral",
        ],
        words: &[],
        styles: &["Classical---%"],
        deezer: &["Classical", "Classique"],
    },
    Genre {
        key: "folk",
        label: "Фолк",
        genitive: "фолка",
        gender: Gender::M,
        stems: &["фолк", "folk", "народн"],
        words: &[],
        styles: &[
            "Folk, World, & Country---Folk",
            "Rock---Folk Rock",
            "Rock---Neofolk",
        ],
        deezer: &[],
    },
    Genre {
        key: "country",
        label: "Кантри",
        genitive: "кантри",
        gender: Gender::N,
        stems: &["кантри", "country"],
        words: &[],
        styles: &["Folk, World, & Country---Country", "Rock---Country Rock"],
        deezer: &["Country"],
    },
    Genre {
        key: "reggae",
        label: "Регги",
        genitive: "регги",
        gender: Gender::N,
        stems: &["регги", "reggae"],
        words: &[],
        styles: &["Reggae---%"],
        deezer: &["Reggae"],
    },
    Genre {
        key: "latin",
        label: "Латино",
        genitive: "латино",
        gender: Gender::N,
        stems: &["латин", "latin", "реггетон", "reggaeton"],
        words: &[],
        styles: &["Latin---%"],
        deezer: &["Latin"],
    },
    Genre {
        key: "chanson",
        label: "Шансон",
        genitive: "шансона",
        gender: Gender::M,
        stems: &["шансон", "chanson"],
        words: &[],
        styles: &["Pop---Chanson"],
        deezer: &["Chanson"],
    },
    Genre {
        key: "soundtrack",
        label: "Саундтреки",
        genitive: "саундтреков",
        gender: Gender::Plural,
        stems: &["саундтрек", "soundtrack"],
        words: &["ost"],
        styles: &["Stage & Screen---%"],
        deezer: &["Films/Games", "Film"],
    },
    Genre {
        key: "acoustic",
        label: "Акустика",
        genitive: "акустики",
        gender: Gender::F,
        stems: &["акустик", "акустич", "acoustic"],
        words: &[],
        styles: &["Rock---Acoustic", "Folk, World, & Country---Folk"],
        deezer: &[],
    },
];

pub fn by_key(key: &str) -> Option<&'static Genre> {
    GENRES.iter().find(|g| g.key == key)
}

/// Жанр, которым может быть слово. Узкие жанры стоят в списке раньше широких: «трэп» — это не «рэп».
pub fn of_word(word: &str) -> Option<&'static Genre> {
    // «популярное» начинается с «поп», но это не жанр
    if word.starts_with("попул") {
        return None;
    }
    GENRES.iter().find(|g| g.matches(word))
}

/// Насколько трек этого жанра (0..): сумма вероятностей подходящих стилей Discogs и половина за жанр
/// альбома у Deezer.
pub fn affinity(genre: &Genre, styles: &[(String, f32)], album_genres: &[String]) -> f32 {
    let by_style: f32 = styles
        .iter()
        .filter(|(name, _)| genre.styles.iter().any(|p| like(name, p)))
        .map(|(_, p)| *p)
        .sum();
    let by_album = album_genres
        .iter()
        .any(|g| genre.deezer.iter().any(|d| g.contains(d)));
    by_style + if by_album { 0.5 } else { 0.0 }
}

/// `LIKE` с `%` в начале и конце — как в базе.
fn like(name: &str, pattern: &str) -> bool {
    match (pattern.strip_prefix('%'), pattern.strip_suffix('%')) {
        (Some(rest), _) if rest.ends_with('%') => name.contains(&rest[..rest.len() - 1]),
        (Some(rest), _) => name.ends_with(rest),
        (None, Some(head)) => name.starts_with(head),
        (None, None) => name == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_find_their_genres() {
        let key = |w: &str| of_word(w).map(|g| g.key);
        assert_eq!(key("фонка"), Some("phonk"));
        assert_eq!(key("phonk"), Some("phonk"));
        assert_eq!(key("инди"), Some("indie"));
        assert_eq!(key("рэпчик"), Some("rap"));
        assert_eq!(key("хип-хоп"), Some("rap"));
        assert_eq!(key("трэп"), Some("trap"));
        assert_eq!(key("рок"), Some("rock"));
        assert_eq!(key("попса"), Some("pop"));
        assert_eq!(key("попсы"), Some("pop"));
        assert_eq!(key("электроники"), Some("electronic"));
        assert_eq!(key("техно"), Some("techno"));
        assert_eq!(key("лофай"), Some("lofi"));
        assert_eq!(key("постпанк"), Some("postpunk"));
        assert_eq!(key("джаза"), Some("jazz"));
        assert_eq!(key("классику"), Some("classical"));
        // не жанры
        assert_eq!(key("популярное"), None);
        assert_eq!(key("рокировка"), None);
        assert_eq!(key("транспорт"), None);
        assert_eq!(key("урок"), None);
        assert_eq!(key("спокойный"), None);
    }

    #[test]
    fn affinity_counts_styles_and_album_genres() {
        let rap = by_key("rap").unwrap();
        let styles = vec![
            ("Hip Hop---Trap".to_string(), 0.4),
            ("Hip Hop---Gangsta".to_string(), 0.2),
            ("Rock---Punk".to_string(), 0.3),
        ];
        assert!((affinity(rap, &styles, &[]) - 0.6).abs() < 1e-6);
        assert!((affinity(rap, &[], &["Rap/Hip Hop".into()]) - 0.5).abs() < 1e-6);
        let metal = by_key("metal").unwrap();
        assert_eq!(affinity(metal, &styles, &[]), 0.0);
        assert!(like("Rock---Heavy Metal", "%Metal%"));
        assert!(like("Electronic---Dub Techno", "%Techno%"));
        assert!(!like("Rock---Punk", "Hip Hop---%"));
    }
}
