import Foundation
import ImageIO
import UIKit
import WidgetKit

// Виджеты: приложение пишет снимок в контейнер App Group (WidgetsShared/WidgetData.swift) и просит
// WidgetKit перерисовать их. Что играет и что показать в «Недавнем» знает Rust
// (apps/shell/src/widgets.rs): он зовёт сюда через рантайм Objective-C JSON-строкой. Обложки
// скачиваются здесь, уменьшаются (виджету — не больше 300 px) и дают цвет фона.

@objc(DalyWaveWidgets)
final class DalyWaveWidgetsBridge: NSObject {
    /// Файлы и сеть — по очереди и не на главном потоке
    fileprivate static let queue = DispatchQueue(label: "rest.daly.wave.widgets", qos: .utility)

    /// Что играет: `{"title","artist","playing","artworkUrl"}`; `null` — ничего.
    @objc static func nowPlaying(_ json: String) {
        queue.async { WidgetWriter.nowPlaying(json) }
    }

    /// Недавнее и сочетания волны: `{"recent":[…],"presets":[…]}`.
    @objc static func library(_ json: String) {
        queue.async { WidgetWriter.library(json) }
    }

    /// Выход из аккаунта: виджеты возвращаются к заглушке.
    @objc static func clear() {
        queue.async { WidgetWriter.clear() }
    }

    /// Кнопка паузы на виджете: состояние — сразу, не дожидаясь события движка.
    static func markPaused() {
        queue.sync {
            guard var now = WidgetStore.nowPlaying, now.playing else { return }
            now.playing = false
            now.updatedAt = Date()
            WidgetStore.write(now, WidgetStore.nowFile)
        }
    }
}

/// Кнопки виджетов в процессе приложения → Rust (siri.rs).
@available(iOS 17.0, *)
enum WidgetActions {
    static func perform(_ command: [String: Any]) throws {
        let code = SiriBridge.perform(command)
        if code != 0 { throw SiriBridgeError(code: code) }
        if command["action"] as? String == "pause" {
            DalyWaveWidgetsBridge.markPaused()
        }
    }
}

private enum WidgetWriter {
    /// Сторона обложки в «Сейчас играет» и «Недавнем», px
    static let side = 300

    private struct NowIn: Decodable {
        var title: String
        var artist: String
        var playing: Bool
        var artworkUrl: String?
    }

    private struct RecentIn: Decodable {
        var kind: String
        var id: String
        var title: String
        var subtitle: String?
        /// Одна обложка или четыре — мозаика плейлиста
        var images: [String]
        var mix: Bool?
        /// Главный цвет обложки от сервера (`#rrggbb`)
        var color: String?
    }

    private struct PresetIn: Decodable {
        var id: String
        var name: String
        var icon: String?
        var mood: String?
        var activity: String?
        var character: String?
        var language: String?
    }

    private struct LibraryIn: Decodable {
        var recent: [RecentIn]
        var presets: [PresetIn]
    }

    static func reload(_ kinds: [String]) {
        for kind in kinds {
            WidgetCenter.shared.reloadTimelines(ofKind: kind)
        }
    }

    static func nowPlaying(_ json: String) {
        let before = WidgetStore.nowPlaying
        guard let incoming = try? JSONDecoder().decode(NowIn.self, from: Data(json.utf8)) else {
            if before != nil {
                WidgetStore.remove(WidgetStore.nowFile)
                cleanup(prefix: "now-", keep: [])
                reload([WidgetKind.nowPlaying])
            }
            return
        }
        var now = WidgetNowPlaying(
            title: incoming.title, artist: incoming.artist, playing: incoming.playing,
            artwork: nil, artworkURL: incoming.artworkUrl, color: nil, updatedAt: Date())
        if let before, before.artworkURL == incoming.artworkUrl, let file = before.artwork,
           let path = WidgetStore.url(file), FileManager.default.fileExists(atPath: path.path)
        {
            now.artwork = file
            now.color = before.color
        } else if let link = incoming.artworkUrl, let url = URL(string: link), let data = download(url),
                  let image = thumbnail(data)
        {
            let file = "now-\(stableHash(link)).jpg"
            if save(image, file) {
                now.artwork = file
                now.color = WidgetColor.dominant(image)
            }
        }
        if let before, before.title == now.title, before.artist == now.artist,
           before.playing == now.playing, before.artwork == now.artwork
        {
            return
        }
        WidgetStore.write(now, WidgetStore.nowFile)
        cleanup(prefix: "now-", keep: Set([now.artwork].compactMap { $0 }))
        reload([WidgetKind.nowPlaying])
    }

    static func library(_ json: String) {
        guard let incoming = try? JSONDecoder().decode(LibraryIn.self, from: Data(json.utf8)) else { return }
        let before = WidgetStore.library
        var recent: [WidgetRecentItem] = []
        for item in incoming.recent {
            let source = item.images.joined(separator: " ")
            let file = "recent-\(stableHash(item.kind + item.id + source)).jpg"
            var out = WidgetRecentItem(
                kind: item.kind, id: item.id, title: item.title, subtitle: item.subtitle ?? "",
                artwork: nil, color: item.color.flatMap(WidgetColor.hex), mix: item.mix)
            if let path = WidgetStore.url(file), FileManager.default.fileExists(atPath: path.path) {
                out.artwork = file
                out.color = out.color ?? before?.recent.first(where: { $0.artwork == file })?.color
            } else if let image = cover(item.images) {
                if save(image, file) {
                    out.artwork = file
                    out.color = out.color ?? WidgetColor.dominant(image)
                }
            }
            recent.append(out)
        }
        let presets = incoming.presets.map {
            WidgetPreset(
                id: $0.id, name: $0.name, icon: $0.icon ?? "sparkle", mood: $0.mood,
                activity: $0.activity, character: $0.character, language: $0.language)
        }
        if let before, before.recent == recent, before.presets == presets { return }
        WidgetStore.write(WidgetLibrary(recent: recent, presets: presets, updatedAt: Date()), WidgetStore.libraryFile)
        cleanup(prefix: "recent-", keep: Set(recent.compactMap(\.artwork)))
        reload([WidgetKind.recent, WidgetKind.wave])
    }

    static func clear() {
        WidgetStore.remove(WidgetStore.nowFile)
        WidgetStore.remove(WidgetStore.libraryFile)
        cleanup(prefix: "now-", keep: [])
        cleanup(prefix: "recent-", keep: [])
        WidgetCenter.shared.reloadAllTimelines()
    }

    // ---- картинки ----

    /// Обложка элемента «Недавнего»: одна картинка или мозаика 2×2 из четырёх.
    static func cover(_ links: [String]) -> CGImage? {
        let images = links.prefix(4).compactMap { link -> CGImage? in
            guard let url = URL(string: link), let data = download(url) else { return nil }
            return thumbnail(data, side: links.count >= 4 ? side / 2 : side)
        }
        guard let first = images.first else { return nil }
        guard images.count >= 4 else { return first }
        let half = side / 2
        guard let ctx = CGContext(
            data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        else { return first }
        ctx.interpolationQuality = .high
        // CGContext — от нижнего левого угла: первая обложка — сверху слева, как в приложении
        let origins = [(0, half), (half, half), (0, 0), (half, 0)]
        for (image, (x, y)) in zip(images, origins) {
            ctx.draw(image, in: CGRect(x: x, y: y, width: half, height: half))
        }
        return ctx.makeImage() ?? first
    }

    static func download(_ url: URL) -> Data? {
        var out: Data?
        let done = DispatchSemaphore(value: 0)
        URLSession.shared.dataTask(with: URLRequest(url: url, timeoutInterval: 15)) { data, response, _ in
            if let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) {
                out = data
            }
            done.signal()
        }.resume()
        done.wait()
        return out
    }

    static func thumbnail(_ data: Data, side: Int = side) -> CGImage? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: side,
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }

    static func save(_ image: CGImage, _ file: String) -> Bool {
        guard let url = WidgetStore.url(file),
              let data = UIImage(cgImage: image).jpegData(compressionQuality: 0.86)
        else { return false }
        return (try? data.write(to: url, options: .atomic)) != nil
    }

    /// Удалить старые файлы обложек с этим началом имени, кроме нужных.
    static func cleanup(prefix: String, keep: Set<String>) {
        guard let dir = WidgetStore.folder,
              let names = try? FileManager.default.contentsOfDirectory(atPath: dir.path)
        else { return }
        for name in names where name.hasPrefix(prefix) && !keep.contains(name) {
            try? FileManager.default.removeItem(at: dir.appendingPathComponent(name))
        }
    }

    /// Имя файла от ссылки: FNV-1a, одинаковое между запусками (в отличие от Hasher).
    static func stableHash(_ s: String) -> String {
        var h: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in s.utf8 {
            h ^= UInt64(byte)
            h = h &* 0x100_0000_01b3
        }
        return String(h, radix: 36)
    }
}
