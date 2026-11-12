import XCTest

/// «Поделиться → DalyWave» на симуляторе: ссылка открыта в Safari, в меню «Поделиться» есть
/// DalyWave, выбор открывает приложение. Снимки — в design/shots/share/ репозитория.
final class ShareExtensionTests: XCTestCase {
    let shots = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("../../../../../design/shots/share")
        .standardizedFileURL

    func save(_ name: String, tree: XCUIApplication? = nil) {
        let png = XCUIScreen.main.screenshot().pngRepresentation
        try? FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        try? png.write(to: shots.appendingPathComponent("\(name).png"))
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        // Дерево элементов — чтобы поправить поиск кнопок, если Safari изменится
        if let tree {
            try? tree.debugDescription.write(
                to: shots.appendingPathComponent("\(name).txt"), atomically: true, encoding: .utf8)
        }
    }

    private func button(_ app: XCUIApplication, _ labels: [String]) -> XCUIElement {
        let predicate = NSPredicate(format: "label IN %@ OR identifier IN %@", labels, labels)
        return app.descendants(matching: .button).matching(predicate).firstMatch
    }

    @available(iOS 16.4, *)
    func testShareSheetOffersDalyWave() throws {
        let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")
        safari.launch()
        // Ссылка Deezer — такую «Поделиться → DalyWave» превращает в трек каталога
        safari.open(URL(string: "https://www.deezer.com/track/3135556")!)
        sleep(6)
        // Первый запуск Safari: приветствие и предложения — закрыть
        for label in ["Continue", "Продолжить", "Not Now", "Не сейчас", "Close", "Закрыть"] {
            let b = safari.buttons[label]
            if b.exists { b.tap() }
        }

        var share = button(safari, ["Share", "Поделиться", "ShareButton"])
        if !share.waitForExistence(timeout: 5) {
            // Новый Safari: «Поделиться» — в меню страницы «…»
            let more = button(safari, ["Page Menu", "Меню страницы", "More", "Ещё", "TabOverflowButton", "PageFormatMenuButton"])
            if more.waitForExistence(timeout: 5) { more.tap() }
            share = button(safari, ["Share", "Поделиться"])
        }
        if !share.waitForExistence(timeout: 5) {
            save("safari-no-share-button", tree: safari)
            XCTFail("нет кнопки «Поделиться» в Safari")
            return
        }
        share.tap()
        sleep(2)

        // В ряду приложений DalyWave может быть за краем — тогда «Ещё»
        var dalywave = safari.descendants(matching: .any).matching(NSPredicate(format: "label == 'DalyWave'")).firstMatch
        if !dalywave.waitForExistence(timeout: 5) {
            let more = safari.descendants(matching: .any).matching(NSPredicate(format: "label IN {'More', 'Ещё'}")).firstMatch
            if more.exists { more.tap(); sleep(2) }
            dalywave = safari.descendants(matching: .any).matching(NSPredicate(format: "label == 'DalyWave'")).firstMatch
        }
        save("share-sheet-iphone")
        XCTAssertTrue(dalywave.waitForExistence(timeout: 5), "нет DalyWave в меню «Поделиться»")
        dalywave.tap()

        // Расширение открывает приложение ссылкой dalywave://share?url=…
        let app = XCUIApplication(bundleIdentifier: "rest.daly.wave")
        let opened = app.wait(for: .runningForeground, timeout: 20)
        sleep(4)
        save("share-opened-app-iphone")
        XCTAssertTrue(opened, "приложение не открылось")
    }
}
