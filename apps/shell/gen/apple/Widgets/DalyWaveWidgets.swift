import SwiftUI
import WidgetKit

// Расширение dalywave-widgets (WidgetKit): виджеты экрана «Домой», экрана блокировки и StandBy.
// Данные — снимок, который пишет приложение (WidgetsShared/WidgetData.swift); сами виджеты в сеть
// не ходят. Таймлайн — одна запись: обновляет приложение (WidgetCenter), когда что-то меняется.

struct SnapshotProvider: TimelineProvider {
    func placeholder(in context: Context) -> DalyWaveEntry {
        DalyWaveEntry(date: Date(), now: nil, library: nil)
    }

    func getSnapshot(in context: Context, completion: @escaping (DalyWaveEntry) -> Void) {
        completion(.current())
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<DalyWaveEntry>) -> Void) {
        completion(Timeline(entries: [.current()], policy: .never))
    }
}

/// Семейства, которые есть на этой iOS: значки экрана блокировки — с iOS 16.
private enum Families {
    static var nowPlaying: [WidgetFamily] {
        if #available(iOS 16.0, *) { return [.systemSmall, .systemMedium, .accessoryRectangular] }
        return [.systemSmall, .systemMedium]
    }

    static var wave: [WidgetFamily] {
        if #available(iOS 16.0, *) { return [.systemSmall, .systemMedium, .accessoryCircular] }
        return [.systemSmall, .systemMedium]
    }
}

extension WidgetConfiguration {
    /// Отступы рисуем сами (WidgetViews.swift): одинаково на всех версиях iOS
    func ownMargins() -> some WidgetConfiguration {
        if #available(iOS 17.0, *) {
            return contentMarginsDisabled()
        }
        return self
    }
}

struct NowPlayingWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetKind.nowPlaying, provider: SnapshotProvider()) { entry in
            FamilyReader { NowPlayingView(entry: entry, family: $0) }
        }
        .configurationDisplayName(L.t("Now Playing"))
        .description(L.t("What's playing, with pause and next."))
        .supportedFamilies(Families.nowPlaying)
        .ownMargins()
    }
}

struct WaveWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetKind.wave, provider: SnapshotProvider()) { entry in
            FamilyReader { WaveView(entry: entry, family: $0) }
        }
        .configurationDisplayName(L.t("My Wave"))
        .description(L.t("My Wave and your saved mixes in one tap."))
        .supportedFamilies(Families.wave)
        .ownMargins()
    }
}

struct RecentWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetKind.recent, provider: SnapshotProvider()) { entry in
            FamilyReader { RecentView(entry: entry, family: $0) }
        }
        .configurationDisplayName(L.t("Recent"))
        .description(L.t("Albums and playlists you've been listening to."))
        .supportedFamilies([.systemMedium, .systemLarge])
        .ownMargins()
    }
}

/// Семейство виджета из окружения.
struct FamilyReader<Content: View>: View {
    @Environment(\.widgetFamily) private var family
    var content: (WidgetFamily) -> Content

    var body: some View {
        content(family)
    }
}

@main
struct DalyWaveWidgetBundle: WidgetBundle {
    var body: some Widget {
        NowPlayingWidget()
        WaveWidget()
        RecentWidget()
    }
}
