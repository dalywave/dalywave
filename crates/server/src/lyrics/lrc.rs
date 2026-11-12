//! Разбор LRC: `[мм:сс.сс] строка`, несколько меток на строке, `[offset:±мс]`, метки слов `<мм:сс.сс>`.

/// Строка с началом в мс от начала трека; пустой текст — проигрыш.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub t_ms: u32,
    pub text: String,
}

/// `мм:сс`, `мм:сс.с…`, `мм:сс:сс` → мс.
fn timestamp(tag: &str) -> Option<u32> {
    let (min, rest) = tag.split_once(':')?;
    let min: u32 = min.trim().parse().ok()?;
    let (sec, frac) = match rest.split_once(['.', ':']) {
        Some((s, f)) => (s, f),
        None => (rest, ""),
    };
    let sec: u32 = sec.trim().parse().ok()?;
    if sec >= 60 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // Дробная часть — доли секунды любой длины: «5» — 500 мс, «05» — 50, «005» — 5
    let frac_ms = frac
        .chars()
        .take(3)
        .chain(std::iter::repeat('0'))
        .take(3)
        .collect::<String>()
        .parse::<u32>()
        .unwrap_or(0);
    Some(min.checked_mul(60_000)? + sec * 1000 + frac_ms)
}

/// Убирает метки слов расширенного LRC: `<00:12.34>`.
fn strip_word_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let Some(len) = rest[start..].find('>') else {
            break;
        };
        out.push_str(&rest[..start]);
        let tag = &rest[start + 1..start + len];
        if timestamp(tag).is_none() {
            out.push_str(&rest[start..=start + len]);
        }
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Строки по возрастанию времени. Подряд идущие пустые схлопываются, пустые в начале убираются.
pub fn parse(lrc: &str) -> Vec<Line> {
    let mut offset: i64 = 0;
    let mut lines: Vec<Line> = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(body) = rest.strip_prefix('[') {
            let Some(end) = body.find(']') else {
                break;
            };
            let tag = &body[..end];
            if let Some(t) = timestamp(tag) {
                times.push(t);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
            }
            rest = body[end + 1..].trim_start();
        }
        if times.is_empty() {
            continue;
        }
        let text = strip_word_tags(rest);
        for t in times {
            lines.push(Line {
                t_ms: t,
                text: text.clone(),
            });
        }
    }
    // Положительный offset — текст показывается раньше
    if offset != 0 {
        for l in &mut lines {
            l.t_ms = (i64::from(l.t_ms) - offset).clamp(0, i64::from(u32::MAX)) as u32;
        }
    }
    lines.sort_by_key(|l| l.t_ms);
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for l in lines {
        if l.text.is_empty() && out.last().is_none_or(|p| p.text.is_empty()) {
            continue;
        }
        out.push(l);
    }
    while out.last().is_some_and(|l| l.text.is_empty()) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(t_ms: u32, text: &str) -> Line {
        Line {
            t_ms,
            text: text.into(),
        }
    }

    #[test]
    fn timestamps() {
        assert_eq!(timestamp("00:12.34"), Some(12_340));
        assert_eq!(timestamp("01:02.5"), Some(62_500));
        assert_eq!(timestamp("01:02.005"), Some(62_005));
        assert_eq!(timestamp("1:02"), Some(62_000));
        assert_eq!(timestamp("01:02:50"), Some(62_500));
        assert_eq!(timestamp("ar:Someone"), None);
        assert_eq!(timestamp("00:75.00"), None);
    }

    #[test]
    fn parses_tags_repeats_offset_and_words() {
        let lrc = "[ar:Lowborn]\n[ti:Light Me Up]\n[offset:+500]\n\n[00:00.00]\n[00:05.00]\n\
                   [00:10.50]Standing face to face\n[00:20.00][01:00.00] Chorus  line \n\
                   [00:30.00]<00:30.00>word <00:31.00>by <00:32.00>word\n[00:40.00]\n[00:41.00]\n\
                   [00:50.00]After the break\nnot a line\n[02:00.00]";
        assert_eq!(
            parse(lrc),
            vec![
                l(10_000, "Standing face to face"),
                l(19_500, "Chorus line"),
                l(29_500, "word by word"),
                l(39_500, ""),
                l(49_500, "After the break"),
                l(59_500, "Chorus line"),
            ]
        );
    }

    #[test]
    fn negative_offset_and_garbage() {
        assert_eq!(
            parse("[offset:-1000]\n[00:01.00]a\n[xx:yy]b"),
            vec![l(2000, "a")]
        );
        assert!(parse("просто текст\nбез меток").is_empty());
    }
}
