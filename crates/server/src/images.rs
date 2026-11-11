//! Копии обложек и фото: три размера JPEG и палитра обложки для цветного фона интерфейса.

use std::io::Cursor;

use anyhow::Context;
use image::{DynamicImage, ImageReader, Limits, codecs::jpeg::JpegEncoder, imageops::FilterType};

use crate::storage::IMAGE_SIZES;

/// Больше Deezer не отдаёт; всё крупнее — подозрительный файл.
const MAX_SIDE: u32 = 4096;
const JPEG_QUALITY: u8 = 86;
const PALETTE_SIDE: u32 = 64;
const PALETTE_K: usize = 5;

pub struct Processed {
    /// (размер, JPEG) от большего к меньшему.
    pub variants: Vec<(u32, Vec<u8>)>,
    /// `#rrggbb`, самый заметный цвет первым.
    pub palette: Vec<String>,
}

impl Processed {
    pub fn total_bytes(&self) -> usize {
        self.variants.iter().map(|(_, b)| b.len()).sum()
    }
}

/// Разбирает картинку и готовит копии. Тяжёлая работа: вызывать через `spawn_blocking`.
pub fn process(bytes: &[u8], with_palette: bool) -> anyhow::Result<Processed> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context("read image")?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let img = DynamicImage::ImageRgb8(reader.decode().context("decode image")?.to_rgb8());

    let mut variants = Vec::with_capacity(IMAGE_SIZES.len());
    for size in IMAGE_SIZES {
        // Обложки квадратные; чужие пропорции обрезаются по центру, а не сплющиваются
        let resized = if img.width() == size && img.height() == size {
            img.clone()
        } else {
            img.resize_to_fill(size, size, FilterType::CatmullRom)
        };
        let mut out = Vec::new();
        resized
            .write_with_encoder(JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY))
            .context("encode jpeg")?;
        variants.push((size, out));
    }
    let palette = if with_palette {
        palette(&img.thumbnail_exact(PALETTE_SIDE, PALETTE_SIDE))
    } else {
        Vec::new()
    };
    Ok(Processed { variants, palette })
}

/// 3–5 основных цветов: k-средних в Oklab (там расстояние ближе к тому, как цвет видит глаз).
pub fn palette(img: &DynamicImage) -> Vec<String> {
    let rgb = img.to_rgb8();
    let pixels: Vec<[f32; 3]> = rgb.pixels().map(|p| to_oklab(p.0)).collect();
    if pixels.is_empty() {
        return Vec::new();
    }
    let mut centers = init_centers(&pixels, PALETTE_K);
    let mut sizes = vec![0usize; centers.len()];
    for _ in 0..12 {
        let mut sums = vec![[0f32; 3]; centers.len()];
        sizes.iter_mut().for_each(|s| *s = 0);
        for p in &pixels {
            let i = nearest(&centers, p);
            sizes[i] += 1;
            for c in 0..3 {
                sums[i][c] += p[c];
            }
        }
        for (i, center) in centers.iter_mut().enumerate() {
            if sizes[i] > 0 {
                let n = sizes[i] as f32;
                *center = [sums[i][0] / n, sums[i][1] / n, sums[i][2] / n];
            }
        }
    }
    let mut ranked: Vec<(usize, [f32; 3])> = sizes.into_iter().zip(centers).collect();
    ranked.sort_by_key(|r| std::cmp::Reverse(r.0));
    let total = pixels.len() as f32;
    let mut chosen: Vec<[f32; 3]> = Vec::new();
    for (size, color) in &ranked {
        let share = *size as f32 / total;
        let distinct = chosen.iter().all(|c| dist(c, color) > 0.05);
        if share >= 0.03 && distinct {
            chosen.push(*color);
        }
    }
    // Почти одноцветная обложка: добираем до трёх оставшимися центрами
    for (_, color) in &ranked {
        if chosen.len() >= 3 {
            break;
        }
        if !chosen.contains(color) {
            chosen.push(*color);
        }
    }
    chosen.truncate(PALETTE_K);
    chosen.into_iter().map(|c| hex(from_oklab(c))).collect()
}

/// Первый центр — средний цвет, следующие — самые далёкие от уже выбранных. Без случайности:
/// одна и та же обложка всегда даёт одну палитру.
fn init_centers(pixels: &[[f32; 3]], k: usize) -> Vec<[f32; 3]> {
    let n = pixels.len() as f32;
    let mut mean = [0f32; 3];
    for p in pixels {
        for c in 0..3 {
            mean[c] += p[c] / n;
        }
    }
    let mut centers = vec![mean];
    while centers.len() < k {
        let far = pixels
            .iter()
            .max_by(|a, b| {
                let da = centers.iter().map(|c| dist(c, a)).fold(f32::MAX, f32::min);
                let db = centers.iter().map(|c| dist(c, b)).fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            })
            .copied()
            .unwrap_or(mean);
        centers.push(far);
    }
    centers
}

fn nearest(centers: &[[f32; 3]], p: &[f32; 3]) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| dist(a, p).total_cmp(&dist(b, p)))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn dist(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn srgb_to_linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

fn to_oklab([r, g, b]: [u8; 3]) -> [f32; 3] {
    let (r, g, b) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab([l, a, b]: [f32; 3]) -> [u8; 3] {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        linear_to_srgb(4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_),
        linear_to_srgb(-1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_),
        linear_to_srgb(-0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_),
    ]
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::*;

    /// Обложка из двух половин и полоски третьего цвета.
    pub fn sample_cover(side: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(side, side, |x, y| {
            if y < side / 10 {
                Rgb([250, 200, 30])
            } else if x < side / 2 {
                Rgb([200, 30, 60])
            } else {
                Rgb([20, 40, 160])
            }
        });
        let mut out = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_with_encoder(JpegEncoder::new_with_quality(&mut out, 95))
            .unwrap();
        out
    }

    #[test]
    fn oklab_roundtrip() {
        for c in [
            [0, 0, 0],
            [255, 255, 255],
            [200, 30, 60],
            [20, 40, 160],
            [128, 128, 0],
        ] {
            let back = from_oklab(to_oklab(c));
            for i in 0..3 {
                assert!(
                    (i32::from(back[i]) - i32::from(c[i])).abs() <= 1,
                    "{c:?} -> {back:?}"
                );
            }
        }
    }

    #[test]
    fn three_sizes_and_palette() {
        let out = process(&sample_cover(600), true).unwrap();
        let sizes: Vec<u32> = out.variants.iter().map(|(s, _)| *s).collect();
        assert_eq!(sizes, vec![1000, 500, 250]);
        for (size, jpeg) in &out.variants {
            let decoded = image::load_from_memory(jpeg).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (*size, *size));
        }
        assert!((3..=5).contains(&out.palette.len()), "{:?}", out.palette);
        assert!(
            out.palette
                .iter()
                .all(|c| c.len() == 7 && c.starts_with('#'))
        );
        // Самые большие области — красная и синяя половины; жёлтая полоска тоже в палитре
        let rgb: Vec<[u8; 3]> = out
            .palette
            .iter()
            .map(|c| {
                let v = u32::from_str_radix(&c[1..], 16).unwrap();
                [(v >> 16) as u8, (v >> 8) as u8, v as u8]
            })
            .collect();
        let near = |target: [u8; 3]| {
            rgb.iter()
                .any(|c| (0..3).all(|i| (i32::from(c[i]) - i32::from(target[i])).abs() < 40))
        };
        assert!(near([200, 30, 60]), "{:?}", out.palette);
        assert!(near([20, 40, 160]), "{:?}", out.palette);
        assert!(near([250, 200, 30]), "{:?}", out.palette);
        // Одна и та же картинка — одна и та же палитра
        assert_eq!(
            process(&sample_cover(600), true).unwrap().palette,
            out.palette
        );
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(process(b"not an image", true).is_err());
    }
}
