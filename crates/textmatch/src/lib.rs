//! Нечёткое сравнение названий для поиска. Человек набирает имя артиста как слышит («капсайз» вместо
//! Capsize), не в той раскладке («сфзышяу»), с опечаткой («lowbron») — поиск всё равно находит.
//!
//! [`Query`] разбирает запрос один раз и оценивает названия от 0 до 1: точное совпадение, та же
//! запись латиницей, начало названия, одинаковое звучание ([`sound_key`]), опечатки. Оценка общая для
//! сервера (каталог Deezer, медиатека) и приложений без сети, поэтому порядок результатов один.

mod fuzzy;
mod layout;
mod sound;
mod text;
mod translit;

pub use fuzzy::{edits, similarity};
pub use layout::fix_layout;
pub use sound::{shape_key, sound_key};
pub use text::{latin, normalize};
pub use translit::{cyrillic, english};

use sound::{key_len, shape_of_latin, sound_of_latin};
use text::{is_cyrillic, latin_of};

/// Совпадение уверенное: то же имя, та же запись латиницей, то же звучание. Ниже — «возможно, вы искали».
pub const STRONG: f32 = 0.9;
/// Ниже этого совпадение случайное: не подсказывать.
pub const SUGGEST: f32 = 0.72;

struct Word {
    latin: String,
    sound: String,
    shape: String,
}

/// Название, разобранное для сравнения. Для многих сравнений с одним названием — создать один раз.
pub struct Name {
    norm: String,
    latin: String,
    sound: String,
    shape: String,
    /// Без артикля в начале: «The Weeknd» звучит как «викенд»
    core_sound: String,
    core_shape: String,
    core_len: usize,
    /// Записано кириллицей
    cyrillic: bool,
    words: Vec<Word>,
}

impl Name {
    pub fn new(s: &str) -> Self {
        let norm = normalize(s);
        let latin = latin_of(&norm);
        let core = latin.strip_prefix("the ").unwrap_or(&latin);
        let words = latin
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| Word {
                latin: w.to_string(),
                sound: sound_of_latin(w),
                shape: shape_of_latin(w),
            })
            .collect();
        Self {
            sound: sound_of_latin(&latin),
            shape: shape_of_latin(&latin),
            core_sound: sound_of_latin(core),
            core_shape: shape_of_latin(core),
            core_len: len(core),
            cyrillic: s.chars().any(is_cyrillic),
            words,
            norm,
            latin,
        }
    }

    /// Ключ звучания (как колонка `sound` в базе).
    pub fn sound(&self) -> &str {
        &self.sound
    }

    /// Латиница (как колонка `latin` в базе).
    pub fn latin(&self) -> &str {
        &self.latin
    }
}

fn len(s: &str) -> usize {
    s.chars().count()
}

/// Слово запроса против слова названия.
fn word_match(q: &Word, n: &Word) -> f32 {
    if q.latin == n.latin {
        return 1.0;
    }
    if len(&q.latin) >= 2 && n.latin.starts_with(&q.latin) {
        return 0.92;
    }
    if key_len(&q.sound) >= 2 && q.sound == n.sound {
        return if q.shape == n.shape { 0.9 } else { 0.86 };
    }
    if len(&q.latin) >= 4 && edits(&q.latin, &n.latin, 1).is_some() {
        return 0.85;
    }
    0.0
}

/// Запрос против названия, без веса формы запроса.
fn compare(q: &Name, n: &Name) -> f32 {
    if q.norm.is_empty() || n.norm.is_empty() {
        return 0.0;
    }
    if q.norm == n.norm {
        return 1.0;
    }
    if q.latin == n.latin {
        return 0.97;
    }
    let ql = len(&q.latin);
    let ratio = ql as f32 / len(&n.latin).max(1) as f32;
    let mut best: f32 = 0.0;
    // Начало названия, начало слова, кусок слова
    if n.norm.starts_with(&q.norm) || n.latin.starts_with(&q.latin) {
        best = 0.82 + 0.12 * ratio;
    } else if ql >= 2 && format!(" {}", n.latin).contains(&format!(" {}", q.latin)) {
        best = 0.74 + 0.12 * ratio;
    } else if ql >= 3 && n.latin.contains(&q.latin) {
        best = 0.55 + 0.12 * ratio;
    }
    // Звучание: по короткому ключу совпадает слишком многое
    let qk = key_len(&q.sound);
    if qk >= 3 {
        // Тот же ключ согласных: насколько совпадают гласные и длина
        let same_sound = |shape: &str, latin_len: usize| {
            // Гласная лишняя или пропущена — так бывает при записи другим алфавитом («викенд» —
            // Weeknd), а в одном алфавите это скорее опечатка («capsze»), и уверенности нет
            let vowels = if shape == q.shape {
                1.0
            } else if q.cyrillic != n.cyrillic
                && len(shape) >= 5
                && edits(shape, &q.shape, 1).is_some()
            {
                0.85
            } else {
                0.6 * similarity(shape, &q.shape)
            };
            let mut s: f32 = 0.8 + 0.13 * vowels;
            // Звучание уверенно связывает только разные алфавиты: в одном несовпадение букв — скорее
            // опечатка или другое имя («capsze» звучит и как Kopps)
            if q.cyrillic == n.cyrillic {
                s = s.min(0.84);
            }
            // «caps» для «capsze»: ключ тот же, но слово заметно короче — это не оно
            let (a, b) = (ql.min(latin_len), ql.max(latin_len));
            if (a as f32) < 0.75 * b as f32 {
                s = s.min(0.68);
            }
            // Из одинаково звучащих (Capsize и CUPSIZE для «капсайз») выше то, что ближе по буквам
            s + 0.02 * similarity(&q.latin, &n.latin)
        };
        if n.sound == q.sound {
            best = best.max(same_sound(&n.shape, len(&n.latin)));
        } else if n.core_sound == q.sound {
            best = best.max(same_sound(&n.core_shape, n.core_len));
        } else if n.sound.starts_with(&q.sound) || n.core_sound.starts_with(&q.sound) {
            best = best.max(0.6 + 0.2 * qk as f32 / key_len(&n.sound).max(1) as f32);
        } else if q
            .sound
            .split(' ')
            .all(|w| n.sound.split(' ').any(|nw| nw == w))
        {
            best = best.max(0.68 + 0.12 * qk as f32 / key_len(&n.sound).max(1) as f32);
        }
    }
    // Опечатки: одна правка на короткое слово, две — на длинное
    if ql >= 4 {
        let max = if ql >= 8 { 2 } else { 1 };
        if let Some(d) = edits(&q.latin, &n.latin, max) {
            best = best.max(if ql >= 8 {
                0.88 - 0.04 * d as f32
            } else {
                0.84
            });
        }
    }
    let sim = similarity(&q.latin, &n.latin);
    if sim >= 0.3 {
        best = best.max(0.8 * sim);
    }
    // Несколько слов: каждое должно найтись в названии, порядок не важен
    if q.words.len() >= 2 {
        let mut sum = 0.0;
        let mut all = true;
        for w in &q.words {
            let m = n
                .words
                .iter()
                .map(|nw| word_match(w, nw))
                .fold(0.0, f32::max);
            if m == 0.0 {
                all = false;
                break;
            }
            sum += m;
        }
        if all {
            best = best.max(0.86 * sum / q.words.len() as f32);
        }
    }
    best
}

/// Разобранный запрос: сам запрос и, если похоже, он же в правильной раскладке.
pub struct Query {
    raw: String,
    forms: Vec<(f32, Name)>,
    layout: Option<String>,
    /// Ключи звучания для [`Query::hits`]: « kps» — с начала слова
    marks: Vec<String>,
}

impl Query {
    pub fn new(q: &str) -> Self {
        let raw = q.trim().to_string();
        let layout = fix_layout(&raw);
        let mut forms = vec![(1.0, Name::new(&raw))];
        if let Some(l) = &layout {
            forms.push((0.98, Name::new(l)));
        }
        let mut q = Self {
            raw,
            forms,
            layout,
            marks: Vec::new(),
        };
        q.marks = q.sounds().into_iter().map(|k| format!(" {k}")).collect();
        q
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Запрос в правильной раскладке, если он был набран не в той.
    pub fn layout(&self) -> Option<&str> {
        self.layout.as_deref()
    }

    /// Нормализованные формы запроса: сам и в другой раскладке.
    pub fn norms(&self) -> Vec<String> {
        self.forms.iter().map(|(_, n)| n.norm.clone()).collect()
    }

    /// Латиница форм запроса — для `search_latin` в базе.
    pub fn latins(&self) -> Vec<String> {
        self.forms.iter().map(|(_, n)| n.latin.clone()).collect()
    }

    /// Ключи звучания форм, которым можно доверять (от трёх согласных).
    pub fn sounds(&self) -> Vec<String> {
        self.forms
            .iter()
            .map(|(_, n)| n.sound.clone())
            .filter(|s| key_len(s) >= 3)
            .collect()
    }

    /// Запрос русскими буквами — тогда для Deezer есть «английское» написание.
    pub fn is_cyrillic(&self) -> bool {
        self.raw.chars().any(is_cyrillic) && !self.raw.chars().any(|c| c.is_ascii_alphabetic())
    }

    /// Быстрый отбор по готовым колонкам — латинице и ключу звучания (названия вместе с артистами), как
    /// условие в базе сервера: каждое слово формы запроса — кусок латиницы или отличается от слова
    /// названия одной буквой, либо ключ звучания запроса стоит в ключе названия с начала слова.
    pub fn hits(&self, latin: &str, sound: &str) -> bool {
        let words: Vec<&str> = latin.split(' ').filter(|w| !w.is_empty()).collect();
        let by_text = self.forms.iter().any(|(_, f)| {
            let mut any = false;
            let all = f
                .latin
                .split(|c: char| c.is_whitespace())
                .filter(|w| !w.is_empty())
                .all(|w| {
                    any = true;
                    latin.contains(w)
                        || (len(w) >= 4 && words.iter().any(|d| edits(w, d, 1).is_some()))
                });
            any && all
        });
        if by_text || self.marks.is_empty() {
            return by_text;
        }
        let sound = format!(" {sound}");
        self.marks.iter().any(|k| sound.contains(k.as_str()))
    }

    /// Насколько название похоже на запрос: 1 — то же самое, от [`STRONG`] — уверенно, 0 — не похоже.
    pub fn score(&self, name: &str) -> f32 {
        self.score_name(&Name::new(name))
    }

    pub fn score_name(&self, name: &Name) -> f32 {
        self.forms
            .iter()
            .map(|(w, f)| w * compare(f, name))
            .fold(0.0, f32::max)
    }

    /// Трек: по названию, по артистам и по всему вместе («lowborn devil»).
    pub fn score_track<S: AsRef<str>>(&self, title: &str, artists: &[S]) -> f32 {
        let by_title = self.score(title);
        let by_artist = artists
            .iter()
            .map(|a| self.score(a.as_ref()))
            .fold(0.0, f32::max);
        let names: Vec<&str> = artists.iter().map(AsRef::as_ref).collect();
        let together = self.score(&format!("{title} {}", names.join(" ")));
        by_title.max(0.95 * by_artist).max(0.95 * together)
    }

    /// Что показать первым и что подсказать. На входе — оценённое ([`Query::score`]) в порядке выдачи:
    /// артисты и альбомы `(id, название, оценка)`, названия треков `(название, оценка)`.
    ///
    /// Лучший результат — артист, в котором поиск уверен (от [`STRONG`]), иначе альбом с точно таким
    /// названием. «Возможно, вы искали» — только без лучшего результата: самое похожее, если оно похоже
    /// не уверенно (от [`SUGGEST`] до [`STRONG`]) и не просто продолжение набранного.
    pub fn verdict<T: Copy>(
        &self,
        artists: &[(T, String, f32)],
        albums: &[(T, String, f32)],
        titles: &[(String, f32)],
    ) -> (Option<Top<T>>, Option<String>) {
        // Из равных — первый: списки уже в порядке выдачи (популярнее — раньше)
        let best = |list: &[(T, String, f32)], min: f32| {
            list.iter()
                .filter(|a| a.2 >= min)
                .fold(None, |best: Option<&(T, String, f32)>, a| match best {
                    Some(b) if b.2 >= a.2 => Some(b),
                    _ => Some(a),
                })
                .map(|a| a.0)
        };
        if let Some(id) = best(artists, STRONG) {
            return (Some(Top::Artist(id)), None);
        }
        if let Some(id) = best(albums, EXACT) {
            return (Some(Top::Album(id)), None);
        }
        let letters = self.raw.chars().filter(|c| c.is_alphanumeric()).count();
        // Есть трек точно с таким названием — человек нашёл, что искал
        if letters < 4 || titles.iter().any(|t| t.1 >= STRONG) {
            return (None, None);
        }
        let q = normalize(&self.raw);
        let all = artists
            .iter()
            .map(|a| (&a.1, a.2))
            .chain(albums.iter().map(|a| (&a.1, a.2)))
            .chain(titles.iter().map(|t| (&t.0, t.1)));
        let suggestion = all
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(name, score)| {
                (SUGGEST..STRONG).contains(score) && !normalize(name).starts_with(&q)
            })
            .map(|(name, _)| name.clone());
        (None, suggestion)
    }
}

/// Совпадение по названию альбома для лучшего результата — только точное.
const EXACT: f32 = 0.97;

/// Лучший результат поиска.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top<T> {
    Artist(T),
    Album(T),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Русская запись ↔ настоящее имя: из реальной медиатеки и общеизвестные.
    const PAIRS: &[(&str, &str)] = &[
        ("капсайз", "Capsize"),
        ("скриптонит", "Scriptonite"),
        ("пирокинезис", "Pyrokinesis"),
        ("лоуборн", "Lowborn"),
        ("мзлф", "Mzlff"),
        ("рашекс", "Rushex"),
        ("оксимирон", "Oxxxymiron"),
        ("моргенштерн", "MORGENSHTERN"),
        ("викенд", "The Weeknd"),
        ("имеджин драгонс", "Imagine Dragons"),
        ("колдплей", "Coldplay"),
        ("биг бэйби тейп", "Big Baby Tape"),
        ("хаски", "Husky"),
        ("Scriptonite", "Скриптонит"),
        ("pyrokinesis", "Пирокинезис"),
    ];

    #[test]
    fn russian_spelling_finds_the_artist() {
        for (q, name) in PAIRS {
            let s = Query::new(q).score(name);
            assert!(s >= 0.84, "{q} → {name}: {s}");
        }
        // Самые частые — уверенно, в «Лучший результат»
        for (q, name) in &PAIRS[..10] {
            let s = Query::new(q).score(name);
            assert!(s >= STRONG, "{q} → {name}: {s}");
        }
    }

    #[test]
    fn the_real_artist_beats_lookalikes() {
        let q = Query::new("капсайз");
        let capsize = q.score("Capsize");
        for other in [
            "Капсайд",
            "Cups",
            "Caps",
            "Kapsalon",
            "Capital Cities",
            "Cassius",
        ] {
            let s = q.score(other);
            assert!(s < capsize, "{other}: {s} ≥ {capsize}");
            assert!(s < STRONG, "{other}: {s}");
        }
    }

    #[test]
    fn closer_spelling_wins_among_same_sound() {
        // CUPSIZE тоже читается «капсайз», но Capsize ближе по буквам
        let q = Query::new("капсайз");
        assert!(q.score("Capsize") > q.score("CUPSIZE"));
        assert!(q.score("CUPSIZE") >= STRONG);
        // Пирокинез — другой артист: звучит похоже, но не уверенно
        let q = Query::new("пирокинезис");
        assert!(q.score("Pyrokinesis") >= STRONG);
        assert!(q.score("Пирокинез") < STRONG);
        // Опечатка «capsze» — не «caps» и не уверенно «Capuze»
        let q = Query::new("capsze");
        assert!(q.score("caps") < SUGGEST);
        assert!(q.score("Capuze") < STRONG);
        assert!(q.score("Kopps") < q.score("Capsize"));
        assert!(q.score("Capsize") >= SUGGEST && q.score("Capsize") < STRONG);
    }

    #[test]
    fn wrong_layout() {
        assert!(Query::new("сфзышяу").score("Capsize") >= STRONG);
        assert!(Query::new("ghbdtn").score("Привет") >= STRONG);
        assert!(Query::new("дщцищкт").score("Lowborn") >= STRONG);
        assert_eq!(Query::new("сфзышяу").layout(), Some("capsize"));
    }

    #[test]
    fn typos() {
        for (q, name) in [
            ("lowbron", "Lowborn"),
            ("capsze", "Capsize"),
            ("scriptonit", "Scriptonite"),
            ("pyrokinessis", "Pyrokinesis"),
            ("imagine dragns", "Imagine Dragons"),
            ("oxxymiron", "Oxxxymiron"),
        ] {
            let s = Query::new(q).score(name);
            assert!(s >= SUGGEST, "{q} → {name}: {s}");
        }
    }

    #[test]
    fn exact_and_prefix_on_top() {
        let q = Query::new("lowborn");
        assert_eq!(q.score("Lowborn"), 1.0);
        assert!(q.score("Lowborn") > q.score("Lowborn Saints"));
        assert!(q.score("Lowborn Saints") > q.score("The Last Lowborn"));
        assert!(Query::new("капс").score("Capsize") < STRONG);
        assert!(Query::new("capsi").score("Capsize") > Query::new("capsi").score("Cassie"));
    }

    #[test]
    fn short_queries_do_not_match_everything() {
        for (q, name) in [
            ("ks", "Kiss"),
            ("мот", "Metallica"),
            ("кап", "Cups"),
            ("lil", "Lowborn"),
            ("rap", "Rihanna"),
        ] {
            let s = Query::new(q).score(name);
            assert!(s < SUGGEST, "{q} → {name}: {s}");
        }
        // Без гласных звучание угадывается, но не уверенно
        assert!(Query::new("lsp").score("Lisp") < STRONG);
        // Начало названия — в выдаче выше, но не «лучший результат»
        assert!(Query::new("ab").score("Abba") < STRONG);
        // Совсем разные — около нуля
        for (q, name) in [
            ("капсайз", "Metallica"),
            ("lowborn", "Rushex"),
            ("мзлф", "Moby"),
        ] {
            let s = Query::new(q).score(name);
            assert!(s < 0.5, "{q} → {name}: {s}");
        }
    }

    #[test]
    fn quick_filter_by_stored_keys() {
        let doc = Name::new("Taking Back My Crown Capsize");
        let hit = |q: &str| Query::new(q).hits(doc.latin(), doc.sound());
        assert!(hit("капсайз"));
        assert!(hit("capsize crown"));
        assert!(hit("crwn capsize"));
        assert!(hit("сфзышяу"));
        assert!(hit("cap"));
        assert!(!hit("lowborn"));
        assert!(!hit("crown lowborn"));
    }

    #[test]
    fn tracks_by_title_and_artist() {
        let q = Query::new("lowborn dev");
        assert!(
            q.score_track("Devil", &["Lowborn"]) > q.score_track("Doctor Doctor", &["Lowborn"])
        );
        let q = Query::new("капсайз");
        assert!(q.score_track("Taking Back My Crown", &["Capsize"]) >= 0.85);
        assert!(q.score_track("Капсайд", &["Кто-то"]) < 0.85);
    }
}
