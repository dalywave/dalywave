import SwiftUI

/// «Полночь» (apps/ui/app/assets/css/tokens.css): тёмная основа, нейтральные поверхности, цвет — из
/// обложки или настроения волны. Градиент — только у «Моей волны» (бренд «Сумерки»).
enum Theme {
    static let ink = Color(red: 0.031, green: 0.035, blue: 0.047) // #08090c
    static let fill1 = Color.white.opacity(0.06)
    static let fill2 = Color.white.opacity(0.10)
    static let fill3 = Color.white.opacity(0.16)
    static let text1 = Color(red: 0.953, green: 0.961, blue: 0.976) // #f3f5f9
    static let text2 = Color(red: 0.91, green: 0.925, blue: 0.957).opacity(0.68)
    static let text3 = Color(red: 0.91, green: 0.925, blue: 0.957).opacity(0.56)
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

    var title: LocalizedStringKey {
        switch self {
        case .energetic: "Energetic"
        case .happy: "Happy"
        case .calm: "Calm"
        case .sad: "Sad"
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
