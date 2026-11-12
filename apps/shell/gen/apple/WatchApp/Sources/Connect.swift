import Foundation

/// WebSocket `/api/ws` — как у приложений и сайта: часы представляются устройством «Где играть»
/// (`connect.hello`, вид `watch`, только пульт), получают список устройств и снимки активного,
/// шлют команды (`connect.command`) и перенос (`connect.transfer`). Звук здесь не играет.
final class ConnectSocket: NSObject, URLSessionWebSocketDelegate {
    enum Event {
        case open
        case closed(unauthorized: Bool)
        case message([String: Any], Data)
    }

    var onEvent: ((Event) -> Void)?

    private var task: URLSessionWebSocketTask?
    private lazy var session = URLSession(configuration: .default, delegate: self, delegateQueue: .main)
    private var heartbeat: Timer?
    private var generation = 0

    func open(_ credentials: Credentials) {
        close()
        let base = credentials.server
            .replacingOccurrences(of: "https://", with: "wss://")
            .replacingOccurrences(of: "http://", with: "ws://")
        guard let url = URL(string: base + "/api/ws") else { return }
        var req = URLRequest(url: url)
        req.setValue("Bearer \(credentials.token)", forHTTPHeaderField: "Authorization")
        generation += 1
        let task = session.webSocketTask(with: req)
        task.maximumMessageSize = 1 << 20
        self.task = task
        task.resume()
        receive(task, generation)
    }

    func close() {
        heartbeat?.invalidate()
        heartbeat = nil
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
    }

    var isOpen: Bool { task?.state == .running }

    func send(_ message: [String: Any]) {
        guard let task, let data = try? JSONSerialization.data(withJSONObject: message),
              let text = String(data: data, encoding: .utf8)
        else { return }
        task.send(.string(text)) { _ in }
    }

    private func receive(_ task: URLSessionWebSocketTask, _ gen: Int) {
        task.receive { [weak self] result in
            DispatchQueue.main.async {
                guard let self, gen == self.generation else { return }
                switch result {
                case let .success(message):
                    let data: Data? = switch message {
                    case let .string(s): s.data(using: .utf8)
                    case let .data(d): d
                    @unknown default: nil
                    }
                    if let data, let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                        self.onEvent?(.message(obj, data))
                    }
                    self.receive(task, gen)
                case .failure:
                    let status = (task.response as? HTTPURLResponse)?.statusCode
                    let closedForSession = task.closeCode.rawValue == 4001
                    self.heartbeat?.invalidate()
                    self.task = nil
                    self.onEvent?(.closed(unauthorized: status == 401 || closedForSession))
                }
            }
        }
    }

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        heartbeat?.invalidate()
        // «Я на связи» раз в 5 с: без него через 15 с устройство считается пропавшим
        heartbeat = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
            self?.send(["type": "connect.heartbeat"])
        }
        onEvent?(.open)
    }
}
