import CoreImage.CIFilterBuiltins
import SwiftUI
import UIKit

/// «Полночь» (apps/ui/app/assets/css/tokens.css): тёмная основа, нейтральные поверхности, цвет — из
/// обложки. Градиент — только у «Моей волны» (бренд «Сумерки»).
enum Theme {
    static let ink = Color(red: 0.031, green: 0.035, blue: 0.047) // #08090c
    static let fill1 = Color.white.opacity(0.06)
    static let fill2 = Color.white.opacity(0.10)
    static let fill3 = Color.white.opacity(0.16)
    static let text1 = Color(red: 0.953, green: 0.961, blue: 0.976) // #f3f5f9
    static let text2 = Color(red: 0.91, green: 0.925, blue: 0.957).opacity(0.68)
    static let text3 = Color(red: 0.91, green: 0.925, blue: 0.957).opacity(0.5)
    static let accent = Color(red: 1, green: 0.494, blue: 0.702) // #ff7eb3

    /// Бренд «Сумерки»: спектр настроений волны
    static let brand = LinearGradient(
        colors: [
            Color(red: 0.294, green: 0.129, blue: 1), // #4b21ff
            Color(red: 0.690, green: 0.110, blue: 0.941), // #b01cf0
            Color(red: 1, green: 0.239, blue: 0.478), // #ff3d7a
            Color(red: 1, green: 0.604, blue: 0.235), // #ff9a3c
        ],
        startPoint: .topLeading,
        endPoint: .bottomTrailing
    )
}

/// Настроения волны — те же значения и цвета, что в интерфейсе (`WaveMood`, --mood-*-2).
enum Mood: String, CaseIterable, Identifiable {
    case energetic, happy, calm, sad

    var id: String { rawValue }

    var title: String {
        switch self {
        case .energetic: String(localized: "Бодрое")
        case .happy: String(localized: "Весёлое")
        case .calm: String(localized: "Спокойное")
        case .sad: String(localized: "Грустное")
        }
    }

    var color: Color {
        switch self {
        case .energetic: Color(red: 1, green: 0.416, blue: 0.102) // #ff6a1a
        case .happy: Color(red: 1, green: 0.310, blue: 0.659) // #ff4fa8
        case .calm: Color(red: 0.114, green: 0.549, blue: 0.737) // #1d8cbc
        case .sad: Color(red: 0.357, green: 0.314, blue: 0.769) // #5b50c4
        }
    }

    var symbol: String {
        switch self {
        case .energetic: "bolt.fill"
        case .happy: "sun.max.fill"
        case .calm: "leaf.fill"
        case .sad: "cloud.rain.fill"
        }
    }
}

/// Значки сочетаний волны (имена Phosphor из интерфейса) — ближайшие SF Symbols.
func presetSymbol(_ icon: String?) -> String {
    switch icon {
    case "laptop": "laptopcomputer"
    case "car": "car.fill"
    case "moon-stars": "moon.stars.fill"
    case "coffee": "cup.and.saucer.fill"
    case "barbell": "dumbbell.fill"
    case "sun-horizon": "sunrise.fill"
    case "confetti": "party.popper.fill"
    case "headphones": "headphones"
    case "waves": "water.waves"
    case "lightning": "bolt.fill"
    case "cloud-rain": "cloud.rain.fill"
    case "heart": "heart.fill"
    case "book-open": "book.fill"
    default: "sparkle"
    }
}

func formatTime(_ ms: Int) -> String {
    let s = max(0, ms / 1000)
    return String(format: "%d:%02d", s / 60, s % 60)
}

// MARK: Обложки

/// Обложки с сервера и цвет обложки для фона сцены.
actor Artwork {
    static let shared = Artwork()

    struct Art {
        let image: UIImage
        /// Цвет обложки, приглушённый для тёмного фона
        let tint: Color
    }

    private var cache: [String: Art] = [:]
    private var order: [String] = []

    func load(_ path: String, api: Api) async -> Art? {
        if let hit = cache[path] { return hit }
        guard let data = await api.image(path), let image = UIImage(data: data) else { return nil }
        let (r, g, b) = Self.average(image)
        let art = Art(image: image, tint: Color(red: r, green: g, blue: b))
        cache[path] = art
        order.append(path)
        if order.count > 120 { cache.removeValue(forKey: order.removeFirst()) }
        return art
    }

    /// Средний цвет обложки под тёмный фон: не грязно-серый и не кислотный.
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
        let tuned = UIColor(hue: h, saturation: min(0.8, max(0.3, s * 1.2)), brightness: min(0.7, max(0.45, v)), alpha: 1)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0
        tuned.getRed(&r, green: &g, blue: &b, alpha: &a)
        return (Double(r), Double(g), Double(b))
    }
}

/// Обложка с сервера: пока грузится — плоская подложка.
struct Cover: View {
    @EnvironmentObject private var store: Store
    let path: String?
    var size: CGFloat
    var radius: CGFloat = 12
    var onTint: ((Color) -> Void)?

    @State private var art: Artwork.Art?

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Theme.fill2)
            if let art {
                Image(uiImage: art.image).resizable().scaledToFill()
            } else {
                Image(systemName: "music.note").font(.system(size: size * 0.3)).foregroundStyle(Theme.text3)
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
        .task(id: path) {
            guard let path, let api = store.api else {
                art = nil
                return
            }
            let loaded = await Artwork.shared.load(path, api: api)
            withAnimation(.easeOut(duration: 0.25)) { art = loaded }
            if let loaded { onTint?(loaded.tint) }
        }
        .accessibilityHidden(true)
    }
}

// MARK: QR

/// QR-код ссылки: тёмные модули на светлом поле — так его читает любая камера.
struct QRCode: View {
    let value: String
    var size: CGFloat

    var body: some View {
        Group {
            if let image = QRCode.make(value) {
                Image(uiImage: image).interpolation(.none).resizable().scaledToFit()
            } else {
                Color.white
            }
        }
        .padding(size * 0.07)
        .frame(width: size, height: size)
        .background(Color.white, in: RoundedRectangle(cornerRadius: size * 0.08, style: .continuous))
        .accessibilityLabel(Text("QR-код"))
    }

    static func make(_ value: String) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(value.utf8)
        filter.correctionLevel = "M"
        guard let out = filter.outputImage,
              let cg = CIContext().createCGImage(out, from: out.extent)
        else { return nil }
        return UIImage(cgImage: cg)
    }
}

// MARK: Карточки

/// Плитка с обложкой и подписью: фокус-движок tvOS поднимает её карточкой.
struct Tile: View {
    let title: String
    var subtitle: String?
    let cover: String?
    var size: CGFloat = 300
    let action: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Button(action: action) {
                Cover(path: cover, size: size, radius: 14)
            }
            .buttonStyle(.card)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.system(size: 26, weight: .semibold)).foregroundStyle(Theme.text1).lineLimit(1)
                if let subtitle {
                    Text(subtitle).font(.system(size: 22)).foregroundStyle(Theme.text2).lineLimit(1)
                }
            }
            .frame(width: size, alignment: .leading)
        }
    }
}
