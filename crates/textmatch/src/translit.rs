//! Запрос другим алфавитом — для поиска в Deezer. Deezer прощает одну-две ошибки в слове, поэтому
//! русскую запись английского имени выгоднее превратить в «английское» написание, а не в ГОСТ:
//! «капсайз» — `capsiz` (находит Capsize), «имеджин драгонс» — `imejin dragons`.

fn english_word(w: &[char]) -> String {
    let mut out = String::with_capacity(w.len() + 4);
    let mut i = 0;
    while i < w.len() {
        let c = w[i];
        let next = w.get(i + 1).copied();
        // Пара букв в конце слова
        let last_pair = i + 2 >= w.len();
        let pair = next.map(|n| [c, n]);
        let two = match pair {
            Some(['а', 'й']) => Some(if last_pair { "y" } else { "i" }),
            Some(['е' | 'э', 'й']) => Some(if last_pair { "ey" } else { "a" }),
            Some(['о', 'й']) => Some("oy"),
            Some(['о' | 'а', 'у']) => Some("ow"),
            Some(['ь', 'ю']) => Some("ew"),
            Some(['д', 'ж']) => Some("j"),
            Some(['к', 'с']) => Some("x"),
            _ => None,
        };
        if let Some(t) = two {
            out.push_str(t);
            i += 2;
            continue;
        }
        let one = match c {
            // В начале слова перед a/o/u/l/r английское «c»: Capsize, Coldplay, Crystal
            'к' if i == 0 && matches!(next, Some('а' | 'о' | 'у' | 'л' | 'р')) => "c",
            'а' => "a",
            'б' => "b",
            'в' => "v",
            'г' | 'ґ' => "g",
            'д' => "d",
            'е' | 'э' | 'є' | 'ё' => "e",
            'ж' => "zh",
            'з' => "z",
            'и' | 'і' | 'ї' => "i",
            'й' => "y",
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
                i += 1;
                continue;
            }
        };
        out.push_str(one);
        i += 1;
    }
    out
}

/// Русская запись — английским написанием: «капсайз» — `capsiz`, «лоуборн» — `lowborn`.
pub fn english(q: &str) -> String {
    q.to_lowercase()
        .split_whitespace()
        .map(|w| english_word(&w.chars().collect::<Vec<_>>()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn cyrillic_word(w: &str) -> String {
    let mut out = String::with_capacity(w.len() * 2);
    let b = w.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let rest = &w[i..];
        let multi = [
            ("shch", "щ"),
            ("sch", "ш"),
            ("sh", "ш"),
            ("ch", "ч"),
            ("zh", "ж"),
            ("kh", "х"),
            ("ts", "ц"),
            ("ya", "я"),
            ("yu", "ю"),
            ("yo", "ё"),
            ("ph", "ф"),
            ("ck", "к"),
            ("ee", "и"),
            ("oo", "у"),
        ]
        .into_iter()
        .find(|(from, _)| rest.starts_with(from));
        if let Some((from, to)) = multi {
            out.push_str(to);
            i += from.len();
            continue;
        }
        let Some(c) = rest.chars().next() else { break };
        let next = rest[c.len_utf8()..].chars().next();
        let prev_vowel = out.chars().last().is_some_and(|p| "аеёиоуыэюя".contains(p));
        let t = match c {
            'a' => "а",
            'b' => "б",
            'c' if matches!(next, Some('e' | 'i' | 'y')) => "с",
            'c' | 'k' | 'q' => "к",
            'd' => "д",
            'e' if i == 0 => "э",
            'e' => "е",
            'f' => "ф",
            'g' => "г",
            'h' => "х",
            'i' => "и",
            'j' => "дж",
            'l' => "л",
            'm' => "м",
            'n' => "н",
            'o' => "о",
            'p' => "п",
            'r' => "р",
            's' => "с",
            't' => "т",
            'u' => "у",
            'v' | 'w' => "в",
            'x' => "кс",
            'y' if prev_vowel => "й",
            'y' => "и",
            'z' => "з",
            c => {
                out.push(c);
                i += c.len_utf8();
                continue;
            }
        };
        out.push_str(t);
        i += c.len_utf8();
    }
    out
}

/// Латинская запись — кириллицей: `skriptonit` — «скриптонит».
pub fn cyrillic(q: &str) -> String {
    q.to_lowercase()
        .split_whitespace()
        .map(cyrillic_word)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_to_english_spelling() {
        for (ru, en) in [
            ("капсайз", "capsiz"),
            ("лоуборн", "lowborn"),
            ("мзлф", "mzlf"),
            ("пирокинезис", "pirokinezis"),
            ("имеджин драгонс", "imejin dragons"),
            ("оксимирон", "oximiron"),
            ("рашекс", "rashex"),
            ("колдплей", "coldpley"),
            ("скай", "sky"),
            ("Лана Дел Рей", "lana del rey"),
        ] {
            assert_eq!(english(ru), en, "{ru}");
        }
    }

    #[test]
    fn latin_to_cyrillic() {
        assert_eq!(cyrillic("skriptonit"), "скриптонит");
        assert_eq!(cyrillic("Morgenshtern"), "моргенштерн");
        assert_eq!(cyrillic("eminem"), "эминем");
        assert_eq!(cyrillic("boy"), "бой");
    }
}
