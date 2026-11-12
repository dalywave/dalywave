import Foundation

/// Кнопки виджета выполняет приложение (AudioPlaybackIntent — в его процессе, см.
/// WidgetsShared/WidgetIntents.swift и Sources/dalywave-shell/DalyWaveWidgets.swift). Расширению
/// нечего делать: движок и токен — только у приложения.
@available(iOS 17.0, *)
enum WidgetActions {
    static func perform(_ command: [String: Any]) throws {}
}
