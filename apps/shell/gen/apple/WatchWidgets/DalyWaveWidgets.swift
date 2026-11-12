import SwiftUI
import WidgetKit

// Осложнения и виджеты Smart Stack часов: кнопка «Моя волна» и мини «Сейчас играет». Данные —
// снимок, который пишет приложение часов (WatchShared/Snapshot.swift); своей сети у виджета нет.
// Нажатие открывает приложение: «Моя волна» включает волну, «Сейчас играет» — экран плеера.

@main
struct DalyWaveWidgets: WidgetBundle {
    var body: some Widget {
        WaveWidget()
        NowPlayingWidget()
    }
}

struct Entry: TimelineEntry {
    let date: Date
    let snapshot: NowPlayingSnapshot?
}

struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> Entry {
        Entry(date: .now, snapshot: NowPlayingSnapshot(title: "Lacuna", artist: "Lowborn", playing: true, cover: nil, tint: nil, updatedAt: .now))
    }

    func getSnapshot(in context: Context, completion: @escaping (Entry) -> Void) {
        completion(context.isPreview ? placeholder(in: context) : Entry(date: .now, snapshot: NowPlayingSnapshot.load()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<Entry>) -> Void) {
        // Приложение обновляет виджет само при смене трека; старый снимок через час гаснет
        let snapshot = NowPlayingSnapshot.load().flatMap { Date().timeIntervalSince($0.updatedAt) < 3600 ? $0 : nil }
        completion(Timeline(entries: [Entry(date: .now, snapshot: snapshot)], policy: .after(.now.addingTimeInterval(3600))))
    }
}

private let brand = LinearGradient(
    colors: [
        Color(red: 0.294, green: 0.129, blue: 1),
        Color(red: 0.690, green: 0.110, blue: 0.941),
        Color(red: 1, green: 0.239, blue: 0.478),
        Color(red: 1, green: 0.604, blue: 0.235),
    ],
    startPoint: .topLeading,
    endPoint: .bottomTrailing
)

// MARK: «Моя волна»

struct WaveWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WatchShared.waveWidgetKind, provider: Provider()) { _ in
            WaveWidgetView()
                .containerBackground(for: .widget) { Color.clear }
                .widgetURL(WatchShared.waveURL)
        }
        .configurationDisplayName(Text("My Wave"))
        .description(Text("Start My Wave on your device"))
        .supportedFamilies([.accessoryCircular, .accessoryCorner, .accessoryInline, .accessoryRectangular])
    }
}

struct WaveWidgetView: View {
    @Environment(\.widgetFamily) private var family
    @Environment(\.widgetRenderingMode) private var mode

    var body: some View {
        switch family {
        case .accessoryCorner:
            Image(systemName: "waveform")
                .font(.system(size: 22, weight: .semibold))
                .widgetLabel { Text("My Wave") }
        case .accessoryInline:
            Label { Text("My Wave") } icon: { Image(systemName: "waveform") }
        case .accessoryRectangular:
            HStack(spacing: 8) {
                circle.frame(width: 40, height: 40)
                Text("My Wave")
                    .font(.headline)
                    .widgetAccentable()
                Spacer(minLength: 0)
            }
        default:
            circle
        }
    }

    /// Круг в цветах бренда на цветных циферблатах, в одноцветных — системный фон
    @ViewBuilder private var circle: some View {
        ZStack {
            if mode == .fullColor {
                Circle().fill(brand)
            } else {
                AccessoryWidgetBackground().clipShape(Circle())
            }
            Image(systemName: "waveform")
                .font(.system(size: 20, weight: .bold))
                .foregroundStyle(.white)
                .widgetAccentable()
        }
    }
}

// MARK: «Сейчас играет»

struct NowPlayingWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WatchShared.widgetKind, provider: Provider()) { entry in
            NowPlayingWidgetView(snapshot: entry.snapshot)
                .containerBackground(for: .widget) { tint(entry.snapshot) }
                .widgetURL(entry.snapshot == nil ? WatchShared.waveURL : WatchShared.nowURL)
        }
        .configurationDisplayName(Text("Now Playing"))
        .description(Text("What's playing in DalyWave"))
        .supportedFamilies([.accessoryRectangular])
    }

    private func tint(_ snapshot: NowPlayingSnapshot?) -> Color {
        guard let hex = snapshot?.tint, hex.count == 7, let v = Int(hex.dropFirst(), radix: 16) else {
            return Color(red: 0.055, green: 0.063, blue: 0.082)
        }
        return Color(red: Double((v >> 16) & 0xff) / 255, green: Double((v >> 8) & 0xff) / 255, blue: Double(v & 0xff) / 255)
            .opacity(0.45)
    }
}

struct NowPlayingWidgetView: View {
    let snapshot: NowPlayingSnapshot?

    var body: some View {
        if let snapshot {
            HStack(spacing: 8) {
                cover(snapshot)
                VStack(alignment: .leading, spacing: 1) {
                    HStack(spacing: 4) {
                        Image(systemName: snapshot.playing ? "waveform" : "pause.fill")
                            .font(.system(size: 10, weight: .bold))
                            .widgetAccentable()
                        Text(snapshot.playing ? "Now Playing" : "Paused")
                            .font(.system(size: 12, weight: .semibold))
                            .widgetAccentable()
                    }
                    Text(snapshot.title).font(.headline).lineLimit(1)
                    Text(snapshot.artist).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 0)
            }
        } else {
            HStack(spacing: 8) {
                ZStack {
                    Circle().fill(brand)
                    Image(systemName: "play.fill").font(.system(size: 14, weight: .bold)).foregroundStyle(.white)
                }
                .frame(width: 36, height: 36)
                VStack(alignment: .leading, spacing: 1) {
                    Text("My Wave").font(.headline).widgetAccentable()
                    Text("Nothing playing").font(.caption2).foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
            }
        }
    }

    @ViewBuilder private func cover(_ snapshot: NowPlayingSnapshot) -> some View {
        if let data = snapshot.cover, let image = UIImage(data: data) {
            Image(uiImage: image)
                .resizable()
                .widgetAccentedRenderingMode(.fullColor)
                .scaledToFill()
                .frame(width: 40, height: 40)
                .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        } else {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(.white.opacity(0.12))
                .overlay(Image(systemName: "music.note").font(.system(size: 14)))
                .frame(width: 40, height: 40)
        }
    }
}
