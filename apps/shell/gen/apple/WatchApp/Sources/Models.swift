import Foundation

// Модели API сервера — только поля, нужные часам (crates/protocol: catalog, connect, library, wave).
// Незнакомые поля пропускаются; необязательные — с запасным значением, чтобы новое поле на сервере
// не ломало часы.

struct ImageSet: Codable, Hashable {
    var small: String?
    var medium: String?
    var large: String?
}

struct ArtistRef: Codable, Hashable {
    var id: String
    var name: String
}

struct AlbumRef: Codable, Hashable {
    var id: String
    var title: String
    var image: ImageSet?
}

struct Track: Codable, Hashable, Identifiable {
    var id: String
    var title: String
    var artists: [ArtistRef]
    var album: AlbumRef?
    var durationMs: Int?
    var liked: Bool?

    var artistLine: String { artists.map(\.name).joined(separator: ", ") }
    var cover: String? { album?.image?.small }
}

struct QueueItem: Codable, Hashable {
    var uid: String
    var track: Track
}

struct PlayContext: Codable, Hashable {
    var type: String
    var id: String?
    var title: String
}

/// Настройки волны: сервер не принимает незнакомых полей, поэтому только эти.
struct WaveSettings: Codable, Hashable {
    var activity: String?
    var character: String?
    var mood: String?
    var language: String?
    var tempo: Int?
}

struct PlaybackState: Decodable, Equatable {
    var current: QueueItem?
    var context: PlayContext?
    var playing = false
    var pending: String?
    var positionMs = 0
    var durationMs = 0
    var volume = 1.0
    var muted = false
    var wave: WaveSettings?
    var at: Date?

    enum CodingKeys: String, CodingKey {
        case current, context, playing, pending, positionMs, durationMs, volume, muted, wave, at
    }

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        current = try? c.decodeIfPresent(QueueItem.self, forKey: .current)
        context = try? c.decodeIfPresent(PlayContext.self, forKey: .context)
        playing = (try? c.decodeIfPresent(Bool.self, forKey: .playing)) ?? false
        pending = try? c.decodeIfPresent(String.self, forKey: .pending)
        positionMs = (try? c.decodeIfPresent(Int.self, forKey: .positionMs)) ?? 0
        durationMs = (try? c.decodeIfPresent(Int.self, forKey: .durationMs)) ?? 0
        volume = (try? c.decodeIfPresent(Double.self, forKey: .volume)) ?? 1
        muted = (try? c.decodeIfPresent(Bool.self, forKey: .muted)) ?? false
        wave = try? c.decodeIfPresent(WaveSettings.self, forKey: .wave)
        at = try? c.decodeIfPresent(Date.self, forKey: .at)
    }

    /// Позиция на момент `now` (время сервера): пока играет, время идёт.
    func position(at now: Date) -> Int {
        guard playing, pending == nil, let at else { return positionMs }
        let pos = positionMs + max(0, Int(now.timeIntervalSince(at) * 1000))
        return durationMs > 0 ? min(durationMs, pos) : pos
    }
}

struct Capabilities: Codable, Hashable {
    var volume: Bool?
    var dj: Bool?
    var remote: Bool?
}

/// Устройство в «Где играть».
struct ConnectDevice: Codable, Hashable, Identifiable {
    var id: String
    var name: String
    var kind: String
    var platform: String
    var capabilities: Capabilities
    var online: Bool
    var active: Bool

    /// Звук там играет (часы — только пульт)
    var playable: Bool { capabilities.remote != true && kind != "watch" }

    var symbol: String {
        switch kind {
        case "phone": "iphone"
        case "tablet": "ipad"
        case "laptop": "laptopcomputer"
        case "desktop": "desktopcomputer"
        case "watch": "applewatch"
        default: "globe"
        }
    }
}

struct PlaylistSummary: Codable, Hashable, Identifiable {
    var id: String
    var title: String
    var tracksCount: Int?
    var covers: [ImageSet]?
    var image: ImageSet?

    var cover: String? { image?.small ?? covers?.first?.small }
}

struct WavePreset: Codable, Hashable, Identifiable {
    var id: String
    var name: String
    var icon: String?
    var settings: WaveSettings
}

/// Элемент «Продолжить» с главной: альбом, плейлист или артист.
struct RecentItem: Decodable, Hashable, Identifiable {
    var type: String
    var id: String
    var title: String
    var subtitle: String?
    var cover: String?

    enum CodingKeys: String, CodingKey { case type, id, title, name, artist, image, covers, description }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        type = try c.decode(String.self, forKey: .type)
        id = try c.decode(String.self, forKey: .id)
        title = (try? c.decodeIfPresent(String.self, forKey: .title))
            ?? (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
        subtitle = (try? c.decodeIfPresent(ArtistRef.self, forKey: .artist))?.name
        let image = try? c.decodeIfPresent(ImageSet.self, forKey: .image)
        let covers = try? c.decodeIfPresent([ImageSet].self, forKey: .covers)
        cover = image?.small ?? covers?.first?.small
    }
}

struct Page<T: Decodable>: Decodable {
    var items: [T]
}

struct TrackRow: Decodable {
    var track: Track?
}

struct HomeShelf: Decodable {
    var id: String
    var items: [FailableDecodable<RecentItem>]
}

struct Home: Decodable {
    var shelves: [HomeShelf]
}

/// Элемент, который не разобрался, пропускается, а не ломает весь список.
struct FailableDecodable<T: Decodable>: Decodable {
    var value: T?

    init(from decoder: Decoder) throws {
        value = try? T(from: decoder)
    }
}

struct AlbumDetail: Decodable {
    var tracks: [Track]
}

struct ArtistDetail: Decodable {
    var topTracks: [Track]
}
