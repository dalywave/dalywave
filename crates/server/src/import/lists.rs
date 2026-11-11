//! Разбор списков: строки «Артист — Трек», таблица CSV (artist, title[, album], заголовок
//! необязателен; понимает выгрузки Exportify и «Яндекс Музыки») и плейлист M3U/M3U8 (`#EXTINF`).

use dalywave_protocol::imports::ListFormat;

/// Позиция списка.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<i32>,
    pub isrc: Option<String>,
    /// Строка как была — для подписи несопоставленного.
    pub raw: String,
}

/// Больше строк за один импорт не берём.
pub const MAX_LINES: usize = 20_000;

pub fn detect(text: &str) -> ListFormat {
    let head: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(5)
        .collect();
    if head
        .iter()
        .any(|l| l.starts_with("#EXTM3U") || l.starts_with("#EXTINF"))
    {
        return ListFormat::M3u;
    }
    if let Some(first) = head.first() {
        let cells = split_csv_line(first, delimiter(first));
        if cells.len() >= 2 && header_map(&cells).is_some() {
            return ListFormat::Csv;
        }
        // Без заголовка CSV узнаём по одинаковому числу разделителей во всех строках
        let d = delimiter(first);
        let counts: Vec<usize> = head.iter().map(|l| split_csv_line(l, d).len()).collect();
        if counts[0] >= 2
            && counts.iter().all(|c| *c == counts[0])
            && !head.iter().any(|l| dash_split(l).is_some())
        {
            return ListFormat::Csv;
        }
    }
    ListFormat::Text
}

pub fn parse(text: &str, format: Option<ListFormat>) -> (ListFormat, Vec<Line>) {
    let format = format.unwrap_or_else(|| detect(text));
    let mut lines = match format {
        ListFormat::Text => parse_text(text),
        ListFormat::Csv => parse_csv(text),
        ListFormat::M3u => parse_m3u(text),
    };
    lines.truncate(MAX_LINES);
    (format, lines)
}

/// «1. Артист — Трек», «Артист - Трек», «Артист – Трек (2019)»; без разделителя — только название.
fn parse_text(text: &str) -> Vec<Line> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//"))
        .map(|raw| {
            let l = strip_numbering(raw);
            let (artist, title) = dash_split(l).unwrap_or(("", l));
            Line {
                artist: artist.trim().to_string(),
                title: title.trim().to_string(),
                raw: raw.to_string(),
                ..Line::default()
            }
        })
        .filter(|l| !l.title.is_empty())
        .collect()
}

fn strip_numbering(l: &str) -> &str {
    let digits = l.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 4 {
        return l;
    }
    let rest = &l[digits..];
    for sep in [". ", ") ", " - ", " — ", ".\t", " "] {
        if let Some(r) = rest.strip_prefix(sep) {
            return r.trim_start();
        }
    }
    l
}

/// «Артист — Трек»: первое тире с пробелами вокруг (дефис внутри слов — не разделитель).
fn dash_split(l: &str) -> Option<(&str, &str)> {
    [" — ", " – ", " - ", "\t"]
        .iter()
        .filter_map(|sep| l.find(sep).map(|i| (i, sep.len())))
        .min_by_key(|(i, _)| *i)
        .map(|(i, n)| (&l[..i], &l[i + n..]))
}

fn delimiter(line: &str) -> char {
    let count = |c: char| line.chars().filter(|x| *x == c).count();
    [',', ';', '\t']
        .into_iter()
        .max_by_key(|c| count(*c))
        .unwrap_or(',')
}

/// Строка CSV с кавычками («"a, b"», «""» внутри).
fn split_csv_line(line: &str, delim: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == delim && !quoted => cells.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    cells.push(cur);
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

#[derive(Debug, Default)]
struct Columns {
    artist: Option<usize>,
    title: Option<usize>,
    album: Option<usize>,
    duration: Option<usize>,
    isrc: Option<usize>,
}

fn header_map(cells: &[String]) -> Option<Columns> {
    let mut c = Columns::default();
    for (i, cell) in cells.iter().enumerate() {
        let h = cell.trim().trim_start_matches('\u{feff}').to_lowercase();
        let h = h.as_str();
        if c.artist.is_none()
            && matches!(
                h,
                "artist"
                    | "artists"
                    | "artist name"
                    | "artist name(s)"
                    | "artist(s)"
                    | "исполнитель"
                    | "исполнители"
                    | "артист"
            )
        {
            c.artist = Some(i);
        } else if c.title.is_none()
            && matches!(
                h,
                "title" | "track" | "track name" | "name" | "song" | "название" | "трек" | "песня"
            )
        {
            c.title = Some(i);
        } else if c.album.is_none() && matches!(h, "album" | "album name" | "альбом") {
            c.album = Some(i);
        } else if c.duration.is_none()
            && (h.starts_with("duration")
                || h.starts_with("длительность")
                || h == "length"
                || h == "time")
        {
            c.duration = Some(i);
        } else if c.isrc.is_none() && h == "isrc" {
            c.isrc = Some(i);
        }
    }
    c.title.is_some().then_some(c)
}

/// «3:25», «205», «205000» (мс, если в заголовке ms или число большое).
fn duration(cell: &str, header_ms: bool) -> Option<i32> {
    let cell = cell.trim();
    if let Some((m, s)) = cell.split_once(':') {
        let (m, s) = (m.trim().parse::<i32>().ok()?, s.trim().parse::<i32>().ok()?);
        return Some((m * 60 + s) * 1000);
    }
    let n: i64 = cell.parse().ok()?;
    if n <= 0 {
        return None;
    }
    Some(if header_ms || n > 10_000 { n } else { n * 1000 }.min(i64::from(i32::MAX)) as i32)
}

fn parse_csv(text: &str) -> Vec<Line> {
    let mut rows = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let Some(first) = rows.next() else {
        return Vec::new();
    };
    let delim = delimiter(first);
    let first_cells = split_csv_line(first, delim);
    let (cols, header_ms, pending) = match header_map(&first_cells) {
        Some(c) => {
            let ms = c
                .duration
                .is_some_and(|i| first_cells[i].to_lowercase().contains("ms"));
            (c, ms, None)
        }
        None => (
            Columns {
                artist: Some(0),
                title: Some(1),
                album: (first_cells.len() > 2).then_some(2),
                ..Columns::default()
            },
            false,
            Some(first),
        ),
    };
    let cell = |cells: &[String], i: Option<usize>| -> String {
        i.and_then(|i| cells.get(i)).cloned().unwrap_or_default()
    };
    pending
        .into_iter()
        .chain(rows)
        .filter_map(|raw| {
            let cells = split_csv_line(raw, delim);
            let title = cell(&cells, cols.title);
            if title.is_empty() {
                return None;
            }
            // Exportify пишет нескольких артистов через запятую внутри ячейки — первый главный
            let artist = cell(&cells, cols.artist);
            let album = cell(&cells, cols.album);
            let isrc = cell(&cells, cols.isrc);
            Some(Line {
                artist,
                title,
                album: (!album.is_empty()).then_some(album),
                duration_ms: cols
                    .duration
                    .and_then(|i| cells.get(i))
                    .and_then(|c| duration(c, header_ms)),
                isrc: (!isrc.is_empty()).then_some(isrc),
                raw: raw.to_string(),
            })
        })
        .collect()
}

/// `#EXTINF:205,Артист - Трек`, затем путь к файлу; без `#EXTINF` — имя файла «Артист - Трек.mp3».
fn parse_m3u(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut info: Option<(Option<i32>, String)> = None;
    for raw in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let raw = raw.trim_start_matches('\u{feff}');
        if let Some(rest) = raw.strip_prefix("#EXTINF:") {
            let (secs, name) = rest.split_once(',').unwrap_or((rest, ""));
            // Длительность может идти с атрибутами: `#EXTINF:205 tvg-id="…",Имя`
            let secs = secs
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|s| *s > 0.0)
                .map(|s| (s * 1000.0).round() as i32);
            info = Some((secs, name.trim().to_string()));
            continue;
        }
        if raw.starts_with('#') {
            continue;
        }
        let (duration_ms, name) = match info.take() {
            Some((d, name)) if !name.is_empty() => (d, name),
            other => (other.and_then(|(d, _)| d), file_stem(raw)),
        };
        let l = strip_numbering(&name);
        let (artist, title) = dash_split(l).unwrap_or(("", l));
        if title.trim().is_empty() {
            continue;
        }
        out.push(Line {
            artist: artist.trim().to_string(),
            title: title.trim().to_string(),
            duration_ms,
            raw: name.clone(),
            ..Line::default()
        });
    }
    out
}

/// `%20` и прочие коды в пути (плюс остаётся плюсом: это не форма).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && s.is_char_boundary(i + 3)
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn file_stem(path: &str) -> String {
    let decoded = percent_decode(path);
    let name = decoded.rsplit(['/', '\\']).next().unwrap_or(&decoded);
    match name.rsplit_once('.') {
        Some((stem, ext)) if ext.len() <= 5 && !stem.is_empty() => stem.to_string(),
        _ => name.to_string(),
    }
}

/// Название файла без расширения в «Артист — Трек»: для загруженных файлов без тегов.
pub fn from_file_name(name: &str) -> (String, String) {
    let stem = file_stem(name);
    let l = strip_numbering(&stem);
    match dash_split(l) {
        Some((a, t)) => (a.trim().to_string(), t.trim().to_string()),
        None => (String::new(), l.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lines() {
        let (f, lines) = parse(
            "Lowborn — Devil\n2. Mzlff - Ёлка\n\n3) Rushex – Night Drive\nСам по себе\n# комментарий\nT-Fest - Улети",
            None,
        );
        assert_eq!(f, ListFormat::Text);
        let pairs: Vec<(&str, &str)> = lines
            .iter()
            .map(|l| (l.artist.as_str(), l.title.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("Lowborn", "Devil"),
                ("Mzlff", "Ёлка"),
                ("Rushex", "Night Drive"),
                ("", "Сам по себе"),
                ("T-Fest", "Улети"),
            ]
        );
    }

    #[test]
    fn csv_with_and_without_header() {
        let exportify = "\"Track URI\",\"Track Name\",\"Artist Name(s)\",\"Album Name\",\"Duration (ms)\",\"ISRC\"\n\
            \"spotify:track:1\",\"Devil\",\"Lowborn\",\"Devil\",\"162000\",\"QZ5FN2194847\"\n\
            \"spotify:track:2\",\"Light Me Up\",\"Lowborn, vista\",\"Light Me Up\",\"150000\",\"\"";
        let (f, lines) = parse(exportify, None);
        assert_eq!(f, ListFormat::Csv);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].artist, "Lowborn");
        assert_eq!(lines[0].album.as_deref(), Some("Devil"));
        assert_eq!(lines[0].duration_ms, Some(162_000));
        assert_eq!(lines[0].isrc.as_deref(), Some("QZ5FN2194847"));
        assert_eq!(lines[1].artist, "Lowborn, vista");
        assert_eq!(lines[1].isrc, None);

        let (f, lines) = parse("Lowborn;Devil;Devil\nMzlff;\"Ёлка; зима\";", None);
        assert_eq!(f, ListFormat::Csv);
        assert_eq!(lines[1].title, "Ёлка; зима");
        assert_eq!(lines[1].album, None);

        let (_, lines) = parse(
            "исполнитель,название,длительность\nLowborn,Devil,2:42",
            None,
        );
        assert_eq!(lines[0].duration_ms, Some(162_000));
    }

    #[test]
    fn m3u_playlist() {
        let text = "#EXTM3U\n#EXTINF:162,Lowborn - Devil\n/Music/Lowborn/Devil.mp3\n\
            #EXTINF:-1,Mzlff – Ёлка\nhttp://example.com/x.mp3\n\
            /Music/Rushex%20-%20Night%20Drive.flac\n#EXTINF:150 tvg-id=\"x\",vista - Light Me Up\nC:\\m\\a.mp3";
        let (f, lines) = parse(text, None);
        assert_eq!(f, ListFormat::M3u);
        let got: Vec<(&str, &str, Option<i32>)> = lines
            .iter()
            .map(|l| (l.artist.as_str(), l.title.as_str(), l.duration_ms))
            .collect();
        assert_eq!(
            got,
            [
                ("Lowborn", "Devil", Some(162_000)),
                ("Mzlff", "Ёлка", None),
                ("Rushex", "Night Drive", None),
                ("vista", "Light Me Up", Some(150_000)),
            ]
        );
    }

    #[test]
    fn file_names() {
        assert_eq!(
            from_file_name("01 - Lowborn - Devil.flac"),
            ("Lowborn".into(), "Devil".into())
        );
        assert_eq!(from_file_name("track.wav"), (String::new(), "track".into()));
    }
}
