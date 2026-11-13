import XCTest

/// «Назад» смахиванием от левого края (src/ios.rs, `install_back_swipe`; где жест включён — решает
/// интерфейс, apps/ui/app/plugins/back-swipe.client.ts).
///
/// Приложение запускает не тест, а скрипт снаружи (`xcrun simctl launch` с токеном устройства
/// локального сервера, `DALYWAVE_DEV_TOKEN`): под XCTest вошедшее приложение на симуляторе не
/// доходит до первой отрисовки (движок звука ждёт CoreAudio), а дерево доступности WebView с живой
/// волной XCTest разбирает дольше минуты. Поэтому тест только касается экрана — по координатам,
/// через домашний экран — и снимает кадры; середину жеста снимает тот же скрипт
/// (`xcrun simctl io … screenshot`), XCUIScreen во время касания недоступен.
///
/// Договор со скриптом через папку снимков (`TEST_RUNNER_SHOTS_DIR`): тест кладёт `ready` и ждёт
/// `launched`. Без папки тест пропускается — в `make ios-test` он не участвует.
///
/// Ручной, без проверок результата: на симуляторе XCTest показывает интерактивную часть (страница
/// едет за пальцем, под ней — предыдущая), но WebKit не завершает переход после синтетического
/// касания — так же и в Safari на странице с pushState, тогда как UINavigationController в
/// «Настройках» тот же жест завершает. Сам переход назад проверяется на устройстве (IOS.md, п. 14).
final class BackSwipeTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")

    var shots: URL!

    func save(_ name: String) {
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: shots.appendingPathComponent("\(name).png"))
    }

    func point(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate {
        springboard.coordinate(withNormalizedOffset: CGVector(dx: x, dy: y))
    }

    /// Смахнуть от левого края: `to` — доля ширины экрана, `hold` — подержать палец в конце
    func edgeSwipe(to: CGFloat, velocity: XCUIGestureVelocity, hold: TimeInterval = 0) {
        let start = point(0, 0.5).withOffset(CGVector(dx: 2, dy: 0))
        start.press(forDuration: 0.05, thenDragTo: point(to, 0.5), withVelocity: velocity, thenHoldForDuration: hold)
    }

    func testEdgeSwipeGoesBack() throws {
        guard let dir = env["SHOTS_DIR"] else { throw XCTSkip("ручной тест: нужны TEST_RUNNER_SHOTS_DIR и скрипт запуска") }
        shots = URL(fileURLWithPath: dir)
        try FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        FileManager.default.createFile(atPath: shots.appendingPathComponent("ready").path, contents: nil)
        let launched = shots.appendingPathComponent("launched").path
        for _ in 0..<120 where !FileManager.default.fileExists(atPath: launched) { sleep(1) }
        XCTAssertTrue(FileManager.default.fileExists(atPath: launched), "скрипт не запустил приложение")
        // Системные вопросы (Siri, уведомления) — «Не разрешать»
        for label in ["Don’t Allow", "Don't Allow", "Не разрешать"] {
            let button = springboard.buttons[label]
            if button.exists { button.tap() }
        }
        sleep(1)
        save("00-home")

        // Нижняя вкладка «Медиатека» (правая из трёх)
        point(0.83, 0.945).tap()
        sleep(3)
        save("01-library")

        // Корневая вкладка: позади главная, но жест ничего не делает
        edgeSwipe(to: 0.8, velocity: .fast)
        sleep(2)
        save("02-library-after-swipe")

        // Первая строка списка «Недавние» — вложенная страница
        point(0.5, 0.445).tap()
        sleep(4)
        save("03-nested")

        // Середина жеста: палец держит страницу на полпути (кадры — снаружи) и отпускает
        FileManager.default.createFile(atPath: shots.appendingPathComponent("hold").path, contents: nil)
        edgeSwipe(to: 0.5, velocity: .slow, hold: 3)
        sleep(2)
        save("04-after-half-swipe")

        // Полный быстрый жест
        point(0.5, 0.445).tap()
        sleep(4)
        save("05-nested-again")
        edgeSwipe(to: 0.95, velocity: .fast)
        sleep(2)
        save("06-back-fast")

        // Отмена: короткий медленный жест — страница возвращается на место
        point(0.5, 0.445).tap()
        sleep(4)
        edgeSwipe(to: 0.15, velocity: .slow)
        sleep(2)
        save("07-cancelled")

        // Лист поверх вложенной страницы («⋯» справа вверху): жест не должен уводить назад
        point(0.93, 0.1).tap()
        sleep(2)
        save("08-sheet")
        edgeSwipe(to: 0.9, velocity: .fast)
        sleep(2)
        save("09-sheet-after-swipe")
    }
}
