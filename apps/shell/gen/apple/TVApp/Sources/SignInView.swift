import SwiftUI
import UIKit

/// Вход без клавиатуры: короткий код и QR на экране, подтверждение — на телефоне или сайте, где
/// человек уже вошёл (`/tv?code=…`). Телевизор раз в пару секунд спрашивает, подтверждён ли вход;
/// код живёт 10 минут, истёк — показываем новый.
@MainActor
final class SignInModel: ObservableObject {
    @Published private(set) var code: TvCode?
    @Published private(set) var failed = false

    private var task: Task<Void, Never>?

    func start(server: String, _ done: @escaping (Credentials) -> Void) {
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

struct SignInView: View {
    @EnvironmentObject private var store: Store
    @StateObject private var model = SignInModel()
    /// Выбранный на экране подключения сервер
    let server: String
    var serverName: String?
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
                HStack(spacing: 24) {
                    Text(serverName.map { "\($0) · \(ServerAddress.host(server))" } ?? ServerAddress.host(server))
                        .font(.system(size: 26))
                        .foregroundStyle(Theme.text3)
                    Button("Другой сервер", action: back)
                }
                .padding(.top, 8)
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
        .onAppear { model.start(server: server) { store.signedIn($0) } }
        .onDisappear { model.stop() }
    }

    private var shortURL: String {
        guard let url = model.code.flatMap({ URL(string: $0.verificationUrl) }) else { return ServerAddress.host(server) + "/tv" }
        return (url.host ?? "") + (url.port.map { ":\($0)" } ?? "") + url.path
    }
}
