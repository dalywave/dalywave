import AppIntents
import Foundation

// Siri и «Команды» (iOS 17+): фразы App Shortcuts на русском и английском (ru.lproj, en.lproj —
// AppShortcuts.strings) и действия для приложения «Команды». Само действие делает Rust
// (apps/shell/src/siri.rs, статическая библиотека libapp.a): сюда — только описание для системы.
// Мост — C ABI, команда — JSON (тот же, что у расширения SiriKit и у события `siri:command`).

@_silgen_name("dalywave_siri_perform")
private func dalywaveSiriPerform(_ request: UnsafePointer<CChar>?) -> Int32

@_silgen_name("dalywave_siri_search")
private func dalywaveSiriSearch(_ request: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("dalywave_siri_free")
private func dalywaveSiriFree(_ pointer: UnsafeMutablePointer<CChar>?)

enum SiriBridge {
    private static func json(_ object: [String: Any]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: object) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// Выполнить команду. 0 — принято, 2 — не тот запрос, 3 — приложение ещё не запустилось.
    static func perform(_ command: [String: Any]) -> Int32 {
        guard let request = json(command) else { return 2 }
        return request.withCString { dalywaveSiriPerform($0) }
    }

    /// Найти в медиатеке и каталоге (`["query": …]`) или подсказки из медиатеки (`[:]`). Сеть —
    /// поэтому не в общем пуле Swift Concurrency.
    static func search(_ request: [String: Any]) async -> [[String: Any]] {
        guard let body = json(request) else { return [] }
        return await withCheckedContinuation { done in
            DispatchQueue.global(qos: .userInitiated).async {
                let out = body.withCString { dalywaveSiriSearch($0) }
                defer { dalywaveSiriFree(out) }
                guard let out,
                      let parsed = try? JSONSerialization.jsonObject(with: Data(String(cString: out).utf8)),
                      let list = parsed as? [[String: Any]]
                else {
                    done.resume(returning: [])
                    return
                }
                done.resume(returning: list)
            }
        }
    }
}

@available(iOS 17.0, *)
struct SiriBridgeError: Error, CustomLocalizedStringResourceConvertible {
    let code: Int32

    var localizedStringResource: LocalizedStringResource {
        code == 3 ? "DalyWave is still starting. Try again in a moment." : "DalyWave couldn't do that."
    }
}

@available(iOS 17.0, *)
private func run(_ command: [String: Any]) throws {
    let code = SiriBridge.perform(command)
    if code != 0 { throw SiriBridgeError(code: code) }
}

/// Настроение волны — те же значения, что в настройках волны интерфейса (`WaveMood`).
@available(iOS 17.0, *)
enum WaveMood: String, AppEnum {
    case energetic, happy, calm, sad

    static var typeDisplayRepresentation: TypeDisplayRepresentation = "Mood"
    static var caseDisplayRepresentations: [WaveMood: DisplayRepresentation] = [
        .energetic: DisplayRepresentation(title: "energetic", synonyms: ["upbeat"]),
        .happy: DisplayRepresentation(title: "happy", synonyms: ["cheerful"]),
        .calm: DisplayRepresentation(title: "calm", synonyms: ["chill"]),
        .sad: DisplayRepresentation(title: "sad", synonyms: ["melancholic"]),
    ]
}

/// Занятие — как `WaveActivity` интерфейса.
@available(iOS 17.0, *)
enum WaveActivity: String, AppEnum {
    case wake, road, work, sport, rest, sleep, party

    static var typeDisplayRepresentation: TypeDisplayRepresentation = "Activity"
    static var caseDisplayRepresentations: [WaveActivity: DisplayRepresentation] = [
        .wake: "waking up",
        .road: "the road",
        .work: "work",
        .sport: "a workout",
        .rest: "rest",
        .sleep: "sleep",
        .party: "a party",
    ]
}

@available(iOS 17.0, *)
struct PlayWaveIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Play My Wave"
    static var description = IntentDescription("Starts My Wave in DalyWave, with a mood or an activity if you like.")

    @Parameter(title: "Mood")
    var mood: WaveMood?

    @Parameter(title: "Activity")
    var activity: WaveActivity?

    init() {}

    init(mood: WaveMood? = nil, activity: WaveActivity? = nil) {
        self.mood = mood
        self.activity = activity
    }

    func perform() async throws -> some IntentResult {
        var command: [String: Any] = ["action": "wave"]
        if let mood { command["mood"] = mood.rawValue }
        if let activity { command["activity"] = activity.rawValue }
        try run(command)
        return .result()
    }
}

/// Плейлист, альбом или артист DalyWave: подсказки — из медиатеки, остальное — поиском по
/// каталогу сервера (через Rust, с токеном устройства).
@available(iOS 17.0, *)
struct MediaEntity: AppEntity {
    static var typeDisplayRepresentation: TypeDisplayRepresentation = "Music"
    static var defaultQuery = MediaQuery()

    /// `вид|id|название|подпись`: по нему сущность восстанавливается без сети
    let id: String
    let kind: String
    let serverId: String
    let title: String
    let subtitle: String

    var displayRepresentation: DisplayRepresentation {
        subtitle.isEmpty
            ? DisplayRepresentation(title: "\(title)")
            : DisplayRepresentation(title: "\(title)", subtitle: "\(subtitle)")
    }

    init?(id: String) {
        let parts = id.split(separator: "|", maxSplits: 3, omittingEmptySubsequences: false).map(String.init)
        guard parts.count == 4, !parts[0].isEmpty, !parts[1].isEmpty else { return nil }
        self.id = id
        kind = parts[0]
        serverId = parts[1]
        title = parts[2]
        subtitle = parts[3]
    }

    init?(found: [String: Any]) {
        guard let kind = found["kind"] as? String, let serverId = found["id"] as? String,
              let title = found["title"] as? String
        else { return nil }
        let subtitle = found["subtitle"] as? String ?? ""
        self.init(id: [kind, serverId, title.replacingOccurrences(of: "|", with: "/"), subtitle].joined(separator: "|"))
    }
}

@available(iOS 17.0, *)
struct MediaQuery: EntityStringQuery {
    func entities(for identifiers: [MediaEntity.ID]) async throws -> [MediaEntity] {
        identifiers.compactMap { MediaEntity(id: $0) }
    }

    func entities(matching string: String) async throws -> [MediaEntity] {
        await SiriBridge.search(["query": string]).compactMap { MediaEntity(found: $0) }
    }

    func suggestedEntities() async throws -> [MediaEntity] {
        await SiriBridge.search([:]).compactMap { MediaEntity(found: $0) }
    }
}

@available(iOS 17.0, *)
struct PlayMediaIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Play in DalyWave"
    static var description = IntentDescription("Plays a playlist, an album or an artist from DalyWave.")

    @Parameter(title: "Music")
    var item: MediaEntity

    @Parameter(title: "Shuffle", default: false)
    var shuffle: Bool

    init() {}

    func perform() async throws -> some IntentResult {
        try run(["action": "play", "kind": item.kind, "id": item.serverId, "title": item.title, "shuffle": shuffle])
        return .result()
    }
}

/// «Включи … в DalyWave» текстом (приложение «Команды»): найти и включить лучшее совпадение.
@available(iOS 17.0, *)
struct SearchAndPlayIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Find and Play in DalyWave"
    static var description = IntentDescription("Finds a playlist, an artist, an album or a song in DalyWave and plays it.")

    @Parameter(title: "Search")
    var query: String

    init() {}

    func perform() async throws -> some IntentResult {
        try run(["action": "search", "query": query])
        return .result()
    }
}

@available(iOS 17.0, *)
struct PauseIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Pause DalyWave"

    func perform() async throws -> some IntentResult {
        try run(["action": "pause"])
        return .result()
    }
}

@available(iOS 17.0, *)
struct ResumeIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Resume DalyWave"

    func perform() async throws -> some IntentResult {
        try run(["action": "resume"])
        return .result()
    }
}

@available(iOS 17.0, *)
struct NextTrackIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Next Track"

    func perform() async throws -> some IntentResult {
        try run(["action": "next"])
        return .result()
    }
}

@available(iOS 17.0, *)
struct PreviousTrackIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Previous Track"

    func perform() async throws -> some IntentResult {
        try run(["action": "previous"])
        return .result()
    }
}

@available(iOS 17.0, *)
struct LikeTrackIntent: AppIntent {
    static var title: LocalizedStringResource = "Like This Song"
    static var description = IntentDescription("Adds the song that is playing in DalyWave to your likes.")

    func perform() async throws -> some IntentResult & ProvidesDialog {
        try run(["action": "like"])
        return .result(dialog: "Added to your likes")
    }
}

@available(iOS 17.0, *)
struct DalyWaveAppShortcuts: AppShortcutsProvider {
    static var shortcutTileColor: ShortcutTileColor = .purple

    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: PlayWaveIntent(),
            phrases: [
                "Play my wave in \(.applicationName)",
                "Turn on my wave in \(.applicationName)",
                "Start my wave in \(.applicationName)",
                "Play \(\.$mood) wave in \(.applicationName)",
                "Play a wave for \(\.$activity) in \(.applicationName)",
            ],
            shortTitle: "My Wave",
            systemImageName: "waveform"
        )
        AppShortcut(
            intent: PlayMediaIntent(),
            phrases: [
                "Play \(\.$item) in \(.applicationName)",
                "Turn on \(\.$item) in \(.applicationName)",
            ],
            shortTitle: "Play",
            systemImageName: "play.fill"
        )
        AppShortcut(
            intent: PauseIntent(),
            phrases: [
                "Pause \(.applicationName)",
                "Pause the music in \(.applicationName)",
            ],
            shortTitle: "Pause",
            systemImageName: "pause.fill"
        )
        AppShortcut(
            intent: ResumeIntent(),
            phrases: [
                "Resume \(.applicationName)",
                "Continue the music in \(.applicationName)",
            ],
            shortTitle: "Resume",
            systemImageName: "play.circle"
        )
        AppShortcut(
            intent: NextTrackIntent(),
            phrases: [
                "Next track in \(.applicationName)",
                "Skip this song in \(.applicationName)",
            ],
            shortTitle: "Next",
            systemImageName: "forward.fill"
        )
        AppShortcut(
            intent: PreviousTrackIntent(),
            phrases: [
                "Previous track in \(.applicationName)",
            ],
            shortTitle: "Previous",
            systemImageName: "backward.fill"
        )
        AppShortcut(
            intent: LikeTrackIntent(),
            phrases: [
                "Like this song in \(.applicationName)",
                "I like this song in \(.applicationName)",
            ],
            shortTitle: "Like",
            systemImageName: "heart.fill"
        )
    }
}

/// Rust после входа (siri.rs, `prepare`): подсказки параметра «что включить» — плейлисты,
/// альбомы и артисты медиатеки — пересчитать.
@objc(DalyWaveShortcuts)
final class DalyWaveShortcutsBridge: NSObject {
    @objc static func refresh() {
        if #available(iOS 17.0, *) {
            DalyWaveAppShortcuts.updateAppShortcutParameters()
        }
    }
}
