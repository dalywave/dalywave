import Foundation

// Модели API сервера — только нужные телевизору поля (crates/protocol: catalog, connect, library, wave, tv).
// Трек хранит и исходный JSON: телевизор — активное устройство «Где играть», в снимке состояния он
// отдаёт трек серверу и пультам целиком, как получил.

/// Любое значение JSON: для полей, которые телевизор передаёт дальше, не разбирая.
enum JSON: Codable, Hashable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let b = try? c.decode(Bool.self) { self = .bool(b) }
        else if let n = try? c.decode(Double.self) { self = .number(n) }
        else if let s = try? c.decode(String.self) { self = .string(s) }
        else if let a = try? c.decode([JSON].self) { self = .array(a) }
        else { self = .object(try c.decode([String: JSON].self)) }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case let .bool(b): try c.encode(b)
        case let .number(n): try c.encode(n)
        case let .string(s): try c.encode(s)
        case let .array(a): try c.encode(a)
        case let .object(o): try c.encode(o)
        }
    }

    /// Для JSONSerialization (сообщения WebSocket собираются словарями).
    var any: Any {
        switch self {
        case .null: NSNull()
        case let .bool(b): b
        case let .number(n): n.rounded() == n && abs(n) < 1e15 ? Int(n) as Any : n
        case let .string(s): s
        case let .array(a): a.map(\.any)
        case let .object(o): o.mapValues(\.any)
        }
    }

    init(any value: Any?) {
        switch value {
        case nil, is NSNull: self = .null
        case let n as NSNumber:
            self = CFGetTypeID(n) == CFBooleanGetTypeID() ? .bool(n.boolValue) : .number(n.doubleValue)
        case let s as String: self = .string(s)
        case let a as [Any]: self = .array(a.map { JSON(any: $0) })
        case let o as [String: Any]: self = .object(o.mapValues { JSON(any: $0) })
        default: self = .null
        }
    }

    subscript(key: String) -> JSON? {
        if case let .object(o) = self { return o[key] }
        return nil
    }

    var string: String? {
        if case let .string(s) = self { return s }
        return nil
    }
}

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
    var version: String?
    var artists: [ArtistRef]
    var album: AlbumRef?
    var durationMs: Int?
    var liked: Bool?
    /// Трек, как его прислал сервер
    var raw: JSON = .null

    enum CodingKeys: String, CodingKey { case id, title, version, artists, album, durationMs, liked }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        title = try c.decode(String.self, forKey: .title)
        version = try? c.decodeIfPresent(String.self, forKey: .version)
        artists = (try? c.decodeIfPresent([ArtistRef].self, forKey: .artists)) ?? []
        album = try? c.decodeIfPresent(AlbumRef.self, forKey: .album)
        durationMs = try? c.decodeIfPresent(Int.self, forKey: .durationMs)
        liked = try? c.decodeIfPresent(Bool.self, forKey: .liked)
        raw = (try? JSON(from: decoder)) ?? .null
    }

    func encode(to encoder: Encoder) throws {
        try raw.encode(to: encoder)
    }

    var artistLine: String { artists.map(\.name).joined(separator: ", ") }
    var fullTitle: String {
        guard let version, !version.isEmpty else { return title }
        // Версия бывает уже в скобках: «(Slowed)»
        return version.hasPrefix("(") || version.hasPrefix("[") ? "\(title) \(version)" : "\(title) (\(version))"
    }
    var cover: String? { album?.image?.large ?? album?.image?.medium ?? album?.image?.small }
    var thumb: String? { album?.image?.medium ?? album?.image?.small }
}

/// Что играет: альбом, плейлист, волна…
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

struct WaveReason: Decodable, Hashable {
    var text: String
}

struct WaveItem: Decodable {
    var track: Track
    var reason: WaveReason?
}

struct WaveSessionView: Decodable {
    var id: String
    var title: String
    var state: String?
    var queue: [WaveItem]
}

struct WaveItems: Decodable {
    var items: [WaveItem]
}

struct WavePreset: Decodable, Hashable, Identifiable {
    var id: String
    var name: String
    var icon: String?
    var settings: WaveSettings
}

struct PlaylistSummary: Decodable, Hashable, Identifiable {
    var id: String
    var title: String
    var tracksCount: Int?
    var covers: [ImageSet]?
    var image: ImageSet?

    var cover: String? { image?.medium ?? covers?.first?.medium ?? covers?.first?.small }
}

struct AlbumSummary: Decodable, Hashable, Identifiable {
    var id: String
    var title: String
    var artist: ArtistRef?
    var image: ImageSet?
}

struct ArtistSummary: Decodable, Hashable, Identifiable {
    var id: String
    var name: String
    var image: ImageSet?
}

struct SearchResults: Decodable {
    var tracks: [Track]
    var albums: [AlbumSummary]
    var artists: [ArtistSummary]
}

/// Элемент «Продолжить» с главной: альбом, плейлист или артист.
struct RecentItem: Decodable, Hashable, Identifiable {
    var type: String
    var id: String
    var title: String
    var subtitle: String?
    var cover: String?

    enum CodingKeys: String, CodingKey { case type, id, title, name, artist, image, covers }

    /// Из ссылки Top Shelf: альбом, плейлист или артист.
    init?(type: String, id: String, title: String) {
        guard ["album", "playlist", "artist"].contains(type) else { return nil }
        self.type = type
        self.id = id
        self.title = title
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        type = try c.decode(String.self, forKey: .type)
        id = try c.decode(String.self, forKey: .id)
        title = (try? c.decodeIfPresent(String.self, forKey: .title))
            ?? (try? c.decodeIfPresent(String.self, forKey: .name)) ?? ""
        subtitle = (try? c.decodeIfPresent(ArtistRef.self, forKey: .artist))?.name
        let image = try? c.decodeIfPresent(ImageSet.self, forKey: .image)
        let covers = try? c.decodeIfPresent([ImageSet].self, forKey: .covers)
        cover = image?.medium ?? covers?.first?.medium ?? image?.small
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

struct TrackList: Decodable {
    var items: [Track]
}

// MARK: Поток

/// `GET /api/tracks/{id}/stream?aac=true`
struct StreamReply: Decodable {
    var status: String
    var url: String?
    var durationMs: Int?
    var retryAfterMs: Int?
    var loudnessLufs: Double?
}

// MARK: Текст песни

struct LyricsLine: Decodable, Hashable {
    var tMs: Int
    var text: String
}

struct TrackLyrics: Decodable {
    var kind: String
    var lines: [LyricsLine]
    var plain: String?
}

// MARK: Вход на телевизоре

struct TvCode: Decodable {
    var deviceCode: String
    var userCode: String
    var verificationUrl: String
    var verificationUrlComplete: String
    var expiresAt: Date
    var intervalSecs: Int
}

struct TvUser: Decodable {
    var name: String
}

struct TvToken: Decodable {
    var status: String
    var token: String?
    var user: TvUser?
}

// MARK: Вечеринка и «Слушать вместе»

struct PartySettings: Codable, Hashable {
    var energetic: Bool
    var noArtistRepeat: Bool
    var dj: Bool
}

struct Party: Decodable, Hashable {
    var active: Bool
    var settings: PartySettings
}

struct JamGuest: Decodable, Hashable, Identifiable {
    var id: String
    var name: String
    var online: Bool
}

struct JamTrackRef: Decodable, Hashable {
    var id: String
}

struct JamRequest: Decodable, Hashable {
    var track: JamTrackRef
    var guestName: String
    var status: String
}

struct Jam: Decodable, Hashable {
    var id: String
    var url: String
    var guests: [JamGuest]
    var requests: [JamRequest]
}

struct JamOverview: Decodable {
    var jam: Jam?
}

// MARK: «Где играть»

struct Capabilities: Codable, Hashable {
    var volume: Bool?
    var dj: Bool?
    var remote: Bool?
}

struct ConnectDevice: Codable, Hashable, Identifiable {
    var id: String
    var name: String
    var kind: String
    var platform: String
    var capabilities: Capabilities
    var online: Bool
    var active: Bool
}

/// Элемент очереди: `uid` — id элемента у активного устройства (команды ссылаются на него).
struct QueueEntry: Codable, Hashable, Identifiable {
    var uid: String
    var track: Track
    /// manual | context | wave
    var source: String
    var reason: String?

    var id: String { uid }

    init(track: Track, source: String, reason: String? = nil) {
        uid = UUID().uuidString.lowercased()
        self.track = track
        self.source = source
        self.reason = reason
    }

    enum CodingKeys: String, CodingKey { case uid, track, source, reason }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        uid = (try? c.decode(String.self, forKey: .uid)) ?? UUID().uuidString.lowercased()
        track = try c.decode(Track.self, forKey: .track)
        source = (try? c.decode(String.self, forKey: .source)) ?? "context"
        reason = try? c.decodeIfPresent(String.self, forKey: .reason)
    }

    var json: [String: Any] {
        var out: [String: Any] = ["uid": uid, "track": track.raw.any, "source": source]
        if let reason { out["reason"] = reason }
        return out
    }
}

/// Снимок состояния активного устройства (чужого — для сцены; своего — при переносе сюда).
struct PlaybackState: Decodable, Equatable {
    var current: QueueEntry?
    var manual: [QueueEntry] = []
    var upcoming: [QueueEntry] = []
    var history: [QueueEntry] = []
    var context: PlayContext?
    var playing = false
    var pending: String?
    var positionMs = 0
    var durationMs = 0
    var volume = 1.0
    var shuffle = false
    var repeatMode = "off"
    var waveSessionId: String?
    var wave: WaveSettings?
    var at: Date?

    enum CodingKeys: String, CodingKey {
        case current, manual, upcoming, history, context, playing, pending, positionMs, durationMs, volume, shuffle
        case repeatMode = "repeat"
        case waveSessionId, wave, at
    }

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        current = try? c.decodeIfPresent(QueueEntry.self, forKey: .current)
        manual = (try? c.decodeIfPresent([QueueEntry].self, forKey: .manual)) ?? []
        upcoming = (try? c.decodeIfPresent([QueueEntry].self, forKey: .upcoming)) ?? []
        history = (try? c.decodeIfPresent([QueueEntry].self, forKey: .history)) ?? []
        context = try? c.decodeIfPresent(PlayContext.self, forKey: .context)
        playing = (try? c.decodeIfPresent(Bool.self, forKey: .playing)) ?? false
        pending = try? c.decodeIfPresent(String.self, forKey: .pending)
        positionMs = (try? c.decodeIfPresent(Int.self, forKey: .positionMs)) ?? 0
        durationMs = (try? c.decodeIfPresent(Int.self, forKey: .durationMs)) ?? 0
        volume = (try? c.decodeIfPresent(Double.self, forKey: .volume)) ?? 1
        shuffle = (try? c.decodeIfPresent(Bool.self, forKey: .shuffle)) ?? false
        repeatMode = (try? c.decodeIfPresent(String.self, forKey: .repeatMode)) ?? "off"
        waveSessionId = try? c.decodeIfPresent(String.self, forKey: .waveSessionId)
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
