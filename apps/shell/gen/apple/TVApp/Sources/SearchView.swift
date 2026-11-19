import SwiftUI

/// Поиск: системная клавиатура и диктовка tvOS (`.searchable`), треки, альбомы и артисты каталога.
struct SearchView: View {
    @EnvironmentObject private var store: Store
    @State private var query = ""
    @State private var results: SearchResults?
    @State private var loading = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 64) {
                if let r = results {
                    if r.tracks.isEmpty, r.albums.isEmpty, r.artists.isEmpty {
                        Text("Ничего не нашлось")
                            .font(.system(size: 34, weight: .semibold))
                            .foregroundStyle(Theme.text2)
                            .frame(maxWidth: .infinity)
                            .padding(.top, 80)
                    }
                    if !r.tracks.isEmpty {
                        shelf(String(localized: "Треки")) {
                            ForEach(Array(r.tracks.enumerated()), id: \.element.id) { i, t in
                                Tile(title: t.fullTitle, subtitle: t.artistLine, cover: t.thumb, size: 260) {
                                    store.play(r.tracks, index: i, context: PlayContext(type: "search", id: nil, title: String(localized: "Поиск")))
                                }
                            }
                        }
                    }
                    if !r.albums.isEmpty {
                        shelf(String(localized: "Альбомы")) {
                            ForEach(r.albums) { a in
                                Tile(title: a.title, subtitle: a.artist?.name, cover: a.image?.medium, size: 260) {
                                    store.play(album: a)
                                }
                            }
                        }
                    }
                    if !r.artists.isEmpty {
                        shelf(String(localized: "Артисты")) {
                            ForEach(r.artists) { a in
                                Tile(title: a.name, cover: a.image?.medium, size: 220) { store.play(artist: a) }
                            }
                        }
                    }
                }
            }
            .padding(.vertical, 30)
        }
        .searchable(text: $query, prompt: "Песни, альбомы, артисты")
        .background(Theme.ink.ignoresSafeArea())
        .task(id: query) {
            let q = query.trimmingCharacters(in: .whitespaces)
            guard q.count >= 2, let api = store.api else {
                results = nil
                return
            }
            try? await Task.sleep(for: .milliseconds(350))
            guard !Task.isCancelled else { return }
            loading = true
            defer { loading = false }
            if let r = try? await api.search(q) { results = r }
        }
        #if targetEnvironment(simulator)
        .onAppear { if let q = store.simulatorSearch, query.isEmpty { query = q } }
        #endif
    }

    private func shelf(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 20) {
            Text(title).font(.system(size: 34, weight: .bold)).foregroundStyle(Theme.text1).padding(.horizontal, 90)
            ScrollView(.horizontal) {
                LazyHStack(alignment: .top, spacing: 44) { content() }
                    .padding(.horizontal, 90)
                    .padding(.vertical, 24)
            }
            .scrollClipDisabled()
        }
    }
}
