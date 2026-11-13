import SwiftUI

/// Медиатека: «Любимые», недавнее и плейлисты — полками, как в системных приложениях tvOS.
struct LibraryView: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 64) {
                shelf("Любимые") {
                    Button { store.playLiked() } label: {
                        VStack(alignment: .leading, spacing: 10) {
                            Image(systemName: "heart.fill").font(.system(size: 54)).foregroundStyle(Theme.accent)
                            Spacer()
                            Text("Любимые треки").font(.system(size: 32, weight: .bold)).foregroundStyle(Theme.text1)
                            Text(countLabel(store.liked.count)).font(.system(size: 24)).foregroundStyle(Theme.text2)
                        }
                        .padding(34)
                        .frame(width: 300, height: 300, alignment: .leading)
                        .background(Theme.fill1)
                    }
                    .buttonStyle(.card)
                    ForEach(Array(store.liked.prefix(12).enumerated()), id: \.element.id) { i, track in
                        Tile(title: track.fullTitle, subtitle: track.artistLine, cover: track.thumb) {
                            store.playLiked(from: i)
                        }
                    }
                }
                if !store.recent.isEmpty {
                    shelf("Недавнее") {
                        ForEach(store.recent) { item in
                            Tile(title: item.title, subtitle: item.subtitle, cover: item.cover) { store.play(item) }
                        }
                    }
                }
                if !store.playlists.isEmpty {
                    shelf("Плейлисты") {
                        ForEach(store.playlists) { p in
                            Tile(title: p.title, subtitle: p.tracksCount.map(countLabel), cover: p.cover) { store.play(p) }
                        }
                    }
                }
                account
            }
            .padding(.vertical, 40)
        }
        .background(Theme.ink.ignoresSafeArea())
        .task { await store.loadLibrary() }
    }

    private func shelf(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 20) {
            Text(title).font(.system(size: 36, weight: .bold)).foregroundStyle(Theme.text1).padding(.horizontal, 90)
            ScrollView(.horizontal) {
                LazyHStack(alignment: .top, spacing: 48) { content() }
                    .padding(.horizontal, 90)
                    .padding(.vertical, 24)
            }
            .scrollClipDisabled()
        }
    }

    private var account: some View {
        HStack(spacing: 30) {
            if let name = store.credentials?.userName {
                Text(name).font(.system(size: 28)).foregroundStyle(Theme.text2)
            }
            Button("Выйти") { store.signOut() }
        }
        .padding(.horizontal, 90)
        .padding(.top, 20)
    }
}

/// «12 треков»
func countLabel(_ n: Int) -> String {
    let mod10 = n % 10, mod100 = n % 100
    let word = if mod10 == 1, mod100 != 11 { "трек" }
        else if (2...4).contains(mod10), !(12...14).contains(mod100) { "трека" }
        else { "треков" }
    return "\(n) \(word)"
}
