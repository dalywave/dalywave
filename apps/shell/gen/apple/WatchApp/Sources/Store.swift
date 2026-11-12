import Foundation
import SwiftUI
import WidgetKit

/// Всё состояние часов: ключ от iPhone, связь с сервером, «Где играть» и медиатека.
///
/// Связь — как у приложений: WebSocket `/api/ws` (`connect.*`). На настоящих часах watchOS даёт
/// WebSocket только приложению со своим звуком, поэтому, если он не открылся, часы — пульт по HTTP:
/// раз в 2 с отмечаются (`POST /api/devices/remote`) и шлют команды запросом с подтверждением.
@MainActor
final class Store: ObservableObject {
    enum Phase: Equatable {
        case starting
        /// Нет ключа: ждём iPhone
        case needsPhone(PhoneProblem)
        case ready
    }

    enum PhoneProblem: Equatable {
        /// iPhone не рядом или приложение не ответило
        case unreachable
        /// На iPhone нет входа
        case signedOut
        /// У iPhone нет сети
        case offline
    }

    @Published private(set) var phase: Phase = .starting
    /// Сервер не отвечает (нет сети у часов)
    @Published private(set) var offline = false
    @Published private(set) var devices: [ConnectDevice] = []
    @Published private(set) var activeId: String?
    @Published private(set) var state: PlaybackState?
    /// Куда идут команды, пока активного нет (выбор в «Где играть»)
    @Published var preferredId: String?
    @Published private(set) var likedOverride: [String: Bool] = [:]
    /// Громкость, пока крутят колёсико: снимок догонит позже
    @Published var volumeDraft: Double?
    @Published private(set) var busy = false
    /// Короткая подсказка после неудачной команды
    @Published var notice: String?
    /// Пролистать на «Сейчас играет» (после запуска волны или плейлиста)
    @Published var page: Int = 0

    @Published private(set) var presets: [WavePreset] = []
    @Published private(set) var playlists: [PlaylistSummary] = []
    @Published private(set) var recent: [RecentItem] = []
    @Published private(set) var libraryLoaded = false

    private(set) var credentials: Credentials?
    private let phone = PhoneLink()
    private let socket = ConnectSocket()
    private var socketFailed = false
    private var pollTask: Task<Void, Never>?
    private var reconnectTask: Task<Void, Never>?
    private var pendingAcks: [String: (Bool, String?) -> Void] = [:]
    /// Время сервера минус время часов
    private var clockOffset: TimeInterval = 0
    /// Приложение на экране: связь держится только тогда
    private var foreground = false
    private var socketOpened = false

    let deviceId: String = {
        let key = "connect.deviceId"
        if let id = UserDefaults.standard.string(forKey: key) { return id }
        let id = "watch-" + UUID().uuidString.lowercased()
        UserDefaults.standard.set(id, forKey: key)
        return id
    }()

    init() {
        phone.onDelivery = { [weak self] reply in self?.handle(reply, requested: false) }
        phone.onReachable = { [weak self] reachable in
            guard let self, reachable, case .needsPhone = self.phase else { return }
            self.requestKey()
        }
        socket.onEvent = { [weak self] event in self?.onSocket(event) }
        phone.start()
        if let saved = Credentials.load() {
            credentials = saved
            phase = .ready
        }
    }

    // MARK: Жизненный цикл

    func becameActive() {
        guard !foreground else { return }
        foreground = true
        guard let credentials else {
            requestKey()
            return
        }
        connect(credentials)
        Task { await loadLibrary() }
        #if targetEnvironment(simulator)
        runSimulatorScript()
        #endif
    }

    #if targetEnvironment(simulator)
    /// Только симулятор: проверки экранов без касаний — `DALYWAVE_WATCH_PAGE` (0–2),
    /// `DALYWAVE_WATCH_SHEET=devices`, `DALYWAVE_WATCH_ACTIONS` (toggle,next,previous,like,wave,calm,liked,volume).
    @Published var simulatorSheet = false

    private func runSimulatorScript() {
        let env = ProcessInfo.processInfo.environment
        if let page = env["DALYWAVE_WATCH_PAGE"].flatMap(Int.init) { self.page = page }
        if env["DALYWAVE_WATCH_SHEET"] == "devices" { simulatorSheet = true }
        let actions = env["DALYWAVE_WATCH_ACTIONS"]?.split(separator: ",").map(String.init) ?? []
        Task {
            for action in actions {
                try? await Task.sleep(for: .seconds(2.5))
                switch action {
                case "toggle": togglePlay()
                case "next": next()
                case "previous": previous()
                case "like": toggleLike()
                case "wave": startWave()
                case "calm": startWave(WaveSettings(mood: "calm"))
                case "liked": playLiked()
                case "volume": setVolume(0.3)
                default: break
                }
            }
        }
    }
    #endif

    func becameInactive() {
        foreground = false
        pollTask?.cancel()
        pollTask = nil
        reconnectTask?.cancel()
        socket.close()
    }

    func requestKey() {
        if phase != .ready { phase = .starting }
        phone.requestPairing { [weak self] reply in self?.handle(reply, requested: true) }
        // iPhone не ответил (часы без пары, связь не поднялась) — не крутить ожидание бесконечно
        Task { [weak self] in
            try? await Task.sleep(for: .seconds(8))
            guard let self, self.phase == .starting else { return }
            self.phase = .needsPhone(.unreachable)
        }
    }

    private func handle(_ reply: PhoneLink.Reply, requested: Bool) {
        switch reply {
        case let .paired(credentials):
            credentials.save()
            self.credentials = credentials
            phase = .ready
            offline = false
            if foreground {
                connect(credentials)
                Task { await loadLibrary(force: true) }
            }
        case .signedOut:
            forget()
            phase = .needsPhone(.signedOut)
        case .offline:
            if credentials == nil { phase = .needsPhone(.offline) }
        case .unreachable, .failed:
            if credentials == nil { phase = .needsPhone(.unreachable) }
        }
    }

    /// Ключ больше не действует: стереть и попросить новый.
    private func forget() {
        Credentials.delete()
        credentials = nil
        socket.close()
        pollTask?.cancel()
        devices = []
        state = nil
        activeId = nil
        NowPlayingSnapshot.save(nil)
        WidgetCenter.shared.reloadAllTimelines()
    }

    private func unauthorized() {
        forget()
        phase = .needsPhone(.unreachable)
        requestKey()
    }

    // MARK: Связь с сервером

    private var forceHTTP: Bool {
        ProcessInfo.processInfo.environment["DALYWAVE_WATCH_TRANSPORT"] == "http"
    }

    private func connect(_ credentials: Credentials) {
        reconnectTask?.cancel()
        if socketFailed || forceHTTP {
            startPolling()
        } else {
            socketOpened = false
            socket.open(credentials)
            // Пока WebSocket не открылся — первое состояние запросом
            Task { await poll() }
        }
    }

    private func onSocket(_ event: ConnectSocket.Event) {
        switch event {
        case .open:
            socketOpened = true
            offline = false
            pollTask?.cancel()
        case let .closed(unauthorized):
            if unauthorized {
                self.unauthorized()
                return
            }
            // Не открылся ни разу — значит, watchOS не даёт WebSocket: дальше по HTTP
            if !socketOpened { socketFailed = true }
            guard foreground, let credentials else { return }
            reconnectTask = Task { [weak self] in
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled else { return }
                self?.connect(credentials)
            }
        case let .message(obj, data):
            onMessage(obj, data)
        }
    }

    private func onMessage(_ obj: [String: Any], _ data: Data) {
        guard let type = obj["type"] as? String else { return }
        switch type {
        case "hello":
            if let s = obj["serverTime"] as? String, let t = Api.isoFractional.date(from: s) ?? Api.iso.date(from: s) {
                clockOffset = t.timeIntervalSinceNow
            }
            socket.send(helloMessage(type: true))
        case "connect.devices":
            if let list = try? Api.decoder.decode(DevicesEvent.self, from: data) {
                devices = list.devices
                activeId = list.activeDeviceId
            }
        case "connect.state":
            if let e = try? Api.decoder.decode(StateEvent.self, from: data) {
                apply(state: e.state)
            }
        case "connect.active":
            if let e = try? Api.decoder.decode(ActiveEvent.self, from: data) {
                activeId = e.deviceId
                if let s = e.state { apply(state: s) }
            }
        case "connect.ack":
            if let id = obj["id"] as? String, let done = pendingAcks.removeValue(forKey: id) {
                done(obj["ok"] as? Bool ?? false, obj["error"] as? String)
            }
        default:
            break
        }
    }

    private func helloMessage(type: Bool) -> [String: Any] {
        var hello: [String: Any] = [
            "deviceId": deviceId,
            "name": "Apple Watch",
            "kind": "watch",
            "platform": "watchos",
            "appVersion": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "",
            "capabilities": ["volume": false, "dj": false, "remote": true],
        ]
        if type { hello["type"] = "connect.hello" }
        return hello
    }

    private func startPolling() {
        pollTask?.cancel()
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                await self?.poll()
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }

    /// Отметиться и забрать «Где играть» и снимок активного.
    private func poll() async {
        guard let credentials else { return }
        let api = Api(credentials: credentials)
        guard let body = try? JSONSerialization.data(withJSONObject: helloMessage(type: false)),
              let url = api.url("/api/devices/remote")
        else { return }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.httpBody = body
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.setValue("Bearer \(credentials.token)", forHTTPHeaderField: "Authorization")
        do {
            let (data, res) = try await Api.session.data(for: req)
            let status = (res as? HTTPURLResponse)?.statusCode ?? 0
            if status == 401 {
                unauthorized()
                return
            }
            guard status == 200, let overview = try? Api.decoder.decode(Overview.self, from: data) else { return }
            offline = false
            if let date = (res as? HTTPURLResponse)?.value(forHTTPHeaderField: "Date"),
               let server = Self.httpDate.date(from: date), abs(server.timeIntervalSinceNow) > 2 {
                clockOffset = server.timeIntervalSinceNow
            }
            devices = overview.devices
            activeId = overview.activeDeviceId
            apply(state: overview.state)
        } catch {
            offline = !socket.isOpen
        }
    }

    private static let httpDate: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "EEE, dd MMM yyyy HH:mm:ss zzz"
        return f
    }()

    /// Кнопка «Моя волна» на циферблате: ничего не играет — включить волну, играет — показать её экран.
    func openWave() {
        page = 1
        waveFromWidget = true
    }

    private var waveFromWidget = false

    private func apply(state new: PlaybackState?) {
        let old = state
        state = new
        if waveFromWidget, !devices.isEmpty {
            waveFromWidget = false
            if new?.current == nil, target != nil { startWave() }
        }
        if new?.current?.track.id != old?.current?.track.id { likedOverride = [:] }
        if old?.current?.track.id != new?.current?.track.id || old?.playing != new?.playing {
            updateWidget()
        }
    }

    // MARK: Что показывать

    var active: ConnectDevice? { devices.first { $0.id == activeId } }

    /// Куда идут команды: активное в сети, иначе выбранное, иначе первое в сети, где играет звук.
    var target: ConnectDevice? {
        if let a = active, a.online { return a }
        if let p = devices.first(where: { $0.id == preferredId && $0.online }) { return p }
        return devices.first { $0.online && $0.playable && $0.id != deviceId }
    }

    var playableDevices: [ConnectDevice] {
        devices.filter { $0.playable && $0.id != deviceId }
    }

    var track: Track? { state?.current?.track }

    var liked: Bool {
        guard let track else { return false }
        return likedOverride[track.id] ?? track.liked ?? false
    }

    var serverNow: Date { Date().addingTimeInterval(clockOffset) }

    var canChangeVolume: Bool { target?.capabilities.volume == true && state != nil }

    // MARK: Команды

    func send(_ command: [String: Any], optimistic: (() -> Void)? = nil) {
        guard let to = target?.id, let credentials else {
            notice = String(localized: "No device to play on")
            return
        }
        optimistic?()
        let id = UUID().uuidString
        if socket.isOpen {
            socket.send(["type": "connect.command", "id": id, "to": to, "command": command])
            pendingAcks[id] = { [weak self] ok, error in self?.acked(ok, error) }
            Task { [weak self] in
                try? await Task.sleep(for: .seconds(5))
                guard let self, self.pendingAcks.removeValue(forKey: id) != nil else { return }
                self.acked(false, "timeout")
            }
            return
        }
        Task {
            let ack = await post(credentials, "/api/devices/remote/command", ["id": id, "to": to, "command": command])
            acked(ack?.ok ?? false, ack?.error ?? "offline")
            await poll()
        }
    }

    private func acked(_ ok: Bool, _ error: String?) {
        guard !ok else { return }
        notice = switch error {
        case "offline": String(localized: "Device is offline")
        default: String(localized: "Device didn't respond")
        }
    }

    private func post(_ credentials: Credentials, _ path: String, _ body: [String: Any]) async -> AckReply? {
        let api = Api(credentials: credentials)
        guard let url = api.url(path), let data = try? JSONSerialization.data(withJSONObject: body) else { return nil }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.httpBody = data
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.setValue("Bearer \(credentials.token)", forHTTPHeaderField: "Authorization")
        guard let (out, res) = try? await Api.session.data(for: req) else { return nil }
        if (res as? HTTPURLResponse)?.statusCode == 401 {
            unauthorized()
            return nil
        }
        return try? Api.decoder.decode(AckReply.self, from: out)
    }

    func togglePlay() {
        let playing = state?.playing ?? false
        send(["action": playing ? "pause" : "play"]) { [weak self] in self?.state?.playing = !playing }
    }

    func next() { send(["action": "next"]) }

    func previous() { send(["action": "previous"]) }

    func setVolume(_ value: Double) {
        send(["action": "setVolume", "volume": max(0, min(1, value))]) { [weak self] in
            self?.state?.volume = value
        }
    }

    func toggleLike() {
        guard let track, let credentials else { return }
        let now = !liked
        likedOverride[track.id] = now
        Task {
            do {
                try await Api(credentials: credentials).setLiked(track.id, now)
            } catch {
                likedOverride[track.id] = !now
                notice = String(localized: "Couldn't save")
            }
        }
    }

    /// «Моя волна»: без настроек — как настроено на устройстве; с настроением или сочетанием — с ними.
    func startWave(_ settings: WaveSettings? = nil) {
        var command: [String: Any] = ["action": "startWave"]
        if let settings, let data = try? JSONEncoder().encode(settings),
           let dict = try? JSONSerialization.jsonObject(with: data) {
            command["settings"] = dict
        }
        send(command)
        page = 0
    }

    func play(_ tracks: [Track], context: PlayContext) {
        guard !tracks.isEmpty else { return }
        let encoded = (try? JSONSerialization.jsonObject(with: JSONEncoder().encode(context))) ?? [:]
        send(["action": "playTracks", "trackIds": tracks.prefix(300).map(\.id), "index": 0, "context": encoded, "shuffle": false])
        page = 0
    }

    /// Звук — на это устройство. Ничего не играло — оно станет получателем команд.
    func choose(_ device: ConnectDevice) {
        preferredId = device.id
        guard let credentials, activeId != nil, device.id != activeId else { return }
        let id = UUID().uuidString
        if socket.isOpen {
            socket.send(["type": "connect.transfer", "id": id, "to": device.id])
            pendingAcks[id] = { [weak self] ok, error in self?.acked(ok, error) }
            return
        }
        Task {
            let ack = await post(credentials, "/api/devices/remote/transfer", ["id": id, "to": device.id])
            acked(ack?.ok ?? false, ack?.error ?? "offline")
            await poll()
        }
    }

    // MARK: Медиатека

    func loadLibrary(force: Bool = false) async {
        guard let credentials, force || !libraryLoaded else { return }
        let api = Api(credentials: credentials)
        async let presets = try? api.wavePresets()
        async let playlists = try? api.playlists()
        async let recent = try? api.recent()
        let (p, l, r) = await (presets, playlists, recent)
        if let p { self.presets = p }
        if let l { self.playlists = l }
        if let r { self.recent = r }
        libraryLoaded = p != nil || l != nil
    }

    func playLiked() {
        load { api in
            (try await api.likedTracks(), PlayContext(type: "library", id: "liked", title: String(localized: "Liked Songs")))
        }
    }

    func play(_ playlist: PlaylistSummary) {
        load { api in
            (try await api.playlistTracks(playlist.id), PlayContext(type: "playlist", id: playlist.id, title: playlist.title))
        }
    }

    func play(_ item: RecentItem) {
        load { api in
            let tracks: [Track] = switch item.type {
            case "album": try await api.albumTracks(item.id)
            case "artist": try await api.artistTracks(item.id)
            default: try await api.playlistTracks(item.id)
            }
            return (tracks, PlayContext(type: item.type, id: item.id, title: item.title))
        }
    }

    private func load(_ fetch: @escaping (Api) async throws -> ([Track], PlayContext)) {
        guard let credentials else { return }
        busy = true
        Task {
            defer { busy = false }
            do {
                let (tracks, context) = try await fetch(Api(credentials: credentials))
                play(tracks, context: context)
            } catch ApiError.unauthorized {
                unauthorized()
            } catch {
                notice = String(localized: "Couldn't load")
            }
        }
    }

    // MARK: Виджет

    private func updateWidget() {
        guard let track else {
            NowPlayingSnapshot.save(nil)
            WidgetCenter.shared.reloadAllTimelines()
            return
        }
        let playing = state?.playing ?? false
        Task {
            var snapshot = NowPlayingSnapshot(title: track.title, artist: track.artistLine, playing: playing, cover: nil, tint: nil, updatedAt: Date())
            if let path = track.cover, let credentials, let art = await Artwork.shared.load(path, credentials: credentials) {
                snapshot.cover = art.thumbnail
                snapshot.tint = art.tintHex
            }
            NowPlayingSnapshot.save(snapshot)
            WidgetCenter.shared.reloadAllTimelines()
        }
    }
}

// MARK: События сервера

private struct DevicesEvent: Decodable {
    var devices: [ConnectDevice]
    var activeDeviceId: String?
}

private struct StateEvent: Decodable {
    var state: PlaybackState
}

private struct ActiveEvent: Decodable {
    var deviceId: String?
    var state: PlaybackState?
}

private struct Overview: Decodable {
    var devices: [ConnectDevice]
    var activeDeviceId: String?
    var state: PlaybackState?
}

private struct AckReply: Decodable {
    var ok: Bool
    var error: String?
}
