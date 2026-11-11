#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Границы треков в секундах — те же, что в tools/gen_audio.py.
pub const BOUNDS_S: [f64; 5] = [0.0, 5.98623, 12.25901, 18.08277, 24.25];

/// (имя набора, частота эталона, расширение)
pub const FORMATS: [(&str, u32, &str); 8] = [
    ("opus_ogg", 48000, "opus"),
    ("opus_mp4", 48000, "mp4"),
    ("opus_webm", 48000, "webm"),
    ("flac16_44", 44100, "flac"),
    ("flac24_96", 96000, "flac"),
    ("mp3_320", 44100, "mp3"),
    ("aac_256", 44100, "m4a"),
    ("vorbis_q6", 44100, "ogg"),
];

pub fn bounds(rate: u32) -> Vec<usize> {
    BOUNDS_S
        .iter()
        .map(|b| (b * rate as f64).round() as usize)
        .collect()
}

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Каталог тестового аудио; генерирует его через ffmpeg, если нет.
pub fn testdata() -> PathBuf {
    static ONCE: OnceLock<PathBuf> = OnceLock::new();
    ONCE.get_or_init(|| {
        let dir = root().join("testdata");
        if !dir.join("manifest.json").exists() {
            let st = std::process::Command::new("python3")
                .arg(root().join("tools/gen_audio.py"))
                .arg("--out")
                .arg(&dir)
                .status()
                .expect("python3 + ffmpeg нужны для генерации тестового аудио");
            assert!(st.success(), "gen_audio.py failed");
        }
        dir
    })
    .clone()
}

pub fn part_path(format: &str, ext: &str, n: usize) -> PathBuf {
    testdata()
        .join("gapless")
        .join(format)
        .join(format!("part{n}.{ext}"))
}

/// Эталоны по частоте: (частота, стерео-сэмплы).
type RefCache = Mutex<Vec<(u32, Vec<f32>)>>;

pub fn reference(rate: u32) -> Vec<f32> {
    static CACHE: OnceLock<RefCache> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some((_, v)) = cache.lock().unwrap().iter().find(|(r, _)| *r == rate) {
        return v.clone();
    }
    let (r, v) =
        dw_audio::testkit::read_wav(&testdata().join("ref").join(format!("ref_{rate}.wav")))
            .unwrap();
    assert_eq!(r, rate);
    cache.lock().unwrap().push((rate, v.clone()));
    v
}

/// Пишет стерео WAV float32 (для синтетических тестов микшера).
pub fn write_wav(path: &Path, rate: u32, samples: &[f32]) {
    let mut f = std::fs::File::create(path).unwrap();
    let data_len = (samples.len() * 4) as u32;
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&3u16.to_le_bytes()).unwrap(); // IEEE float
    f.write_all(&2u16.to_le_bytes()).unwrap();
    f.write_all(&rate.to_le_bytes()).unwrap();
    f.write_all(&(rate * 8).to_le_bytes()).unwrap();
    f.write_all(&8u16.to_le_bytes()).unwrap();
    f.write_all(&32u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&data_len.to_le_bytes()).unwrap();
    for s in samples {
        f.write_all(&s.to_le_bytes()).unwrap();
    }
}

/// Мини-сервер с Range для тестов HTTP-источника (тот же протокол, что у R2).
pub fn http_server() -> String {
    static ADDR: OnceLock<String> = OnceLock::new();
    ADDR.get_or_init(|| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let dir = testdata();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    let _ = serve(stream, &dir);
                });
            }
        });
        addr
    })
    .clone()
}

fn serve(stream: TcpStream, dir: &Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut out = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
        let mut range = None;
        loop {
            let mut h = String::new();
            reader.read_line(&mut h)?;
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some(v) = h
                .strip_prefix("Range: ")
                .or_else(|| h.strip_prefix("range: "))
            {
                range = Some(v.to_string());
            }
        }
        let file = dir.join(path.trim_start_matches('/'));
        let Ok(mut f) = std::fs::File::open(&file) else {
            out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?;
            continue;
        };
        let size = f.metadata()?.len();
        let (start, end, partial) = match range.as_deref().and_then(|r| r.strip_prefix("bytes=")) {
            Some(r) => {
                let (a, b) = r.split_once('-').unwrap();
                if a.is_empty() {
                    let n: u64 = b.parse().unwrap();
                    (size.saturating_sub(n), size - 1, true)
                } else {
                    let a: u64 = a.parse().unwrap();
                    let b = if b.is_empty() {
                        size - 1
                    } else {
                        b.parse::<u64>().unwrap().min(size - 1)
                    };
                    (a, b, true)
                }
            }
            None => (0, size - 1, false),
        };
        let len = end + 1 - start;
        let head = if partial {
            format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {len}\r\nContent-Range: bytes {start}-{end}/{size}\r\nAccept-Ranges: bytes\r\n\r\n"
            )
        } else {
            format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\n\r\n")
        };
        out.write_all(head.as_bytes())?;
        f.seek(SeekFrom::Start(start))?;
        let mut left = len;
        let mut buf = vec![0u8; 64 * 1024];
        while left > 0 {
            let n = f.read(&mut buf[..(left as usize).min(64 * 1024)])?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            left -= n as u64;
        }
    }
}
