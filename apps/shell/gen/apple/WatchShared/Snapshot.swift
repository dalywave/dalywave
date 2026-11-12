import Foundation

// Общее для приложения часов и его виджетов (WatchWidgets): что играет — для осложнения и Smart Stack.
// Приложение пишет снимок в контейнер группы, виджет читает. Секретов здесь нет.

enum WatchShared {
    static let appGroup = "group.rest.daly.wave"
    /// Нажатие на виджет: открыть «Мою волну» и включить её / открыть «Сейчас играет».
    static let waveURL = URL(string: "dalywave-watch://wave")!
    static let nowURL = URL(string: "dalywave-watch://now")!
    static let widgetKind = "rest.daly.wave.watch.now"
    static let waveWidgetKind = "rest.daly.wave.watch.wave"
}

struct NowPlayingSnapshot: Codable, Equatable {
    var title: String
    var artist: String
    var playing: Bool
    /// Обложка 96×96, JPEG
    var cover: Data?
    /// Цвет обложки, #rrggbb
    var tint: String?
    var updatedAt: Date

    private static let key = "nowPlaying"

    static func load() -> NowPlayingSnapshot? {
        guard let data = UserDefaults(suiteName: WatchShared.appGroup)?.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(NowPlayingSnapshot.self, from: data)
    }

    static func save(_ snapshot: NowPlayingSnapshot?) {
        let defaults = UserDefaults(suiteName: WatchShared.appGroup)
        guard let snapshot, let data = try? JSONEncoder().encode(snapshot) else {
            defaults?.removeObject(forKey: key)
            return
        }
        defaults?.set(data, forKey: key)
    }
}
