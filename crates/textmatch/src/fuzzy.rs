//! Опечатки: триграммы как у pg_trgm и расстояние Дамерау — Левенштейна.

use std::collections::HashSet;

fn trigrams(s: &str) -> HashSet<[char; 3]> {
    let mut set = HashSet::new();
    for word in s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let padded: Vec<char> = "  "
            .chars()
            .chain(word.chars())
            .chain(" ".chars())
            .collect();
        for w in padded.windows(3) {
            set.insert([w[0], w[1], w[2]]);
        }
    }
    set
}

/// Доля общих триграмм (как `similarity()` в pg_trgm): 1 — одинаковые, 0 — ничего общего.
pub fn similarity(a: &str, b: &str) -> f32 {
    let (a, b) = (trigrams(a), trigrams(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let common = a.intersection(&b).count();
    common as f32 / (a.len() + b.len() - common) as f32
}

/// Сколько правок (вставка, удаление, замена, перестановка соседних) отделяет строки; `None`,
/// если больше `max`.
pub fn edits(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let w = b.len() + 1;
    let mut d = vec![0usize; (a.len() + 1) * w];
    for i in 0..=a.len() {
        d[i * w] = i;
    }
    for (j, cell) in d.iter_mut().enumerate().take(w) {
        *cell = j;
    }
    for i in 1..=a.len() {
        let mut row_min = usize::MAX;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[(i - 1) * w + j] + 1)
                .min(d[i * w + j - 1] + 1)
                .min(d[(i - 1) * w + j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[(i - 2) * w + j - 2] + 1);
            }
            d[i * w + j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            return None;
        }
    }
    let r = d[a.len() * w + b.len()];
    (r <= max).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigram_similarity() {
        assert_eq!(similarity("capsize", "capsize"), 1.0);
        assert_eq!(similarity("abc", ""), 0.0);
        let s = similarity("lowborn", "lowbron");
        assert!(s > 0.3 && s < 0.8, "{s}");
    }

    #[test]
    fn edit_distance() {
        assert_eq!(edits("capsize", "capsize", 2), Some(0));
        assert_eq!(edits("capsize", "capdize", 2), Some(1));
        assert_eq!(edits("lowborn", "lwoborn", 2), Some(1));
        assert_eq!(edits("capsize", "kapsiz", 2), Some(2));
        assert_eq!(edits("capsize", "metallica", 2), None);
    }
}
