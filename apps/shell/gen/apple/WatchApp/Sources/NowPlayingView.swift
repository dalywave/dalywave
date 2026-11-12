import SwiftUI

/// «Сейчас играет»: обложка, название и исполнитель, ⏮ ⏯ ⏭, «Нравится», громкость колёсиком
/// (если активное устройство её меняет) и «Где играть». Фон — цвет обложки, уходящий в «Полночь».
struct NowPlayingView: View {
    @EnvironmentObject private var store: Store
    @State private var tint: Color = Theme.fill2
    @State private var devicesShown = false
    @State private var crown = 0.0
    @State private var volumeShown = false
    @State private var crownSynced = false

    var body: some View {
        Group {
            if store.offline && store.devices.isEmpty {
                EmptyStateView(symbol: "wifi.slash", title: "No connection")
            } else if store.target == nil && store.track == nil {
                EmptyStateView(symbol: "speaker.slash", title: "No device", caption: "Open DalyWave on your iPhone or computer")
            } else if let track = store.track {
                player(track)
            } else {
                idle
            }
        }
        .sheet(isPresented: $devicesShown) { DevicesView() }
        #if targetEnvironment(simulator)
        .onChange(of: store.simulatorSheet) { _, shown in devicesShown = shown }
        #endif
    }

    // MARK: Играет

    private func player(_ track: Track) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Cover(path: track.cover, size: 42, radius: 7) { color in
                    withAnimation(.easeInOut(duration: 0.8)) { tint = color }
                }
                VStack(alignment: .leading, spacing: 1) {
                    Text(track.title)
                        .font(.system(.headline, design: .rounded))
                        .foregroundStyle(Theme.text1)
                        .lineLimit(1)
                    Text(track.artistLine)
                        .font(.footnote)
                        .foregroundStyle(Theme.text2)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .accessibilityElement(children: .combine)

            ProgressLine(tint: tint)
                .padding(.top, 8)

            Spacer(minLength: 6)

            controls

            Spacer(minLength: 6)

            HStack {
                deviceButton
                Spacer(minLength: 4)
                likeButton
            }
        }
        .padding(.horizontal, 4)
        .padding(.bottom, 2)
        .background(backdrop)
        .overlay(alignment: .trailing) { volumeBar }
        .focusable(store.canChangeVolume)
        .digitalCrownRotation(
            $crown,
            from: 0,
            through: 1,
            by: 0.02,
            sensitivity: .low,
            isContinuous: false,
            isHapticFeedbackEnabled: true
        )
        .onChange(of: crown) { _, value in
            guard crownSynced, store.canChangeVolume else { return }
            store.volumeDraft = value
            volumeShown = true
            store.setVolume(value)
        }
        .onAppear { syncCrown() }
        .onChange(of: store.state?.volume) { _, _ in
            if !volumeShown { syncCrown() }
        }
        .task(id: volumeShown) {
            guard volumeShown else { return }
            try? await Task.sleep(for: .seconds(1.4))
            withAnimation(.easeOut(duration: 0.3)) { volumeShown = false }
            store.volumeDraft = nil
        }
    }

    private func syncCrown() {
        crownSynced = false
        crown = store.state?.volume ?? 1
        DispatchQueue.main.async { crownSynced = true }
    }

    private var backdrop: some View {
        LinearGradient(colors: [tint.opacity(0.55), tint.opacity(0.18), Theme.ink], startPoint: .top, endPoint: .bottom)
            .ignoresSafeArea()
    }

    private var controls: some View {
        HStack(spacing: 6) {
            ControlButton(symbol: "backward.fill", size: 22, label: "Previous") { store.previous() }
            Button {
                store.togglePlay()
            } label: {
                Image(systemName: store.state?.playing == true ? "pause.fill" : "play.fill")
                    .font(.system(size: 26, weight: .semibold))
                    .foregroundStyle(Theme.ink)
                    .frame(width: 58, height: 58)
                    .background(Circle().fill(Theme.text1))
                    .contentTransition(.symbolEffect(.replace))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(store.state?.playing == true ? Text("Pause") : Text("Play"))
            ControlButton(symbol: "forward.fill", size: 22, label: "Next") { store.next() }
        }
    }

    private var deviceButton: some View {
        Button {
            devicesShown = true
        } label: {
            HStack(spacing: 4) {
                Image(systemName: store.target?.symbol ?? "hifispeaker")
                    .font(.system(size: 12, weight: .semibold))
                Text(store.target?.name ?? "")
                    .font(.caption2.weight(.medium))
                    .lineLimit(1)
            }
            .foregroundStyle(Theme.text2)
            .padding(.horizontal, 8)
            .frame(height: 30)
            .background(Capsule().fill(Theme.fill1))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Text("Where to Play"))
    }

    private var likeButton: some View {
        Button {
            store.toggleLike()
        } label: {
            Image(systemName: store.liked ? "heart.fill" : "heart")
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(store.liked ? Theme.accent : Theme.text2)
                .frame(width: 36, height: 30)
                .background(Capsule().fill(Theme.fill1))
                .contentTransition(.symbolEffect(.replace))
        }
        .buttonStyle(.plain)
        .sensoryFeedback(.success, trigger: store.liked)
        .accessibilityLabel(Text("Like"))
        .accessibilityAddTraits(store.liked ? .isSelected : [])
    }

    /// Громкость при повороте колёсика: тонкая полоска у края, как у системного плеера.
    @ViewBuilder private var volumeBar: some View {
        if volumeShown {
            let value = store.volumeDraft ?? store.state?.volume ?? 0
            VStack(spacing: 4) {
                Image(systemName: "speaker.wave.2.fill").font(.system(size: 9)).foregroundStyle(Theme.text2)
                GeometryReader { geo in
                    ZStack(alignment: .bottom) {
                        Capsule().fill(Theme.fill2)
                        Capsule().fill(Theme.text1).frame(height: geo.size.height * value)
                    }
                }
                .frame(width: 4, height: 64)
            }
            .padding(.trailing, 2)
            .transition(.opacity)
            .accessibilityElement()
            .accessibilityLabel(Text("Volume"))
            .accessibilityValue(Text("\(Int(value * 100))%"))
        }
    }

    // MARK: Ничего не играет

    private var idle: some View {
        VStack(spacing: 10) {
            Spacer(minLength: 0)
            WaveButton(size: 74) { store.startWave() }
            Text("Nothing playing")
                .font(.footnote)
                .foregroundStyle(Theme.text3)
            Spacer(minLength: 0)
            HStack {
                deviceButton
                Spacer()
            }
        }
        .padding(.horizontal, 4)
    }
}

/// Тонкая линия прогресса; позиция досчитывается по времени снимка.
struct ProgressLine: View {
    @EnvironmentObject private var store: Store
    let tint: Color

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { _ in
            let duration = max(1, store.state?.durationMs ?? 1)
            let position = store.state?.position(at: store.serverNow) ?? 0
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.fill2)
                    Capsule().fill(Theme.text1.opacity(0.9))
                        .frame(width: geo.size.width * min(1, Double(position) / Double(duration)))
                }
            }
            .frame(height: 3)
        }
        .accessibilityHidden(true)
    }
}

struct ControlButton: View {
    let symbol: String
    var size: CGFloat = 20
    let label: LocalizedStringKey
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size, weight: .semibold))
                .foregroundStyle(Theme.text1)
                .frame(width: 48, height: 48)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Text(label))
    }
}

/// «Моя волна»: круг в цветах бренда — единственный градиент на часах.
struct WaveButton: View {
    var size: CGFloat = 84
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 6) {
                ZStack {
                    Circle().fill(Theme.brand)
                    Image(systemName: "play.fill")
                        .font(.system(size: size * 0.3, weight: .bold))
                        .foregroundStyle(.white)
                        .offset(x: size * 0.03)
                }
                .frame(width: size, height: size)
                Text("My Wave")
                    .font(.system(.headline, design: .rounded))
                    .foregroundStyle(Theme.text1)
            }
        }
        .buttonStyle(.plain)
    }
}

struct EmptyStateView: View {
    let symbol: String
    let title: LocalizedStringKey
    var caption: LocalizedStringKey?

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 30, weight: .light))
                .foregroundStyle(Theme.text3)
            Text(title)
                .font(.headline)
                .foregroundStyle(Theme.text1)
            if let caption {
                Text(caption)
                    .font(.footnote)
                    .foregroundStyle(Theme.text3)
                    .multilineTextAlignment(.center)
            }
        }
        .padding(.horizontal, 8)
    }
}
