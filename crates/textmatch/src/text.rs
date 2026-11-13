//! Нормализация и латиница. Латиница — та же, что у SQL-функции `search_latin` (миграция
//! 20261113000800_search_sound): ключи, посчитанные здесь и в базе, совпадают.

use unicode_normalization::UnicodeNormalization;

/// Надстрочные знаки латиницы и кириллицы (U+0300–U+036F) — то, что снимает unaccent. Знаки других
/// письменностей (японские дакутэн) остаются: в базе они тоже остаются.
fn is_accent(c: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&c)
}

/// Без регистра и диакритики, «ё» = «е», пробелы схлопнуты. Как `search_norm` на сервере (unaccent):
/// лигатуры и буквы вроде «ß», «ø», «ł» раскладываются так же.
pub fn normalize(s: &str) -> String {
    // Знаки, которые разложение поняло бы иначе, чем unaccent
    let mut pre = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '™' => pre.push(' '),
            '®' => pre.push_str("(r)"),
            'ð' | 'Ð' => pre.push('d'),
            'Ⱥ' | 'ⱥ' => pre.push('a'),
            c => pre.push(c),
        }
    }
    let s = pre;
    let mut out = String::with_capacity(s.len());
    for c in s
        .nfkd()
        .filter(|c| !is_accent(*c))
        .flat_map(char::to_lowercase)
    {
        match c {
            'ё' => out.push('е'),
            'ß' => out.push_str("ss"),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            'ø' => out.push('o'),
            'ł' => out.push('l'),
            'đ' => out.push('d'),
            'þ' => out.push_str("th"),
            'ı' => out.push('i'),
            c => out.push(c),
        }
    }
    // Слоги хангыля и кана с дакутэн — снова целыми, как в базе
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .nfc()
        .collect()
}

/// Кириллица латиницей — по звучанию, а не по ГОСТу: «х» — h, «щ» — sh, «й» и «и» — i.
/// Строка уже нормализована ([`normalize`]).
pub(crate) fn latin_of(norm: &str) -> String {
    let mut out = String::with_capacity(norm.len() + 8);
    for c in norm.chars() {
        let s = match c {
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' | 'ґ' => "g",
            'д' => "d",
            'е' | 'э' | 'є' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' | 'й' | 'і' | 'ї' => "i",
            'к' => "k",
            'л' => "l",
            'м' => "m",
            'н' => "n",
            'о' => "o",
            'п' => "p",
            'р' => "r",
            'с' => "s",
            'т' => "t",
            'у' | 'ў' => "u",
            'ф' => "f",
            'х' => "h",
            'ц' => "ts",
            'ч' => "ch",
            'ш' | 'щ' => "sh",
            'ы' => "y",
            'ю' => "yu",
            'я' => "ya",
            'ъ' | 'ь' => "",
            c => {
                out.push(c);
                continue;
            }
        };
        out.push_str(s);
    }
    out
}

/// Название латиницей: «Скриптонит» — `skriptonit`.
pub fn latin(s: &str) -> String {
    latin_of(&normalize(s))
}

pub(crate) fn is_cyrillic(c: char) -> bool {
    matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё' | 'і' | 'ї' | 'є' | 'ґ' | 'ў' | 'І' | 'Ї' | 'Є' | 'Ґ' | 'Ў')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_marks() {
        assert_eq!(normalize("Ёлка"), "елка");
        assert_eq!(normalize("  Beyoncé   Knowles "), "beyonce knowles");
        // «й» после разложения — «и» с меткой: в поиске это одна буква
        assert_eq!(normalize("Йога"), "иога");
        assert_eq!(normalize("Straße Ørsted"), "strasse orsted");
    }

    #[test]
    fn latin_by_sound() {
        assert_eq!(latin("Скриптонит"), "skriptonit");
        assert_eq!(latin("Моргенштерн"), "morgenshtern");
        assert_eq!(latin("Хаски"), "haski");
        assert_eq!(latin("Пьяный ёжик"), "pyanyi ezhik");
        assert_eq!(latin("Capsize"), "capsize");
    }
}
