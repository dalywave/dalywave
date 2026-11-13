import AVFoundation
import MediaPlayer
import UIKit

/// Плеер телевизора: своя очередь (как у приложений: история, текущий, «добавлено вручную», дальше),
/// звук — AVQueuePlayer. Ссылки на аудио выдаёт сервер (`/stream?aac=true`): AVPlayer не играет Opus,
/// поэтому сервер отдаёт AAC/ALAC/MP3 или делает копию AAC 256 — пока её делают, ответ `preparing`.
/// Следующий трек загружается заранее и встаёт в AVQueuePlayer — переход без паузы.
@MainActor
final class Player: ObservableObject {
    @Published private(set) var current: QueueEntry?
    @Published private(set) var manual: [QueueEntry] = []
    @Published private(set) var upcoming: [QueueEntry] = []
    @Published private(set) var history: [QueueEntry] = []
    @Published private(set) var context: PlayContext?
    /// Намерение: играть (звук может ещё грузиться)
    @Published private(set) var playing = false
    /// Файла ещё нет: `acquiring` или `waiting_for_downloader`
    @Published private(set) var pending: String?
    @Published private(set) var positionMs = 0
    @Published private(set) var durationMs = 0
    @Published private(set) var volume: Double = 1
    @Published private(set) var muted = false
    @Published private(set) var shuffle = false
    @Published private(set) var repeatMode = "off"
    @Published private(set) var waveSessionId: String?
    @Published private(set) var wave: JSON?
    @Published var notice: String?

    var api: Api?
    /// Состояние изменилось: `true` — важное (трек, пауза, очередь), публиковать сразу
    var onChange: ((Bool) -> Void)?
    /// Звук начал играть по действию здесь: телевизор заявляет, что активен он
    var onStartedHere: (() -> Void)?

    private let av = AVQueuePlayer()
    private var item: AVPlayerItem?
    private var preloaded: (uid: String, item: AVPlayerItem)?
    private var loadTask: Task<Void, Never>?
    private var preloadTask: Task<Void, Never>?
    private var refilling = false
    private var timeObserver: Any?
    private var endObserver: NSObjectProtocol?
    private var statusObserver: NSKeyValueObservation?

    // Прослушивание для событий
    private var eventEntry: QueueEntry?
    private var eventStartedAt = Date()
    private var eventStart = 0
    private var playedMs = 0
    private var lastTick: CFTimeInterval?
    private var unsent: [[String: Any]] = []

    init() {
        av.automaticallyWaitsToMinimizeStalling = true
        av.actionAtItemEnd = .advance
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
        timeObserver = av.addPeriodicTimeObserver(forInterval: CMTime(value: 1, timescale: 2), queue: .main) { [weak self] time in
            MainActor.assumeIsolated { self?.tick(time) }
        }
        endObserver = NotificationCenter.default.addObserver(
            forName: AVPlayerItem.didPlayToEndTimeNotification, object: nil, queue: .main
        ) { [weak self] note in
            let ended = note.object as? AVPlayerItem
            MainActor.assumeIsolated { self?.itemEnded(ended) }
        }
        applyVolume()
        setupRemoteCommands()
    }

    var hasQueue: Bool { current != nil }
    var nextEntry: QueueEntry? { manual.first ?? upcoming.first }

    // MARK: Что играть

    func playTracks(_ tracks: [Track], index: Int = 0, context: PlayContext, shuffle: Bool = false) {
        guard !tracks.isEmpty else { return }
        finishEvent(playing ? "skipped" : "stopped")
        var entries = tracks.map { QueueEntry(track: $0, source: "context") }
        let start = min(max(0, index), entries.count - 1)
        var first = entries.remove(at: start)
        var rest = Array(entries[start...]) + (shuffle ? Array(entries[..<start]) : [])
        if shuffle { rest.shuffle() }
        if !shuffle, start > 0 { history = Array(entries[..<start].suffix(30)) } else { history = [] }
        first.source = "context"
        upcoming = rest
        self.context = context
        self.shuffle = shuffle
        waveSessionId = nil
        wave = nil
        current = first
        playing = true
        load()
        onStartedHere?()
    }

    /// «Моя волна»: настройки — как в команде пульта (без них — прежние волны или по умолчанию).
    func startWave(_ settings: JSON? = nil) {
        guard let api else { return }
        let use = settings ?? wave ?? .object([:])
        finishEvent(playing ? "skipped" : "stopped")
        loadTask?.cancel()
        av.pause()
        playing = true
        pending = "acquiring"
        onChange?(true)
        onStartedHere?()
        loadTask = Task { [weak self] in
            do {
                var session = try await api.startWave((use.any as? [String: Any]) ?? [:])
                // Начало ещё качается: ждём, пока сервер не скажет «готово» (до полутора минут)
                var waited = 0
                while session.state == "preparing", session.queue.count < 1 || waited < 4, waited < 45 {
                    try await Task.sleep(for: .seconds(2))
                    session = try await api.waveSession(session.id)
                    waited += 1
                }
                guard let self, !Task.isCancelled else { return }
                var entries = session.queue.map { QueueEntry(track: $0.track, source: "wave", reason: $0.reason?.text) }
                guard !entries.isEmpty else {
                    self.pending = nil
                    self.playing = false
                    self.notice = "Волна не собралась"
                    self.onChange?(true)
                    return
                }
                self.waveSessionId = session.id
                self.wave = use
                self.context = PlayContext(type: "wave", id: nil, title: session.title)
                self.history = []
                self.current = entries.removeFirst()
                self.upcoming = entries
                self.shuffle = false
                self.load()
            } catch {
                guard let self, !Task.isCancelled else { return }
                self.pending = nil
                self.playing = false
                self.notice = "Не удалось включить волну"
                self.onChange?(true)
            }
        }
    }

    /// Перенос сюда: та же очередь, то же место.
    func adopt(_ s: PlaybackState, position: Int, play: Bool) {
        finishEvent("transferred")
        current = s.current
        manual = s.manual
        upcoming = s.upcoming
        history = s.history
        context = s.context
        shuffle = s.shuffle
        repeatMode = s.repeatMode
        waveSessionId = s.waveSessionId
        if let w = s.wave, let data = try? JSONEncoder().encode(w) { wave = try? JSONDecoder().decode(JSON.self, from: data) }
        playing = play && s.current != nil
        load(seekTo: position)
    }

    /// Активным стало другое устройство: здесь звук останавливается.
    func yield() {
        guard current != nil else { return }
        finishEvent("transferred")
        loadTask?.cancel()
        preloadTask?.cancel()
        av.pause()
        av.removeAllItems()
        item = nil
        preloaded = nil
        playing = false
        current = nil
        manual = []
        upcoming = []
        history = []
        pending = nil
        updateNowPlaying()
        onChange?(false)
    }

    // MARK: Управление

    func play() {
        guard current != nil else { return }
        playing = true
        if item == nil { load(seekTo: positionMs) } else { av.play() }
        onStartedHere?()
        changed()
    }

    func pause() {
        playing = false
        av.pause()
        changed()
    }

    func toggle() { playing ? pause() : play() }

    func next(_ reason: String = "skipped") {
        if reason == "skipped", waveSessionId != nil, let track = current?.track {
            feedback(track, kind: "skip")
        }
        advance(reason)
    }

    func previous() {
        if positionMs > 3000 || history.isEmpty {
            seek(0)
            return
        }
        finishEvent("previous")
        if let cur = current {
            if cur.source == "manual" { manual.insert(cur, at: 0) } else { upcoming.insert(cur, at: 0) }
        }
        current = history.removeLast()
        load()
    }

    func seek(_ ms: Int) {
        let target = max(0, durationMs > 0 ? min(ms, durationMs - 500) : ms)
        positionMs = target
        lastTick = nil
        av.seek(to: CMTime(value: CMTimeValue(target), timescale: 1000), toleranceBefore: .zero, toleranceAfter: .zero)
        changed()
    }

    func setVolume(_ v: Double) {
        volume = max(0, min(1, v))
        applyVolume()
        onChange?(false)
    }

    func setMuted(_ m: Bool) {
        muted = m
        applyVolume()
        onChange?(true)
    }

    func setShuffle(_ on: Bool) {
        shuffle = on
        if on { upcoming.shuffle() }
        queueChanged()
    }

    func setRepeat(_ mode: String) {
        repeatMode = ["off", "all", "one"].contains(mode) ? mode : "off"
        queueChanged()
    }

    func jumpTo(_ uid: String) {
        if let i = manual.firstIndex(where: { $0.uid == uid }) {
            finishEvent("skipped")
            if let cur = current { pushHistory(cur) }
            current = manual.remove(at: i)
        } else if let i = upcoming.firstIndex(where: { $0.uid == uid }) {
            finishEvent("skipped")
            if let cur = current { pushHistory(cur) }
            upcoming.prefix(i).forEach { pushHistory($0) }
            current = upcoming[i]
            upcoming.removeFirst(i + 1)
        } else if let i = history.firstIndex(where: { $0.uid == uid }) {
            finishEvent("previous")
            let tail = Array(history[(i + 1)...])
            if let cur = current { upcoming.insert(cur, at: 0) }
            upcoming.insert(contentsOf: tail, at: 0)
            current = history[i]
            history.removeSubrange(i...)
        } else {
            return
        }
        playing = true
        load()
    }

    func remove(_ uid: String) {
        manual.removeAll { $0.uid == uid }
        upcoming.removeAll { $0.uid == uid }
        queueChanged()
    }

    func enqueue(_ track: Track) {
        manual.append(QueueEntry(track: track, source: "manual"))
        startIfIdle()
    }

    func playNext(_ track: Track) {
        manual.insert(QueueEntry(track: track, source: "manual"), at: 0)
        startIfIdle()
    }

    func clearManual() {
        manual = []
        queueChanged()
    }

    func moveManual(from: Int, to: Int) {
        guard manual.indices.contains(from) else { return }
        let e = manual.remove(at: from)
        manual.insert(e, at: min(max(0, to), manual.count))
        queueChanged()
    }

    func dislike() {
        if let track = current?.track { feedback(track, kind: "dislike") }
        advance("skipped")
    }

    func notNow() {
        if let track = current?.track { feedback(track, kind: "not_now") }
        advance("skipped")
    }

    /// Трек гостя или с пульта, а здесь ничего не играет — сразу играть его.
    private func startIfIdle() {
        if current == nil, !manual.isEmpty {
            current = manual.removeFirst()
            playing = true
            load()
            onStartedHere?()
        } else {
            queueChanged()
        }
    }

    private func pushHistory(_ e: QueueEntry) {
        history.append(e)
        if history.count > 50 { history.removeFirst(history.count - 50) }
    }

    private func advance(_ reason: String) {
        finishEvent(reason)
        if reason == "completed", repeatMode == "one", current != nil {
            beginEvent(at: 0)
            seek(0)
            av.play()
            return
        }
        if let cur = current { pushHistory(cur) }
        if !manual.isEmpty {
            current = manual.removeFirst()
        } else if !upcoming.isEmpty {
            current = upcoming.removeFirst()
        } else if repeatMode == "all", context?.type != "wave", !history.isEmpty {
            upcoming = history
            history = []
            current = upcoming.removeFirst()
        } else {
            // Очередь кончилась
            current = history.popLast()
            playing = false
            av.pause()
            positionMs = 0
            changed()
            return
        }
        load()
        refillWave()
    }

    private func refillWave() {
        guard context?.type == "wave", let id = waveSessionId, upcoming.count < 4, !refilling, let api else { return }
        refilling = true
        Task { [weak self] in
            let items = (try? await api.waveNext(id, count: 10)) ?? []
            guard let self else { return }
            self.refilling = false
            guard self.waveSessionId == id else { return }
            self.upcoming += items.map { QueueEntry(track: $0.track, source: "wave", reason: $0.reason?.text) }
            self.queueChanged()
        }
    }

    private func feedback(_ track: Track, kind: String) {
        guard let api, let id = waveSessionId else { return }
        let pos = positionMs
        Task { try? await api.call("POST", "/api/wave/sessions/\(id)/feedback", ["trackId": track.id, "kind": kind, "positionMs": pos]) }
    }

    // MARK: Звук

    private func load(seekTo: Int = 0) {
        loadTask?.cancel()
        preloadTask?.cancel()
        guard let entry = current, let api else {
            av.removeAllItems()
            item = nil
            changed()
            return
        }
        durationMs = entry.track.durationMs ?? 0
        positionMs = seekTo
        beginEvent(at: seekTo)
        // Уже загружен заранее — переключаемся на него
        if let p = preloaded, p.uid == entry.uid {
            preloaded = nil
            av.removeAllItems()
            av.insert(p.item, after: nil)
            install(p.item, seekTo: seekTo)
            return
        }
        preloaded = nil
        av.pause()
        av.removeAllItems()
        item = nil
        pending = nil
        changed()
        loadTask = Task { [weak self] in
            let url = await Self.resolve(api, entry.track.id) { status in
                self?.pending = status == "waiting_for_downloader" ? status : "acquiring"
                self?.onChange?(true)
            }
            guard let self, !Task.isCancelled, self.current?.uid == entry.uid else { return }
            self.pending = nil
            guard let url else {
                // Трека нет в источниках или не дождались — следующий
                self.notice = "Трек недоступен"
                self.advance("error")
                return
            }
            let item = AVPlayerItem(url: url)
            self.av.insert(item, after: nil)
            self.install(item, seekTo: seekTo)
        }
    }

    private func install(_ item: AVPlayerItem, seekTo: Int) {
        self.item = item
        if seekTo > 0 {
            av.seek(to: CMTime(value: CMTimeValue(seekTo), timescale: 1000), toleranceBefore: .zero, toleranceAfter: .zero)
        }
        applyVolume()
        if playing { av.play() } else { av.pause() }
        updateNowPlaying()
        changed()
        preloadNext()
    }

    /// Ссылка на файл для AVPlayer. Ждёт копию AAC (`preparing`) и скачивание (до 2 минут).
    static func resolve(_ api: Api, _ trackId: String, waiting: @escaping (String) -> Void) async -> URL? {
        let deadline = Date().addingTimeInterval(120)
        var failures = 0
        while !Task.isCancelled, Date() < deadline {
            do {
                let reply = try await api.stream(trackId)
                switch reply.status {
                case "ready":
                    return reply.url.flatMap { URL(string: $0) }
                case "preparing":
                    waiting("preparing")
                    try await Task.sleep(for: .milliseconds(max(1000, reply.retryAfterMs ?? 2000)))
                case "waiting_for_downloader":
                    waiting("waiting_for_downloader")
                    try await Task.sleep(for: .seconds(5))
                default:
                    waiting("acquiring")
                    try await Task.sleep(for: .seconds(3))
                }
            } catch ApiError.server(let code) where code == 404 || code == 409 {
                return nil
            } catch {
                failures += 1
                if failures > 5 || Task.isCancelled { return nil }
                try? await Task.sleep(for: .seconds(2))
            }
        }
        return nil
    }

    /// Следующий — заранее, если его файл уже готов: AVQueuePlayer перейдёт без паузы.
    private func preloadNext() {
        preloadTask?.cancel()
        if let p = preloaded {
            av.remove(p.item)
            preloaded = nil
        }
        guard repeatMode != "one", let next = nextEntry, let api, item != nil else { return }
        preloadTask = Task { [weak self] in
            guard let reply = try? await api.stream(next.track.id), reply.status == "ready",
                  let url = reply.url.flatMap({ URL(string: $0) }) else { return }
            guard let self, !Task.isCancelled, self.nextEntry?.uid == next.uid, let cur = self.item else { return }
            let item = AVPlayerItem(url: url)
            if self.av.canInsert(item, after: cur) {
                self.av.insert(item, after: cur)
                self.preloaded = (next.uid, item)
            }
        }
    }

    private func itemEnded(_ ended: AVPlayerItem?) {
        guard let ended, ended === item else { return }
        if let p = preloaded, p.uid == nextEntry?.uid, repeatMode != "one" {
            // AVQueuePlayer уже играет следующий: только очередь и события
            finishEvent("completed")
            if let cur = current { pushHistory(cur) }
            current = manual.first?.uid == p.uid ? manual.removeFirst() : upcoming.removeFirst()
            preloaded = nil
            item = p.item
            durationMs = current?.track.durationMs ?? 0
            positionMs = 0
            beginEvent(at: 0)
            updateNowPlaying()
            changed()
            preloadNext()
            refillWave()
        } else {
            advance("completed")
        }
    }

    private func queueChanged() {
        if preloaded?.uid != nextEntry?.uid { preloadNext() }
        onChange?(true)
    }

    private func changed() {
        updateNowPlaying()
        onChange?(true)
    }

    private func applyVolume() {
        av.volume = Float(volume)
        av.isMuted = muted
        #if targetEnvironment(simulator)
        // Проверки без звука в колонках. Звук — только с DALYWAVE_TV_SOUND=1
        if ProcessInfo.processInfo.environment["DALYWAVE_TV_SOUND"] != "1" { av.isMuted = true }
        #endif
    }

    #if targetEnvironment(simulator)
    /// Только симулятор: играет ли AVPlayer файл (без звука) — статус, время через 3 с и ошибка.
    static func probe(_ url: URL) async -> String {
        let p = AVPlayer(url: url)
        p.isMuted = true
        p.play()
        try? await Task.sleep(for: .seconds(3))
        let item = p.currentItem
        let out = "status=\(item?.status.rawValue ?? -1) t=\(String(format: "%.1f", p.currentTime().seconds)) err=\(item?.error.map { "\($0)" } ?? "-")"
        p.pause()
        return out
    }
    #endif

    /// Только для проверок: что делает AVPlayer.
    var debugStatus: String {
        let status = switch av.timeControlStatus {
        case .playing: "playing"
        case .paused: "paused"
        case .waitingToPlayAtSpecifiedRate: "waiting"
        @unknown default: "?"
        }
        let err = av.currentItem?.error?.localizedDescription ?? "-"
        return "\(status) t=\(String(format: "%.1f", av.currentTime().seconds)) item=\(av.currentItem?.status.rawValue ?? -1) err=\(err) muted=\(av.isMuted)"
    }

    private func tick(_ time: CMTime) {
        guard av.currentItem != nil, av.currentItem === item else {
            lastTick = nil
            return
        }
        let ms = Int(time.seconds.isFinite ? time.seconds * 1000 : 0)
        positionMs = ms
        if durationMs == 0, let d = item?.duration.seconds, d.isFinite { durationMs = Int(d * 1000) }
        let now = CACurrentMediaTime()
        if av.timeControlStatus == .playing, let last = lastTick {
            let delta = now - last
            if delta < 1.5 { playedMs += Int(delta * 1000) }
        }
        lastTick = now
    }

    // MARK: События прослушивания

    private func beginEvent(at pos: Int) {
        eventEntry = current
        eventStartedAt = Date()
        eventStart = pos
        playedMs = 0
        lastTick = nil
    }

    private func finishEvent(_ reason: String) {
        guard let entry = eventEntry else { return }
        eventEntry = nil
        guard playedMs > 0 else { return }
        var event: [String: Any] = [
            "clientEventId": UUID().uuidString.lowercased(),
            "trackId": entry.track.id,
            "startedAt": Api.isoFractional.string(from: eventStartedAt),
            "playedMs": playedMs,
            "positionStartMs": eventStart,
            "durationMs": max(durationMs, entry.track.durationMs ?? 0),
            "endReason": reason,
            "source": "stream",
            "transition": "gapless",
        ]
        let contexts = ["album", "playlist", "artist", "wave", "radio", "mix", "search", "library", "queue", "remote"]
        if let ctx = context, contexts.contains(ctx.type) {
            event["contextType"] = ctx.type
            if let id = ctx.id { event["contextId"] = id }
        }
        if let wave = waveSessionId { event["waveSessionId"] = wave }
        unsent.append(event)
        flushEvents()
    }

    func flushEvents() {
        guard let api, !unsent.isEmpty else { return }
        let batch = Array(unsent.prefix(500))
        Task { [weak self] in
            do {
                try await api.events(batch)
                let sent = Set(batch.compactMap { $0["clientEventId"] as? String })
                self?.unsent.removeAll { sent.contains($0["clientEventId"] as? String ?? "") }
            } catch {
                // Повтор — со следующим прослушиванием: clientEventId делает его безопасным
            }
        }
    }

    // MARK: Снимок для «Где играть»

    func snapshot() -> [String: Any] {
        var s: [String: Any] = [
            "manual": manual.prefix(50).map(\.json),
            "upcoming": upcoming.prefix(50).map(\.json),
            "history": history.suffix(20).map(\.json),
            "playing": playing,
            "positionMs": positionMs,
            "durationMs": durationMs,
            "volume": volume,
            "muted": muted,
            "transition": "gapless",
            "shuffle": shuffle,
            "repeat": repeatMode,
            "peaks": [Double](),
        ]
        if let current { s["current"] = current.json }
        if let context, let data = try? JSONEncoder().encode(context), let obj = try? JSONSerialization.jsonObject(with: data) {
            s["context"] = obj
        }
        if let pending { s["pending"] = pending }
        if let waveSessionId { s["waveSessionId"] = waveSessionId }
        if let wave { s["wave"] = wave.any }
        return s
    }

    // MARK: Пульт Siri Remote и экран «Сейчас играет» системы

    private func setupRemoteCommands() {
        let c = MPRemoteCommandCenter.shared()
        c.playCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.play() }; return .success }
        c.pauseCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.pause() }; return .success }
        c.togglePlayPauseCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.toggle() }; return .success }
        c.nextTrackCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.next() }; return .success }
        c.previousTrackCommand.addTarget { [weak self] _ in MainActor.assumeIsolated { self?.previous() }; return .success }
        c.changePlaybackPositionCommand.addTarget { [weak self] e in
            guard let e = e as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            MainActor.assumeIsolated { self?.seek(Int(e.positionTime * 1000)) }
            return .success
        }
    }

    private var artworkPath: String?

    private func updateNowPlaying() {
        let center = MPNowPlayingInfoCenter.default()
        guard let track = current?.track else {
            center.nowPlayingInfo = nil
            center.playbackState = .stopped
            return
        }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: track.fullTitle,
            MPMediaItemPropertyArtist: track.artistLine,
            MPMediaItemPropertyPlaybackDuration: Double(durationMs) / 1000,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: Double(positionMs) / 1000,
            MPNowPlayingInfoPropertyPlaybackRate: playing ? 1.0 : 0.0,
        ]
        if let album = track.album?.title { info[MPMediaItemPropertyAlbumTitle] = album }
        if let old = center.nowPlayingInfo?[MPMediaItemPropertyArtwork], artworkPath == track.cover {
            info[MPMediaItemPropertyArtwork] = old
        }
        center.nowPlayingInfo = info
        center.playbackState = playing ? .playing : .paused
        if artworkPath != track.cover, let path = track.cover, let api {
            artworkPath = path
            Task {
                guard let data = await api.image(path), let image = UIImage(data: data) else { return }
                let art = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
                var info = center.nowPlayingInfo ?? [:]
                info[MPMediaItemPropertyArtwork] = art
                center.nowPlayingInfo = info
            }
        }
    }
}
