import CoreGraphics
import Foundation

/// Цвет обложки для фона виджета (приложение считает его, когда сохраняет обложку).
enum WidgetColor {
    /// Главный цвет обложки: самый частый заметный оттенок (по 12 секторам цветового круга);
    /// серую обложку — средним цветом.
    static func dominant(_ image: CGImage) -> WidgetRGB? {
        let n = 24
        var pixels = [UInt8](repeating: 0, count: n * n * 4)
        let drawn: Bool = pixels.withUnsafeMutableBytes { buffer in
            guard let ctx = CGContext(
                data: buffer.baseAddress, width: n, height: n, bitsPerComponent: 8, bytesPerRow: n * 4,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
            else { return false }
            ctx.interpolationQuality = .medium
            ctx.draw(image, in: CGRect(x: 0, y: 0, width: n, height: n))
            return true
        }
        guard drawn else { return nil }
        var buckets = [(w: Double, r: Double, g: Double, b: Double)](repeating: (0, 0, 0, 0), count: 12)
        var all = (r: 0.0, g: 0.0, b: 0.0)
        for i in stride(from: 0, to: pixels.count, by: 4) {
            let r = Double(pixels[i]) / 255, g = Double(pixels[i + 1]) / 255, b = Double(pixels[i + 2]) / 255
            all.r += r; all.g += g; all.b += b
            let hi = max(r, g, b), lo = min(r, g, b)
            let saturation = hi == 0 ? 0 : (hi - lo) / hi
            guard saturation > 0.22, hi > 0.18 else { continue }
            var hue: Double
            if hi == r { hue = (g - b) / (hi - lo) } else if hi == g { hue = 2 + (b - r) / (hi - lo) } else { hue = 4 + (r - g) / (hi - lo) }
            hue = (hue < 0 ? hue + 6 : hue) / 6
            let k = min(11, Int(hue * 12))
            let w = saturation * (0.4 + hi)
            buckets[k].w += w
            buckets[k].r += r * w; buckets[k].g += g * w; buckets[k].b += b * w
        }
        let count = Double(n * n)
        if let best = buckets.max(by: { $0.w < $1.w }), best.w > count * 0.04 {
            return WidgetRGB(r: best.r / best.w, g: best.g / best.w, b: best.b / best.w)
        }
        return WidgetRGB(r: all.r / count, g: all.g / count, b: all.b / count)
    }

    static func hex(_ hex: String) -> WidgetRGB? {
        let s = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        guard s.count == 6, let v = UInt32(s, radix: 16) else { return nil }
        return WidgetRGB(
            r: Double((v >> 16) & 0xff) / 255, g: Double((v >> 8) & 0xff) / 255, b: Double(v & 0xff) / 255)
    }
}
