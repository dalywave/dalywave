import TVServices

/// Верхняя полка Apple TV, когда DalyWave стоит в верхнем ряду: «Моя волна» по настроениям,
/// недавнее и плейлисты. Данные — снимок приложения в группе group.rest.daly.wave.
final class ContentProvider: TVTopShelfContentProvider {
    override func loadTopShelfContent() async -> TVTopShelfContent? {
        var sections: [TVTopShelfItemCollection<TVTopShelfSectionedItem>] = []

        let moods: [(String, String)] = [
            ("energetic", String(localized: "Бодрое")), ("happy", String(localized: "Весёлое")),
            ("calm", String(localized: "Спокойное")), ("sad", String(localized: "Грустное")),
        ]
        let wave = TVTopShelfItemCollection(items: [item(id: "wave", title: String(localized: "Моя волна"), image: bundled("wave"), link: "dalywave-tv://wave")]
            + moods.map { item(id: "wave-\($0.0)", title: $0.1, image: bundled("mood-\($0.0)"), link: "dalywave-tv://wave?mood=\($0.0)") })
        wave.title = String(localized: "Моя волна")
        sections.append(wave)

        if let snap = TopShelfSnapshot.load() {
            for (title, list) in [(String(localized: "Недавнее"), snap.recent), (String(localized: "Плейлисты"), snap.playlists)] where !list.isEmpty {
                let c = TVTopShelfItemCollection(items: list.prefix(12).map { item(id: $0.id, title: $0.title, image: $0.imageURL, link: $0.link) })
                c.title = title
                sections.append(c)
            }
        }
        return TVTopShelfSectionedContent(sections: sections)
    }

    /// Картинка из расширения (Images/): волна и настроения не зависят от сети.
    private func bundled(_ name: String) -> String? {
        Bundle(for: ContentProvider.self).url(forResource: name, withExtension: "png", subdirectory: "Images")?.absoluteString
    }

    private func item(id: String, title: String, image: String?, link: String) -> TVTopShelfSectionedItem {
        let item = TVTopShelfSectionedItem(identifier: id)
        item.title = title
        item.imageShape = .square
        if let image, let url = URL(string: image) {
            item.setImageURL(url, for: .screenScale1x)
            item.setImageURL(url, for: .screenScale2x)
        }
        if let url = URL(string: link) {
            item.displayAction = TVTopShelfAction(url: url)
            item.playAction = TVTopShelfAction(url: url)
        }
        return item
    }
}
