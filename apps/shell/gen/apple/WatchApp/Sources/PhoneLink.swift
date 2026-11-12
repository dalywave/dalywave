import Foundation
import WatchConnectivity

/// Связь с iPhone (WatchConnectivity): часы просят ключ, iPhone отвечает сессией от сервера
/// (Sources/dalywave-shell/WatchLink.swift). Ключ приходит ответом на просьбу или доставкой системы,
/// если iPhone был недоступен; выход на iPhone тоже приходит сюда.
final class PhoneLink: NSObject, WCSessionDelegate {
    enum Reply {
        case paired(Credentials)
        /// На iPhone нет входа
        case signedOut
        /// iPhone рядом, но у него нет сети или сервер не ответил
        case offline
        /// iPhone не рядом (или приложение не отвечает)
        case unreachable
        case failed
    }

    /// Ключ или выход, пришедшие от iPhone без просьбы
    var onDelivery: ((Reply) -> Void)?

    private var activated: [() -> Void] = []

    func start() {
        guard WCSession.isSupported() else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
    }

    /// Попросить ключ. Нет iPhone рядом — просьба уходит доставкой системы, ответ придёт в `onDelivery`.
    func requestPairing(_ done: @escaping (Reply) -> Void) {
        whenActive {
            let session = WCSession.default
            guard session.activationState == .activated else {
                done(.unreachable)
                return
            }
            guard session.isReachable else {
                session.transferUserInfo(["request": "pair"])
                DispatchQueue.main.async { done(.unreachable) }
                return
            }
            session.sendMessage(["request": "pair"], replyHandler: { reply in
                let parsed = Self.parse(reply)
                DispatchQueue.main.async { done(parsed) }
            }, errorHandler: { _ in
                session.transferUserInfo(["request": "pair"])
                DispatchQueue.main.async { done(.unreachable) }
            })
        }
    }

    var phoneReachable: Bool {
        WCSession.isSupported() && WCSession.default.activationState == .activated && WCSession.default.isReachable
    }

    private func whenActive(_ run: @escaping () -> Void) {
        guard WCSession.isSupported() else {
            run()
            return
        }
        // Проверка и очередь — на главном потоке, как и завершение активации: ничего не теряется
        DispatchQueue.main.async {
            if WCSession.default.activationState == .activated {
                run()
            } else {
                self.activated.append(run)
            }
        }
    }

    private static func parse(_ dict: [String: Any]) -> Reply {
        if let server = dict["server"] as? String, let token = dict["token"] as? String, token.hasPrefix("dwd_") {
            return .paired(Credentials(server: server, token: token, userName: dict["userName"] as? String))
        }
        switch dict["error"] as? String {
        case "signedOut": return .signedOut
        case "offline": return .offline
        default: return .failed
        }
    }

    // MARK: WCSessionDelegate

    func session(_ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?) {
        DispatchQueue.main.async {
            let waiting = self.activated
            self.activated = []
            waiting.forEach { $0() }
        }
    }

    func session(_ session: WCSession, didReceiveUserInfo userInfo: [String: Any] = [:]) {
        let reply = Self.parse(userInfo)
        DispatchQueue.main.async { self.onDelivery?(reply) }
    }

    func sessionReachabilityDidChange(_ session: WCSession) {
        DispatchQueue.main.async { self.onReachable?(session.isReachable) }
    }

    var onReachable: ((Bool) -> Void)?
}
