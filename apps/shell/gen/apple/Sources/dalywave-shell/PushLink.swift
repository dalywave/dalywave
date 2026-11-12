import Foundation
import UserNotifications

// Push-уведомления (APNs): нажатие на уведомление и показ, пока приложение на экране. Разрешение,
// токен устройства и значок — в Rust (apps/shell/src/push.rs): туда же уходит ссылка из уведомления.
//
// Полезная нагрузка сервера: {"aps": {"alert": {"title", "body"}, "sound", "thread-id", "badge"},
//                             "link": "dalywave://album/<id>", "id": "<id уведомления>"}
// Делегата назначает main.mm до запуска приложения: нажатие, которое запустило приложение,
// система отдаёт делегату сразу после запуска — без него ссылка потерялась бы.

@_silgen_name("dalywave_push_open")
private func dalywavePushOpen(_ link: UnsafePointer<CChar>?)

final class PushLink: NSObject, UNUserNotificationCenterDelegate {
    static let shared = PushLink()

    func start() {
        UNUserNotificationCenter.current().delegate = self
    }

    /// Приложение на экране: уведомление покажет сам интерфейс (тост по WebSocket), в системе —
    /// только запись в центре уведомлений, без баннера и звука.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        if #available(iOS 14.0, *) {
            completionHandler([.list])
        } else {
            completionHandler([])
        }
    }

    /// Нажали на уведомление: открыть его ссылку dalywave://… (как любую ссылку приложения).
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        if response.actionIdentifier == UNNotificationDefaultActionIdentifier,
           let link = response.notification.request.content.userInfo["link"] as? String,
           link.hasPrefix("dalywave://") {
            link.withCString { dalywavePushOpen($0) }
        }
        completionHandler()
    }
}

/// Зовёт main.mm до запуска приложения.
@_cdecl("dalywave_push_start")
public func dalywavePushStart() {
    PushLink.shared.start()
}
