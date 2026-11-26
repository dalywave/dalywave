import SwiftUI
import UIKit

/// Вход без клавиатуры: короткий код и QR на экране, подтверждение — на телефоне или сайте, где
/// человек уже вошёл (`/tv?code=…`). Телевизор раз в пару секунд спрашивает, подтверждён ли вход;
/// код живёт 10 минут, истёк — показываем новый.
@MainActor
final class SignInModel: ObservableObject {
    @Published private(set) var code: TvCode?
    @Published private(set) var failed = false
    /// На сервере есть демо: рядом с кодом — «Войти в демо»
    @Published private(set) var demo = false
    @Published private(set) var demoBusy = false
    /// Почему не вошли в демо — здесь или ещё на экране подключения
    @Published private(set) var demoError: String?

    private var task: Task<Void, Never>?
    private var server = ""
    private var done: ((Credentials) -> Void)?

    func start(server: String, demoError: String? = nil, _ done: @escaping (Credentials) -> Void) {
        self.server = server
        self.done = done
        self.demoError = demoError
        poll(server, done)
        Task { [weak self] in
            let demo = await DemoSignIn.available(server)
            self?.demo = demo
        }
    }

    /// Демо без телефона. Код на экране живёт дальше: не вышло — входят им.
    func signInDemo() {
        guard !demoBusy else { return }
        demoBusy = true
        demoError = nil
        Task {
            do {
                let creds = try await DemoSignIn.signIn(server: server)
                task?.cancel()
                done?(creds)
            } catch {
                demoError = DemoSignIn.message(for: error)
                if case .server(404)? = error as? ApiError { demo = false }
            }
            demoBusy = false
        }
    }

    private func poll(_ server: String, _ done: @escaping (Credentials) -> Void) {
        task?.cancel()
        let api = Api(server: server)
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        task = Task { [weak self] in
            while !Task.isCancelled {
                do {
                    let code = try await api.tvCode(deviceName: UIDevice.current.name)
                    self?.code = code
                    self?.failed = false
                    // Опрос до подтверждения или конца срока кода
                    while !Task.isCancelled, Date() < code.expiresAt {
                        try await Task.sleep(for: .seconds(max(2, code.intervalSecs)))
                        guard let reply = try await api.tvToken(code.deviceCode, appVersion: version) else { break }
                        if reply.status == "ready", let token = reply.token {
                            done(Credentials(server: server, token: token, userName: reply.user?.name))
                            return
                        }
                    }
                } catch is CancellationError {
                    return
                } catch {
                    self?.failed = true
                    try? await Task.sleep(for: .seconds(5))
                }
            }
        }
    }

    func stop() {
        task?.cancel()
    }
}

/// Общий аккаунт демо без телефона (`/api/auth/providers` → `demo: true`): тот же код телевизора, но
/// подтверждает его сам телевизор (`/api/auth/demo/tv`), а ключ отдаёт обычный `/api/auth/tv/token`.
enum DemoSignIn {
    /// Ошибка запроса или прежний сервер без поля `demo` — демо нет, вход как обычно.
    static func available(_ server: String) async -> Bool {
        (try? await Api(server: server).providers().demo) == true
    }

    @MainActor
    static func signIn(server: String) async throws -> Credentials {
        let api = Api(server: server)
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        let code = try await api.tvCode(deviceName: UIDevice.current.name)
        try await api.approveDemo(code.deviceCode)
        // Код уже подтверждён: ключ готов сразу, без опроса
        guard let reply = try await api.tvToken(code.deviceCode, appVersion: version),
              reply.status == "ready", let token = reply.token
        else { throw ApiError.server(400) }
        return Credentials(server: server, token: token, userName: reply.user?.name)
    }

    static func message(for error: Error) -> String {
        switch error as? ApiError {
        case .server(404)?: String(localized: "На этом сервере нет демо")
        case .server(429)?: String(localized: "Слишком много входов в демо. Попробуйте через минуту")
        case .server(403)?: String(localized: "Демо сейчас закрыто")
        default: String(localized: "Не удалось войти в демо")
        }
    }
}

struct SignInView: View {
    @EnvironmentObject private var store: Store
    @StateObject private var model = SignInModel()
    /// Выбранный на экране подключения сервер
    let server: String
    var serverName: String?
    /// Демо не открылось сразу с экрана подключения — показать почему
    var demoError: String?
    /// «Другой сервер» — назад к выбору
    var back: () -> Void

    var body: some View {
        HStack(spacing: 120) {
            VStack(alignment: .leading, spacing: 36) {
                Image("Glyph")
                    .renderingMode(.template)
                    .resizable()
                    .frame(width: 150, height: 150)
                    .padding(.leading, -36)
                    .padding(.vertical, -30)
                    .foregroundStyle(Theme.text1)
                Text("Войдите в DalyWave")
                    .font(.system(size: 64, weight: .bold))
                    .foregroundStyle(Theme.text1)
                (Text("Наведите камеру телефона на код или откройте ")
                    + Text(shortURL).foregroundStyle(Theme.text1)
                    + Text(" и введите код с экрана."))
                    .font(.system(size: 32))
                    .foregroundStyle(Theme.text2)
                    .lineSpacing(8)
                    .fixedSize(horizontal: false, vertical: true)
                Text(model.code?.userCode ?? "····-····")
                    .font(.system(size: 104, weight: .semibold, design: .monospaced))
                    .tracking(8)
                    .foregroundStyle(Theme.text1)
                    .padding(.top, 12)
                    .redacted(reason: model.code == nil ? .placeholder : [])
                if model.failed {
                    Text("Нет связи с сервером. Пробуем снова…")
                        .font(.system(size: 28))
                        .foregroundStyle(Theme.text3)
                }
                if let error = model.demoError {
                    Text(error)
                        .font(.system(size: 28))
                        .foregroundStyle(Theme.accent)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if model.demo {
                    // Две кнопки и адрес в одну строку не помещаются: адрес — над кнопками
                    VStack(alignment: .leading, spacing: 20) {
                        serverLabel
                        HStack(spacing: 24) {
                            Button { model.signInDemo() } label: {
                                HStack(spacing: 16) {
                                    if model.demoBusy { ProgressView() }
                                    Text("Войти в демо")
                                }
                            }
                            Button("Другой сервер", action: back)
                        }
                    }
                    .padding(.top, 8)
                } else {
                    HStack(spacing: 24) {
                        serverLabel
                        Button("Другой сервер", action: back)
                    }
                    .padding(.top, 8)
                }
            }
            .frame(maxWidth: 900, alignment: .leading)
            Group {
                if let code = model.code {
                    QRCode(value: code.verificationUrlComplete, size: 460)
                } else {
                    RoundedRectangle(cornerRadius: 36, style: .continuous).fill(Theme.fill1).frame(width: 460, height: 460)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.ink.ignoresSafeArea())
        .onAppear { model.start(server: server, demoError: demoError) { store.signedIn($0) } }
        .onDisappear { model.stop() }
    }

    private var serverLabel: some View {
        Text(serverName.map { "\($0) · \(ServerAddress.host(server))" } ?? ServerAddress.host(server))
            .font(.system(size: 26))
            .foregroundStyle(Theme.text3)
    }

    private var shortURL: String {
        guard let url = model.code.flatMap({ URL(string: $0.verificationUrl) }) else { return ServerAddress.host(server) + "/tv" }
        return (url.host ?? "") + (url.port.map { ":\($0)" } ?? "") + url.path
    }
}
