import Foundation
import Network
import SwiftUI
import UIKit

/// Адрес сервера: `https://host[:port]` без пути. Без схемы — https; простой http — только для своей
/// сети (ATS пускает его к локальным адресам).
enum ServerAddress {
    static func normalize(_ raw: String) -> String? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !trimmed.contains(" ") else { return nil }
        let withScheme = trimmed.contains("://") ? trimmed : "https://" + trimmed
        guard let url = URLComponents(string: withScheme),
              let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http",
              let host = url.host, !host.isEmpty,
              url.user == nil, url.query == nil, url.fragment == nil,
              url.path.isEmpty || url.path == "/"
        else { return nil }
        return "\(scheme)://\(host.lowercased())" + (url.port.map { ":\($0)" } ?? "")
    }

    static func host(_ server: String) -> String {
        guard let url = URL(string: server), let host = url.host else { return server }
        return host + (url.port.map { ":\($0)" } ?? "")
    }

    /// Демо-сервер с открытой музыкой
    static let demo = "https://demo.wave.daly.rest"
}

/// Сервер-кандидат: из iCloud (выбран на iPhone), из локальной сети или введённый вручную.
struct ServerChoice: Identifiable, Hashable {
    var url: String
    var name: String?
    var id: String { url }
}

/// Что ответил сервер по адресу.
struct ServerProbe: Decodable {
    var app: String?
    var name: String?
    var version: String?
    var deezer: Bool?
}

enum ProbeError: Error {
    case unreachable, notDalyWave

    var text: String {
        switch self {
        case .unreachable: String(localized: "Сервер не отвечает. Проверьте адрес и сеть")
        case .notDalyWave: String(localized: "По этому адресу не DalyWave")
        }
    }
}

/// Проверить, что по адресу DalyWave (`/api/config`): у прежних серверов нет поля `app`.
func probe(_ server: String) async -> Result<ServerProbe, ProbeError> {
    guard let url = URL(string: server + "/api/config") else { return .failure(.notDalyWave) }
    var req = URLRequest(url: url)
    req.timeoutInterval = 10
    req.setValue("application/json", forHTTPHeaderField: "Accept")
    guard let (data, res) = try? await Api.session.data(for: req) else { return .failure(.unreachable) }
    guard (res as? HTTPURLResponse)?.statusCode == 200,
          let reply = try? JSONDecoder().decode(ServerProbe.self, from: data),
          reply.app == "dalywave" || (reply.app == nil && reply.version != nil && reply.deezer != nil)
    else { return .failure(.notDalyWave) }
    return .success(reply)
}

/// Экран подключения: серверы с iPhone (iCloud), из этой сети (Bonjour `_dalywave._tcp`), ручной
/// ввод, демо — и «Подключить с iPhone»: iPhone рядом с входом передаёт адрес и подтверждает вход.
@MainActor
final class ConnectModel: ObservableObject {
    @Published private(set) var fromPhone: [ServerChoice] = []
    @Published private(set) var nearby: [ServerChoice] = []
    @Published private(set) var checking: String?
    @Published var error: String?
    @Published private(set) var pairing: PairListener.Status = .waiting

    private var browser: NWBrowser?
    private let listener = PairListener()
    private var kvsObserver: NSObjectProtocol?

    func start(done: @escaping (Credentials) -> Void, chosen: @escaping (String, String?) -> Void) {
        loadCloud()
        kvsObserver = NotificationCenter.default.addObserver(
            forName: NSUbiquitousKeyValueStore.didChangeExternallyNotification,
            object: NSUbiquitousKeyValueStore.default, queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.loadCloud() }
        }
        NSUbiquitousKeyValueStore.default.synchronize()
        browse()
        listener.onStatus = { [weak self] status in self?.pairing = status }
        listener.onSignedIn = { creds in done(creds) }
        listener.start()
        self.chosen = chosen
    }

    func stop() {
        browser?.cancel()
        browser = nil
        listener.stop()
        if let kvsObserver { NotificationCenter.default.removeObserver(kvsObserver) }
        kvsObserver = nil
    }

    private var chosen: ((String, String?) -> Void)?

    /// Проверить и перейти ко входу на этом сервере.
    func choose(_ raw: String, name: String? = nil) {
        guard let server = ServerAddress.normalize(raw) else {
            error = String(localized: "Это не похоже на адрес. Например: music.example.com")
            return
        }
        checking = server
        error = nil
        Task {
            let result = await probe(server)
            checking = nil
            switch result {
            case let .success(reply):
                chosen?(server, reply.name ?? name)
            case let .failure(e):
                error = e.text
            }
        }
    }

    // MARK: iCloud: серверы, выбранные на iPhone с тем же Apple ID

    private func loadCloud() {
        let items = NSUbiquitousKeyValueStore.default.array(forKey: "servers") as? [[String: Any]] ?? []
        fromPhone = items.compactMap { item in
            guard let url = (item["url"] as? String).flatMap(ServerAddress.normalize) else { return nil }
            return ServerChoice(url: url, name: item["name"] as? String)
        }
    }

    // MARK: Серверы DalyWave в этой сети

    private func browse() {
        let browser = NWBrowser(for: .bonjourWithTXTRecord(type: "_dalywave._tcp", domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            let found: [ServerChoice] = results.compactMap { result in
                guard case let .bonjour(txt) = result.metadata,
                      let url = txt["url"].flatMap(ServerAddress.normalize)
                else { return nil }
                var name = txt["name"]
                if name == nil, case let .service(service, _, _, _) = result.endpoint { name = service }
                return ServerChoice(url: url, name: name)
            }
            Task { @MainActor in
                var seen = Set<String>()
                self?.nearby = found.filter { seen.insert($0.url).inserted }
            }
        }
        browser.start(queue: .main)
        self.browser = browser
    }
}

/// «Подключить с iPhone»: Apple TV объявляет `_dalywave-pair._tcp`; iPhone с входом (TVPairing.swift)
/// присылает адрес сервера, телевизор просит у сервера код входа и отдаёт его iPhone — тот
/// подтверждает вход, телевизор забирает ключ обычным опросом.
final class PairListener {
    enum Status: Equatable {
        case waiting
        case connecting(String)
        case failed(String)
    }

    var onStatus: ((Status) -> Void)?
    var onSignedIn: ((Credentials) -> Void)?

    private let queue = DispatchQueue(label: "rest.daly.wave.tv.pair")
    private var listener: NWListener?
    private var connection: NWConnection?
    private var task: Task<Void, Never>?

    func start() {
        guard listener == nil, let listener = try? NWListener(using: .tcp) else { return }
        listener.service = NWListener.Service(name: UIDevice.current.name, type: "_dalywave-pair._tcp")
        listener.newConnectionHandler = { [weak self] connection in
            guard let self else { return }
            // Один разговор за раз: новый iPhone сменяет прежний
            self.connection?.cancel()
            self.connection = connection
            connection.start(queue: self.queue)
            self.receive(on: connection, buffer: Data())
        }
        listener.start(queue: queue)
        self.listener = listener
    }

    func stop() {
        task?.cancel()
        connection?.cancel()
        listener?.cancel()
        listener = nil
    }

    private func status(_ s: Status) {
        DispatchQueue.main.async { self.onStatus?(s) }
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
                let line = Data(buffer[buffer.startIndex..<newline])
                buffer = Data(buffer[buffer.index(after: newline)...])
                self.handle(line, on: connection)
            }
            if done || error != nil || buffer.count > 16_384 { return }
            self.receive(on: connection, buffer: buffer)
        }
    }

    private func handle(_ line: Data, on connection: NWConnection) {
        guard let message = (try? JSONSerialization.jsonObject(with: line)) as? [String: Any],
              let type = message["type"] as? String
        else { return }
        switch type {
        case "hello":
            guard let server = (message["server"] as? String).flatMap(ServerAddress.normalize) else {
                status(.failed(String(localized: "iPhone прислал непонятный адрес сервера")))
                return
            }
            status(.connecting(server))
            signIn(server: server, connection: connection)
        case "error":
            let text = switch message["code"] as? String {
            case "signedOut": String(localized: "На iPhone нет входа в DalyWave")
            case "offline": String(localized: "iPhone не достучался до сервера")
            default: String(localized: "iPhone не смог подтвердить вход")
            }
            task?.cancel()
            status(.failed(text))
        default:
            break
        }
    }

    /// Код входа на этом сервере → iPhone; затем опрос, пока iPhone не подтвердит.
    private func signIn(server: String, connection: NWConnection) {
        task?.cancel()
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        let name = UIDevice.current.name
        task = Task { [weak self] in
            let api = Api(server: server)
            do {
                let code = try await api.tvCode(deviceName: name)
                self?.send(["type": "code", "userCode": code.userCode], on: connection)
                while !Task.isCancelled, Date() < code.expiresAt {
                    try await Task.sleep(for: .seconds(max(2, code.intervalSecs)))
                    guard let reply = try await api.tvToken(code.deviceCode, appVersion: version) else { break }
                    if reply.status == "ready", let token = reply.token {
                        let creds = Credentials(server: server, token: token, userName: reply.user?.name)
                        await MainActor.run { self?.onSignedIn?(creds) }
                        connection.cancel()
                        return
                    }
                }
            } catch is CancellationError {
                return
            } catch {
                self?.status(.failed(String(localized: "Сервер \(ServerAddress.host(server)) не отвечает")))
            }
        }
    }
}

struct ConnectView: View {
    @EnvironmentObject private var store: Store
    @StateObject private var model = ConnectModel()
    @State private var address = ""
    @State private var server: ServerChoice?

    var body: some View {
        Group {
            if let server {
                SignInView(server: server.url, serverName: server.name) { self.server = nil }
            } else {
                picker
            }
        }
        .background(Theme.ink.ignoresSafeArea())
        .onAppear {
            model.start(done: { store.signedIn($0) }, chosen: { url, name in
                Credentials.lastServer = url
                server = ServerChoice(url: url, name: name)
            })
        }
        .onDisappear { model.stop() }
    }

    /// Прежний сервер (после выхода) — первым; затем с iPhone и из сети, без повторов.
    private var suggestions: [ServerChoice] {
        var seen = Set<String>()
        var list: [ServerChoice] = []
        if let last = Credentials.lastServer { list.append(ServerChoice(url: last, name: nil)) }
        list += model.fromPhone + model.nearby
        return list.filter { seen.insert($0.url).inserted }
    }

    private var picker: some View {
        HStack(alignment: .top, spacing: 100) {
            VStack(alignment: .leading, spacing: 28) {
                Image("Glyph")
                    .renderingMode(.template)
                    .resizable()
                    .frame(width: 120, height: 120)
                    .padding(.leading, -28)
                    .padding(.vertical, -24)
                    .foregroundStyle(Theme.text1)
                Text("Подключите DalyWave")
                    .font(.system(size: 60, weight: .bold))
                    .foregroundStyle(Theme.text1)
                Text("DalyWave играет музыку с вашего сервера. Выберите его — или загляните в демо с открытой музыкой.")
                    .font(.system(size: 30))
                    .foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        ForEach(suggestions) { choice in
                            Button {
                                model.choose(choice.url, name: choice.name)
                            } label: {
                                row(title: choice.name ?? ServerAddress.host(choice.url),
                                    subtitle: choice.name == nil ? origin(of: choice) : ServerAddress.host(choice.url),
                                    busy: model.checking == choice.url)
                            }
                        }
                        HStack(spacing: 20) {
                            TextField("Адрес сервера, например music.example.com", text: $address)
                                .textContentType(.URL)
                                .keyboardType(.URL)
                                .autocorrectionDisabled()
                                .textInputAutocapitalization(.never)
                                .onSubmit { model.choose(address) }
                            Button("Подключиться") { model.choose(address) }
                                .disabled(address.trimmingCharacters(in: .whitespaces).isEmpty || model.checking != nil)
                        }
                        Button {
                            model.choose(ServerAddress.demo, name: "DalyWave Demo")
                        } label: {
                            row(title: String(localized: "Попробовать демо"), subtitle: String(localized: "Открытая музыка, вход appreview"), busy: model.checking == ServerAddress.demo)
                        }
                        if let error = model.error {
                            Text(error).font(.system(size: 26)).foregroundStyle(Theme.accent)
                        }
                    }
                    .padding(.vertical, 20)
                }
                .scrollClipDisabled()
            }
            .frame(maxWidth: 1000, alignment: .leading)

            VStack(alignment: .leading, spacing: 24) {
                Image(systemName: "iphone.radiowaves.left.and.right")
                    .font(.system(size: 64))
                    .foregroundStyle(Theme.text1)
                Text("Подключить с iPhone")
                    .font(.system(size: 40, weight: .bold))
                    .foregroundStyle(Theme.text1)
                Text(pairingText)
                    .font(.system(size: 28))
                    .foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(48)
            .frame(width: 620, alignment: .leading)
            .background(Theme.fill1, in: RoundedRectangle(cornerRadius: 36, style: .continuous))
            .padding(.top, 200)
        }
        .padding(.horizontal, 90)
        .padding(.vertical, 60)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private func origin(of choice: ServerChoice) -> String {
        if choice.url == Credentials.lastServer { return String(localized: "Прежний сервер") }
        if model.fromPhone.contains(where: { $0.url == choice.url }) { return String(localized: "С вашего iPhone") }
        return String(localized: "В этой сети")
    }

    private var pairingText: String {
        switch model.pairing {
        case .waiting:
            String(localized: "Откройте DalyWave на iPhone в этой же сети — он предложит подключить этот Apple TV.")
        case let .connecting(server):
            String(localized: "Подтвердите на iPhone. Сервер: \(ServerAddress.host(server))")
        case let .failed(text):
            text
        }
    }

    private func row(title: String, subtitle: String, busy: Bool) -> some View {
        HStack(spacing: 20) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.system(size: 32, weight: .semibold))
                Text(subtitle).font(.system(size: 24)).opacity(0.7)
            }
            Spacer()
            if busy { ProgressView() }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 8)
    }
}
