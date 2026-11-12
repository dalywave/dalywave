import SwiftUI
import UIKit

/// Обложки с сервера (250 px — самый малый размер) и цвет из обложки для фона «Сейчас играет».
actor Artwork {
    static let shared = Artwork()

    struct Art {
        let image: UIImage
        /// Цвет обложки, приглушённый для тёмного фона
        let tint: Color
        let tintHex: String
        /// 96×96 JPEG для виджета
        let thumbnail: Data?
    }

    private var cache: [String: Art] = [:]
    private var order: [String] = []

    func load(_ path: String, credentials: Credentials) async -> Art? {
        if let hit = cache[path] { return hit }
        guard let data = await Api(credentials: credentials).image(path), let image = UIImage(data: data) else { return nil }
        let (r, g, b) = Self.average(image)
        let art = Art(
            image: image,
            tint: Color(red: r, green: g, blue: b),
            tintHex: String(format: "#%02x%02x%02x", Int(r * 255), Int(g * 255), Int(b * 255)),
            thumbnail: Self.thumbnail(image)
        )
        cache[path] = art
        order.append(path)
        if order.count > 40 { cache.removeValue(forKey: order.removeFirst()) }
        return art
    }

    /// Средний цвет обложки, с насыщенностью и яркостью под тёмный фон: не грязно-серый и не кислотный.
    private static func average(_ image: UIImage) -> (Double, Double, Double) {
        guard let cg = image.cgImage else { return (0.29, 0.13, 1) }
        var px = [UInt8](repeating: 0, count: 4)
        let space = CGColorSpaceCreateDeviceRGB()
        guard let ctx = CGContext(data: &px, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return (0.29, 0.13, 1) }
        ctx.interpolationQuality = .medium
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        var h: CGFloat = 0, s: CGFloat = 0, v: CGFloat = 0, a: CGFloat = 0
        UIColor(red: CGFloat(px[0]) / 255, green: CGFloat(px[1]) / 255, blue: CGFloat(px[2]) / 255, alpha: 1)
            .getHue(&h, saturation: &s, brightness: &v, alpha: &a)
        let tuned = UIColor(hue: h, saturation: min(0.85, max(0.35, s * 1.25)), brightness: min(0.85, max(0.55, v * 1.2)), alpha: 1)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0
        tuned.getRed(&r, green: &g, blue: &b, alpha: &a)
        return (Double(r), Double(g), Double(b))
    }

    private static func thumbnail(_ image: UIImage) -> Data? {
        guard let cg = image.cgImage,
              let ctx = CGContext(data: nil, width: 96, height: 96, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        else { return nil }
        ctx.interpolationQuality = .high
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: 96, height: 96))
        return ctx.makeImage().flatMap { UIImage(cgImage: $0).jpegData(compressionQuality: 0.8) }
    }
}

/// Обложка с сервера: пока грузится — плоская подложка.
struct Cover: View {
    @EnvironmentObject private var store: Store
    let path: String?
    var size: CGFloat
    var radius: CGFloat = 6
    var onTint: ((Color) -> Void)?

    @State private var art: Artwork.Art?

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Theme.fill2)
            if let art {
                Image(uiImage: art.image).resizable().scaledToFill()
            } else {
                Image(systemName: "music.note").font(.system(size: size * 0.36)).foregroundStyle(Theme.text3)
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
        .task(id: path) {
            guard let path, let credentials = store.credentials else {
                art = nil
                return
            }
            let loaded = await Artwork.shared.load(path, credentials: credentials)
            art = loaded
            if let loaded { onTint?(loaded.tint) }
        }
        .accessibilityHidden(true)
    }
}
