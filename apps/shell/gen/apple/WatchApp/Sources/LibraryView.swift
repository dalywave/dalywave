import SwiftUI

/// Медиатека: «Любимые треки», плейлисты и недавнее. Нажатие включает на активном устройстве.
struct LibraryView: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        NavigationStack {
            List {
                Button {
                    store.playLiked()
                } label: {
                    Row(title: Text("Liked Songs")) {
                        // Единственная плитка с градиентом бренда — как в медиатеке приложения
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .fill(Theme.brand)
                            .overlay(Image(systemName: "heart.fill").font(.system(size: 13, weight: .bold)).foregroundStyle(.white))
                            .frame(width: 32, height: 32)
                    }
                }

                NavigationLink {
                    PlaylistsView()
                } label: {
                    Row(title: Text("Playlists")) {
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .fill(Theme.fill2)
                            .overlay(Image(systemName: "music.note.list").font(.system(size: 13, weight: .semibold)).foregroundStyle(Theme.text2))
                            .frame(width: 32, height: 32)
                    }
                }

                if !store.recent.isEmpty {
                    Section {
                        ForEach(store.recent) { item in
                            Button {
                                store.play(item)
                            } label: {
                                Row(title: Text(item.title), subtitle: item.subtitle) {
                                    Cover(path: item.cover, size: 32, radius: item.type == "artist" ? 16 : 6)
                                }
                            }
                        }
                    } header: {
                        Text("Recent").foregroundStyle(Theme.text3)
                    }
                }
            }
            .listStyle(.plain)
            .navigationTitle(Text("Library"))
            .overlay { if store.busy { ProgressView().tint(Theme.text1) } }
            .task { await store.loadLibrary() }
        }
    }
}

struct PlaylistsView: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        List(store.playlists) { playlist in
            Button {
                store.play(playlist)
            } label: {
                Row(title: Text(playlist.title)) {
                    Cover(path: playlist.cover, size: 32)
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle(Text("Playlists"))
        .overlay { if store.busy { ProgressView().tint(Theme.text1) } }
    }
}

/// Строка списка: картинка слева, подписи справа.
struct Row<Leading: View>: View {
    let title: Text
    var subtitle: String?
    @ViewBuilder let leading: () -> Leading

    var body: some View {
        HStack(spacing: 10) {
            leading()
            VStack(alignment: .leading, spacing: 1) {
                title
                    .font(.body)
                    .foregroundStyle(Theme.text1)
                    .lineLimit(subtitle == nil ? 2 : 1)
                    .minimumScaleFactor(0.85)
                if let subtitle {
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(Theme.text3)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 2)
    }
}

/// «Где играть»: куда идёт звук. Часы — пульт, их в списке нет.
struct DevicesView: View {
    @EnvironmentObject private var store: Store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if store.playableDevices.isEmpty {
                    Text("Open DalyWave on your iPhone or computer")
                        .font(.footnote)
                        .foregroundStyle(Theme.text3)
                        .listRowBackground(Color.clear)
                }
                ForEach(store.playableDevices) { device in
                    Button {
                        store.choose(device)
                        dismiss()
                    } label: {
                        HStack(spacing: 10) {
                            Image(systemName: device.symbol)
                                .font(.system(size: 16, weight: .medium))
                                .foregroundStyle(isOn(device) ? Theme.accent : Theme.text2)
                                .frame(width: 24)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(device.name)
                                    .font(.body)
                                    .foregroundStyle(isOn(device) ? Theme.accent : Theme.text1)
                                    .lineLimit(1)
                                Text(status(device))
                                    .font(.footnote)
                                    .foregroundStyle(Theme.text3)
                            }
                            Spacer(minLength: 0)
                            if isOn(device) {
                                Image(systemName: "checkmark").font(.footnote.weight(.bold)).foregroundStyle(Theme.accent)
                            }
                        }
                        .opacity(device.online ? 1 : 0.55)
                    }
                    .disabled(!device.online)
                }
            }
            .listStyle(.plain)
            .navigationTitle(Text("Where to Play"))
        }
        // Кнопка закрытия — нейтральная, акцент только у выбранного устройства
        .tint(Theme.text2)
    }

    private func isOn(_ device: ConnectDevice) -> Bool { device.id == store.target?.id }

    private func status(_ device: ConnectDevice) -> LocalizedStringKey {
        if device.active { return device.online ? "Playing" : "Offline" }
        return device.online ? "Online" : "Offline"
    }
}
