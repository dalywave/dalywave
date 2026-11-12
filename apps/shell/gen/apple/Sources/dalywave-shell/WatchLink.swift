import Foundation
import WatchConnectivity

// Apple Watch: часы — пульт DalyWave со своей сессией устройства. Своего входа у них нет: часы
// просят ключ у iPhone, iPhone просит у сервера дочернюю сессию (Rust, apps/shell/src/watch.rs —
// там токен iPhone и сеть) и передаёт её сюда, а отсюда — часам по WatchConnectivity. Вход и выход
// на iPhone Rust сообщает уведомлениями DalyWaveWatchSignedIn / DalyWaveWatchSignedOut.
//
// Сообщения (словари):
//   часы → iPhone  ["request": "pair"]                     — sendMessage с ответом или transferUserInfo
//   iPhone → часы  ["server", "token", "userName"]         — ключ (ответ или transferUserInfo)
//                  ["error": "signedOut" | "offline" | "failed"]
// Токен не пишется в журнал.

@_silgen_name("dalywave_watch_pair")
private func dalywaveWatchPair() -> UnsafeMutablePointer<CChar>?

@_silgen_name("dalywave_watch_free")
private func dalywaveWatchFree(_ pointer: UnsafeMutablePointer<CChar>?)

final class WatchLink: NSObject, WCSessionDelegate {
    static let shared = WatchLink()

    private let queue = DispatchQueue(label: "rest.daly.wave.watch", qos: .userInitiated)

    func start() {
        guard WCSession.isSupported() else { return }
        let center = NotificationCenter.default
        center.addObserver(forName: Notification.Name("DalyWaveWatchSignedIn"), object: nil, queue: nil) { [weak self] _ in
            self?.accountChanged(signedIn: true)
        }
        center.addObserver(forName: Notification.Name("DalyWaveWatchSignedOut"), object: nil, queue: nil) { [weak self] _ in
            self?.accountChanged(signedIn: false)
        }
        let session = WCSession.default
        session.delegate = self
        session.activate()
    }

    /// Ключ для часов от сервера. Блокирует (сеть) — только из своей очереди.
    private func pair() -> [String: Any] {
        let out = dalywaveWatchPair()
        defer { dalywaveWatchFree(out) }
        guard let out,
              let parsed = try? JSONSerialization.jsonObject(with: Data(String(cString: out).utf8)),
              let reply = parsed as? [String: Any]
        else { return ["error": "failed"] }
        return reply
    }

    private var watchReady: Bool {
        let session = WCSession.default
        return session.activationState == .activated && session.isPaired && session.isWatchAppInstalled
    }

    /// Новый ключ — часам, даже если их приложение сейчас закрыто (доставит система).
    private func push() {
        queue.async {
            guard self.watchReady else { return }
            WCSession.default.transferUserInfo(self.pair())
        }
    }

    /// Вход или выход на iPhone (уведомление от Rust).
    func accountChanged(signedIn: Bool) {
        if signedIn {
            push()
        } else {
            queue.async {
                guard self.watchReady else { return }
                WCSession.default.transferUserInfo(["error": "signedOut"])
            }
        }
    }

    // MARK: WCSessionDelegate

    func session(_ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?) {}

    func sessionDidBecomeInactive(_ session: WCSession) {}

    /// Сменились часы: связь заново с новыми
    func sessionDidDeactivate(_ session: WCSession) {
        session.activate()
    }

    /// Часы поставили приложение: ключ сразу, чтобы первое открытие не ждало
    func sessionWatchStateDidChange(_ session: WCSession) {
        if session.isPaired && session.isWatchAppInstalled && !UserDefaults.standard.bool(forKey: Self.deliveredKey) {
            UserDefaults.standard.set(true, forKey: Self.deliveredKey)
            push()
        }
    }

    private static let deliveredKey = "watch.firstKeyDelivered"

    /// Часы просят ключ, пока iPhone рядом: ответ сразу
    func session(_ session: WCSession, didReceiveMessage message: [String: Any], replyHandler: @escaping ([String: Any]) -> Void) {
        guard message["request"] as? String == "pair" else {
            replyHandler([:])
            return
        }
        queue.async { replyHandler(self.pair()) }
    }

    /// Просьба, отправленная, когда iPhone был недоступен: ответ — тоже доставкой системы
    func session(_ session: WCSession, didReceiveUserInfo userInfo: [String: Any] = [:]) {
        if userInfo["request"] as? String == "pair" { push() }
    }
}

/// Зовёт main.mm до запуска приложения.
@_cdecl("dalywave_watch_start")
public func dalywaveWatchStart() {
    WatchLink.shared.start()
}
