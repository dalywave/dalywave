import Foundation

// Общие данные виджетов (WidgetKit): приложение пишет снимок в контейнер App Group, расширение
// dalywave-widgets только читает. Файл собирается в обоих target — формат один.
//
// Контейнер `group.rest.daly.wave/Widgets`:
//   now.json      — что играет (WidgetNowPlaying) и файл обложки now-<хеш>.jpg рядом;
//   library.json  — недавнее и сохранённые сочетания волны (WidgetLibrary), обложки recent-<…>.jpg.

enum WidgetKind {
    static let nowPlaying = "rest.daly.wave.now-playing"
    static let wave = "rest.daly.wave.wave"
    static let recent = "rest.daly.wave.recent"
}

/// Цвет обложки, 0…1.
struct WidgetRGB: Codable, Equatable {
    var r: Double
    var g: Double
    var b: Double
}

/// Что играет. `playing` — играет сейчас или на паузе; последний трек остаётся и после остановки.
struct WidgetNowPlaying: Codable, Equatable {
    var title: String
    var artist: String
    var playing: Bool
    /// Файл уменьшенной обложки в контейнере
    var artwork: String?
    /// Откуда обложка: та же ссылка — файл не перекачивается
    var artworkURL: String?
    var color: WidgetRGB?
    var updatedAt: Date
}

/// Альбом, плейлист или артист из «Недавнего».
struct WidgetRecentItem: Codable, Equatable, Identifiable {
    /// `album`, `playlist`, `artist`
    var kind: String
    var id: String
    var title: String
    var subtitle: String
    var artwork: String?
    var color: WidgetRGB?
    /// Плейлист без обложки: микс — собранная обложка, свой — заглушка с нотой
    var mix: Bool?

    /// Экран в приложении: dalywave://album/<id>
    var link: URL? { URL(string: "dalywave://\(kind)/\(id)") }
}

/// Сохранённое сочетание волны (как на главной: «Работа», «Дорога», «Ночь»).
struct WidgetPreset: Codable, Equatable, Identifiable {
    var id: String
    var name: String
    /// Имя иконки Phosphor без префикса: `laptop`, `moon-stars`
    var icon: String
    var mood: String?
    var activity: String?
    var character: String?
    var language: String?
}

struct WidgetLibrary: Codable, Equatable {
    var recent: [WidgetRecentItem]
    var presets: [WidgetPreset]
    var updatedAt: Date
}

enum WidgetStore {
    static let group = "group.rest.daly.wave"
    static let nowFile = "now.json"
    static let libraryFile = "library.json"

    /// Папка виджетов в общем контейнере; nil — нет App Group (подпись без группы)
    static var folder: URL? {
        guard let base = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
        else { return nil }
        let dir = base.appendingPathComponent("Widgets", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    static func url(_ name: String) -> URL? {
        folder?.appendingPathComponent(name)
    }

    static func read<T: Decodable>(_ type: T.Type, _ name: String) -> T? {
        guard let url = url(name), let data = try? Data(contentsOf: url) else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        return try? decoder.decode(type, from: data)
    }

    static func write<T: Encodable>(_ value: T, _ name: String) {
        guard let url = url(name) else { return }
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .secondsSince1970
        guard let data = try? encoder.encode(value) else { return }
        try? data.write(to: url, options: .atomic)
    }

    static func remove(_ name: String) {
        guard let url = url(name) else { return }
        try? FileManager.default.removeItem(at: url)
    }

    static var nowPlaying: WidgetNowPlaying? { read(WidgetNowPlaying.self, nowFile) }
    static var library: WidgetLibrary? { read(WidgetLibrary.self, libraryFile) }
}
