//! Неверная раскладка: «сфзышяу» — это capsize, «ghbdtn» — «привет» (ЙЦУКЕН ↔ QWERTY).

use crate::text::is_cyrillic;

const EN: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`{}:\"<>~";
const RU: &str = "йцукенгшщзхъфывапролджэячсмитьбюёхъжэбюё";

fn to_ru(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| {
            EN.chars()
                .position(|e| e == c)
                .and_then(|i| RU.chars().nth(i))
                .unwrap_or(c)
        })
        .collect()
}

fn to_en(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| {
            RU.chars()
                .position(|r| r == c)
                .and_then(|i| EN.chars().nth(i))
                .unwrap_or(c)
        })
        .collect()
}

const RU_VOWELS: &str = "аеёиоуыэюя";
const EN_VOWELS: &str = "aeiouy";
/// Сочетания, которых в русском не бывает: «жы», «шя», мягкий знак после гласной…
const RU_NEVER: &[&str] = &[
    "жы", "шы", "щы", "чы", "жя", "шя", "щя", "чя", "жю", "шю", "щю", "ьь", "ъъ", "йй", "ыы", "ьъ",
    "ъь", "аь", "еь", "иь", "оь", "уь", "ыь", "эь", "юь", "яь", "аъ", "оъ", "уъ", "иъ", "ыъ", "эъ",
    "йь", "йъ", "ць", "ьы", "ъы", "йы", "ьй",
];

/// Похоже на слова языка: есть гласные, и согласных подряд не больше четырёх.
fn natural(word: &str, vowels: &str) -> bool {
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return true;
    }
    let v = letters.iter().filter(|c| vowels.contains(**c)).count();
    if letters.len() >= 3 {
        let ratio = v as f32 / letters.len() as f32;
        if !(0.2..=0.8).contains(&ratio) {
            return false;
        }
    }
    let mut run = 0;
    for c in &letters {
        run = if vowels.contains(*c) { 0 } else { run + 1 };
        if run > 4 {
            return false;
        }
    }
    true
}

fn plausible_ru(s: &str) -> bool {
    s.split_whitespace().all(|w| {
        w.chars()
            .all(|c| is_cyrillic(c) || c.is_ascii_digit() || c == '-')
            && !w.starts_with(['ь', 'ъ', 'ы'])
            && !RU_NEVER.iter().any(|p| w.contains(p))
            && natural(w, RU_VOWELS)
    })
}

fn plausible_en(s: &str) -> bool {
    s.split_whitespace().all(|w| {
        w.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '&' | '\''))
            && natural(w, EN_VOWELS)
    })
}

/// Частые пары букв: 90 % всех пар в русских и английских текстах (посчитано по документации проекта и
/// README пакетов). Доля частых пар показывает, насколько строка похожа на слова языка.
const RU_PAIRS: &str = "стпоретоеркананиранеетенноанрокоовзаодлипртристаосолгоотеквелельваалсеорлоомводатемеесаткиитслныойедтидеоксядочаиниеогаетьилжескриелчедиизтсхоейияласоасакемоевыазачобннуспеадбомиавонезарсиайчиляпигрсавиимруйлыйегамапбешеивмоикочкуожрвлуоптвплзвссийпадлкрщертыеаярыузднкетыткьксыиимандутушпунувнужоздршаидйсвкшисвсплюбрняееучагклытлнфамучтдуыхьньшэтциятряждицютывсьнтвстнаювуихбырнылжизыснирвлчнабаублщитуксктуюцегиигудлкоиврнквтзиежхрбаяезнжн";
const EN_PAIRS: &str = "thinreeronheoresatenancotetisestdeitlealntngarioedndtoisnsroomliverimattasratroumepallpechhtceecettagenouracilcttpsipshaearslofousodsocaneprunfinctsulhiopwiicgiexrtimelefutlaambepttynapldipoolofemadifssapmpowotagrcjsieueckdoigbumohokeluvasumiubscblepabidltrnlyfaypocnpshhuturrwhevcrpmyofeobbaruairysppildlsoodseeniovrgumirauppvidatcsaivosipayupavclcifuquuiwabriaweegbofrrmcswohr";

fn common_pairs(s: &str, table: &str) -> f32 {
    let pairs: Vec<char> = table.chars().collect();
    let (mut all, mut common) = (0, 0);
    for w in s.split_whitespace() {
        let w: Vec<char> = w.chars().collect();
        for p in w.windows(2) {
            all += 1;
            if pairs.chunks(2).any(|t| t == p) {
                common += 1;
            }
        }
    }
    if all == 0 {
        0.0
    } else {
        common as f32 / all as f32
    }
}

/// Запрос, набранный не в той раскладке, — в правильной; `None`, если запрос и так похож на слова.
pub fn fix_layout(q: &str) -> Option<String> {
    let q = q.trim().to_lowercase().replace('ё', "е");
    // Короткое без знаков препинания внутри — скорее сокращение (LSP), чем не та раскладка
    let letters = q.chars().filter(|c| c.is_alphabetic()).count();
    let marks = q
        .chars()
        .any(|c| matches!(c, ';' | '\'' | '[' | ']' | ',' | '.' | '`'));
    if letters < 4 && !marks {
        return None;
    }
    let cyr = q.chars().any(is_cyrillic);
    let lat = q.chars().any(|c| c.is_ascii_alphabetic());
    // Исправленное должно быть похоже на слова, а исходное — нет (или заметно меньше)
    match (cyr, lat) {
        (true, false) => {
            let fixed = to_en(&q);
            let better = common_pairs(&fixed, EN_PAIRS) - common_pairs(&q, RU_PAIRS) >= 0.25;
            (plausible_en(&fixed) && (!plausible_ru(&q) || better)).then_some(fixed)
        }
        (false, true) => {
            let fixed = to_ru(&q);
            let better = common_pairs(&fixed, RU_PAIRS) - common_pairs(&q, EN_PAIRS) >= 0.25;
            (plausible_ru(&fixed) && (!plausible_en(&q) || better)).then_some(fixed)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_wrong_layout() {
        assert_eq!(fix_layout("сфзышяу").as_deref(), Some("capsize"));
        assert_eq!(fix_layout("ghbdtn").as_deref(), Some("привет"));
        assert_eq!(fix_layout("Ghbdtn").as_deref(), Some("привет"));
        assert_eq!(fix_layout("дщцищкт").as_deref(), Some("lowborn"));
        assert_eq!(fix_layout("ыскшзещтшеу").as_deref(), Some("scriptonite"));
        assert_eq!(fix_layout("знкщлштуышы").as_deref(), Some("pyrokinesis"));
        assert_eq!(fix_layout("k.,jdm").as_deref(), Some("любовь"));
        assert_eq!(fix_layout("rbyj").as_deref(), Some("кино"));
        assert_eq!(fix_layout("ptvabhf").as_deref(), Some("земфира"));
    }

    #[test]
    fn leaves_real_words() {
        for q in [
            "капсайз",
            "скриптонит",
            "Capsize",
            "Mzlff",
            "Lowborn",
            "привет",
            "моргенштерн",
            "lsp",
            "The Weeknd",
            "Imagine Dragons",
            "Oxxxymiron",
            "50 cent",
            "eminem",
            "Rushex",
            "Lana Del Rey",
            "Big Baby Tape",
            "kizaru",
            "земфира",
            "кино",
            "баста",
        ] {
            assert_eq!(fix_layout(q), None, "{q}");
        }
    }
}
