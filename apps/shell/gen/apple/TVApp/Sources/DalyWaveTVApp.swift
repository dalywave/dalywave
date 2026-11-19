import SwiftUI

@main
struct DalyWaveTVApp: App {
    @StateObject private var store = Store()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            Group {
                if store.credentials == nil {
                    // Сервер — с iPhone (iCloud), из этой сети, вручную или демо; затем вход кодом
                    ConnectView()
                } else {
                    MainView()
                }
            }
            .environmentObject(store)
            .preferredColorScheme(.dark)
            .onOpenURL { store.open($0) }
            .task(id: store.credentials?.token) {
                if store.credentials != nil { store.start() }
            }
            .onChange(of: phase) { _, now in
                if now == .background { store.player.flushEvents() }
            }
        }
    }
}

/// Разделы сверху, как у системных приложений tvOS.
struct MainView: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        TabView(selection: $store.tab) {
            NowPlayingView().tag(Tab.now).tabItem { Text(Tab.now.title) }
            WaveView().tag(Tab.wave).tabItem { Text(Tab.wave.title) }
            LibraryView().tag(Tab.library).tabItem { Text(Tab.library.title) }
            SearchView().tag(Tab.search).tabItem { Text(Tab.search.title) }
            PartyView().tag(Tab.party).tabItem { Text(Tab.party.title) }
        }
        .background(Theme.ink.ignoresSafeArea())
        .overlay(alignment: .bottom) {
            if let notice = store.notice {
                Text(notice)
                    .font(.system(size: 28, weight: .medium))
                    .foregroundStyle(Theme.text1)
                    .padding(.horizontal, 36)
                    .padding(.vertical, 18)
                    .background(.regularMaterial, in: Capsule())
                    .padding(.bottom, 60)
                    .transition(.opacity)
                    .task(id: notice) {
                        try? await Task.sleep(for: .seconds(3))
                        store.notice = nil
                    }
            }
        }
    }
}
