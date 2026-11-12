import Foundation
import Security

/// Ключ часов: адрес сервера и токен устройства `dwd_…` от iPhone. Хранится в связке ключей часов,
/// доступен после первой разблокировки (часы просыпаются для осложнений и без разблокировки экрана).
struct Credentials: Codable, Equatable {
    var server: String
    var token: String
    var userName: String?

    private static let service = "rest.daly.wave.watch"
    private static let account = "device-token"

    static func load() -> Credentials? {
        #if targetEnvironment(simulator)
        // Только симулятор: ключ локального сервера для проверок экранов без iPhone рядом
        if let token = ProcessInfo.processInfo.environment["DALYWAVE_DEV_TOKEN"], token.hasPrefix("dwd_") {
            let server = ProcessInfo.processInfo.environment["DALYWAVE_DEV_SERVER"] ?? "http://localhost:8400"
            return Credentials(server: server, token: token, userName: nil)
        }
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
