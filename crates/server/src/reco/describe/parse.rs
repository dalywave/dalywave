//! Разбор описания правилами: русский язык (и латиница), без внешних моделей.
//!
//! Текст режется на слова; проходы по очереди забирают своё: длительность и число треков («40 минут»,
//! «на час», «20 треков»), ориентиры («как Скриптонит», «в духе Rushex»), исключения («без медляков»,
//! «без мата», «без Моргенштерна», «не попса»), язык, характер («редкое», «новое», «популярное»), занятие
//! и обстановку («для зала», «под дождь»), настроение и жанры. Слова о звуке (жанр, настроение,
//! обстановка, незнакомые слова) остаются в тексте для поиска по звучанию — его переводит и понимает
//! сервис анализа (CLAP); служебное (длительность, язык, исключения) из него уходит.

use dalywave_protocol::{
    describe::{BriefArtist, DescribeBrief},
    wave::{WaveActivity, WaveCharacter, WaveLanguage, WaveMood},
};

use super::genres::{self, stem_match};

/// Длительность — от 10 минут до 4 часов; треков — от 5 до 60.
pub const MINUTES: (u32, u32) = (10, 240);
pub const COUNT: (u32, u32) = (5, 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Use {
    /// Ещё ничей.
    Free,
    /// Понят и описывает звук: остаётся в тексте для поиска по звучанию.
    Keep,
    /// Понят и служебный: в поиск по звучанию не идёт.
    Drop,
}

#[derive(Debug, Clone)]
struct Tok {
    raw: String,
    norm: String,
    used: Use,
}

impl Tok {
    fn sep(&self) -> bool {
        self.norm == ","
    }
}

/// Итог разбора: что понято и короткая фраза для заголовка, если в описании нет ничего «заголовочного».
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub brief: DescribeBrief,
    /// Описание без служебного: «спокойное на вечер».
    pub phrase: String,
}

fn norm(s: &str) -> String {
    s.to_lowercase().replace('ё', "е")
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '&' | '\'' | '+')
}

/// Слова и разделители (`,`). Дробь «1,5» остаётся числом, «40мин» делится на «40» и «мин»,
/// «90х» и «80-х» остаются словом.
fn tokenize(text: &str) -> Vec<Tok> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Tok> = Vec::new();
    let mut cur = String::new();
    let push = |cur: &mut String, out: &mut Vec<Tok>| {
        let w = cur
            .trim_matches(|c| matches!(c, '-' | '\'' | '+' | '&'))
            .to_string();
        // «18+» — со своим плюсом
        let w = if cur.ends_with('+') && w.chars().all(|c| c.is_ascii_digit()) {
            format!("{w}+")
        } else {
            w
        };
        if !w.is_empty() {
            out.push(Tok {
                norm: norm(&w),
                raw: w,
                used: Use::Free,
            });
        }
        cur.clear();
    };
    let sep = |out: &mut Vec<Tok>| {
        if out.last().is_some_and(|t| !t.sep()) {
            out.push(Tok {
                raw: ",".into(),
                norm: ",".into(),
                used: Use::Drop,
            });
        }
    };
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        if matches!(c, ',' | '.')
            && prev.is_some_and(|p| p.is_ascii_digit())
            && next.is_some_and(|n| n.is_ascii_digit())
        {
            cur.push('.');
            continue;
        }
        if word_char(c) {
            // Число вплотную к слову: «40мин», «2ч» — но «90х», «80s» остаются эпохой
            if let Some(p) = prev
                && !cur.is_empty()
                && p.is_ascii_digit()
                && c.is_alphabetic()
            {
                let rest: String = chars[i..].iter().take_while(|c| word_char(**c)).collect();
                if !matches!(norm(&rest).as_str(), "х" | "x" | "s" | "ых" | "е") {
                    push(&mut cur, &mut out);
                }
            }
            cur.push(c);
            continue;
        }
        push(&mut cur, &mut out);
        if matches!(
            c,
            ',' | '.' | ';' | '!' | '?' | ':' | '(' | ')' | '—' | '–' | '/' | '\n'
        ) {
            sep(&mut out);
        }
    }
    push(&mut cur, &mut out);
    join_phrases(out)
}

/// Жанры из нескольких слов — одним словом: «хип хоп» → «хип-хоп», «драм н бейс» → «драмнбейс».
fn join_phrases(toks: Vec<Tok>) -> Vec<Tok> {
    const PHRASES: &[(&[&str], &str)] = &[
        (&["хип", "хоп"], "хип-хоп"),
        (&["hip", "hop"], "хип-хоп"),
        (&["драм", "н", "бейс"], "драмнбейс"),
        (&["драм", "энд", "бейс"], "драмнбейс"),
        (&["drum", "and", "bass"], "драмнбейс"),
        (&["drum", "n", "bass"], "драмнбейс"),
        (&["лоу", "фай"], "лофай"),
        (&["ло", "фай"], "лофай"),
        (&["lo", "fi"], "лофай"),
        (&["пост", "панк"], "пост-панк"),
        (&["post", "punk"], "пост-панк"),
        (&["кей", "поп"], "кпоп"),
        (&["k", "pop"], "кпоп"),
        (&["дрим", "поп"], "дрим-поп"),
        (&["dream", "pop"], "дрим-поп"),
        (&["трип", "хоп"], "трип-хоп"),
        (&["trip", "hop"], "трип-хоп"),
        (&["cloud", "rap"], "клауд"),
        (&["клауд", "рэп"], "клауд"),
        (&["клауд", "реп"], "клауд"),
        (&["r", "n", "b"], "рнб"),
        (&["r'n'b"], "рнб"),
        (&["р", "н", "б"], "рнб"),
        (&["a", "la"], "а-ля"),
        (&["а", "ля"], "а-ля"),
    ];
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    let mut i = 0;
    'outer: while i < toks.len() {
        for (words, joined) in PHRASES {
            let n = words.len();
            if i + n <= toks.len() && (0..n).all(|k| toks[i + k].norm == words[k]) {
                let raw = toks[i..i + n]
                    .iter()
                    .map(|t| t.raw.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push(Tok {
                    raw,
                    norm: (*joined).to_string(),
                    used: Use::Free,
                });
                i += n;
                continue 'outer;
            }
        }
        out.push(toks[i].clone());
        i += 1;
    }
    out
}

fn any_stem(w: &str, stems: &[&str], extra: usize) -> bool {
    stems.iter().any(|s| stem_match(w, s, extra))
}

// ───────────────────────────── числа ─────────────────────────────

fn number_word(w: &str) -> Option<f32> {
    Some(match w {
        "один" | "одна" | "одну" | "одного" => 1.0,
        "два" | "две" | "двух" | "пара" | "пару" | "парочку" | "пары" => {
            2.0
        }
        "три" | "трех" | "трёх" => 3.0,
        "четыре" | "четырех" => 4.0,
        "пять" | "пяти" => 5.0,
        "шесть" | "шести" => 6.0,
        "семь" | "семи" => 7.0,
        "восемь" | "восьми" => 8.0,
        "девять" | "девяти" => 9.0,
        "десять" | "десяти" | "десяток" | "десятка" => 10.0,
        "двенадцать" | "двенадцати" => 12.0,
        "пятнадцать" | "пятнадцати" => 15.0,
        "двадцать" | "двадцати" => 20.0,
        "тридцать" | "тридцати" => 30.0,
        "сорок" | "сорока" => 40.0,
        "пятьдесят" | "пятидесяти" => 50.0,
        "шестьдесят" | "шестидесяти" => 60.0,
        "семьдесят" => 70.0,
        "восемьдесят" => 80.0,
        "девяносто" => 90.0,
        "сто" => 100.0,
        "полтора" | "полторы" => 1.5,
        _ => return None,
    })
}

/// Число с позиции `i` и сколько слов оно заняло: «40», «1.5», «сорок», «двадцать пять».
fn number_at(toks: &[Tok], i: usize) -> Option<(f32, usize)> {
    let t = toks.get(i)?;
    if t.used != Use::Free {
        return None;
    }
    if let Ok(n) = t.norm.parse::<f32>() {
        return Some((n, 1));
    }
    let n = number_word(&t.norm)?;
    if (20.0..100.0).contains(&n)
        && let Some(u) = toks.get(i + 1).and_then(|x| number_word(&x.norm))
        && (1.0..10.0).contains(&u)
        && u.fract() == 0.0
    {
        return Some((n + u, 2));
    }
    Some((n, 1))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Unit {
    Minutes,
    Hours,
    Tracks,
}

fn unit_of(w: &str) -> Option<Unit> {
    if w.starts_with("мин") && w.chars().count() <= 8
        || matches!(w, "м" | "m" | "min" | "mins" | "minutes")
    {
        return Some(Unit::Minutes);
    }
    if matches!(
        w,
        "час"
            | "часа"
            | "часов"
            | "часик"
            | "часика"
            | "часок"
            | "ч"
            | "h"
            | "hour"
            | "hours"
    ) {
        return Some(Unit::Hours);
    }
    if any_stem(w, &["трек", "композиц", "песен"], 4)
        || matches!(
            w,
            "песни" | "песня" | "песню" | "вещей" | "штук" | "tracks" | "songs"
        )
    {
        return Some(Unit::Tracks);
    }
    None
}

// ───────────────────────────── словари ─────────────────────────────

/// Служебные слова: не имена и не звук.
const STOP: &[&str] = &[
    "и",
    "а",
    "но",
    "или",
    "в",
    "во",
    "на",
    "с",
    "со",
    "к",
    "ко",
    "по",
    "под",
    "для",
    "из",
    "от",
    "до",
    "о",
    "об",
    "про",
    "чтобы",
    "как",
    "же",
    "бы",
    "то",
    "это",
    "чем",
    "что",
    "так",
    "где",
    "когда",
    "у",
    "за",
    "перед",
    "при",
    "мой",
    "моё",
    "мое",
    "моя",
    "мои",
    "свой",
    "свое",
    "своё",
    "чтоб",
    "ну",
    "вот",
    "был",
    "было",
    "были",
    "есть",
];

/// Слова просьбы: из всего уходят.
const FILLER: &[&str] = &[
    "хочу",
    "хотим",
    "хочется",
    "хотелось",
    "включи",
    "включить",
    "поставь",
    "собери",
    "собрать",
    "сделай",
    "сделать",
    "подбери",
    "давай",
    "дай",
    "найди",
    "нужно",
    "нужен",
    "нужна",
    "нужны",
    "надо",
    "мне",
    "нам",
    "плейлист",
    "плейлиста",
    "плейлистик",
    "плейлистом",
    "подборку",
    "подборка",
    "подборочку",
    "что-нибудь",
    "что-то",
    "чего-нибудь",
    "чего-то",
    "немного",
    "пожалуйста",
    "плиз",
    "какой-нибудь",
    "какую-нибудь",
    "какое-нибудь",
    "какие-нибудь",
    "только",
    "лишь",
    "просто",
    "чисто",
    "прям",
    "прямо",
    "очень",
    "побольше",
    "еще",
    "ещё",
    "именно",
    "музыку",
    "музыка",
    "музыки",
    "музыкой",
    "музон",
    "музло",
    "музыкальный",
    "песни",
    "песен",
    "песенки",
    "треки",
    "треков",
    "трека",
    "трек",
    "трэки",
    "композиции",
    "вещи",
    "чтонибудь",
    "всякое",
    "разное",
    "типа",
    "вроде",
    "штуки",
    "минут",
];

fn stop(w: &str) -> bool {
    STOP.contains(&w)
}

fn filler(w: &str) -> bool {
    FILLER.contains(&w)
}

const PREPS: &[&str] = &[
    "для",
    "под",
    "в",
    "во",
    "на",
    "перед",
    "за",
    "с",
    "по",
    "после",
];

fn slow_word(w: &str) -> bool {
    any_stem(
        w,
        &["медляк", "медлен", "баллад", "сопл", "слоу", "slow"],
        5,
    )
}

fn explicit_word(w: &str) -> bool {
    matches!(
        w,
        "мат"
            | "мата"
            | "матов"
            | "матом"
            | "матюков"
            | "explicit"
            | "18+"
            | "ругани"
            | "брани"
    ) || any_stem(w, &["матерн", "матюк", "нецензур", "ругат", "обсцен"], 6)
}

fn lyrics_word(w: &str) -> bool {
    matches!(w, "слов" | "слова" | "текста" | "пения" | "лирики-нет")
        || any_stem(w, &["вокал", "голос"], 4)
}

fn russian_word(w: &str) -> bool {
    any_stem(w, &["русск", "русскояз", "отечествен", "российск"], 6) || matches!(w, "ru" | "ру")
}

fn foreign_word(w: &str) -> bool {
    any_stem(
        w,
        &[
            "иностран",
            "зарубеж",
            "забугор",
            "западн",
            "англ",
            "буржуй",
            "импортн",
            "англояз",
            "немецк",
            "француз",
            "испан",
            "итальян",
            "корейск",
            "японск",
        ],
        6,
    ) || matches!(w, "foreign" | "english")
}

/// «Новое», «незнакомое».
fn new_word(w: &str) -> bool {
    if w.starts_with("новогод") {
        return false;
    }
    any_stem(
        w,
        &["незнаком", "неслышан", "неизведан", "открыти", "свеж"],
        5,
    ) || matches!(
        w,
        "новое"
            | "новые"
            | "новых"
            | "новенькое"
            | "новенького"
            | "новинки"
            | "новинок"
            | "новый"
            | "новая"
            | "новую"
            | "нового"
            | "новьё"
            | "новье"
    )
}

fn rare_word(w: &str) -> bool {
    any_stem(
        w,
        &[
            "редк",
            "малоизвест",
            "малопопуляр",
            "неизвест",
            "андеграунд",
            "андерграунд",
            "underground",
            "нишев",
            "непопуляр",
            "нераскруч",
        ],
        6,
    ) || matches!(w, "андер" | "rare")
}

fn fame_word(w: &str) -> bool {
    any_stem(
        w,
        &["известн", "популярн", "раскруч", "мейнстрим", "попсов"],
        6,
    )
}

fn pop_word(w: &str) -> bool {
    any_stem(w, &["популярн", "известн", "топов", "мейнстрим", "чарт"], 6)
        || matches!(
            w,
            "хит" | "хиты" | "хитов" | "хитами" | "хитяры" | "хитяра" | "hits"
        )
}

fn fav_word(w: &str) -> bool {
    !w.starts_with("незнаком")
        && (any_stem(
            w,
            &["знаком", "любим", "лайкнут", "проверенн", "заслушан"],
            5,
        ) || matches!(w, "лайки" | "лайков" | "лайках"))
}

struct Activity {
    activity: WaveActivity,
    stems: &'static [&'static str],
    words: &'static [&'static str],
    /// Как назвать в заголовке, если в описании нет своего «для …».
    phrase: &'static str,
}

const ACTIVITIES: &[Activity] = &[
    Activity {
        activity: WaveActivity::Sport,
        stems: &[
            "спортзал",
            "качалк",
            "трениров",
            "тренажер",
            "фитнес",
            "кроссфит",
            "пробежк",
            "пробеж",
            "спорт",
            "workout",
            "кардио",
        ],
        words: &[
            "зал",
            "зала",
            "зале",
            "залу",
            "gym",
            "бег",
            "бега",
            "бегу",
            "бегать",
            "бегаю",
            "пробег",
        ],
        phrase: "для тренировки",
    },
    Activity {
        activity: WaveActivity::Work,
        stems: &[
            "работ",
            "учеб",
            "учёб",
            "фокус",
            "концентрац",
            "сосредоточ",
            "кодинг",
            "программир",
            "домашк",
            "чтени",
            "учит",
        ],
        words: &[
            "код",
            "кода",
            "коде",
            "кодить",
            "focus",
            "work",
            "study",
            "читать",
        ],
        phrase: "для работы",
    },
    Activity {
        activity: WaveActivity::Sleep,
        stems: &["засып", "усып", "колыбельн"],
        words: &[
            "сон",
            "сна",
            "сну",
            "сном",
            "спать",
            "сплю",
            "sleep",
            "засыпания",
        ],
        phrase: "перед сном",
    },
    Activity {
        activity: WaveActivity::Wake,
        stems: &["утр", "просып", "проснут", "пробужд"],
        words: &["подъем", "подъём", "morning"],
        phrase: "с утра",
    },
    Activity {
        activity: WaveActivity::Party,
        stems: &[
            "вечеринк",
            "тусовк",
            "дискотек",
            "танцевальн",
            "танц",
            "клуб",
            "корпоратив",
            "днюх",
            "рейв",
            "party",
            "пати",
        ],
        words: &["туса", "тусы", "тусу", "тусить", "тусе", "rave", "dance"],
        phrase: "для вечеринки",
    },
    Activity {
        activity: WaveActivity::Road,
        stems: &["дорог", "машин", "трасс", "поездк", "путешеств", "автомоб"],
        words: &["руль", "рулем", "рулём", "пути", "drive", "driving"],
        phrase: "в дорогу",
    },
    Activity {
        activity: WaveActivity::Rest,
        stems: &[
            "отдых",
            "расслаб",
            "релакс",
            "чилл",
            "лаунж",
            "lounge",
            "медитац",
            "йог",
            "chill",
            "relax",
        ],
        words: &["чил", "чила", "чилить", "чиле", "фоном", "фон", "спа"],
        phrase: "для отдыха",
    },
];

/// Обстановка без занятия: остаётся для звука и заголовка, «под дождь» ещё и успокаивает.
const SCENES: &[(&[&str], &[&str], Option<WaveMood>)] = &[
    (&["дожд", "ливн", "rain"], &[], Some(WaveMood::Calm)),
    (&["ноч", "night"], &[], None),
    (&["осен", "autumn"], &[], None),
    (&["зимн", "winter"], &["зима", "зиму", "зимой"], None),
    (&["летн", "summer"], &["лето", "летом"], None),
    (&["вечер", "evening"], &[], None),
    (&["весн", "spring"], &[], None),
    (&["прогулк", "прогул"], &["гулять"], None),
    (&["готовк", "уборк"], &["кофе", "душ", "душе"], None),
    (
        &["пляж", "beach", "морск"],
        &["море", "морю", "моря", "морем"],
        None,
    ),
    (&["город", "космос", "космич"], &[], None),
];

const MOODS: &[(WaveMood, &[&str], &[&str])] = &[
    (
        WaveMood::Energetic,
        &[
            "бодр",
            "энергичн",
            "энерги",
            "драйв",
            "мощн",
            "качов",
            "качев",
            "качающ",
            "качает",
            "агрессив",
            "жестк",
            "жёстк",
            "тяжел",
            "тяжёл",
            "громк",
            "взрывн",
            "заряжа",
            "быстр",
            "ярост",
            "злые",
            "злая",
            "злой",
            "злое",
            "темпов",
            "energetic",
            "upbeat",
            "hard",
        ],
        &["огонь", "жара", "жару", "разъеб", "качалово"],
    ),
    (
        WaveMood::Calm,
        &[
            "спокойн",
            "тих",
            "мягк",
            "нежн",
            "медлен",
            "медляк",
            "умиротвор",
            "уютн",
            "ламповн",
            "лампов",
            "легк",
            "лёгк",
            "сонн",
            "мечтат",
            "плавн",
            "расслабля",
            "calm",
            "soft",
            "chill",
        ],
        &[],
    ),
    (
        WaveMood::Happy,
        &[
            "весел",
            "весёл",
            "радост",
            "позитив",
            "солнечн",
            "счастлив",
            "жизнерадост",
            "праздничн",
            "задорн",
            "happy",
        ],
        &[],
    ),
    (
        WaveMood::Sad,
        &[
            "грустн",
            "грусть",
            "печал",
            "меланхол",
            "тоск",
            "депрес",
            "дождлив",
            "разбит",
            "расставан",
            "слезн",
            "sad",
        ],
        &["грустинка", "грустинку", "плакать", "поплакать"],
    ),
];

fn mood_of(w: &str) -> Option<WaveMood> {
    MOODS
        .iter()
        .find(|(_, stems, words)| words.contains(&w) || any_stem(w, stems, 6))
        .map(|(m, _, _)| *m)
}

fn activity_of(w: &str) -> Option<&'static Activity> {
    // «утро» — занятие, но «утренний» — тоже; «работает» и «зала» — да, «залип» — нет
    ACTIVITIES
        .iter()
        .find(|a| a.words.contains(&w) || any_stem(w, a.stems, 6))
}

fn scene_of(w: &str) -> Option<Option<WaveMood>> {
    SCENES
        .iter()
        .find(|(stems, words, _)| words.contains(&w) || any_stem(w, stems, 6))
        .map(|(_, _, m)| *m)
}

/// Слово что-то значит для разбора: на нём кончается имя артиста.
fn known(w: &str) -> bool {
    stop(w)
        || filler(w)
        || genres::of_word(w).is_some()
        || mood_of(w).is_some()
        || activity_of(w).is_some()
        || scene_of(w).is_some()
        || unit_of(w).is_some()
        || number_word(w).is_some()
        || w.parse::<f32>().is_ok()
        || russian_word(w)
        || foreign_word(w)
        || new_word(w)
        || rare_word(w)
        || pop_word(w)
        || slow_word(w)
        || explicit_word(w)
        || matches!(
            w,
            "без"
                | "кроме"
                | "не"
                | "менее"
                | "более"
                | "поменьше"
                | "мало"
                | "самое"
                | "самые"
                | "будто"
                | "обычно"
                | "всегда"
                | "раньше"
                | "можно"
                | "плюс"
                | "минус"
                | "исключая"
                | "похоже"
                | "похожее"
                | "чтобы"
                | "который"
                | "которые"
                | "такое"
                | "такой"
                | "такие"
        )
}

// ───────────────────────────── разбор ─────────────────────────────

struct Parser {
    toks: Vec<Tok>,
    brief: DescribeBrief,
    /// Настроение обстановки («под дождь» — спокойное), если своего не назвали.
    scene_mood: Option<WaveMood>,
}

impl Parser {
    fn free(&self, i: usize) -> bool {
        self.toks.get(i).is_some_and(|t| t.used == Use::Free)
    }

    fn word(&self, i: usize) -> &str {
        self.toks.get(i).map_or("", |t| t.norm.as_str())
    }

    fn mark(&mut self, from: usize, to: usize, used: Use) {
        let last = to.min(self.toks.len() - 1);
        for t in &mut self.toks[from..=last] {
            t.used = used;
        }
    }

    fn set_minutes(&mut self, m: f32) {
        let m = m.round().clamp(MINUTES.0 as f32, MINUTES.1 as f32) as u32;
        self.brief.minutes = Some(m);
        self.brief.count = None;
    }

    fn set_count(&mut self, n: f32) {
        if self.brief.minutes.is_none() {
            self.brief.count = Some(n.round().clamp(COUNT.0 as f32, COUNT.1 as f32) as u32);
        }
    }

    fn apply_unit(&mut self, n: f32, unit: Unit) {
        match unit {
            Unit::Minutes => self.set_minutes(n),
            Unit::Hours => self.set_minutes(n * 60.0),
            Unit::Tracks => self.set_count(n),
        }
    }

    /// «на 40 минут», «час», «полчаса», «20 треков», «минут на сорок», «треков 15».
    fn duration(&mut self) {
        let mut i = 0;
        while i < self.toks.len() {
            if !self.free(i) {
                i += 1;
                continue;
            }
            if let Some((n, len)) = number_at(&self.toks, i)
                && let Some(unit) = unit_of(self.word(i + len))
            {
                self.apply_unit(n, unit);
                let from = if i > 0 && matches!(self.word(i - 1), "на" | "где-то" | "около")
                {
                    i - 1
                } else {
                    i
                };
                self.mark(from, i + len, Use::Drop);
                i += len + 1;
                continue;
            }
            let w = self.word(i).to_string();
            if let Some(unit) = unit_of(&w) {
                // единица впереди: «минут на сорок», «треков 20»
                let mut j = i + 1;
                if self.word(j) == "на" {
                    j += 1;
                }
                if let Some((n, len)) = number_at(&self.toks, j) {
                    self.apply_unit(n, unit);
                    self.mark(i, j + len - 1, Use::Drop);
                    i = j + len;
                    continue;
                }
                if unit == Unit::Hours && self.brief.minutes.is_none() {
                    // «на час», «часик музыки»
                    self.set_minutes(60.0);
                    let from = if i > 0 && self.word(i - 1) == "на" {
                        i - 1
                    } else {
                        i
                    };
                    self.mark(from, i, Use::Drop);
                }
            } else if w.starts_with("полчас") {
                self.set_minutes(30.0);
                let from = if i > 0 && self.word(i - 1) == "на" {
                    i - 1
                } else {
                    i
                };
                self.mark(from, i, Use::Drop);
            }
            i += 1;
        }
    }

    /// Имена с позиции `i`: слова до разделителя или знакомого слова, через «и», «или» и запятую — несколько.
    /// Возвращает имена (позиции слов) и позицию после них.
    fn names(&self, mut i: usize) -> (Vec<Vec<usize>>, usize) {
        let mut names: Vec<Vec<usize>> = Vec::new();
        let mut cur: Vec<usize> = Vec::new();
        while i < self.toks.len() {
            let t = &self.toks[i];
            if t.sep() {
                // «как Скриптонит, Мияги» — дальше имя, если оно с большой буквы или латиницей
                let next = self.toks.get(i + 1);
                let name_next = next.is_some_and(|n| {
                    n.used == Use::Free
                        && !known(&n.norm)
                        && n.raw
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_uppercase() || c.is_ascii_alphabetic())
                });
                if !cur.is_empty() && name_next {
                    names.push(std::mem::take(&mut cur));
                    i += 1;
                    continue;
                }
                break;
            }
            if t.used != Use::Free {
                break;
            }
            if matches!(t.norm.as_str(), "и" | "или" | "&") && !cur.is_empty() {
                let next_name = self
                    .toks
                    .get(i + 1)
                    .is_some_and(|n| n.used == Use::Free && !n.sep() && !known(&n.norm));
                if next_name {
                    names.push(std::mem::take(&mut cur));
                    i += 1;
                    continue;
                }
                break;
            }
            if known(&t.norm) || cur.len() >= 4 {
                break;
            }
            cur.push(i);
            i += 1;
        }
        if !cur.is_empty() {
            names.push(cur);
        }
        (names, i)
    }

    fn artist(&self, idx: &[usize]) -> BriefArtist {
        let said = idx
            .iter()
            .map(|&k| self.toks[k].raw.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        BriefArtist {
            name: said.clone(),
            said,
            id: None,
        }
    }

    /// Длина оборота-ориентира на позиции `i`: «как», «типа», «в духе», «в стиле», «похоже на» …
    fn reference_at(&self, i: usize) -> Option<usize> {
        let w = self.word(i);
        let next = self.word(i + 1);
        let len = match w {
            "как" | "типа" | "вроде" | "наподобие" | "а-ля" | "аля" | "like" => {
                1
            }
            "в" | "во" if matches!(next, "духе" | "стиле" | "стилистике" | "манере") => {
                2
            }
            "по" if matches!(next, "типу" | "мотивам") => 2,
            "из" if next == "разряда" => 2,
            _ if w.starts_with("похож") && next == "на" => 2,
            _ => return None,
        };
        (0..len).all(|k| self.free(i + k)).then_some(len)
    }

    /// «как Скриптонит», «в духе Rushex и Kai Angel»; «в стиле фонк» — жанр, его заберёт проход жанров.
    fn references(&mut self) {
        let mut i = 0;
        while i < self.toks.len() {
            let Some(len) = self.reference_at(i) else {
                i += 1;
                continue;
            };
            let (names, end) = self.names(i + len);
            if names.is_empty() {
                if len > 1 {
                    self.mark(i, i + len - 1, Use::Drop);
                }
                i += len;
                continue;
            }
            for n in &names {
                let a = self.artist(n);
                if !self.brief.like.contains(&a) {
                    self.brief.like.push(a);
                }
            }
            self.mark(i, end - 1, Use::Drop);
            i = end;
        }
    }

    /// Слово после «без»/«не», пропуская «всяких», «очень», «самое»…
    fn skip_soft(&self, mut j: usize) -> usize {
        while matches!(
            self.word(j),
            "всяких"
                | "всякой"
                | "всякого"
                | "всякие"
                | "никаких"
                | "лишних"
                | "лишнего"
                | "особо"
                | "очень"
                | "слишком"
                | "самое"
                | "самый"
                | "самые"
                | "такое"
                | "такой"
                | "сильно"
                | "этих"
                | "этой"
                | "этого"
                | "всего"
                | "всей"
        ) {
            j += 1;
        }
        j
    }

    fn avoid_genre(&mut self, key: &'static str) {
        if !self.brief.avoid_genres.iter().any(|g| g == key) {
            self.brief.avoid_genres.push(key.to_string());
        }
    }

    /// Что после «без …», «не …», «кроме …»: свойство, жанр или артист.
    fn exclusions(&mut self) {
        let mut i = 0;
        while i < self.toks.len() {
            if !self.free(i) {
                i += 1;
                continue;
            }
            let head = self.word(i).to_string();
            let negation = head == "не";
            let without = matches!(head.as_str(), "без" | "кроме" | "исключая" | "минус");
            if !negation && !without {
                i += 1;
                continue;
            }
            let j = self.skip_soft(i + 1);
            if !self.free(j) {
                i += 1;
                continue;
            }
            let w = self.word(j).to_string();
            let next = self.word(j + 1).to_string();
            let mut end = j;
            if slow_word(&w) {
                self.brief.no_slow = true;
            } else if explicit_word(&w) {
                self.brief.no_explicit = true;
            } else if matches!(w.as_str(), "плохих" | "грязных" | "бранных") && next == "слов"
            {
                self.brief.no_explicit = true;
                end = j + 1;
            } else if without && lyrics_word(&w) {
                self.brief.language = WaveLanguage::None;
            } else if russian_word(&w) || (w == "на" && russian_word(&next)) {
                self.brief.language = WaveLanguage::Foreign;
                if w == "на" {
                    end = j + 1;
                }
            } else if foreign_word(&w) || (w == "на" && foreign_word(&next)) {
                self.brief.language = WaveLanguage::Ru;
                if w == "на" {
                    end = j + 1;
                }
            } else if negation && fame_word(&w)
                || without && (pop_word(&w) || w.starts_with("мейнстрим"))
            {
                self.brief.character = WaveCharacter::Rare;
            } else if negation
                && matches!(
                    w.as_str(),
                    "слушал" | "слышал" | "слушала" | "слышала" | "слушали"
                )
                || without && any_stem(&w, &["повтор", "знаком"], 5)
            {
                self.brief.character = WaveCharacter::New;
            } else if negation && any_stem(&w, &["грустн", "печал", "депрес", "уныл", "тоскл"], 6)
            {
                self.brief.mood = WaveMood::Happy;
            } else if negation
                && any_stem(
                    &w,
                    &[
                        "бодр",
                        "энергичн",
                        "громк",
                        "тяжел",
                        "тяжёл",
                        "агрессив",
                        "жестк",
                        "жёстк",
                        "быстр",
                    ],
                    6,
                )
            {
                self.brief.mood = WaveMood::Calm;
            } else if negation && any_stem(&w, &["спокойн", "скучн", "тих"], 6) {
                self.brief.mood = WaveMood::Energetic;
            } else if let Some(g) = genres::of_word(&w) {
                self.avoid_genre(g.key);
                // «без рэпа и попсы»
                let mut k = j + 1;
                while matches!(self.word(k), "и" | "или")
                    && let Some(g) = genres::of_word(self.word(k + 1))
                {
                    self.avoid_genre(g.key);
                    k += 2;
                    end = k - 1;
                }
            } else if without {
                let (names, after) = self.names(j);
                if names.is_empty() {
                    i += 1;
                    continue;
                }
                for n in &names {
                    let a = self.artist(n);
                    if !self.brief.without.contains(&a) {
                        self.brief.without.push(a);
                    }
                }
                end = after - 1;
            } else {
                i += 1;
                continue;
            }
            self.mark(i, end, Use::Drop);
            i = end + 1;
        }
    }

    /// «менее известное», «мало кто знает», «самое популярное».
    fn fame(&mut self) {
        for i in 0..self.toks.len() {
            if !self.free(i) {
                continue;
            }
            let w = self.word(i).to_string();
            let next = self.word(i + 1).to_string();
            if matches!(w.as_str(), "менее" | "поменьше" | "мало" | "малоизвестные")
                && fame_word(&next)
                && self.free(i + 1)
            {
                self.brief.character = WaveCharacter::Rare;
                self.mark(i, i + 1, Use::Drop);
            } else if w == "мало" && next == "кто" {
                self.brief.character = WaveCharacter::Rare;
                let end = if matches!(self.word(i + 2), "знает" | "слышал" | "слушает")
                {
                    i + 2
                } else {
                    i + 1
                };
                self.mark(i, end, Use::Drop);
            } else if matches!(
                w.as_str(),
                "более" | "самое" | "самые" | "самый" | "максимально"
            ) && fame_word(&next)
                && self.free(i + 1)
            {
                self.brief.character = WaveCharacter::Pop;
                self.mark(i, i + 1, Use::Drop);
            }
        }
    }

    fn language(&mut self) {
        for i in 0..self.toks.len() {
            if !self.free(i) {
                continue;
            }
            let w = self.word(i).to_string();
            let lang = if russian_word(&w) {
                WaveLanguage::Ru
            } else if foreign_word(&w) {
                WaveLanguage::Foreign
            } else if any_stem(&w, &["инструментал", "instrumental"], 5) {
                // «инструментал» — ещё и про звук: остаётся для поиска по звучанию
                self.brief.language = WaveLanguage::None;
                self.toks[i].used = Use::Keep;
                continue;
            } else {
                continue;
            };
            if self.brief.language == WaveLanguage::Any {
                self.brief.language = lang;
            }
            let from = if i > 0 && self.free(i - 1) && self.word(i - 1) == "на" {
                i - 1
            } else {
                i
            };
            self.mark(from, i, Use::Drop);
        }
    }

    fn character(&mut self) {
        for i in 0..self.toks.len() {
            if !self.free(i) {
                continue;
            }
            let w = self.word(i).to_string();
            let c = if new_word(&w) {
                WaveCharacter::New
            } else if rare_word(&w) {
                WaveCharacter::Rare
            } else if pop_word(&w) {
                WaveCharacter::Pop
            } else if fav_word(&w) {
                WaveCharacter::Fav
            } else {
                continue;
            };
            if self.brief.character == WaveCharacter::Mix {
                self.brief.character = c;
            }
            // «из лайков», «из любимого»
            let from = if i > 0 && self.free(i - 1) && self.word(i - 1) == "из" {
                i - 1
            } else {
                i
            };
            self.mark(from, i, Use::Drop);
        }
    }

    /// Предлог перед словом `i` (возможно, через одно прилагательное): «для зала», «в долгую дорогу».
    fn phrase_before(&self, i: usize) -> Option<String> {
        let raw = |k: usize| self.toks[k].raw.to_lowercase();
        if i >= 1 && PREPS.contains(&self.word(i - 1)) {
            return Some(format!("{} {}", raw(i - 1), raw(i)));
        }
        if i >= 2
            && PREPS.contains(&self.word(i - 2))
            && self.free(i - 1)
            && !self.toks[i - 1].sep()
            && !known(self.word(i - 1))
        {
            return Some(format!("{} {} {}", raw(i - 2), raw(i - 1), raw(i)));
        }
        None
    }

    fn activity(&mut self) {
        for i in 0..self.toks.len() {
            if !self.free(i) {
                continue;
            }
            let w = self.word(i).to_string();
            if let Some(a) = activity_of(&w) {
                if self.brief.activity == WaveActivity::Any {
                    self.brief.activity = a.activity;
                    self.brief.scene = Some(
                        self.phrase_before(i)
                            .unwrap_or_else(|| a.phrase.to_string()),
                    );
                }
                self.keep_with_prep(i);
            } else if let Some(mood) = scene_of(&w) {
                if self.brief.scene.is_none()
                    && let Some(p) = self.phrase_before(i)
                {
                    self.brief.scene = Some(p);
                }
                if self.scene_mood.is_none() {
                    self.scene_mood = mood;
                }
                self.keep_with_prep(i);
                self.brief.focused = true;
            }
        }
    }

    fn keep_with_prep(&mut self, i: usize) {
        self.toks[i].used = Use::Keep;
        if i >= 1 && PREPS.contains(&self.word(i - 1)) && self.free(i - 1) {
            self.toks[i - 1].used = Use::Keep;
        }
    }

    /// Настроение: первое названное. «Под дождь» без своего настроения — спокойное.
    fn mood(&mut self) {
        for i in 0..self.toks.len() {
            if !self.free(i) {
                continue;
            }
            // «hardstyle» — жанр, а не настроение
            if genres::of_word(self.word(i)).is_some() {
                continue;
            }
            if let Some(m) = mood_of(self.word(i)) {
                if self.brief.mood == WaveMood::Any {
                    self.brief.mood = m;
                }
                // «медляки» — это про спокойное, а не исключение
                self.toks[i].used = Use::Keep;
            }
        }
        if self.brief.mood == WaveMood::Any
            && let Some(m) = self.scene_mood
        {
            self.brief.mood = m;
        }
    }

    fn genres(&mut self) {
        for i in 0..self.toks.len() {
            if !self.free(i) {
                continue;
            }
            if let Some(g) = genres::of_word(self.word(i)) {
                if !self.brief.genres.iter().any(|k| k == g.key)
                    && !self.brief.avoid_genres.iter().any(|k| k == g.key)
                {
                    self.brief.genres.push(g.key.to_string());
                }
                self.toks[i].used = Use::Keep;
                // «в стиле фонк»
                if i >= 2 && self.word(i - 2) == "в" && matches!(self.word(i - 1), "стиле" | "духе")
                {
                    self.mark(i - 2, i - 1, Use::Drop);
                }
            }
        }
    }

    /// Что осталось: просьбы уходят, незнакомые слова — в звук (и делают описание конкретным).
    fn rest(&mut self) {
        for t in &mut self.toks {
            if t.used != Use::Free || t.sep() {
                continue;
            }
            if filler(&t.norm) {
                t.used = Use::Drop;
            } else if !stop(&t.norm) && t.norm.chars().count() > 1 && t.norm.parse::<f32>().is_err()
            {
                self.brief.focused = true;
            }
        }
    }

    /// Текст для поиска по звучанию и фраза для заголовка: всё, что не служебное.
    fn finish(mut self) -> Parsed {
        let kept: Vec<&Tok> = self
            .toks
            .iter()
            .filter(|t| t.used != Use::Drop && !t.sep())
            .collect();
        let sound: Vec<&str> = kept
            .iter()
            .filter(|t| !(t.used == Use::Free && stop(&t.norm)) && t.norm.parse::<f32>().is_err())
            .map(|t| t.norm.as_str())
            .collect();
        self.brief.sound = (!sound.is_empty()).then(|| sound.join(" "));
        // Фраза — без служебных слов по краям: «на вечер» в конце остаётся, «и» — нет
        let mut words: Vec<String> = kept.iter().map(|t| t.raw.to_lowercase()).collect();
        while words
            .first()
            .is_some_and(|w| stop(&norm(w)) && !PREPS.contains(&norm(w).as_str()))
        {
            words.remove(0);
        }
        while words.last().is_some_and(|w| stop(&norm(w))) {
            words.pop();
        }
        if !self.brief.genres.is_empty() || !self.brief.like.is_empty() {
            self.brief.focused = true;
        }
        Parsed {
            brief: self.brief,
            phrase: words.join(" "),
        }
    }
}

/// Разбор описания.
pub fn parse(text: &str) -> Parsed {
    let text = text.trim();
    let mut p = Parser {
        toks: tokenize(text),
        brief: DescribeBrief {
            text: text.to_string(),
            ..Default::default()
        },
        scene_mood: None,
    };
    p.duration();
    p.references();
    p.exclusions();
    p.fame();
    p.language();
    p.character();
    p.activity();
    p.mood();
    p.genres();
    p.rest();
    p.finish()
}
