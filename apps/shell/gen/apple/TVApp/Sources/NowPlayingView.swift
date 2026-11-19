import SwiftUI

/// «Сейчас играет» — сцена для большого экрана: обложка, крупно название и исполнитель, текст песни,
/// следующий трек и, на вечеринке, QR «Добавить трек». Читается с дивана: мало элементов, крупный шрифт.
struct NowPlayingView: View {
    @EnvironmentObject private var store: Store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var tint: Color = Theme.fill1

    var body: some View {
        TimelineView(.periodic(from: .now, by: 0.5)) { ctx in
            if let now = store.shown(at: ctx.date) {
                stage(now)
            } else {
                empty
            }
        }
        .onPlayPauseCommand { store.togglePlay() }
    }

    // MARK: Сцена

    private func stage(_ now: NowShown) -> some View {
        ZStack {
            backdrop(now.track)
            HStack(alignment: .center, spacing: 90) {
                Cover(path: now.track.cover, size: 620, radius: 24) { color in
                    withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.8)) { tint = color }
                }
                .shadow(color: .black.opacity(0.45), radius: 40, y: 20)
                VStack(alignment: .leading, spacing: 0) {
                    if let label = contextLabel(now) {
                        Text(label)
                            .font(.system(size: 26, weight: .semibold))
                            .foregroundStyle(Theme.text2)
                            .padding(.bottom, 18)
                    }
                    Text(now.track.fullTitle)
                        .font(.system(size: 76, weight: .bold))
                        .foregroundStyle(Theme.text1)
                        .lineLimit(2)
                        .minimumScaleFactor(0.7)
                    Text(now.track.artistLine)
                        .font(.system(size: 42, weight: .medium))
                        .foregroundStyle(Theme.text2)
                        .lineLimit(1)
                        .padding(.top, 10)
                    if let guest = store.addedBy(now.track.id) {
                        Label("Добавил(а) \(guest)", systemImage: "person.fill")
                            .font(.system(size: 28, weight: .medium))
                            .foregroundStyle(Theme.text1)
                            .padding(.top, 18)
                    }
                    lyrics(now)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                    progress(now)
                    controls(now).padding(.top, 34)
                    HStack(alignment: .bottom, spacing: 40) {
                        upNext(now)
                        Spacer(minLength: 0)
                        partyCode
                    }
                    .padding(.top, 40)
                }
                .frame(maxWidth: .infinity, maxHeight: 800, alignment: .leading)
            }
            .padding(.horizontal, 110)
            .padding(.top, 40)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func backdrop(_ track: Track) -> some View {
        ZStack {
            Theme.ink
            // Цвет сцены — из обложки: интерфейс нейтральный, цвет приносит музыка
            RadialGradient(colors: [tint.opacity(0.55), tint.opacity(0.12), .clear],
                           center: .topLeading, startRadius: 0, endRadius: 1500)
            LinearGradient(colors: [.clear, Theme.ink.opacity(0.7)], startPoint: .top, endPoint: .bottom)
        }
        .ignoresSafeArea()
    }

    private func contextLabel(_ now: NowShown) -> String? {
        if !now.local, let name = now.deviceName { return String(localized: "Играет на «\(name)»") }
        guard let ctx = now.context, !ctx.title.isEmpty else { return nil }
        return ctx.title
    }

    @ViewBuilder
    private func lyrics(_ now: NowShown) -> some View {
        if let l = store.lyrics, l.kind == "synced", !l.lines.isEmpty, now.local {
            let index = l.lines.lastIndex { $0.tMs <= now.positionMs } ?? -1
            let window = (max(0, index - 1)...min(l.lines.count - 1, max(0, index) + 2))
            VStack(alignment: .leading, spacing: 16) {
                ForEach(Array(window), id: \.self) { i in
                    let line = l.lines[i].text.isEmpty ? "♪" : l.lines[i].text
                    Text(line)
                        .font(.system(size: i == index ? 48 : 38, weight: i == index ? .bold : .semibold))
                        .foregroundStyle(i == index ? Theme.text1 : Theme.text3)
                        .lineLimit(2)
                }
            }
            .animation(reduceMotion ? nil : .easeOut(duration: 0.3), value: index)
            .padding(.vertical, 30)
        } else if let pending = now.pending {
            Text(pending == "waiting_for_downloader" ? String(localized: "Ждём загрузчик…") : String(localized: "Готовим трек…"))
                .font(.system(size: 30))
                .foregroundStyle(Theme.text3)
                .padding(.vertical, 30)
        } else {
            Spacer(minLength: 30)
        }
    }

    private func progress(_ now: NowShown) -> some View {
        let fraction = now.durationMs > 0 ? min(1, Double(now.positionMs) / Double(now.durationMs)) : 0
        return VStack(spacing: 12) {
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.fill3)
                    Capsule().fill(Theme.text1).frame(width: geo.size.width * fraction)
                }
            }
            .frame(height: 8)
            HStack {
                Text(formatTime(now.positionMs))
                Spacer()
                Text(formatTime(now.durationMs))
            }
            .font(.system(size: 24, weight: .medium).monospacedDigit())
            .foregroundStyle(Theme.text3)
        }
    }

    private func controls(_ now: NowShown) -> some View {
        HStack(spacing: 30) {
            Button { store.previous() } label: {
                Image(systemName: "backward.fill").font(.system(size: 36)).frame(width: 90, height: 70)
            }
            Button { store.togglePlay() } label: {
                Image(systemName: now.playing ? "pause.fill" : "play.fill").font(.system(size: 44)).frame(width: 110, height: 80)
            }
            Button { store.next() } label: {
                Image(systemName: "forward.fill").font(.system(size: 36)).frame(width: 90, height: 70)
            }
            if !now.local {
                Button { store.playHere() } label: {
                    Label("Играть здесь", systemImage: "tv").font(.system(size: 28, weight: .semibold))
                        .lineLimit(1).fixedSize().frame(height: 70)
                }
            }
        }
        .buttonStyle(.bordered)
    }

    @ViewBuilder
    private func upNext(_ now: NowShown) -> some View {
        if let next = now.next {
            HStack(spacing: 20) {
                Cover(path: next.thumb, size: 72, radius: 8)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Дальше").font(.system(size: 22, weight: .semibold)).foregroundStyle(Theme.text3)
                    Text("\(next.fullTitle) — \(next.artistLine)")
                        .font(.system(size: 26, weight: .medium))
                        .foregroundStyle(Theme.text2)
                        .lineLimit(1)
                }
            }
        }
    }

    /// Вечеринка: QR «Добавить трек» в углу — гости без аккаунта добавляют треки с телефона.
    @ViewBuilder
    private var partyCode: some View {
        if store.party?.active == true, let url = store.jam?.url {
            HStack(spacing: 22) {
                VStack(alignment: .trailing, spacing: 4) {
                    Text("Добавить трек").font(.system(size: 26, weight: .semibold)).foregroundStyle(Theme.text1)
                    Text("Наведите камеру").font(.system(size: 22)).foregroundStyle(Theme.text3)
                }
                QRCode(value: url, size: 160)
            }
        }
    }

    // MARK: Пусто

    private var empty: some View {
        VStack(spacing: 34) {
            Text("Ничего не играет")
                .font(.system(size: 56, weight: .bold))
                .foregroundStyle(Theme.text1)
            Text("Включите волну — она подстроится под вас")
                .font(.system(size: 30))
                .foregroundStyle(Theme.text2)
            Button { store.startWave() } label: {
                Label("Моя волна", systemImage: "play.fill")
                    .font(.system(size: 32, weight: .semibold))
                    .padding(.horizontal, 30)
            }
            .padding(.top, 10)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.ink.ignoresSafeArea())
    }
}
