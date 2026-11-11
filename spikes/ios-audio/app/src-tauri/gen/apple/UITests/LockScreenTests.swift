import XCTest

/// Проверка экрана блокировки на симуляторе: приложение играет, экран блокируется на 65 с,
/// затем на экране блокировки ищется карточка «Сейчас играет» и нажимаются её кнопки.
/// Что команды дошли до движка, видно в журнале приложения (`remote command: …`).
final class LockScreenTests: XCTestCase {
    func log(_ s: String) {
        print("DW-UITEST \(ISO8601DateFormatter().string(from: Date())) \(s)")
    }

    func buttons(_ sb: XCUIApplication, matching words: [String]) -> [XCUIElement] {
        sb.buttons.allElementsBoundByIndex.filter { b in
            let l = b.label.lowercased()
            return words.contains { l.contains($0) }
        }
    }

    func testLockScreenNowPlayingAndRemoteCommands() throws {
        let app = XCUIApplication()
        app.launchEnvironment = ["DW_SCENARIO": "background", "DW_VOLUME": "0"]
        app.launch()
        sleep(12)
        log("locking")
        XCUIDevice.shared.perform(NSSelectorFromString("pressLockButton"))
        sleep(65)
        log("waking lock screen")
        XCUIDevice.shared.press(.home)
        sleep(3)
        let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        log("springboard tree:\n\(sb.debugDescription)")

        let texts = sb.staticTexts.allElementsBoundByIndex.map { $0.label }
        log("lock screen texts: \(texts)")
        XCTAssertTrue(texts.contains { $0.contains("Part") || $0.contains("DalyWave") }, "нет карточки «Сейчас играет»")

        if let pause = buttons(sb, matching: ["pause", "пауза"]).first {
            log("tap \(pause.label)")
            pause.tap()
            sleep(4)
        } else {
            XCTFail("нет кнопки паузы")
        }
        if let play = buttons(sb, matching: ["play", "воспр"]).first {
            log("tap \(play.label)")
            play.tap()
            sleep(4)
        }
        if let next = buttons(sb, matching: ["next", "forward", "след", "вперёд"]).first {
            log("tap \(next.label)")
            next.tap()
            sleep(4)
        }
        if let prev = buttons(sb, matching: ["previous", "back", "пред", "назад"]).first {
            log("tap \(prev.label)")
            prev.tap()
            sleep(4)
        }
        log("done")
    }
}

/// Держит экран блокировки открытым, пока с хоста снимаются скриншоты, и нажимает по
/// координатам из окружения (DW_TAPS="x,y;x,y" в точках экрана), если они заданы.
final class LockScreenHoldTests: XCTestCase {
    func testHoldLockScreen() throws {
        let app = XCUIApplication()
        app.launchEnvironment = ["DW_SCENARIO": "background", "DW_VOLUME": "0"]
        app.launch()
        sleep(10)
        print("DW-UITEST hold: locking")
        XCUIDevice.shared.perform(NSSelectorFromString("pressLockButton"))
        sleep(5)
        XCUIDevice.shared.press(.home)
        print("DW-UITEST hold: lock screen visible")
        sleep(25)
        let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let taps = ProcessInfo.processInfo.environment["DW_TAPS"] ?? ""
        for pair in taps.split(separator: ";") {
            let xy = pair.split(separator: ",").compactMap { Double($0) }
            guard xy.count == 2 else { continue }
            let origin = sb.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0))
            print("DW-UITEST hold: tap \(xy[0]),\(xy[1])")
            origin.withOffset(CGVector(dx: xy[0], dy: xy[1])).tap()
            sleep(5)
        }
        sleep(20)
        print("DW-UITEST hold: done")
    }
}
