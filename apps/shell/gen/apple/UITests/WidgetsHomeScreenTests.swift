import XCTest

/// Виджеты на домашнем экране симулятора: приложение играет тестовую очередь (громкость 0,
/// `make test-audio-server`), виджеты «Сейчас играет», «Моя волна» и «Недавнее» добавляются через
/// галерею виджетов, снимки — design/shots/widgets/sim. Нажатие ⏯ на виджете ставит музыку на паузу
/// в процессе приложения (App Intent).
final class WidgetsHomeScreenTests: XCTestCase {
    let shots = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("../../../../../design/shots/widgets/sim")
        .standardizedFileURL

    let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")

    func save(_ name: String, tree: Bool = false) {
        try? FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: shots.appendingPathComponent("\(name).png"))
        if tree {
            try? springboard.debugDescription.write(
                to: shots.appendingPathComponent("\(name).txt"), atomically: true, encoding: .utf8)
        }
    }

    private func any(_ labels: [String]) -> XCUIElement {
        springboard.descendants(matching: .any)
            .matching(NSPredicate(format: "label IN %@ OR identifier IN %@", labels, labels))
            .firstMatch
    }

    /// Галерея виджетов → DalyWave → размер (свайпом) → «Добавить виджет».
    private func addWidget(page: Int, name: String) {
        springboard.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.7)).press(forDuration: 1.6)
        sleep(2)
        let edit = any(["Edit", "Изменить"])
        if edit.waitForExistence(timeout: 3) {
            edit.tap()
            sleep(2)
        }
        save("edit-\(name)", tree: true)
        let add = any(["Add Widget", "Добавить виджет", "Add widget"])
        if add.waitForExistence(timeout: 3) { add.tap() } else { save("no-add-\(name)", tree: true) }
        sleep(2)
        let search = springboard.searchFields.firstMatch
        if search.waitForExistence(timeout: 5) {
            search.tap()
            search.typeText("DalyWave")
            sleep(2)
        }
        save("gallery-\(name)", tree: true)
        let app = springboard.cells.matching(NSPredicate(format: "label == 'DalyWave'")).firstMatch
        if app.waitForExistence(timeout: 5) { app.tap() }
        sleep(2)
        for _ in 0..<page {
            springboard.swipeLeft()
            sleep(1)
        }
        save("pick-\(name)", tree: true)
        let confirm = springboard.buttons.matching(
            NSPredicate(format: "label CONTAINS 'Add Widget' OR label CONTAINS 'Добавить виджет'")).firstMatch
        if confirm.waitForExistence(timeout: 3) {
            confirm.tap()
        } else {
            springboard.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.9)).tap()
        }
        sleep(3)
        save("added-\(name)", tree: true)
        // Выйти из правки домашнего экрана
        let done = any(["Done", "Готово"])
        if done.waitForExistence(timeout: 3) { done.tap() }
        sleep(2)
    }

    func testWidgetsOnHomeScreen() throws {
        let app = XCUIApplication()
        app.launchEnvironment = ["DW_VOLUME": "0", "DALYWAVE_SMOKE": "http://127.0.0.1:8420"]
        app.launch()
        sleep(10)
        XCUIDevice.shared.press(.home)
        sleep(2)
        save("home-before", tree: true)
        // Страницы галереи DalyWave: «Сейчас играет» small, medium, «Моя волна» small, medium,
        // «Недавнее» medium, large
        addWidget(page: 4, name: "recent-medium")
        addWidget(page: 2, name: "wave-small")
        addWidget(page: 1, name: "now-medium")
        sleep(3)
        save("home-widgets", tree: true)
    }

    /// После testWidgetsOnHomeScreen (виджеты уже на экране): ⏯ на «Сейчас играет» — команда
    /// доходит до движка в процессе приложения, виджет перерисовывается.
    func testWidgetsTapPlayPause() throws {
        let app = XCUIApplication()
        app.launchEnvironment = ["DW_VOLUME": "0", "DALYWAVE_SMOKE": "http://127.0.0.1:8420"]
        app.launch()
        sleep(8)
        XCUIDevice.shared.press(.home)
        sleep(3)
        save("tap-before", tree: true)
        // Кнопка ⏯ среднего виджета вверху экрана — по координатам: у кнопок виджета нет подписей
        springboard.coordinate(withNormalizedOffset: CGVector(dx: 0.527, dy: 0.24)).tap()
        sleep(4)
        save("tap-after")
        springboard.coordinate(withNormalizedOffset: CGVector(dx: 0.527, dy: 0.24)).tap()
        sleep(4)
        save("tap-again")
    }
}
