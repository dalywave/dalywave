//! Ключ звучания: «капсайз» и «Capsize», «скриптонит» и «Scriptonite», «мзлф» и «Mzlff» дают один ключ.
//!
//! Обе записи сначала приводятся к латинице ([`crate::latin`]), затем к упрощённому звучанию: c/k/q — k,
//! z — s, sh/sch — s, ph — f, j/dzh/zh — g, w перед гласной — v, удвоенные буквы — одна. Ключ
//! [`sound_key`] — только согласные, он ищется индексом; [`shape_key`] оставляет на месте каждой группы
//! гласных одну «a» и уточняет совпадение (Capsize и Cups дают один ключ согласных, но разную форму).
//!
//! [`sound_key`] повторяет SQL-функцию `sound_key` (миграция 20261113000800_search_sound) шаг в шаг:
//! её считают индексы и колонки базы. Изменения — в обоих местах сразу и новой миграцией.

use crate::text::{latin_of, normalize};

const VOWELS: &[char] = &['a', 'e', 'i', 'o', 'u', 'y'];

fn is_vowel(c: Option<char>) -> bool {
    c.is_some_and(|c| VOWELS.contains(&c))
}

/// Подряд идущие одинаковые буквы — одна (`([a-z])\1+` → `\1`).
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev = None;
    for c in s.chars() {
        if c.is_ascii_lowercase() && prev == Some(c) {
            continue;
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// Замена буквы по следующей за ней (`c(?=[eiy])` → `s`): смотрит на исходную строку, как регулярка.
fn by_next(s: &str, from: char, when: impl Fn(Option<char>) -> bool, to: Option<char>) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == from && when(chars.get(i + 1).copied()) {
            if let Some(t) = to {
                out.push(t);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Общая часть обоих ключей: латиница → упрощённое звучание (гласные ещё на месте).
fn phonetic(latin: &str) -> String {
    // Всё, кроме латинских букв и цифр, — разделитель слов
    let mut s = String::with_capacity(latin.len());
    let mut gap = false;
    for c in latin.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            s.push(c);
            gap = false;
        } else if !gap {
            s.push(' ');
            gap = true;
        }
    }
    let mut s = collapse(&s);
    for (from, to) in [
        ("dzh", "g"),
        ("dj", "g"),
        ("zh", "g"),
        ("tch", "ch"),
        ("sch", "sh"),
        ("ch", "C"),
        ("ck", "k"),
        ("ph", "f"),
        ("th", "t"),
        ("kh", "h"),
        ("gh", "g"),
        ("sh", "s"),
        ("qu", "kv"),
    ] {
        s = s.replace(from, to);
    }
    s = by_next(&s, 'c', |n| matches!(n, Some('e' | 'i' | 'y')), Some('s'));
    s = s.replace('c', "k").replace('C', "c");
    s = s
        .chars()
        .map(|c| match c {
            'q' => 'k',
            'j' => 'g',
            'z' => 's',
            c => c,
        })
        .collect();
    s = s.replace('x', "ks");
    s = by_next(&s, 'w', is_vowel, Some('v'));
    s = s.replace('w', "u");
    by_next(&s, 'h', |n| !is_vowel(n), None)
}

fn tidy(s: &str) -> String {
    s.split(' ')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn sound_of_latin(latin: &str) -> String {
    let s: String = phonetic(latin)
        .chars()
        .filter(|c| !VOWELS.contains(c))
        .collect();
    tidy(&collapse(&s))
}

pub(crate) fn shape_of_latin(latin: &str) -> String {
    let p = phonetic(latin);
    let words = p.split(' ').filter(|w| !w.is_empty()).map(|w| {
        let mut out = String::with_capacity(w.len());
        let mut in_vowels = false;
        for c in w.chars() {
            if VOWELS.contains(&c) {
                if !in_vowels {
                    out.push('a');
                }
                in_vowels = true;
            } else {
                out.push(c);
                in_vowels = false;
            }
        }
        // Конечная гласная на слух почти не слышна: capsize — «капсайз»
        if out.len() > 1 && out.ends_with('a') {
            out.pop();
        }
        collapse(&out)
    });
    words.collect::<Vec<_>>().join(" ")
}

/// Ключ звучания — только согласные: «Capsize» и «капсайз» — `kps`.
pub fn sound_key(s: &str) -> String {
    sound_of_latin(&latin_of(&normalize(s)))
}

/// Форма звучания: группа гласных — одна «a»: «Capsize» и «капсайз» — `kapsas`.
pub fn shape_key(s: &str) -> String {
    shape_of_latin(&latin_of(&normalize(s)))
}

/// Сколько букв в ключе (без пробелов): по коротким ключам звучанию не доверяем.
pub(crate) fn key_len(key: &str) -> usize {
    key.chars().filter(|c| *c != ' ').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_sound_same_key() {
        for (a, b) in [
            ("Capsize", "капсайз"),
            ("Scriptonite", "скриптонит"),
            ("Pyrokinesis", "пирокинезис"),
            ("Mzlff", "мзлф"),
            ("Lowborn", "лоуборн"),
            ("Rushex", "рашекс"),
            ("Oxxxymiron", "оксимирон"),
            ("MORGENSHTERN", "моргенштерн"),
            ("Imagine Dragons", "имеджин драгонс"),
            ("Coldplay", "колдплей"),
            ("Queen", "квин"),
            ("Husky", "хаски"),
            ("Face", "фейс"),
            ("Big Baby Tape", "биг бэйби тейп"),
            ("Jah Khalib", "джах халиб"),
        ] {
            assert_eq!(sound_key(a), sound_key(b), "{a} / {b}");
            assert_eq!(shape_key(a), shape_key(b), "{a} / {b}");
        }
        assert_eq!(sound_key("Capsize"), "kps");
        assert_eq!(shape_key("Capsize"), "kapsas");
        assert_eq!(sound_key("The Weeknd"), "t vknd");
        assert_eq!(sound_key("викенд"), "vknd");
    }

    #[test]
    fn different_shape_for_lookalikes() {
        assert_eq!(sound_key("Cups"), sound_key("Capsize"));
        assert_ne!(shape_key("Cups"), shape_key("Capsize"));
    }

    #[test]
    fn keeps_digits_and_drops_symbols() {
        assert_eq!(sound_key("50 Cent"), "50 snt");
        assert_eq!(sound_key("AC/DC"), "k dk");
        assert_eq!(sound_key("!!!"), "");
        assert_eq!(sound_key("宇多田ヒカル"), "");
    }
}
