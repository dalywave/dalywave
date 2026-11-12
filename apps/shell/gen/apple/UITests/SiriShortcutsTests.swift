import XCTest

/// Siri и «Команды» на симуляторе: фразы App Shortcuts DalyWave видны в приложении «Команды», а
/// плитка «Пауза» выполняется в процессе приложения (Rust, siri.rs) — музыка встаёт на паузу.
/// Звук — тестовый сервер (`make test-audio-server`) на громкости 0. Снимки — design/shots/siri.
final class SiriShortcutsTests: XCTestCase {
    let shots = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("../../../../../design/shots/siri")
        .standardizedFileURL

    func save(_ name: String, tree: XCUIApplication? = nil) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        try? FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        try? png.write(to: shots.appendingPathComponent("\(name).png"))
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let tree {
            try? tree.debugDescription.write(
                to: shots.appendingPathComponent("\(name).txt"), atomically: true, encoding: .utf8)
        }
    }

    private func any(_ app: XCUIApplication, _ labels: [String]) -> XCUIElement {
        app.descendants(matching: .any)
            .matching(NSPredicate(format: "label IN %@ OR identifier IN %@", labels, labels))
            .firstMatch
    }

    func testAppShortcutsRunInTheApp() throws {
        let app = XCUIApplication(bundleIdentifier: "rest.daly.wave")
        app.launchEnvironment = ["DW_VOLUME": "0", "DALYWAVE_SMOKE": "http://127.0.0.1:8420"]
        app.launch()
        sleep(6)

        let shortcuts = XCUIApplication(bundleIdentifier: "com.apple.shortcuts")
        shortcuts.launch()
        sleep(3)
        for label in ["Continue", "Продолжить", "Not Now", "Не сейчас"] {
            let b = shortcuts.buttons[label]
            if b.exists { b.tap(); sleep(1) }
        }
        // Плитки App Shortcuts — в «Библиотеке» под названием приложения (иногда — ниже, прокруткой)
        var section = any(shortcuts, ["DalyWave"])
        for _ in 0..<3 where !section.waitForExistence(timeout: 3) {
            let back = shortcuts.navigationBars.buttons.element(boundBy: 0)
            if back.exists, back.label != "Select" { back.tap() } else { shortcuts.swipeUp() }
            sleep(2)
            section = any(shortcuts, ["DalyWave"])
        }
        save("shortcuts-home", tree: shortcuts)
        XCTAssertTrue(section.waitForExistence(timeout: 5), "нет DalyWave в «Командах»")
        section.tap()
        sleep(2)
        save("shortcuts-dalywave", tree: shortcuts)

        let pause = any(shortcuts, ["Pause", "Пауза"])
        XCTAssertTrue(pause.waitForExistence(timeout: 5), "нет плитки «Пауза»")
        pause.tap()
        sleep(4)
        save("shortcuts-pause-ran", tree: shortcuts)
    }
}
