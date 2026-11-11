//! Текст для поиска: без регистра и диакритики, «ё» = «е» — как `search_norm` на сервере.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub fn normalize(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_marks() {
        assert_eq!(normalize("Ёлка"), "елка");
        assert_eq!(normalize("  Beyoncé   Knowles "), "beyonce knowles");
        // «й» после разложения — «и» с меткой: в поиске это одна буква, как у unaccent
        assert_eq!(normalize("Йога"), "иога");
    }
}
