import Foundation
import Network
import UIKit

// Apple TV рядом (iPhone, iPad). Rust — apps/shell/src/pairing.rs (там сервер, вход и токен).
//
// 1. iCloud: недавние серверы — в NSUbiquitousKeyValueStore (ключ `servers`, массив словарей
//    url/name/usedAt). Apple TV на том же Apple ID предлагает подключиться к ним.
// 2. «Подключить с iPhone»: пока приложение открыто и вход есть, ищем `_dalywave-pair._tcp` —
//    так объявляет себя Apple TV на экране подключения. Нашли — окно «Apple TV „Гостиная“ хочет
//    подключиться». «Разрешить» — по TCP строками JSON:
//      iPhone → TV  {"type":"hello","server":"https://…","name":"iPhone"}
//      TV → iPhone  {"type":"code","userCode":"ABCD-EFGH"}   (код входа с сервера, как на экране)
//      iPhone → TV  {"type":"approved"} | {"type":"error","code":"signedOut|offline|failed"}
//    Код подтверждает Rust от имени этого iPhone (`/api/auth/tv/approve`), телевизор получает
//    сессию своим обычным опросом.

@_silgen_name("dalywave_pair_info")
private func dalywavePairInfo() -> UnsafeMutablePointer<CChar>?

@_silgen_name("dalywave_pair_approve")
private func dalywavePairApprove(_ code: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?

@_silgen_name("dalywave_pair_free")
private func dalywavePairFree(_ pointer: UnsafeMutablePointer<CChar>?)

final class TVPairing: NSObject {
    static let shared = TVPairing()

    static let serviceType = "_dalywave-pair._tcp"

    private let queue = DispatchQueue(label: "rest.daly.wave.pairing", qos: .userInitiated)
    private var browser: NWBrowser?
    private var connection: NWConnection?
    /// Телевизоры, о которых уже спросили в этот раз: «Не сейчас» не повторяется, пока он не уйдёт
    private var asked = Set<String>()
    private var active = false

    func start() {
        let center = NotificationCenter.default
        center.addObserver(forName: Notification.Name("DalyWaveServersChanged"), object: nil, queue: nil) { [weak self] _ in
            self?.refresh()
        }
        center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: nil) { [weak self] _ in
            self?.active = true
            self?.refresh()
        }
        center.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: nil) { [weak self] _ in
            self?.active = false
            self?.queue.async { self?.stopBrowsing() }
        }
    }

    private struct Info: Decodable {
        struct Server: Decodable {
            let url: String
            let name: String?
            let usedAt: Double?
        }
        let server: String
        let signedIn: Bool
        let servers: [Server]
    }

    private func info() -> Info? {
        let out = dalywavePairInfo()
        defer { dalywavePairFree(out) }
        guard let out else { return nil }
        return try? JSONDecoder().decode(Info.self, from: Data(String(cString: out).utf8))
    }

    /// Сменился сервер или вход, приложение снова на экране: iCloud и поиск телевизоров.
    private func refresh() {
        queue.async {
            guard let info = self.info() else { return }
            self.publish(info.servers)
            if info.signedIn && self.active {
                self.startBrowsing()
            } else {
                self.stopBrowsing()
            }
        }
    }

    // MARK: iCloud

    private func publish(_ servers: [Info.Server]) {
        let list: [[String: Any]] = servers.prefix(8).map { s in
            var item: [String: Any] = ["url": s.url, "usedAt": s.usedAt ?? 0]
            if let name = s.name { item["name"] = name }
            return item
        }
        let store = NSUbiquitousKeyValueStore.default
        if let old = store.array(forKey: "servers") as? [[String: Any]],
           old.map({ $0["url"] as? String }) == list.map({ $0["url"] as? String }) {
            return
        }
        guard !list.isEmpty else { return }
        store.set(list, forKey: "servers")
        store.synchronize()
    }

    // MARK: Bonjour

    private func startBrowsing() {
        guard browser == nil else { return }
        let browser = NWBrowser(for: .bonjour(type: Self.serviceType, domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            guard let self else { return }
            let names = Set(results.compactMap { Self.name(of: $0.endpoint) })
            // Ушёл из сети — в следующий раз снова спросим
            self.asked = self.asked.intersection(names)
            for result in results {
                guard let name = Self.name(of: result.endpoint), !self.asked.contains(name) else { continue }
                self.asked.insert(name)
                self.ask(name: name, endpoint: result.endpoint)
                break
            }
        }
        browser.start(queue: queue)
        self.browser = browser
    }

    private func stopBrowsing() {
        browser?.cancel()
        browser = nil
    }

    private static func name(of endpoint: NWEndpoint) -> String? {
        if case let .service(name, _, _, _) = endpoint { return name }
        return nil
    }

    // MARK: Окно и разговор с телевизором

    private func ask(name: String, endpoint: NWEndpoint) {
        DispatchQueue.main.async {
            guard let top = Self.topController() else { return }
            let alert = UIAlertController(
                title: String(format: String(localized: "%@ wants to connect"), name),
                message: String(localized: "The Apple TV will sign in to DalyWave with your account on this server."),
                preferredStyle: .alert
            )
            alert.addAction(UIAlertAction(title: String(localized: "Not now"), style: .cancel))
            alert.addAction(UIAlertAction(title: String(localized: "Allow"), style: .default) { [weak self] _ in
                self?.queue.async { self?.connect(endpoint) }
            })
            top.present(alert, animated: true)
        }
    }

    private static func topController() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        let window = scenes.flatMap(\.windows).first { $0.isKeyWindow } ?? scenes.first?.windows.first
        var top = window?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        return top
    }

    private func connect(_ endpoint: NWEndpoint) {
        guard let info = info(), info.signedIn else { return }
        connection?.cancel()
        let connection = NWConnection(to: endpoint, using: .tcp)
        self.connection = connection
        connection.stateUpdateHandler = { [weak self] state in
            switch state {
            case .ready:
                self?.send(["type": "hello", "server": info.server, "name": UIDevice.current.model], on: connection)
                self?.receive(on: connection, buffer: Data())
            case .failed, .cancelled:
                if self?.connection === connection { self?.connection = nil }
            default:
                break
            }
        }
        connection.start(queue: queue)
        // Телевизор не ответил за минуту — закрываем
        queue.asyncAfter(deadline: .now() + 60) { connection.cancel() }
    }

    private func send(_ message: [String: Any], on connection: NWConnection) {
        guard var data = try? JSONSerialization.data(withJSONObject: message) else { return }
        data.append(0x0A)
        connection.send(content: data, completion: .contentProcessed { _ in })
    }

    private func receive(on connection: NWConnection, buffer: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 4096) { [weak self] data, _, done, error in
            guard let self else { return }
            var buffer = buffer + (data ?? Data())
            while let newline = buffer.firstIndex(of: 0x0A) {
                let line = buffer[buffer.startIndex..<newline]
                buffer = Data(buffer[buffer.index(after: newline)...])
                self.handle(line: Data(line), on: connection)
            }
            if done || error != nil || buffer.count > 16_384 {
                connection.cancel()
                return
            }
            self.receive(on: connection, buffer: buffer)
        }
    }

    private func handle(line: Data, on connection: NWConnection) {
        guard let message = (try? JSONSerialization.jsonObject(with: line)) as? [String: Any],
              message["type"] as? String == "code",
              let code = message["userCode"] as? String
        else { return }
        let out = code.withCString { dalywavePairApprove($0) }
        defer { dalywavePairFree(out) }
        let reply = out.flatMap { try? JSONSerialization.jsonObject(with: Data(String(cString: $0).utf8)) } as? [String: Any]
        if reply?["ok"] as? Bool == true {
            send(["type": "approved"], on: connection)
        } else {
            send(["type": "error", "code": reply?["error"] as? String ?? "failed"], on: connection)
        }
        queue.asyncAfter(deadline: .now() + 2) { connection.cancel() }
    }
}

/// Зовёт main.mm до запуска приложения.
@_cdecl("dalywave_pair_start")
public func dalywavePairStart() {
    TVPairing.shared.start()
}
