import XCTest

/// Проверки на симуляторе (`make ios-test`). Снимки экрана — в design/shots/ios/ репозитория.
final class LoginSheetTests: XCTestCase {
    /// design/shots/ios рядом с этим файлом: симулятор видит файловую систему Mac
    let shots = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("../../../../../design/shots/ios")
        .standardizedFileURL

    func save(_ name: String) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        try? FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        try? png.write(to: shots.appendingPathComponent("\(name).png"))
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// «Войти через daly.rest» открывает страницу входа поверх приложения
    /// (ASWebAuthenticationSession). До конца вход не проходит: нужен настоящий аккаунт.
    func testLoginOpensSignInSheet() throws {
        let app = XCUIApplication()
        app.launchEnvironment = ["DW_VOLUME": "0"]
        app.launch()
        let login = app.webViews.buttons.matching(NSPredicate(format: "label CONTAINS 'daly.rest'")).firstMatch
        XCTAssertTrue(login.waitForExistence(timeout: 30), "нет кнопки входа")
        save("login-iphone")
        login.tap()
        // Система спрашивает, можно ли сайту выполнить вход в приложение
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.buttons.matching(NSPredicate(format: "label IN {'Continue', 'Продолжить'}")).firstMatch
        if allow.waitForExistence(timeout: 15) {
            save("login-consent-iphone")
            allow.tap()
        }
        // Страница входа daly.rest в системном окне браузера
        sleep(10)
        save("login-sheet-iphone")
    }
}
