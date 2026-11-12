import UIKit
import UniformTypeIdentifiers

/// «Поделиться → DalyWave»: ссылка или текст из Safari, YouTube, Telegram, Spotify… уходит в
/// приложение ссылкой `dalywave://share?url=…`. Своего окна у расширения нет: приложение
/// открывается сразу, а что это за трек, решает его интерфейс (сервер, POST /api/resolve-link).
/// Если система не дала открыть приложение, ссылка остаётся в буфере обмена — вставить в поиск.
final class ShareViewController: UIViewController {
    private var handled = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard !handled else { return }
        handled = true
        Task { @MainActor in
            let shared = await sharedText()
            guard let shared, let link = Self.deepLink(for: shared) else {
                finish(message: "Здесь нет ссылки")
                return
            }
            if openApp(link) {
                finish(message: nil)
            } else {
                UIPasteboard.general.string = shared
                finish(message: "Ссылка скопирована — вставьте её в поиск DalyWave")
            }
        }
    }

    /// Первая ссылка из вложений, иначе текст (в нём ссылку найдёт интерфейс).
    private func sharedText() async -> String? {
        let items = extensionContext?.inputItems.compactMap { $0 as? NSExtensionItem } ?? []
        let providers = items.flatMap { $0.attachments ?? [] }
        for provider in providers where provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
            if let url = try? await provider.loadItem(forTypeIdentifier: UTType.url.identifier) as? URL,
               url.scheme == "http" || url.scheme == "https" {
                return url.absoluteString
            }
        }
        for provider in providers where provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
            if let text = try? await provider.loadItem(forTypeIdentifier: UTType.plainText.identifier) as? String,
               !text.isEmpty {
                return text
            }
        }
        return items.compactMap { $0.attributedContentText?.string }.first { !$0.isEmpty }
    }

    static func deepLink(for shared: String) -> URL? {
        var components = URLComponents()
        components.scheme = "dalywave"
        components.host = "share"
        components.queryItems = [URLQueryItem(name: "url", value: shared)]
        return components.url
    }

    /// Расширению нельзя `UIApplication.shared` и `open(_:)` (Xcode требует
    /// APPLICATION_EXTENSION_API_ONLY), но приложение есть в цепочке ответчиков: метод
    /// `openURL:options:completionHandler:` зовём через рантайм Objective-C.
    private func openApp(_ url: URL) -> Bool {
        let selector = NSSelectorFromString("openURL:options:completionHandler:")
        typealias OpenURL = @convention(c) (AnyObject, Selector, NSURL, NSDictionary, AnyObject?) -> Void
        var responder: UIResponder? = self
        while let current = responder {
            if current is UIApplication, current.responds(to: selector) {
                let open = unsafeBitCast(current.method(for: selector), to: OpenURL.self)
                open(current, selector, url as NSURL, NSDictionary(), nil)
                return true
            }
            responder = current.next
        }
        return false
    }

    private func finish(message: String?) {
        guard let message else {
            extensionContext?.completeRequest(returningItems: nil)
            return
        }
        let alert = UIAlertController(title: "DalyWave", message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default) { [weak self] _ in
            self?.extensionContext?.completeRequest(returningItems: nil)
        })
        present(alert, animated: true)
    }
}
