//! float16 без зависимостей: векторы треков хранятся в базе как `halfvec` и так же едут в пакете обучения.

/// float32 → биты float16 с округлением к ближайшему чётному.
pub fn to_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let man = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        // Субнормальные числа half
        if e < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let mut r = m >> shift;
        let rem = m & ((1 << shift) - 1);
        if rem > half || (rem == half && r & 1 == 1) {
            r += 1;
        }
        return sign | r as u16;
    }
    let mut r = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1fff;
    if rem > 0x1000 || (rem == 0x1000 && r & 1 == 1) {
        r += 1;
    }
    sign | r as u16
}

/// Биты float16 → float32.
pub fn from_bits(h: u16) -> f32 {
    let sign = u32::from(h & 0x8000) << 16;
    let exp = u32::from((h >> 10) & 0x1f);
    let man = u32::from(h & 0x3ff);
    let bits = match exp {
        0 if man == 0 => sign,
        0 => {
            // Субнормальное: нормализуем
            let mut e = 127 - 15 + 1;
            let mut m = man;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | ((e as u32) << 23) | ((m & 0x3ff) << 13)
        }
        0x1f => sign | 0x7f80_0000 | (man << 13),
        _ => sign | ((exp + 127 - 15) << 23) | (man << 13),
    };
    f32::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for x in [
            0.0f32, 1.0, -1.0, 0.5, 0.1, -0.333, 1e-5, 6.1e-5, 65504.0, 0.0312,
        ] {
            let back = from_bits(to_bits(x));
            assert!((back - x).abs() <= x.abs() * 1e-3 + 1e-7, "{x} → {back}");
        }
        assert_eq!(to_bits(1.0), 0x3c00);
        assert_eq!(to_bits(-2.0), 0xc000);
        assert_eq!(to_bits(1e9), 0x7c00);
        assert!(from_bits(0x7c00).is_infinite());
    }
}
