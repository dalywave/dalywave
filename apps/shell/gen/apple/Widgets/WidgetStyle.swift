import SwiftUI
import UIKit
import WidgetKit

// Стиль «Полночь» для виджетов: тёмные нейтральные поверхности (в светлой теме — светлые), цвет —
// из обложки, градиент — только у волны (бренд). Шрифт — системный: виджет рисует система.

/// Строки виджетов: английский — в коде, русский и украинский — ru.lproj и uk.lproj/Localizable.strings расширения.
enum L {
    /// Проверка отрисовки подставляет пакет с нужным языком
    static var bundle = Bundle.main

    static func t(_ key: String) -> String {
        NSLocalizedString(key, bundle: bundle, comment: "")
    }
}

/// Картинки из контейнера App Group (проверка отрисовки берёт свою папку).
enum WidgetImages {
    static var directory: URL?

    static func load(_ file: String?) -> UIImage? {
        guard let file, let url = (directory ?? WidgetStore.folder)?.appendingPathComponent(file) else { return nil }
        return UIImage(contentsOfFile: url.path)
    }
}

/// Проверка отрисовки (ImageRenderer): фон контейнера виджета рисуется обычным фоном.
enum WidgetRender {
    static var preview = false
    /// Тонированный режим для снимков: nil — как скажет система
    static var accented: Bool?
}

extension Color {
    init(hex: UInt32, opacity: Double = 1) {
        self.init(
            .sRGB, red: Double((hex >> 16) & 0xff) / 255, green: Double((hex >> 8) & 0xff) / 255,
            blue: Double(hex & 0xff) / 255, opacity: opacity)
    }

    init(_ rgb: WidgetRGB) {
        self.init(.sRGB, red: rgb.r, green: rgb.g, blue: rgb.b, opacity: 1)
    }
}

extension WidgetRGB {
    init(hex: UInt32) {
        self.init(r: Double((hex >> 16) & 0xff) / 255, g: Double((hex >> 8) & 0xff) / 255, b: Double(hex & 0xff) / 255)
    }

    /// Оттенок, насыщенность, яркость — 0…1
    var hsb: (h: Double, s: Double, v: Double) {
        let hi = max(r, g, b), lo = min(r, g, b), d = hi - lo
        var h = 0.0
        if d > 0 {
            if hi == r { h = (g - b) / d } else if hi == g { h = 2 + (b - r) / d } else { h = 4 + (r - g) / d }
            h = (h < 0 ? h + 6 : h) / 6
        }
        return (h, hi == 0 ? 0 : d / hi, hi)
    }

    func mixed(with other: WidgetRGB, _ t: Double) -> WidgetRGB {
        WidgetRGB(r: r + (other.r - r) * t, g: g + (other.g - g) * t, b: b + (other.b - b) * t)
    }
}

/// Цвета одного виджета.
struct Palette {
    var background: Color
    var primary: Color
    var secondary: Color
    /// Главная кнопка (играть) и знак на ней
    var button: Color
    var onButton: Color
    /// Волоски-разделители, заглушки
    var hair: Color
    var fill: Color

    /// Нейтральная поверхность «Полночи»: --ink-2 в тёмной теме, светлая — в светлой.
    static func neutral(_ scheme: ColorScheme) -> Palette {
        scheme == .dark
            ? Palette(
                background: Color(hex: 0x0e1015), primary: Color(hex: 0xf3f5f9),
                secondary: Color(hex: 0xf3f5f9, opacity: 0.62), button: Color(hex: 0xf3f5f9),
                onButton: Color(hex: 0x0e1015), hair: Color.white.opacity(0.09), fill: Color.white.opacity(0.075))
            : Palette(
                background: Color(hex: 0xf4f5f7), primary: Color(hex: 0x111318),
                secondary: Color(hex: 0x111318, opacity: 0.58), button: Color(hex: 0x111318),
                onButton: Color(hex: 0xf4f5f7), hair: Color.black.opacity(0.08), fill: Color.black.opacity(0.05))
    }

    /// Фон из цвета обложки: глубокий тон в тёмной теме, бледный — в светлой. Серая обложка —
    /// нейтральный фон.
    static func cover(_ rgb: WidgetRGB?, _ scheme: ColorScheme) -> Palette {
        guard let rgb else { return neutral(scheme) }
        let (h, s, _) = rgb.hsb
        if scheme == .dark {
            let bg = Color(hue: h, saturation: min(s, 0.78) * 0.92, brightness: s < 0.1 ? 0.16 : 0.27)
            return Palette(
                background: bg, primary: .white, secondary: Color.white.opacity(0.66), button: .white,
                onButton: bg, hair: Color.white.opacity(0.14), fill: Color.white.opacity(0.14))
        }
        let bg = Color(hue: h, saturation: min(s * 0.32, 0.26), brightness: 0.96)
        let ink = Color(hue: h, saturation: min(s, 0.6) * 0.5, brightness: 0.14)
        return Palette(
            background: bg, primary: ink, secondary: ink.opacity(0.64), button: ink, onButton: bg,
            hair: Color.black.opacity(0.08), fill: Color.black.opacity(0.06))
    }
}

/// Бренд «Сумерки»: спектр волны, 135°.
enum Brand {
    static let gradient = LinearGradient(
        stops: [
            .init(color: Color(hex: 0x4b21ff), location: 0),
            .init(color: Color(hex: 0xb01cf0), location: 0.40),
            .init(color: Color(hex: 0xff3d7a), location: 0.74),
            .init(color: Color(hex: 0xff9a3c), location: 1),
        ],
        startPoint: .topLeading, endPoint: .bottomTrailing)

    /// Цвета настроения (глубина, тело, свет) — как MOOD_ORB интерфейса
    static func mood(_ mood: String?) -> (WidgetRGB, WidgetRGB) {
        switch mood {
        case "energetic": return (WidgetRGB(hex: 0x7a2208), WidgetRGB(hex: 0xff6a1a))
        case "happy": return (WidgetRGB(hex: 0x5c0f5e), WidgetRGB(hex: 0xff4fa8))
        case "calm": return (WidgetRGB(hex: 0x0a2a52), WidgetRGB(hex: 0x1d8cbc))
        case "sad": return (WidgetRGB(hex: 0x1c1946), WidgetRGB(hex: 0x5b50c4))
        default: return (WidgetRGB(hex: 0x4b21ff), WidgetRGB(hex: 0xff3d7a))
        }
    }

    /// Плитка сочетания: тело настроения, приглушённое глубиной (как в «Сохранённых сочетаниях»).
    /// Без настроения — цвет по занятию, чтобы соседние плитки не сливались.
    static func swatch(_ preset: WidgetPreset) -> Color {
        let byActivity = ["work": "calm", "road": "energetic", "sleep": "sad", "sport": "energetic", "party": "happy", "wake": "happy", "rest": "calm"]
        let mood = preset.mood.flatMap { $0 == "any" ? nil : $0 } ?? preset.activity.flatMap { byActivity[$0] }
        let (deep, body) = Brand.mood(mood)
        return Color(body.mixed(with: deep, 0.45))
    }
}

/// Знак DalyWave «Волна‑W» (design/brand/glyph.svg): штрих 11,5 из 100, концы скруглены.
struct WaveGlyphShape: Shape {
    func path(in rect: CGRect) -> Path {
        let s = min(rect.width, rect.height) / 100
        let ox = rect.midX - 50 * s, oy = rect.midY - 50 * s
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: ox + x * s, y: oy + y * s) }
        var path = Path()
        path.move(to: p(16, 31))
        path.addCurve(to: p(33, 69), control1: p(18.83, 37.33), control2: p(27.33, 67.33))
        path.addCurve(to: p(50, 41), control1: p(38.67, 70.67), control2: p(44.33, 41))
        path.addCurve(to: p(67, 69), control1: p(55.67, 41), control2: p(61.33, 70.67))
        path.addCurve(to: p(84, 31), control1: p(72.67, 67.33), control2: p(81.17, 37.33))
        return path
    }
}

/// Тот же знак, вписанный по краям штриха: видимая часть 79,5 × 49,5 из 100.
struct TightGlyphShape: Shape {
    /// Толщина штриха к ширине рамки
    static let stroke: CGFloat = 11.5 / 79.5
    static let aspect: CGFloat = 49.5 / 79.5

    func path(in rect: CGRect) -> Path {
        let s = rect.width / 79.5
        let full = CGRect(x: rect.minX - 10.25 * s, y: rect.minY - 25.25 * s, width: 100 * s, height: 100 * s)
        return WaveGlyphShape().path(in: full)
    }
}

struct WaveGlyph: View {
    var size: CGFloat

    var body: some View {
        WaveGlyphShape()
            .stroke(style: StrokeStyle(lineWidth: size * 0.115, lineCap: .round, lineJoin: .round))
            .frame(width: size, height: size)
    }
}

/// Иконки сочетаний: имена Phosphor с сервера → SF Symbols.
enum PresetIcon {
    static func symbol(_ phosphor: String) -> String {
        let map: [String: String] = [
            "laptop": "laptopcomputer", "car": "car.fill", "moon-stars": "moon.stars.fill",
            "coffee": "cup.and.saucer.fill", "barbell": "dumbbell.fill", "sun-horizon": "sunrise.fill",
            "confetti": "party.popper.fill", "headphones": "headphones", "waves": "water.waves",
            "lightning": "bolt.fill", "cloud-rain": "cloud.rain.fill", "heart": "heart.fill",
            "book-open": "book.fill", "sparkle": "sparkle",
        ]
        let name = map[phosphor] ?? "sparkle"
        return UIImage(systemName: name) == nil ? "sparkle" : name
    }

    /// Пока своих сочетаний нет — три занятия, как на главной
    static var defaults: [WidgetPreset] {
        [
            WidgetPreset(id: "work", name: L.t("Work"), icon: "laptop", mood: nil, activity: "work", character: nil, language: nil),
            WidgetPreset(id: "road", name: L.t("Road"), icon: "car", mood: nil, activity: "road", character: nil, language: nil),
            WidgetPreset(id: "night", name: L.t("Night"), icon: "moon-stars", mood: nil, activity: "sleep", character: nil, language: nil),
        ]
    }
}

extension View {
    /// Фон виджета: containerBackground (iOS 17+) — его система убирает в StandBy и тонированных
    /// значках; раньше — обычный фон.
    @ViewBuilder
    func widgetSurface<Background: View>(_ background: Background) -> some View {
        if #available(iOS 17.0, *), !WidgetRender.preview {
            containerBackground(for: .widget) { background }
        } else if WidgetRender.accented == true {
            // Тонированные значки: фон убирает система
            self
        } else {
            self.background(background)
        }
    }

    @ViewBuilder
    func accentable() -> some View {
        if #available(iOS 16.0, *) {
            self.widgetAccentable()
        } else {
            self
        }
    }
}

extension Image {
    /// Обложка в тонированном режиме значков (iOS 18+): обесцветить и подкрасить, а не залить.
    @ViewBuilder
    func artworkRendering() -> some View {
        if #available(iOS 18.0, *) {
            widgetAccentedRenderingMode(.accentedDesaturated)
        } else {
            self
        }
    }
}

/// Тонированные и «прозрачные» значки (iOS 18+): фон убирает система, цвет — один оттенок.
/// Проверка отрисовки задаёт режим сама.
struct AccentedReader<Content: View>: View {
    @ViewBuilder var content: (Bool) -> Content

    var body: some View {
        if let forced = WidgetRender.accented {
            content(forced)
        } else if #available(iOS 16.0, *) {
            RenderingModeReader(content: content)
        } else {
            content(false)
        }
    }
}

@available(iOS 16.0, *)
private struct RenderingModeReader<Content: View>: View {
    @Environment(\.widgetRenderingMode) private var mode
    var content: (Bool) -> Content

    var body: some View {
        content(mode == .accented)
    }
}
