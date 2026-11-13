import Foundation

/// Снимок для Top Shelf (верхняя полка Apple TV): недавнее и плейлисты с адресами обложек. Пишет
/// приложение, читает расширение dalywave-tv-topshelf — в сеть оно не ходит, обложки открыты всем.
struct TopShelfSnapshot: Codable {
    struct Item: Codable {
        var id: String
        var title: String
        var imageURL: String?
        /// `dalywave-tv://play?type=…&id=…`
        var link: String
    }

    var recent: [Item]
    var playlists: [Item]

    static let group = "group.rest.daly.wave"
    static let key = "tv.topshelf"

    static func load() -> TopShelfSnapshot? {
        guard let data = UserDefaults(suiteName: group)?.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(TopShelfSnapshot.self, from: data)
    }

    func save() {
        guard let data = try? JSONEncoder().encode(self) else { return }
        UserDefaults(suiteName: Self.group)?.set(data, forKey: Self.key)
    }

    static func link(type: String, id: String, title: String) -> String {
        var c = URLComponents()
        c.scheme = "dalywave-tv"
        c.host = "play"
        c.queryItems = [.init(name: "type", value: type), .init(name: "id", value: id), .init(name: "title", value: title)]
        return c.string ?? "dalywave-tv://now"
    }
}
