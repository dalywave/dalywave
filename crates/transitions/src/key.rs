//! Тональность по кругу Камелота: «8A», «12B». Та же шкала совместимости, что у волны на сервере.

/// «8A» → (8, 'A'). Неизвестное и вне круга — None.
pub fn camelot(key: &str) -> Option<(u8, char)> {
    let key = key.trim().to_ascii_uppercase();
    let letter = key.chars().last()?;
    if letter != 'A' && letter != 'B' {
        return None;
    }
    let num: u8 = key[..key.len() - 1].parse().ok()?;
    (1..=12).contains(&num).then_some((num, letter))
}

/// Насколько гладко накладываются две тональности: 1 — та же, 0.8 — соседняя по кругу или
/// параллельная, 0.4 — через одну, 0 — далёкие. Неизвестная — нейтрально, 0.5.
pub fn compat(a: Option<&str>, b: Option<&str>) -> f64 {
    let (Some((na, la)), Some((nb, lb))) = (a.and_then(camelot), b.and_then(camelot)) else {
        return 0.5;
    };
    let diff = (i16::from(na) - i16::from(nb)).rem_euclid(12);
    let step = diff.min(12 - diff);
    match (step, la == lb) {
        (0, true) => 1.0,
        (0, false) | (1, true) => 0.8,
        (2, true) | (1, false) => 0.4,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_of_fifths() {
        assert_eq!(camelot("8a"), Some((8, 'A')));
        assert_eq!(camelot("13A"), None);
        assert_eq!(compat(Some("8A"), Some("8A")), 1.0);
        assert_eq!(compat(Some("12A"), Some("1A")), 0.8);
        assert_eq!(compat(Some("8A"), Some("8B")), 0.8);
        assert_eq!(compat(Some("8A"), Some("10A")), 0.4);
        assert_eq!(compat(Some("8A"), Some("2B")), 0.0);
        assert_eq!(compat(None, Some("2B")), 0.5);
    }
}
