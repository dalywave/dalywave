import Foundation

enum ApiError: Error {
    /// Ключ отозван или истёк: нужен новый от iPhone
    case unauthorized
    case offline
    case server(Int)
}

/// Запросы к серверу DalyWave от имени часов (`Authorization: Bearer dwd_…`).
struct Api {
    let credentials: Credentials

    static let session: URLSession = {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 15
        config.waitsForConnectivity = false
        config.urlCache = URLCache(memoryCapacity: 4 << 20, diskCapacity: 24 << 20)
        return URLSession(configuration: config)
    }()

    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .custom { decoder in
            let s = try decoder.singleValueContainer().decode(String.self)
            if let date = Api.isoFractional.date(from: s) ?? Api.iso.date(from: s) { return date }
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "date"))
        }
        return d
    }()

    nonisolated(unsafe) static let isoFractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    nonisolated(unsafe) static let iso = ISO8601DateFormatter()

    func url(_ path: String) -> URL? {
        URL(string: credentials.server + path)
    }

    private func request(_ method: String, _ path: String, body: Data? = nil) throws -> URLRequest {
        guard let url = url(path) else { throw ApiError.server(0) }
        var req = URLRequest(url: url)
        req.httpMethod = method
        req.setValue("Bearer \(credentials.token)", forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        if let body {
            req.httpBody = body
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        return req
    }

    private func send(_ req: URLRequest) async throws -> Data {
        let (data, res): (Data, URLResponse)
        do {
            (data, res) = try await Api.session.data(for: req)
        } catch {
            throw ApiError.offline
        }
        let status = (res as? HTTPURLResponse)?.statusCode ?? 0
        if status == 401 { throw ApiError.unauthorized }
        guard (200..<300).contains(status) else { throw ApiError.server(status) }
        return data
    }

    func get<T: Decodable>(_ path: String, as: T.Type = T.self) async throws -> T {
        let data = try await send(request("GET", path))
        return try Api.decoder.decode(T.self, from: data)
    }

    func put(_ path: String) async throws {
        _ = try await send(request("PUT", path))
    }

    func delete(_ path: String) async throws {
        _ = try await send(request("DELETE", path))
    }

    /// Картинка с сервера — с тем же ключом (обложки открыты, но так проще и с кешем).
    func image(_ path: String) async -> Data? {
        guard let req = try? request("GET", path) else { return nil }
        return try? await send(req)
    }

    // MARK: Медиатека

    func likedTracks() async throws -> [Track] {
        try await get("/api/library/tracks?limit=200", as: Page<TrackRow>.self).items.compactMap(\.track)
    }

    func playlists() async throws -> [PlaylistSummary] {
        try await get("/api/playlists", as: Page<PlaylistSummary>.self).items
    }

    func playlistTracks(_ id: String) async throws -> [Track] {
        try await get("/api/playlists/\(id)", as: Page<TrackRow>.self).items.compactMap(\.track)
    }

    func albumTracks(_ id: String) async throws -> [Track] {
        try await get("/api/albums/\(id)", as: AlbumDetail.self).tracks
    }

    func artistTracks(_ id: String) async throws -> [Track] {
        try await get("/api/artists/\(id)", as: ArtistDetail.self).topTracks
    }

    /// «Продолжить» с главной: недавние альбомы и плейлисты.
    func recent() async throws -> [RecentItem] {
        let home = try await get("/api/home", as: Home.self)
        let items = home.shelves.first { $0.id == "continue" }?.items.compactMap(\.value) ?? []
        return items.filter { ["album", "playlist", "artist"].contains($0.type) }
    }

    func wavePresets() async throws -> [WavePreset] {
        try await get("/api/wave/presets", as: Page<WavePreset>.self).items
    }

    func setLiked(_ trackId: String, _ liked: Bool) async throws {
        if liked {
            try await put("/api/library/tracks/\(trackId)")
        } else {
            try await delete("/api/library/tracks/\(trackId)")
        }
    }
}
