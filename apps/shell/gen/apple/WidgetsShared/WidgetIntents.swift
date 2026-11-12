import AppIntents
import Foundation

// Кнопки виджетов (iOS 17+): пауза и продолжить, следующий трек, «Моя волна» и её сочетания.
// AudioPlaybackIntent из виджета система выполняет в процессе приложения (тот же тип есть в обоих
// target) и запускает его в фоне, если оно не запущено. Там WidgetActions.perform отдаёт команду
// Rust (siri.rs — тот же JSON, что у Siri), в расширении — заглушка: туда выполнение не приходит.

@available(iOS 17.0, *)
struct WidgetPlayPauseIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Play or Pause"
    static var isDiscoverable = false

    /// Что было на виджете: играет — значит, пауза
    @Parameter(title: "Playing")
    var playing: Bool

    init() {}

    init(playing: Bool) {
        self.playing = playing
    }

    func perform() async throws -> some IntentResult {
        try WidgetActions.perform(["action": playing ? "pause" : "resume"])
        return .result()
    }
}

@available(iOS 17.0, *)
struct WidgetNextIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Next Track"
    static var isDiscoverable = false

    init() {}

    func perform() async throws -> some IntentResult {
        try WidgetActions.perform(["action": "next"])
        return .result()
    }
}

/// «Моя волна»: без параметров — с настройками, что выбраны в приложении; с ними — сочетание.
@available(iOS 17.0, *)
struct WidgetWaveIntent: AudioPlaybackIntent {
    static var title: LocalizedStringResource = "Play My Wave"
    static var isDiscoverable = false

    @Parameter(title: "Mood")
    var mood: String?

    @Parameter(title: "Activity")
    var activity: String?

    @Parameter(title: "Character")
    var character: String?

    @Parameter(title: "Language")
    var language: String?

    init() {}

    init(preset: WidgetPreset?) {
        mood = preset?.mood
        activity = preset?.activity
        character = preset?.character
        language = preset?.language
    }

    func perform() async throws -> some IntentResult {
        var command: [String: Any] = ["action": "wave"]
        if let mood { command["mood"] = mood }
        if let activity { command["activity"] = activity }
        if let character { command["character"] = character }
        if let language { command["language"] = language }
        try WidgetActions.perform(command)
        return .result()
    }
}
