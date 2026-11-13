import Foundation
import Security

/// Ключ телевизора: адрес сервера и токен устройства `dwd_…`, выданный по коду с экрана
/// (`/api/auth/tv/*`). Хранится в связке ключей Apple TV.
struct Credentials: Codable, Equatable {
    var server: String
    var token: String
    var userName: String?

    private static let service = "rest.daly.wave.tv"
    private static let account = "device-token"

    /// Сервер DalyWave. Только симулятор: свой сервер на Mac — `DALYWAVE_DEV_SERVER`.
    static var server: String {
        #if targetEnvironment(simulator)
        if let dev = ProcessInfo.processInfo.environment["DALYWAVE_DEV_SERVER"], !dev.isEmpty { return dev }
        #endif
        return "https://wave.daly.rest"
    }

    static func load() -> Credentials? {
        #if targetEnvironment(simulator)
        // Только симулятор: ключ локального сервера для проверок экранов без входа кодом
        if let token = ProcessInfo.processInfo.environment["DALYWAVE_DEV_TOKEN"], token.hasPrefix("dwd_") {
            return Credentials(server: server, token: token, userName: nil)
        }
        if ProcessInfo.processInfo.environment["DALYWAVE_TV_SIGNED_OUT"] == "1" { return nil }
        #endif
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var out: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &out) == errSecSuccess,
              let data = out as? Data
        else { return nil }
        return try? JSONDecoder().decode(Credentials.self, from: data)
    }

    func save() {
        guard let data = try? JSONEncoder().encode(self) else { return }
        Credentials.delete()
        let item: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Credentials.service,
            kSecAttrAccount as String: Credentials.account,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            kSecValueData as String: data,
        ]
        SecItemAdd(item as CFDictionary, nil)
    }

    static func delete() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(query as CFDictionary)
    }
}
