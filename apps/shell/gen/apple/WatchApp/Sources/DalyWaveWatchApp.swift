import SwiftUI

// DalyWave для Apple Watch: пульт «Где играть». Звук играет на iPhone, компьютере или в браузере,
// часы показывают, что играет, и управляют: ⏮ ⏯ ⏭, громкость колёсиком, «Нравится», «Моя волна»
// с настроением, медиатека. Вход — ключ от iPhone (PhoneLink), своего входа нет.

@main
struct DalyWaveWatchApp: App {
    @StateObject private var store = Store()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(store)
                .onOpenURL { url in
                    // Нажатие на осложнение или виджет Smart Stack
                    switch url {
                    case WatchShared.waveURL:
                        store.openWave()
                    default:
                        store.page = 0
                    }
                }
        }
        .onChange(of: scenePhase) { _, phase in
            // Связь — только пока приложение на экране: опущенная рука её закрывает
            if phase == .active { store.becameActive() } else { store.becameInactive() }
        }
    }
}

struct RootView: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        Group {
            switch store.phase {
            case .starting:
                ProgressView().tint(Theme.text2)
            case let .needsPhone(problem):
                PhoneNeededView(problem: problem)
            case .ready:
                TabView(selection: $store.page) {
                    NowPlayingView().tag(0)
                    WaveView().tag(1)
                    LibraryView().tag(2)
                }
                .tabViewStyle(.page(indexDisplayMode: .automatic))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.ink.ignoresSafeArea())
        .overlay(alignment: .top) { NoticeView() }
        .tint(Theme.accent)
    }
}

/// Короткая подсказка после неудачной команды — сама исчезает.
struct NoticeView: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        if let text = store.notice {
            Text(text)
                .font(.footnote.weight(.medium))
                .foregroundStyle(Theme.text1)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(Capsule().fill(Theme.fill3))
                .transition(.move(edge: .top).combined(with: .opacity))
                .task(id: text) {
                    try? await Task.sleep(for: .seconds(2.2))
                    withAnimation { store.notice = nil }
                }
        }
    }
}

/// Нет ключа: часы ждут iPhone.
struct PhoneNeededView: View {
    @EnvironmentObject private var store: Store
    let problem: Store.PhoneProblem

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: problem == .offline ? "wifi.slash" : "iphone")
                .font(.system(size: 34, weight: .light))
                .foregroundStyle(Theme.text3)
            Text(title)
                .font(.headline)
                .multilineTextAlignment(.center)
                .foregroundStyle(Theme.text1)
            Button("Try Again") { store.requestKey() }
                .buttonStyle(.bordered)
                .padding(.top, 4)
        }
        .padding(.horizontal, 8)
    }

    private var title: LocalizedStringKey {
        switch problem {
        case .unreachable: "Open DalyWave on your iPhone"
        case .signedOut: "Sign in to DalyWave on your iPhone"
        case .offline: "No connection"
        }
    }
}
