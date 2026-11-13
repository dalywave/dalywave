import AppIntents
import Foundation

// Фокусирование (Focus Filters, iOS 16+): «Настройки» → «Фокусирование» → режим → «Фильтры фокусирования»
// → DalyWave → сочетание волны. Система вызывает фильтр, когда фокус включается (с выбранным сочетанием)
// и выключается (без него) — даже если приложение не запущено. Выбор запоминается здесь (UserDefaults),
// а Rust (apps/shell/src/focus.rs) получает его вызовом `dalywave_focus_changed` и при запуске —
// через `+[DalyWaveFocus current]`. Применять ли его (настройка «Фокусирование»), решает интерфейс.
// Сочетания — те же, что у виджета «Моя волна»: их пишет Rust в контейнер App Group (WidgetData.swift).

@_silgen_name("dalywave_focus_changed")
private func dalywaveFocusChanged(_ json: UnsafePointer<CChar>?) -> Int32

@objc(DalyWaveFocus)
final class DalyWaveFocusBridge: NSObject {
    static let key = "dw.focus.wave"

    /// JSON выбранного сочетания (`{"id","name","mood",…}`) или nil — фокус без фильтра DalyWave.
    @objc static func current() -> NSString? {
        UserDefaults.standard.string(forKey: key).map { $0 as NSString }
    }

    static func apply(_ preset: WidgetPreset?) {
        var json = "null"
        if let preset {
            var object: [String: Any] = ["id": preset.id, "name": preset.name]
            if let v = preset.mood { object["mood"] = v }
            if let v = preset.activity { object["activity"] = v }
            if let v = preset.character { object["character"] = v }
            if let v = preset.language { object["language"] = v }
            if let data = try? JSONSerialization.data(withJSONObject: object),
               let text = String(data: data, encoding: .utf8)
            {
                json = text
            }
        }
        if json == "null" {
            UserDefaults.standard.removeObject(forKey: key)
        } else {
            UserDefaults.standard.set(json, forKey: key)
        }
        _ = json.withCString { dalywaveFocusChanged($0) }
    }

    /// Сочетания для выбора: сохранённые слушателем или три занятия, как на главной.
    static var presets: [WidgetPreset] {
        if let saved = WidgetStore.library?.presets, !saved.isEmpty { return saved }
        return [
            WidgetPreset(id: "work", name: String(localized: "Work"), icon: "laptop", mood: nil, activity: "work", character: nil, language: nil),
            WidgetPreset(id: "road", name: String(localized: "Road"), icon: "car", mood: nil, activity: "road", character: nil, language: nil),
            WidgetPreset(id: "night", name: String(localized: "Night"), icon: "moon-stars", mood: nil, activity: "sleep", character: nil, language: nil),
        ]
    }
}

/// Сочетание волны для фильтра фокуса. Всё нужное — в самом id, чтобы фильтр работал и тогда, когда
/// сочетание с тех пор переименовали или удалили.
@available(iOS 16.0, *)
struct WavePresetEntity: AppEntity {
    static var typeDisplayRepresentation: TypeDisplayRepresentation = "Wave"
    static var defaultQuery = WavePresetQuery()

    let id: String
    let preset: WidgetPreset

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(preset.name)", image: .init(systemName: "waveform"))
    }

    init(_ preset: WidgetPreset) {
        self.preset = preset
        let parts = [preset.id, preset.name, preset.mood ?? "", preset.activity ?? "", preset.character ?? "", preset.language ?? ""]
        id = parts.map { $0.replacingOccurrences(of: "|", with: "/") }.joined(separator: "|")
    }

    init?(id: String) {
        let p = id.split(separator: "|", maxSplits: 5, omittingEmptySubsequences: false).map(String.init)
        guard p.count == 6, !p[0].isEmpty else { return nil }
        let opt = { (s: String) in s.isEmpty ? nil : s }
        preset = WidgetPreset(id: p[0], name: p[1], icon: "sparkle", mood: opt(p[2]), activity: opt(p[3]),
                              character: opt(p[4]), language: opt(p[5]))
        self.id = id
    }
}

@available(iOS 16.0, *)
struct WavePresetQuery: EntityQuery {
    func entities(for identifiers: [WavePresetEntity.ID]) async throws -> [WavePresetEntity] {
        identifiers.compactMap { WavePresetEntity(id: $0) }
    }

    func suggestedEntities() async throws -> [WavePresetEntity] {
        DalyWaveFocusBridge.presets.map { WavePresetEntity($0) }
    }
}

@available(iOS 16.0, *)
struct DalyWaveFocusFilter: SetFocusFilterIntent {
    static var title: LocalizedStringResource = "Set My Wave"
    static var description: IntentDescription? = IntentDescription("My Wave plays the chosen mix while this Focus is on.")

    @Parameter(title: "Wave")
    var wave: WavePresetEntity?

    var displayRepresentation: DisplayRepresentation {
        if let wave {
            return DisplayRepresentation(title: "\(wave.preset.name)", subtitle: "My Wave")
        }
        return DisplayRepresentation(title: "My Wave")
    }

    func perform() async throws -> some IntentResult {
        DalyWaveFocusBridge.apply(wave?.preset)
        return .result()
    }
}
