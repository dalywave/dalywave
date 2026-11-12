import SwiftUI
import WidgetKit
import XCTest

/// Снимки виджетов в PNG: каждый виджет в каждом размере, тёмная и светлая тема, тонированные
/// значки (iOS 18+) и заглушка «ничего не играло». Виджет рисуется теми же View, что в расширении,
/// через ImageRenderer — без симулятора домашнего экрана. Обложки — design/fixtures/img
/// (`WIDGET_ART=<папка>`, иначе — цветные квадраты), снимки — design/shots/widgets (`WIDGET_SHOTS`).
@MainActor
final class WidgetRenderTests: XCTestCase {
    let shots: URL = {
        if let dir = ProcessInfo.processInfo.environment["WIDGET_SHOTS"] { return URL(fileURLWithPath: dir) }
        return URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("../../../../../design/shots/widgets")
            .standardizedFileURL
    }()

    /// Размеры виджетов iPhone 6,1″ (393 pt), pt
    let sizes: [(WidgetFamily, String, CGSize, CGFloat)] = [
        (.systemSmall, "small", CGSize(width: 170, height: 170), 22),
        (.systemMedium, "medium", CGSize(width: 364, height: 170), 22),
        (.systemLarge, "large", CGSize(width: 364, height: 382), 22),
        (.accessoryCircular, "circular", CGSize(width: 76, height: 76), 38),
        (.accessoryRectangular, "rectangular", CGSize(width: 172, height: 76), 12),
    ]

    var art: URL!

    override func setUp() async throws {
        WidgetRender.preview = true
        art = FileManager.default.temporaryDirectory.appendingPathComponent("widget-art", isDirectory: true)
        try? FileManager.default.removeItem(at: art)
        try FileManager.default.createDirectory(at: art, withIntermediateDirectories: true)
        let source = ProcessInfo.processInfo.environment["WIDGET_ART"].map { URL(fileURLWithPath: $0) }
        let names = ["now-1", "now-2", "now-3", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "ar1"]
        for (i, name) in names.enumerated() {
            let target = art.appendingPathComponent("\(name).jpg")
            if let source, FileManager.default.fileExists(atPath: source.appendingPathComponent("\(name).jpg").path) {
                try FileManager.default.copyItem(at: source.appendingPathComponent("\(name).jpg"), to: target)
            } else {
                let hue = CGFloat(i) / CGFloat(names.count)
                let image = UIGraphicsImageRenderer(size: CGSize(width: 300, height: 300)).image { ctx in
                    UIColor(hue: hue, saturation: 0.7, brightness: 0.8, alpha: 1).setFill()
                    ctx.fill(CGRect(x: 0, y: 0, width: 300, height: 300))
                }
                try image.jpegData(compressionQuality: 0.9)?.write(to: target)
            }
        }
        // Мозаика плейлиста — как её собирает приложение
        let parts = ["r2", "r3", "r5", "r6"].compactMap { UIImage(contentsOfFile: art.appendingPathComponent("\($0).jpg").path) }
        let mosaic = UIGraphicsImageRenderer(size: CGSize(width: 300, height: 300)).image { _ in
            for (i, image) in parts.enumerated() {
                image.draw(in: CGRect(x: CGFloat(i % 2) * 150, y: CGFloat(i / 2) * 150, width: 150, height: 150))
            }
        }
        try mosaic.jpegData(compressionQuality: 0.9)?.write(to: art.appendingPathComponent("mosaic.jpg"))
        WidgetImages.directory = art
        try FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        WidgetRender.accented = nil
        L.bundle = .main
    }

    private func color(_ file: String) -> WidgetRGB? {
        guard let image = UIImage(contentsOfFile: art.appendingPathComponent(file).path)?.cgImage else { return nil }
        return WidgetColor.dominant(image)
    }

    private func now(_ n: Int, playing: Bool) -> WidgetNowPlaying {
        let titles = [
            ("LOVESONGS", "дора"), ("АКНЕ", "Пошлая Молли"), ("PHANTASMAGORIA — очень длинное название трека", "Mzlff"),
        ]
        return WidgetNowPlaying(
            title: titles[n - 1].0, artist: titles[n - 1].1, playing: playing, artwork: "now-\(n).jpg",
            artworkURL: nil, color: color("now-\(n).jpg"), updatedAt: Date())
    }

    private var library: WidgetLibrary {
        func album(_ file: String, _ title: String, _ artist: String) -> WidgetRecentItem {
            WidgetRecentItem(kind: "album", id: file, title: title, subtitle: artist, artwork: "\(file).jpg", color: color("\(file).jpg"))
        }
        let recent = [
            album("r1", "Вредные советы", "5opka"),
            WidgetRecentItem(kind: "playlist", id: "p1", title: "В дорогу", subtitle: "", artwork: "mosaic.jpg"),
            WidgetRecentItem(kind: "playlist", id: "p2", title: "Микс дня 1", subtitle: "", artwork: nil, mix: true),
            WidgetRecentItem(kind: "artist", id: "a1", title: "Lowborn", subtitle: "", artwork: "ar1.jpg"),
            album("r4", "Leverage", "Mytch"),
            album("r7", "Miss", "дора"),
            album("r6", "WOMANIZER (Deluxe Edition)", "SQWOZ BAB"),
        ]
        let presets = [
            WidgetPreset(id: "1", name: "Работа", icon: "laptop", mood: "calm", activity: "work", character: "mix", language: "none"),
            WidgetPreset(id: "2", name: "Дорога", icon: "car", mood: "energetic", activity: "road", character: "fav", language: "any"),
            WidgetPreset(id: "3", name: "Ночь", icon: "moon-stars", mood: "sad", activity: "sleep", character: "mix", language: "any"),
        ]
        return WidgetLibrary(recent: recent, presets: presets, updatedAt: Date())
    }

    /// Виджет на «обоях»: скругление, как у системы; тонированный — обесцвечен и подкрашен.
    private func render<V: View>(_ view: V, _ size: CGSize, _ radius: CGFloat, _ scheme: ColorScheme, accented: Bool, lock: Bool, name: String) {
        let wallpaper = scheme == .dark ? Color(hex: 0x23262e) : Color(hex: 0xc9ced8)
        let widget = view
            .frame(width: size.width, height: size.height)
            .background(accented && !lock ? Color.white.opacity(0.14) : Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
        let tinted = Group {
            if accented {
                widget.grayscale(1).colorMultiply(Color(hex: 0xffc9a8))
            } else {
                widget
            }
        }
        let framed = tinted
            .padding(20)
            .background(lock || accented ? Color(hex: 0x15171c) : wallpaper)
            .environment(\.colorScheme, lock || accented ? .dark : scheme)
        let renderer = ImageRenderer(content: framed)
        renderer.scale = 3
        guard let data = renderer.uiImage?.pngData() else {
            XCTFail("не отрисовался \(name)")
            return
        }
        XCTAssertNoThrow(try data.write(to: shots.appendingPathComponent("\(name).png")))
    }

    private func entry(_ now: WidgetNowPlaying?, _ library: WidgetLibrary?) -> DalyWaveEntry {
        DalyWaveEntry(date: Date(), now: now, library: library)
    }

    func testRenderAllWidgets() throws {
        for language in ["ru", "en"] {
            if language == "ru", let path = Bundle(for: Self.self).path(forResource: "ru", ofType: "lproj"),
               let bundle = Bundle(path: path)
            {
                L.bundle = bundle
            } else {
                L.bundle = Bundle(for: Self.self)
            }
            let suffix = language == "ru" ? "" : "-en"
            for scheme in [ColorScheme.dark, .light] {
                let theme = scheme == .dark ? "dark" : "light"
                for (family, fname, size, radius) in sizes {
                    let lock = family == .accessoryCircular || family == .accessoryRectangular
                    if lock && scheme == .light { continue }
                    if language == "en" && (lock || scheme == .light) { continue }
                    func shot<V: View>(_ widget: String, _ state: String, _ view: V, accented: Bool = false) {
                        WidgetRender.accented = accented
                        render(view, size, radius, scheme, accented: accented, lock: lock,
                               name: "\(widget)-\(fname)-\(state)-\(theme)\(suffix)")
                    }
                    // Сейчас играет: small, medium, строка экрана блокировки
                    if [.systemSmall, .systemMedium, .accessoryRectangular].contains(family) {
                        for (n, playing) in [(1, true), (2, false), (3, true)] {
                            shot("now", "track\(n)", NowPlayingView(entry: entry(now(n, playing: playing), library), family: family))
                        }
                        shot("now", "empty", NowPlayingView(entry: entry(nil, nil), family: family))
                        if !lock && scheme == .dark && language == "ru" {
                            shot("now", "track1-tinted", NowPlayingView(entry: entry(now(1, playing: true), library), family: family), accented: true)
                        }
                    }
                    // Моя волна: small, medium, круглый значок
                    if [.systemSmall, .systemMedium, .accessoryCircular].contains(family) {
                        shot("wave", "presets", WaveView(entry: entry(nil, library), family: family))
                        if family == .systemMedium {
                            shot("wave", "default", WaveView(entry: entry(nil, nil), family: family))
                        }
                        if !lock && scheme == .dark && language == "ru" {
                            shot("wave", "presets-tinted", WaveView(entry: entry(nil, library), family: family), accented: true)
                        }
                    }
                    // Недавнее: medium, large
                    if [.systemMedium, .systemLarge].contains(family) {
                        shot("recent", "items", RecentView(entry: entry(nil, library), family: family))
                        shot("recent", "empty", RecentView(entry: entry(nil, nil), family: family))
                        if scheme == .dark && language == "ru" {
                            shot("recent", "items-tinted", RecentView(entry: entry(nil, library), family: family), accented: true)
                        }
                    }
                }
            }
        }
    }
}
