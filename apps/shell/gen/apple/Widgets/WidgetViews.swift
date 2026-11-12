import SwiftUI
import WidgetKit

// Виджеты DalyWave: «Сейчас играет», «Моя волна», «Недавнее» и значки экрана блокировки.
// Отступы — свои (contentMarginsDisabled): так виджет одинаков на iOS 15–27 и на снимках проверки.

struct DalyWaveEntry: TimelineEntry {
    var date: Date
    var now: WidgetNowPlaying?
    var library: WidgetLibrary?

    static func current() -> DalyWaveEntry {
        DalyWaveEntry(date: Date(), now: WidgetStore.nowPlaying, library: WidgetStore.library)
    }
}

/// Отступ от края виджета
private let inset: CGFloat = 16

extension WidgetFamily {
    /// Значки экрана блокировки появились в iOS 16
    var isLockCircular: Bool {
        if #available(iOS 16.0, *) { return self == .accessoryCircular }
        return false
    }

    var isLockRectangular: Bool {
        if #available(iOS 16.0, *) { return self == .accessoryRectangular }
        return false
    }
}

// MARK: - Кнопки

/// Действие кнопки: на iOS 17+ — App Intent в процессе приложения, раньше — ссылка (откроет его).
enum WidgetAction {
    case playPause(playing: Bool)
    case next
    case wave(WidgetPreset?)

    var link: URL {
        switch self {
        case .playPause: return URL(string: "dalywave://remote/toggle")!
        case .next: return URL(string: "dalywave://remote/next")!
        case let .wave(preset):
            var parts = URLComponents(string: "dalywave://wave")!
            let items = [
                ("mood", preset?.mood), ("activity", preset?.activity),
                ("character", preset?.character), ("language", preset?.language),
            ].compactMap { k, v in v.map { URLQueryItem(name: k, value: $0) } }
            parts.queryItems = items.isEmpty ? nil : items
            return parts.url!
        }
    }
}

struct ActionButton<Label: View>: View {
    var action: WidgetAction
    @ViewBuilder var label: () -> Label

    var body: some View {
        if #available(iOS 17.0, *) {
            switch action {
            case let .playPause(playing):
                Button(intent: WidgetPlayPauseIntent(playing: playing), label: label).buttonStyle(.plain)
            case .next:
                Button(intent: WidgetNextIntent(), label: label).buttonStyle(.plain)
            case let .wave(preset):
                Button(intent: WidgetWaveIntent(preset: preset), label: label).buttonStyle(.plain)
            }
        } else {
            Link(destination: action.link, label: label)
        }
    }
}

/// Круглая кнопка «играть/пауза»: заливка — главный цвет текста, знак — цвет фона.
struct PlayCircle: View {
    var playing: Bool
    var size: CGFloat
    var palette: Palette
    var accented: Bool

    var body: some View {
        ZStack {
            Circle().fill(accented ? Color.white.opacity(0.2) : palette.button)
            Image(systemName: playing ? "pause.fill" : "play.fill")
                .font(.system(size: size * 0.4, weight: .bold))
                .foregroundColor(accented ? .white : palette.onButton)
                // Треугольник «играть» визуально смещён влево — вернуть в центр
                .offset(x: playing ? 0 : size * 0.035)
        }
        .frame(width: size, height: size)
        .accentable()
    }
}

// MARK: - Обложка

struct Cover: View {
    var file: String?
    var size: CGFloat
    var radius: CGFloat
    var palette: Palette
    var round = false
    var mix = false

    var body: some View {
        let art = Group {
            if let image = WidgetImages.load(file) {
                Image(uiImage: image).resizable().artworkRendering().aspectRatio(contentMode: .fill)
            } else if mix {
                ZStack {
                    Brand.gradient
                    WaveGlyph(size: size * 0.46).foregroundColor(.white)
                }
            } else {
                ZStack {
                    palette.fill
                    Image(systemName: "music.note")
                        .font(.system(size: size * 0.3, weight: .medium))
                        .foregroundColor(palette.secondary)
                }
            }
        }
        .frame(width: size, height: size)
        if round {
            art.clipShape(Circle())
        } else {
            art.clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
        }
    }
}

/// Знак волны крупно: линия в спектре бренда — «гребень волны» «Полночи». Рамка — по видимому
/// штриху, без полей файла знака.
struct WaveMark: View {
    var width: CGFloat
    var accented: Bool

    var body: some View {
        let shape = TightGlyphShape()
        let stroke = StrokeStyle(lineWidth: width * TightGlyphShape.stroke, lineCap: .round, lineJoin: .round)
        Group {
            if accented {
                shape.stroke(Color.white, style: stroke)
            } else {
                shape.stroke(Brand.gradient, style: stroke)
            }
        }
        .frame(width: width, height: width * TightGlyphShape.aspect)
        .accentable()
    }
}

/// Плитка сочетания: цвет настроения и значок занятия.
struct PresetSwatch: View {
    var preset: WidgetPreset
    var size: CGFloat
    var accented: Bool

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
                .fill(accented ? Color.white.opacity(0.18) : Brand.swatch(preset))
            Image(systemName: PresetIcon.symbol(preset.icon))
                .font(.system(size: size * 0.42, weight: .semibold))
                .foregroundColor(Color.white.opacity(0.94))
        }
        .frame(width: size, height: size)
    }
}

// MARK: - Сейчас играет

struct NowPlayingView: View {
    var entry: DalyWaveEntry
    var family: WidgetFamily
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        if let now = entry.now {
            AccentedReader { accented in
                content(now, accented: accented)
            }
        } else {
            // Ещё ничего не играло — кнопка волны
            WaveView(entry: entry, family: family)
        }
    }

    @ViewBuilder
    private func content(_ now: WidgetNowPlaying, accented: Bool) -> some View {
        let palette = Palette.cover(now.color, scheme)
        if family.isLockRectangular {
            rectangular(now)
        } else if family == .systemMedium {
            medium(now, palette, accented)
                .widgetSurface(palette.background)
        } else {
            small(now, palette, accented)
                .widgetSurface(palette.background)
        }
    }

    private func small(_ now: WidgetNowPlaying, _ palette: Palette, _ accented: Bool) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .top, spacing: 0) {
                Cover(file: now.artwork, size: 76, radius: 12, palette: palette)
                Spacer(minLength: 6)
                VStack(spacing: 4) {
                    ActionButton(action: .playPause(playing: now.playing)) {
                        PlayCircle(playing: now.playing, size: 36, palette: palette, accented: accented)
                    }
                    ActionButton(action: .next) {
                        Image(systemName: "forward.fill")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(palette.primary)
                            .frame(width: 36, height: 28)
                    }
                }
            }
            Spacer(minLength: 8)
            titles(now, palette, size: 15, lines: 1)
        }
        .padding(inset)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private func medium(_ now: WidgetNowPlaying, _ palette: Palette, _ accented: Bool) -> some View {
        GeometryReader { geo in
            let side = geo.size.height - inset * 2
            HStack(spacing: 14) {
                Cover(file: now.artwork, size: side, radius: 12, palette: palette)
                VStack(alignment: .leading, spacing: 0) {
                    titles(now, palette, size: 17, lines: 2)
                        .padding(.top, 2)
                    Spacer(minLength: 8)
                    HStack(spacing: 6) {
                        ActionButton(action: .playPause(playing: now.playing)) {
                            PlayCircle(playing: now.playing, size: 44, palette: palette, accented: accented)
                        }
                        ActionButton(action: .next) {
                            Image(systemName: "forward.fill")
                                .font(.system(size: 19, weight: .semibold))
                                .foregroundColor(palette.primary)
                                .frame(width: 44, height: 44)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(inset)
        }
    }

    private func titles(_ now: WidgetNowPlaying, _ palette: Palette, size: CGFloat, lines: Int) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(now.title)
                .font(.system(size: size, weight: .semibold))
                .tracking(-0.2)
                .foregroundColor(palette.primary)
                .lineLimit(lines)
                .accentable()
            Text(now.artist)
                .font(.system(size: size - 2))
                .foregroundColor(palette.secondary)
                .lineLimit(1)
        }
    }

    /// Экран блокировки: название и исполнитель, справа — пауза.
    private func rectangular(_ now: WidgetNowPlaying) -> some View {
        HStack(spacing: 6) {
            VStack(alignment: .leading, spacing: 0) {
                Text(now.title).font(.system(size: 15, weight: .semibold)).lineLimit(1).accentable()
                Text(now.artist).font(.system(size: 13)).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            ActionButton(action: .playPause(playing: now.playing)) {
                Image(systemName: now.playing ? "pause.fill" : "play.fill")
                    .font(.system(size: 18, weight: .semibold))
                    .frame(width: 32, height: 40)
            }
        }
        .widgetSurface(Color.clear)
    }
}

// MARK: - Моя волна

struct WaveView: View {
    var entry: DalyWaveEntry
    var family: WidgetFamily
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        AccentedReader { accented in
            let palette = Palette.neutral(scheme)
            if family.isLockCircular {
                circular
            } else if family.isLockRectangular {
                rectangular
            } else if family == .systemMedium || family == .systemLarge {
                medium(palette, accented).widgetSurface(palette.background)
            } else {
                ActionButton(action: .wave(nil)) { start(palette, accented, glyph: 70) }
                    .widgetURL(WidgetAction.wave(nil).link)
                    .widgetSurface(palette.background)
            }
        }
    }

    /// Знак волны и подпись — весь маленький виджет и левая часть среднего.
    /// В среднем виджете кнопки «играть» нет: кнопка — вся левая часть, справа — сочетания.
    private func start(_ palette: Palette, _ accented: Bool, glyph: CGFloat, play: Bool = true) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .top, spacing: 0) {
                WaveMark(width: glyph, accented: accented)
                    .padding(.top, 4)
                Spacer(minLength: 6)
                if play {
                    PlayCircle(playing: false, size: 32, palette: palette, accented: accented)
                }
            }
            Spacer(minLength: 8)
            Text(L.t("My Wave"))
                .font(.system(size: 17, weight: .semibold))
                .tracking(-0.3)
                .foregroundColor(palette.primary)
                .lineLimit(1)
                .minimumScaleFactor(0.85)
                .accentable()
        }
        .padding(inset)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .contentShape(Rectangle())
    }

    private var presets: [WidgetPreset] {
        let own = entry.library?.presets ?? []
        return Array((own.isEmpty ? PresetIcon.defaults : own).prefix(3))
    }

    private func medium(_ palette: Palette, _ accented: Bool) -> some View {
        HStack(spacing: 0) {
            ActionButton(action: .wave(nil)) { start(palette, accented, glyph: 72, play: false) }
                .frame(width: 150)
            VStack(spacing: 0) {
                ForEach(Array(presets.enumerated()), id: \.element.id) { i, preset in
                    if i > 0 {
                        palette.hair.frame(height: 0.5).padding(.leading, 44)
                    }
                    ActionButton(action: .wave(preset)) {
                        HStack(spacing: 12) {
                            PresetSwatch(preset: preset, size: 32, accented: accented)
                            Text(preset.name)
                                .font(.system(size: 15, weight: .medium))
                                .foregroundColor(palette.primary)
                                .lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        .frame(maxHeight: .infinity)
                        .contentShape(Rectangle())
                    }
                }
            }
            .padding(.vertical, 8)
            .padding(.leading, 4)
            .padding(.trailing, inset)
        }
    }

    private var circular: some View {
        ActionButton(action: .wave(nil)) {
            ZStack {
                if WidgetRender.preview {
                    Circle().fill(Color.white.opacity(0.16))
                } else if #available(iOS 16.0, *) {
                    AccessoryWidgetBackground()
                }
                TightGlyphShape()
                    .stroke(Color.white, style: StrokeStyle(lineWidth: 40 * TightGlyphShape.stroke, lineCap: .round, lineJoin: .round))
                    .frame(width: 40, height: 40 * TightGlyphShape.aspect)
                    .accentable()
            }
        }
        .widgetSurface(Color.clear)
    }

    private var rectangular: some View {
        ActionButton(action: .wave(nil)) {
            HStack(spacing: 8) {
                TightGlyphShape()
                    .stroke(style: StrokeStyle(lineWidth: 24 * TightGlyphShape.stroke, lineCap: .round, lineJoin: .round))
                    .frame(width: 24, height: 24 * TightGlyphShape.aspect)
                    .accentable()
                Text(L.t("My Wave")).font(.system(size: 16, weight: .semibold)).lineLimit(1)
                Spacer(minLength: 0)
            }
        }
        .widgetSurface(Color.clear)
    }
}

// MARK: - Недавнее

struct RecentView: View {
    var entry: DalyWaveEntry
    var family: WidgetFamily
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let items = entry.library?.recent ?? []
        if items.isEmpty {
            WaveView(entry: entry, family: family)
        } else {
            AccentedReader { accented in
                let palette = Palette.neutral(scheme)
                content(items, palette, accented).widgetSurface(palette.background)
            }
        }
    }

    private func content(_ items: [WidgetRecentItem], _ palette: Palette, _ accented: Bool) -> some View {
        GeometryReader { geo in
            let large = family == .systemLarge
            let columns = large ? 3 : 4
            let gap: CGFloat = large ? 12 : 10
            let side = ((geo.size.width - inset * 2 - gap * CGFloat(columns - 1)) / CGFloat(columns)).rounded(.down)
            let rows = large ? 2 : 1
            let shown = Array(items.prefix(columns * rows))
            // Большой: заголовок сверху, ряды делят остальную высоту поровну; средний — по центру
            VStack(alignment: .leading, spacing: 0) {
                if !large { Spacer(minLength: 0) }
                Text(L.t("Recent"))
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(palette.secondary)
                ForEach(0..<rows, id: \.self) { row in
                    if large { Spacer(minLength: 10) } else { Spacer().frame(height: 10) }
                    HStack(alignment: .top, spacing: gap) {
                        ForEach(shown.dropFirst(row * columns).prefix(columns)) { item in
                            cell(item, side: side, large: large, palette: palette)
                        }
                    }
                }
                if !large { Spacer(minLength: 0) }
            }
            .padding(inset)
        }
    }

    @ViewBuilder
    private func cell(_ item: WidgetRecentItem, side: CGFloat, large: Bool, palette: Palette) -> some View {
        let body = VStack(alignment: item.kind == "artist" ? .center : .leading, spacing: large ? 6 : 5) {
            Cover(
                file: item.artwork, size: side, radius: large ? 10 : 8, palette: palette,
                round: item.kind == "artist", mix: item.mix == true)
            VStack(alignment: item.kind == "artist" ? .center : .leading, spacing: 1) {
                Text(item.title)
                    .font(.system(size: large ? 13 : 11, weight: .medium))
                    .foregroundColor(palette.primary)
                    .lineLimit(1)
                if large, !item.subtitle.isEmpty {
                    Text(item.subtitle)
                        .font(.system(size: 12))
                        .foregroundColor(palette.secondary)
                        .lineLimit(1)
                }
            }
        }
        .frame(width: side)
        if let link = item.link {
            Link(destination: link) { body }
        } else {
            body
        }
    }
}
