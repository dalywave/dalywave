import Combine
import Foundation
import SwiftUI

/// Что показывает сцена: свой плеер, если звук здесь, иначе снимок активного устройства.
struct NowShown: Equatable {
    var track: Track
    var next: Track?
    var context: PlayContext?
    var playing: Bool
    var pending: String?
    var positionMs: Int
    var durationMs: Int
    /// Звук играет на этом телевизоре
    var local: Bool
    /// Где играет, если не здесь
    var deviceName: String?
}

enum Tab: String, CaseIterable, Identifiable {
    case now, wave, library, search, party
    var id: String { rawValue }

    var title: String {
        switch self {
        case .now: "Сейчас играет"
        case .wave: "Моя волна"
        case .library: "Медиатека"
        case .search: "Поиск"
        case .party: "Вечеринка"
        }
    }
}

/// Всё состояние телевизора: вход, связь с сервером, «Где играть», плеер, медиатека и вечеринка.
///
/// Связь — WebSocket `/api/ws` (`connect.*`), как у приложений. Телевизор — полноценное устройство:
/// играет звук сам, выполняет команды пультов (телефон, часы, сайт) и публикует снимки состояния;
/// пока звук на другом устройстве — показывает его снимок на сцене и шлёт команды туда.
@MainActor
final class Store: ObservableObject {
    @Published private(set) var credentials: Credentials?
    @Published var tab: Tab = .now

    @Published private(set) var devices: [ConnectDevice] = []
    @Published private(set) var activeId: String?
    /// Снимок другого активного устройства
    @Published private(set) var remote: PlaybackState?
    @Published private(set) var offline = false

    @Published private(set) var presets: [WavePreset] = []
    @Published private(set) var playlists: [PlaylistSummary] = []
    @Published private(set) var recent: [RecentItem] = []
    @Published private(set) var liked: [Track] = []
    @Published private(set) var libraryLoaded = false
    @Published private(set) var busy = false
    @Published var notice: String?

    @Published private(set) var party: Party?
    @Published private(set) var jam: Jam?
    @Published private(set) var lyrics: TrackLyrics?
    private var lyricsFor: String?

    let player = Player()
    private let socket = ConnectSocket()
    private var reconnectTask: Task<Void, Never>?
    private var publishTask: Task<Void, Never>?
    private var heartbeat: Timer?
    private var bag: Set<AnyCancellable> = []
    /// Время сервера минус время телевизора
    private var clockOffset: TimeInterval = 0
    private var claiming = false

    let deviceId: String = {
        let key = "connect.deviceId"
        if let id = UserDefaults.standard.string(forKey: key) { return id }
        let id = "tv-" + UUID().uuidString.lowercased()
        UserDefaults.standard.set(id, forKey: key)
        return id
    }()

    var api: Api? { credentials.map(Api.init(credentials:)) }
    var serverNow: Date { Date().addingTimeInterval(clockOffset) }
    var isActive: Bool { activeId == deviceId }

    init() {
        credentials = Credentials.load()
        player.api = api
        player.onChange = { [weak self] important in self?.playerChanged(important) }
        player.onStartedHere = { [weak self] in self?.claim() }
        socket.onEvent = { [weak self] event in self?.onSocket(event) }
        // Плеер — отдельный ObservableObject: экраны следят за Store
        player.objectWillChange.sink { [weak self] in self?.objectWillChange.send() }.store(in: &bag)
        player.$notice.compactMap { $0 }.sink { [weak self] text in self?.notice = text }.store(in: &bag)
    }

    // MARK: Вход

    func signedIn(_ c: Credentials) {
        c.save()
        credentials = c
        player.api = api
    }

    func signOut() {
        let api = api
        Task { await api?.logout() }
        forget()
    }

    private func forget() {
        Credentials.delete()
        socket.close()
        heartbeat?.invalidate()
        player.yield()
        credentials = nil
        player.api = nil
        devices = []
        activeId = nil
        remote = nil
        party = nil
        jam = nil
        libraryLoaded = false
    }

    /// Ключ больше не действует (отозван в «Доступе»): снова экран кода.
    func unauthorized() {
        forget()
    }

    func start() {
        guard let credentials else { return }
        connect(credentials)
        Task { await loadLibrary() }
        Task { await loadParty() }
        #if targetEnvironment(simulator)
        runSimulatorScript()
        #endif
    }

    // MARK: Связь

    private func connect(_ credentials: Credentials) {
        reconnectTask?.cancel()
        socket.open(credentials)
    }

    private func onSocket(_ event: ConnectSocket.Event) {
        switch event {
        case .open:
            offline = false
            heartbeat?.invalidate()
            // Активное устройство напоминает о позиции раз в 5 с
            heartbeat = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self, self.isActive, self.player.hasQueue else { return }
                    self.publish()
                }
            }
        case let .closed(unauthorized):
            heartbeat?.invalidate()
            if unauthorized {
                self.unauthorized()
                return
            }
            offline = true
            guard let credentials else { return }
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
        #if targetEnvironment(simulator)
        if type == "connect.active" || type == "connect.command" {
            print("DWTV ws \(type) device=\(obj["deviceId"] ?? "-") transfer=\(obj["transfer"] ?? "-") action=\((obj["command"] as? [String: Any])?["action"] ?? "-")")
        }
        #endif
        switch type {
        case "hello":
            if let s = obj["serverTime"] as? String, let t = Api.isoFractional.date(from: s) ?? Api.iso.date(from: s) {
                clockOffset = t.timeIntervalSinceNow
            }
            socket.send(hello())
            if player.hasQueue, player.playing { claim() }
        case "connect.devices":
            if let list = try? Api.decoder.decode(DevicesEvent.self, from: data) {
                devices = list.devices
                if !claiming { activeId = list.activeDeviceId }
            }
        case "connect.active":
            if let e = try? Api.decoder.decode(ActiveEvent.self, from: data) { onActive(e) }
        case "connect.state":
            if let e = try? Api.decoder.decode(StateEvent.self, from: data), e.deviceId != deviceId {
                if activeId != e.deviceId {
                    activeId = e.deviceId
                    player.yield()
                }
                remote = e.state
            }
        case "connect.command":
            if let id = obj["id"] as? String, let from = obj["from"] as? String,
               let command = obj["command"] as? [String: Any] {
                Task { await self.run(command, id: id, from: from) }
            }
        case "jam.changed":
            Task { await loadJam() }
        case "party.changed":
            if let p = try? Api.decoder.decode(Party.self, from: data) { party = p }
        default:
            break
        }
    }

    private func hello() -> [String: Any] {
        [
            "type": "connect.hello",
            "deviceId": deviceId,
            "name": "",
            "kind": "tv",
            "platform": "tvos",
            "appVersion": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "",
            "capabilities": ["volume": true, "dj": false, "remote": false],
        ]
    }

    private func onActive(_ e: ActiveEvent) {
        if e.deviceId == deviceId {
            // Уже знаем, что активны мы: подтверждение снимка, отвечать на него снимком — по кругу
            let was = isActive && !claiming
            claiming = false
            activeId = deviceId
            remote = nil
            // Сюда перенесли воспроизведение — продолжаем с того же места
            if let s = e.state, s.current != nil, e.transfer || !player.hasQueue {
                player.adopt(s, position: s.position(at: serverNow), play: e.transfer ? s.playing : false)
            } else if !was {
                publish()
            }
            return
        }
        // Последний выбор человека главнее: звук переехал — здесь он останавливается
        claiming = false
        activeId = e.deviceId
        if e.deviceId != nil { player.yield() }
        remote = e.state
    }

    private func claim() {
        guard !isActive else { return }
        claiming = true
        activeId = deviceId
        remote = nil
        socket.send(["type": "connect.claim"])
        publish()
    }

    private func playerChanged(_ important: Bool) {
        let id = player.current?.track.id
        if id != lyricsFor {
            lyricsFor = id
            lyrics = nil
            if let id, let api {
                Task {
                    let l = try? await api.lyrics(id)
                    if self.lyricsFor == id { self.lyrics = l }
                }
            }
        }
        guard isActive, important else { return }
        // Пачка изменений (трек и очередь сразу) — один снимок
        publishTask?.cancel()
        publishTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            self?.publish()
        }
    }

    private func publish() {
        guard isActive else { return }
        socket.send(["type": "connect.state", "state": player.snapshot()])
    }

    // MARK: Команды пультов

    private func run(_ command: [String: Any], id: String, from: String) async {
        let action = command["action"] as? String ?? ""
        let ok: Bool
        var error: String?
        // Новое (альбом, волна, трек гостя) играет здесь; остальное — только если звук здесь
        let starts = ["playTracks", "startWave", "enqueue", "playNext"].contains(action)
        if !starts, !player.hasQueue {
            ok = false
            error = "inactive"
        } else {
            ok = await execute(action, command)
            if !ok { error = "failed" }
        }
        socket.send(["type": "connect.ack", "id": id, "to": from, "ok": ok, "error": error ?? NSNull()])
        publish()
    }

    private func execute(_ action: String, _ c: [String: Any]) async -> Bool {
        switch action {
        case "play": player.play()
        case "pause": player.pause()
        case "toggle": player.toggle()
        case "next": player.next()
        case "previous": player.previous()
        case "seek": player.seek(c["positionMs"] as? Int ?? 0)
        case "setVolume": player.setVolume(c["volume"] as? Double ?? 1)
        case "setMuted": player.setMuted(c["muted"] as? Bool ?? false)
        case "setShuffle": player.setShuffle(c["shuffle"] as? Bool ?? false)
        case "setRepeat": player.setRepeat(c["mode"] as? String ?? "off")
        case "setTransition": break
        case "dislike": player.dislike()
        case "notNow": player.notNow()
        case "moreLikeThis": break
        case "jumpTo": player.jumpTo(c["uid"] as? String ?? "")
        case "remove": player.remove(c["uid"] as? String ?? "")
        case "clearManual": player.clearManual()
        case "moveManual": player.moveManual(from: c["from"] as? Int ?? 0, to: c["to"] as? Int ?? 0)
        case "startWave", "setWave":
            let settings = c["settings"].map { JSON(any: $0) }.flatMap { $0 == .null ? nil : $0 }
            player.startWave(settings)
        case "enqueue", "playNext":
            guard let raw = c["track"], let data = try? JSONSerialization.data(withJSONObject: raw),
                  let track = try? Api.decoder.decode(Track.self, from: data)
            else { return false }
            action == "enqueue" ? player.enqueue(track) : player.playNext(track)
        case "playTracks":
            guard let api, let ids = c["trackIds"] as? [String] else { return false }
            guard let found = try? await api.lookup(ids) else { return false }
            let byId = Dictionary(found.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
            let tracks = ids.compactMap { byId[$0] }
            let ctx = (c["context"] as? [String: Any]).flatMap { obj -> PlayContext? in
                guard let data = try? JSONSerialization.data(withJSONObject: obj) else { return nil }
                return try? JSONDecoder().decode(PlayContext.self, from: data)
            } ?? PlayContext(type: "remote", id: nil, title: "")
            player.playTracks(tracks, index: c["index"] as? Int ?? 0, context: ctx, shuffle: c["shuffle"] as? Bool ?? false)
        default:
            return false
        }
        return true
    }

    /// Команда активному устройству, когда звук не здесь.
    private func send(_ command: [String: Any]) {
        guard let to = activeId, to != deviceId else { return }
        socket.send(["type": "connect.command", "id": UUID().uuidString, "to": to, "command": command])
    }

    // MARK: Сцена

    var activeDevice: ConnectDevice? { devices.first { $0.id == activeId } }

    func shown(at now: Date) -> NowShown? {
        if let cur = player.current {
            return NowShown(
                track: cur.track,
                next: player.nextEntry?.track,
                context: player.context,
                playing: player.playing,
                pending: player.pending,
                positionMs: player.positionMs,
                durationMs: player.durationMs,
                local: true,
                deviceName: nil
            )
        }
        guard let s = remote, let cur = s.current else { return nil }
        // Устройство пропало из сети — его звук уже не идёт
        let online = activeDevice?.online ?? false
        return NowShown(
            track: cur.track,
            next: (s.manual.first ?? s.upcoming.first)?.track,
            context: s.context,
            playing: s.playing && online,
            pending: s.pending,
            positionMs: online ? s.position(at: now.addingTimeInterval(clockOffset)) : s.positionMs,
            durationMs: s.durationMs,
            local: false,
            deviceName: online ? activeDevice?.name : nil
        )
    }

    func togglePlay() {
        if player.hasQueue {
            player.toggle()
        } else if let s = remote {
            send(["action": s.playing ? "pause" : "play"])
            remote?.playing.toggle()
        }
    }

    func next() {
        if player.hasQueue { player.next() } else { send(["action": "next"]) }
    }

    func previous() {
        if player.hasQueue { player.previous() } else { send(["action": "previous"]) }
    }

    /// Звук — сюда, на телевизор: с того же места с той же очередью.
    func playHere() {
        guard !player.hasQueue, remote != nil else { return }
        socket.send(["type": "connect.transfer", "to": deviceId])
    }

    /// Кто добавил трек: имя гостя «Слушать вместе».
    func addedBy(_ trackId: String) -> String? {
        jam?.requests.first { $0.track.id == trackId && $0.status == "added" }?.guestName
    }

    // MARK: Медиатека

    func loadLibrary(force: Bool = false) async {
        guard let api, force || !libraryLoaded else { return }
        async let presets = try? api.wavePresets()
        async let playlists = try? api.playlists()
        async let recent = try? api.recent()
        async let liked = try? api.likedTracks()
        let (p, l, r, lk) = await (presets, playlists, recent, liked)
        if let p { self.presets = p }
        if let l { self.playlists = l }
        if let r { self.recent = r }
        if let lk { self.liked = lk }
        libraryLoaded = p != nil || l != nil
        TopShelf.save(recent: self.recent, playlists: self.playlists, api: api)
    }

    func playLiked(from index: Int = 0) {
        let tracks = liked
        if !tracks.isEmpty {
            player.playTracks(tracks, index: index, context: PlayContext(type: "library", id: "liked", title: "Любимые"))
            tab = .now
            return
        }
        load { api in (try await api.likedTracks(), PlayContext(type: "library", id: "liked", title: "Любимые")) }
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

    func play(album: AlbumSummary) {
        load { api in (try await api.albumTracks(album.id), PlayContext(type: "album", id: album.id, title: album.title)) }
    }

    func play(artist: ArtistSummary) {
        load { api in (try await api.artistTracks(artist.id), PlayContext(type: "artist", id: artist.id, title: artist.name)) }
    }

    func play(_ tracks: [Track], index: Int, context: PlayContext) {
        player.playTracks(tracks, index: index, context: context)
        tab = .now
    }

    func startWave(_ settings: WaveSettings? = nil) {
        let json: JSON? = settings.flatMap { s in
            (try? JSONEncoder().encode(s)).flatMap { try? JSONDecoder().decode(JSON.self, from: $0) }
        }
        player.startWave(json)
        tab = .now
    }

    private func load(_ fetch: @escaping (Api) async throws -> ([Track], PlayContext)) {
        guard let api else { return }
        busy = true
        Task {
            defer { busy = false }
            do {
                let (tracks, context) = try await fetch(api)
                guard !tracks.isEmpty else {
                    notice = "Здесь пока пусто"
                    return
                }
                player.playTracks(tracks, context: context)
                tab = .now
            } catch ApiError.unauthorized {
                unauthorized()
            } catch {
                notice = "Не удалось загрузить"
            }
        }
    }

    // MARK: Вечеринка

    func loadParty() async {
        guard let api else { return }
        if let p = try? await api.party() { party = p }
        await loadJam()
    }

    func loadJam() async {
        guard let api else { return }
        do {
            jam = try await api.jam()
        } catch ApiError.unauthorized {
            unauthorized()
        } catch {}
    }

    func updateParty(_ patch: [String: Any]) {
        guard let api else { return }
        Task {
            do {
                party = try await api.updateParty(patch)
                await loadJam()
            } catch {
                notice = "Не удалось сохранить"
            }
        }
    }

    // MARK: Top Shelf: ссылки dalywave-tv://

    func open(_ url: URL) {
        guard url.scheme == "dalywave-tv" else { return }
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let value = { (name: String) in items.first { $0.name == name }?.value }
        switch url.host {
        case "wave":
            startWave(value("mood").map { WaveSettings(mood: $0) })
        case "play":
            if let type = value("type"), let id = value("id") {
                if type == "playlist" {
                    play(PlaylistSummary(id: id, title: value("title") ?? ""))
                } else if let item = RecentItem(type: type, id: id, title: value("title") ?? "") {
                    play(item)
                }
            }
        default:
            tab = .now
        }
    }

    #if targetEnvironment(simulator)
    /// Только симулятор: проверки экранов без пульта — `DALYWAVE_TV_TAB` (now, wave, library, search, party),
    /// `DALYWAVE_TV_ACTIONS` (wave, calm, liked, track:<id>, pause, party, remote), `DALYWAVE_TV_SEARCH`.
    @Published var simulatorSearch: String?

    private func runSimulatorScript() {
        let env = ProcessInfo.processInfo.environment
        if let t = env["DALYWAVE_TV_TAB"].flatMap(Tab.init(rawValue:)) { tab = t }
        simulatorSearch = env["DALYWAVE_TV_SEARCH"]
        let actions = env["DALYWAVE_TV_ACTIONS"]?.split(separator: ",").map(String.init) ?? []
        let keepTab = env["DALYWAVE_TV_TAB"].flatMap(Tab.init(rawValue:))
        Task {
            for action in actions {
                try? await Task.sleep(for: .seconds(2))
                switch action {
                case "wave": startWave()
                case "calm": startWave(WaveSettings(mood: "calm"))
                case "liked": playLiked()
                case "pause": player.pause()
                case "party": updateParty(["active": true])
                case let a where a.hasPrefix("probe:"):
                    // Какие форматы играет AVPlayer: адреса через «|», в журнал — статус и время
                    for url in a.dropFirst(6).split(separator: "|").compactMap({ URL(string: String($0)) }) {
                        print("DWTV probe \(url.lastPathComponent): \(await Player.probe(url))")
                    }
                case let a where a.hasPrefix("track:"):
                    if let api, let tracks = try? await api.lookup(a.dropFirst(6).split(separator: "+").map(String.init)) {
                        play(tracks, index: 0, context: PlayContext(type: "search", id: nil, title: "Поиск"))
                    }
                default: break
                }
                if let keepTab { tab = keepTab }
            }
            // Журнал AVPlayer: проверка звука без колонок
            for _ in 0..<40 {
                try? await Task.sleep(for: .seconds(3))
                print("DWTV player: \(player.debugStatus) pos=\(player.positionMs) track=\(player.current?.track.title ?? "-") pending=\(player.pending ?? "-") active=\(isActive)")
            }
        }
    }
    #endif
}

// MARK: События сервера

private struct DevicesEvent: Decodable {
    var devices: [ConnectDevice]
    var activeDeviceId: String?
}

private struct StateEvent: Decodable {
    var deviceId: String
    var state: PlaybackState
}

private struct ActiveEvent: Decodable {
    var deviceId: String?
    var transfer: Bool
    var state: PlaybackState?
}
