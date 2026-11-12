import Intents

/// SiriKit, запросы музыки (`INPlayMediaIntent`): «Привет, Siri, включи Lowborn в DalyWave» — на
/// iPhone и на HomePod, если DalyWave выбран музыкальным сервисом по умолчанию в «Доме».
///
/// Расширение не ходит в сеть и не знает токена: запрос Siri превращается в команду (JSON, тот же,
/// что у App Intents — apps/shell/src/siri.rs) в id элемента, ответ — `.handleInApp`. Система будит
/// приложение в фоне, оно ищет в каталоге само и играет; на HomePod звук уходит по AirPlay 2.
final class IntentHandler: INExtension, INPlayMediaIntentHandling {
    override func handler(for intent: INIntent) -> Any {
        self
    }

    func resolveMediaItems(
        for intent: INPlayMediaIntent,
        with completion: @escaping ([INPlayMediaMediaItemResolutionResult]) -> Void
    ) {
        let request = SiriRequest(search: intent.mediaSearch)
        let item = INMediaItem(
            identifier: request.json,
            title: request.title,
            type: request.type,
            artwork: nil
        )
        completion([INPlayMediaMediaItemResolutionResult.success(with: item)])
    }

    func handle(intent: INPlayMediaIntent, completion: @escaping (INPlayMediaIntentResponse) -> Void) {
        completion(INPlayMediaIntentResponse(code: .handleInApp, userActivity: nil))
    }
}

/// Запрос Siri → команда приложения.
struct SiriRequest {
    let command: [String: Any]
    let title: String
    let type: INMediaItemType

    var json: String {
        guard let data = try? JSONSerialization.data(withJSONObject: command, options: [.sortedKeys]) else {
            return #"{"action":"wave"}"#
        }
        return String(data: data, encoding: .utf8) ?? #"{"action":"wave"}"#
    }

    /// «Моя волна» по-русски и по-английски — то, что Siri слышит как название
    private static let waveWords = ["волн", "wave", "моя музыка", "мою музыку", "my music", "что-нибудь", "something"]
    private static let likedWords = ["любим", "нравится", "liked", "favorite", "favourite"]
    /// Настроения Siri (`moodNames`) → настроение волны
    private static let moods: [(String, String)] = [
        ("бодр", "energetic"), ("энерг", "energetic"), ("energ", "energetic"), ("upbeat", "energetic"),
        ("весел", "happy"), ("весёл", "happy"), ("радост", "happy"), ("happy", "happy"), ("cheer", "happy"),
        ("спокой", "calm"), ("расслаб", "calm"), ("calm", "calm"), ("chill", "calm"), ("relax", "calm"),
        ("груст", "sad"), ("печал", "sad"), ("sad", "sad"), ("melanch", "sad"),
    ]

    init(search: INMediaSearch?) {
        let name = (search?.mediaName ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let lower = name.lowercased()
        let artist = search?.artistName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let album = search?.albumName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let mood = (search?.moodNames ?? []).lazy.compactMap { Self.mood(of: $0) }.first
            ?? Self.mood(of: lower)

        if search?.reference == .currentlyPlaying && name.isEmpty {
            self.init(command: ["action": "resume"], title: "DalyWave", type: .music)
            return
        }
        if !name.isEmpty && Self.likedWords.contains { lower.contains($0) } {
            self.init(command: ["action": "play", "kind": "liked", "id": "liked"], title: name, type: .playlist)
            return
        }
        // Без названия («включи DalyWave», «включи мою музыку»), «волну», настроение — «Моя волна»
        let nothing = name.isEmpty && artist.isEmpty && album.isEmpty && (search?.genreNames ?? []).isEmpty
        if nothing || Self.waveWords.contains { lower.contains($0) } || (mood != nil && artist.isEmpty && album.isEmpty) {
            var command: [String: Any] = ["action": "wave"]
            if let mood { command["mood"] = mood }
            self.init(command: command, title: name.isEmpty ? "Моя волна" : name, type: .music)
            return
        }
        let kind: String?
        let query: String
        switch search?.mediaType ?? .unknown {
        case .playlist:
            kind = "playlist"
            query = name
        case .album:
            kind = "album"
            query = [album.isEmpty ? name : album, artist].filter { !$0.isEmpty }.joined(separator: " ")
        case .artist:
            kind = "artist"
            query = artist.isEmpty ? name : artist
        case .song:
            kind = "track"
            query = [name, artist].filter { !$0.isEmpty }.joined(separator: " ")
        default:
            kind = nil
            query = [name.isEmpty ? (album.isEmpty ? artist : album) : name, name.isEmpty ? "" : artist]
                .filter { !$0.isEmpty }
                .joined(separator: " ")
        }
        let text = query.isEmpty ? (search?.genreNames?.first ?? "") : query
        var command: [String: Any] = ["action": "search", "query": text]
        if let kind { command["kind"] = kind }
        self.init(command: command, title: text, type: search?.mediaType ?? .music)
    }

    private init(command: [String: Any], title: String, type: INMediaItemType) {
        self.command = command
        self.title = title
        self.type = type == .unknown ? .music : type
    }

    private static func mood(of text: String) -> String? {
        let lower = text.lowercased()
        return moods.first { lower.contains($0.0) }?.1
    }
}
