import Foundation

enum ApiError: Error {
    /// Ключ отозван или истёк: нужен вход заново
    case unauthorized
    case offline
    case server(Int)
}

/// Запросы к серверу DalyWave от имени телевизора (`Authorization: Bearer dwd_…`).
struct Api {
    let server: String
    let token: String?

    init(credentials: Credentials) {
        server = credentials.server
        token = credentials.token
    }

    /// Без входа: код для экрана и опрос.
    init(server: String) {
        self.server = server
        token = nil
    }

    static let session: URLSession = {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 20
        config.waitsForConnectivity = false
        config.urlCache = URLCache(memoryCapacity: 16 << 20, diskCapacity: 128 << 20)
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
        URL(string: server + path)
    }

    /// Адрес картинки: пути сервера (`/api/images/…`) — с адресом сервера, остальное как есть.
    func imageURL(_ path: String) -> URL? {
        path.hasPrefix("http") ? URL(string: path) : url(path)
    }

    private func request(_ method: String, _ path: String, json: Any? = nil) throws -> URLRequest {
        guard let url = url(path) else { throw ApiError.server(0) }
        var req = URLRequest(url: url)
        req.httpMethod = method
        if let token { req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        if let json {
            req.httpBody = try JSONSerialization.data(withJSONObject: json)
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        return req
    }

    @discardableResult
    private func send(_ req: URLRequest) async throws -> (Data, Int) {
        let (data, res): (Data, URLResponse)
        do {
            (data, res) = try await Api.session.data(for: req)
        } catch {
            throw ApiError.offline
        }
        let status = (res as? HTTPURLResponse)?.statusCode ?? 0
        if status == 401 { throw ApiError.unauthorized }
        guard (200..<300).contains(status) else { throw ApiError.server(status) }
        return (data, status)
    }

    func get<T: Decodable>(_ path: String, as: T.Type = T.self) async throws -> T {
        let (data, _) = try await send(request("GET", path))
        return try Api.decoder.decode(T.self, from: data)
    }

    func call<T: Decodable>(_ method: String, _ path: String, _ json: Any?, as: T.Type = T.self) async throws -> T {
        let (data, _) = try await send(request(method, path, json: json))
        return try Api.decoder.decode(T.self, from: data)
    }

    func call(_ method: String, _ path: String, _ json: Any? = nil) async throws {
        try await send(request(method, path, json: json))
    }

    func image(_ path: String) async -> Data? {
        guard let url = imageURL(path) else { return nil }
        var req = URLRequest(url: url)
        if let token { req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        guard let (data, res) = try? await Api.session.data(for: req),
              (res as? HTTPURLResponse)?.statusCode == 200
        else { return nil }
        return data
    }

    // MARK: Вход на телевизоре

    func tvCode(deviceName: String) async throws -> TvCode {
        try await call("POST", "/api/auth/tv/code", ["platform": "tvos", "deviceName": deviceName])
    }

    /// `nil` — код больше не действует (истёк или уже использован).
    func tvToken(_ deviceCode: String, appVersion: String) async throws -> TvToken? {
        do {
            return try await call("POST", "/api/auth/tv/token", ["deviceCode": deviceCode, "appVersion": appVersion])
        } catch ApiError.server(400) {
            return nil
        }
    }

    // MARK: Воспроизведение

    func stream(_ trackId: String) async throws -> StreamReply {
        // 202 тоже ответ: `preparing`, `acquiring`, `waiting_for_downloader`
        try await get("/api/tracks/\(trackId)/stream?aac=true&network=wifi")
    }

    func lookup(_ ids: [String]) async throws -> [Track] {
        try await call("POST", "/api/tracks/lookup", ["ids": Array(ids.prefix(500))], as: TrackList.self).items
    }

    func startWave(_ settings: [String: Any]) async throws -> WaveSessionView {
        try await call("POST", "/api/wave/sessions", ["settings": settings])
    }

    func waveSession(_ id: String) async throws -> WaveSessionView {
        try await get("/api/wave/sessions/\(id)")
    }

    func waveNext(_ id: String, count: Int) async throws -> [WaveItem] {
        try await get("/api/wave/sessions/\(id)/next?count=\(count)", as: WaveItems.self).items
    }

    func events(_ plays: [[String: Any]]) async throws {
        try await call("POST", "/api/events", ["plays": plays, "feedback": [Any]()])
    }

    func lyrics(_ trackId: String) async throws -> TrackLyrics {
        try await get("/api/tracks/\(trackId)/lyrics")
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

    func recent() async throws -> [RecentItem] {
        let home = try await get("/api/home", as: Home.self)
        let items = home.shelves.first { $0.id == "continue" }?.items.compactMap(\.value) ?? []
        return items.filter { ["album", "playlist", "artist"].contains($0.type) }
    }

    func wavePresets() async throws -> [WavePreset] {
        try await get("/api/wave/presets", as: Page<WavePreset>.self).items
    }

    func search(_ q: String) async throws -> SearchResults {
        let query = q.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed.subtracting(["&", "+", "="])) ?? ""
        return try await get("/api/search?q=\(query)&limit=20")
    }

    func setLiked(_ trackId: String, _ liked: Bool) async throws {
        try await call(liked ? "PUT" : "DELETE", "/api/library/tracks/\(trackId)")
    }

    // MARK: Вечеринка

    func party() async throws -> Party {
        try await get("/api/party")
    }

    func updateParty(_ patch: [String: Any]) async throws -> Party {
        try await call("PATCH", "/api/party", patch)
    }

    func jam() async throws -> Jam? {
        try await get("/api/jam", as: JamOverview.self).jam
    }

    func logout() async {
        try? await call("POST", "/api/auth/logout")
    }
}
