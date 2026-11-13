import AppKit
import Metal
import MetalKit
import ScreenSaver

// Заставка DalyWave: живая волна «Полночь» (WaveShader.swift). Если DalyWave играет — волна в цвет
// обложки и название с исполнителем мелко в левом нижнем углу; иначе — спокойная волна в цветах бренда.
// Что играет, отвечает приложение на 127.0.0.1 (apps/shell/src/screensaver.rs): заставка живёт в песочнице
// системы и к файлам приложения не ходит.

/// Порт приложения — тот же, что `PORT` в screensaver.rs.
private let nowPlayingURL = URL(string: "http://127.0.0.1:47815/now-playing")!

private typealias Vec3 = SIMD3<Float>

private func lin(_ hex: UInt32) -> Vec3 {
    let c = [Float((hex >> 16) & 255), Float((hex >> 8) & 255), Float(hex & 255)].map { powf($0 / 255, 2.2) }
    return Vec3(c[0], c[1], c[2])
}

private func mix(_ a: Vec3, _ b: Vec3, _ k: Float) -> Vec3 { a + (b - a) * k }

/// Цвета волны: основа, толща, тело, гребень и доля спектра «Сумерек».
private struct Palette {
    var c: [Vec3]
    var spectrum: Float
    var energy: Float

    /// Без музыки — бренд: спектр вдоль ленты, тихий ход
    static let calm = Palette(c: [lin(0x07050F), lin(0x1D1048), lin(0xB01CF0), lin(0xFFD8EA)], spectrum: 1, energy: 0.3)

    /// Играет: краски из обложки, основа остаётся тёмной
    static func cover(vivid: Vec3, deep: Vec3) -> Palette {
        let base = calm.c
        return Palette(
            c: [mix(base[0], deep, 0.1), mix(base[1], vivid * 0.3, 0.8), mix(base[2], vivid, 0.92), mix(vivid, Vec3(1, 0.96, 0.94), 0.6)],
            spectrum: 0,
            energy: 0.5
        )
    }

    static func lerp(_ a: Palette, _ b: Palette, _ k: Float) -> Palette {
        Palette(c: zip(a.c, b.c).map { mix($0, $1, k) }, spectrum: a.spectrum + (b.spectrum - a.spectrum) * k, energy: a.energy + (b.energy - a.energy) * k)
    }
}

private let spectrumColors: [Vec3] = [lin(0x4B21FF), lin(0xB01CF0), lin(0xFF3D7A), lin(0xFF9A3C)]

private struct NowPlaying: Decodable, Equatable {
    var playing: Bool
    var title: String
    var artist: String
    var artworkUrl: String?
}

@objc(DalyWaveSaverView)
final class DalyWaveSaverView: ScreenSaverView, MTKViewDelegate {
    private var metal: MTKView?
    private var queue: MTLCommandQueue?
    private var pipeline: MTLRenderPipelineState?

    private let titleLabel = NSTextField(labelWithString: "")
    private let artistLabel = NSTextField(labelWithString: "")
    private let caption = NSStackView()

    private var time: Float = 17.3
    private var lastFrame = CACurrentMediaTime()
    private var startedAt = CACurrentMediaTime()
    private var energy: Float = 0.3
    private var pal = Palette.calm
    private var palFrom = Palette.calm
    private var palTo = Palette.calm
    private var palT: Float = 1

    private var poll: Timer?
    private var now: NowPlaying?
    private var artworkURL: String?
    private var stopped = false

    override init?(frame: NSRect, isPreview: Bool) {
        super.init(frame: frame, isPreview: isPreview)
        animationTimeInterval = 1.0 / 30.0
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        setUpMetal()
        setUpCaption()
        // macOS 14+: legacyScreenSaver не выгружает заставку после выхода — останавливаемся сами
        DistributedNotificationCenter.default().addObserver(
            self, selector: #selector(willStop), name: NSNotification.Name("com.apple.screensaver.willstop"), object: nil
        )
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
    }

    deinit {
        DistributedNotificationCenter.default().removeObserver(self)
        poll?.invalidate()
    }

    // MARK: Metal

    private func setUpMetal() {
        guard let device = MTLCreateSystemDefaultDevice(),
              let library = try? device.makeLibrary(source: waveShaderSource, options: nil),
              let vfn = library.makeFunction(name: "wave_vertex"),
              let ffn = library.makeFunction(name: "wave_fragment")
        else { return }
        let desc = MTLRenderPipelineDescriptor()
        desc.vertexFunction = vfn
        desc.fragmentFunction = ffn
        desc.colorAttachments[0].pixelFormat = .bgra8Unorm
        guard let pipeline = try? device.makeRenderPipelineState(descriptor: desc) else { return }
        let view = MTKView(frame: bounds, device: device)
        view.colorPixelFormat = .bgra8Unorm
        view.autoresizingMask = [.width, .height]
        view.isPaused = true
        view.enableSetNeedsDisplay = false
        // Мягкая картинка не требует полного разрешения 5K: размер буфера — по бюджету пикселей
        view.autoResizeDrawable = false
        view.layer?.contentsGravity = .resize
        view.delegate = self
        addSubview(view)
        metal = view
        queue = device.makeCommandQueue()
        self.pipeline = pipeline
        fitDrawable()
    }

    private func fitDrawable() {
        guard let view = metal else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        var w = max(1, bounds.width * scale)
        var h = max(1, bounds.height * scale)
        let budget: CGFloat = 2.4e6
        if w * h > budget {
            let k = sqrt(budget / (w * h))
            w *= k
            h *= k
        }
        view.drawableSize = CGSize(width: w.rounded(), height: h.rounded())
    }

    override func resizeSubviews(withOldSize oldSize: NSSize) {
        super.resizeSubviews(withOldSize: oldSize)
        fitDrawable()
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

    func draw(in view: MTKView) {
        guard let pipeline, let queue, let pass = view.currentRenderPassDescriptor, let drawable = view.currentDrawable,
              let buffer = queue.makeCommandBuffer(), let enc = buffer.makeRenderCommandEncoder(descriptor: pass)
        else { return }
        let size = view.drawableSize
        let since = Float(CACurrentMediaTime() - startedAt)
        // Вход: волна разворачивается, как при включении в приложении
        let k = min(since / 1.6, 1)
        let expand = k >= 1 ? 1 : 1 - powf(2, -10 * k)
        let beat = 0.12 * (0.5 + 0.5 * sinf(since * 0.9))
        let c = pal.c
        var u: [SIMD4<Float>] = [
            SIMD4(Float(size.width), Float(size.height), time, beat),
            SIMD4(energy, expand, 0, pal.spectrum),
            // Раскладка «весь экран» (LAYOUTS.full в engine.ts)
            SIMD4(-0.12, 0.02, 0, 1.25),
            SIMD4(0, 0, 1, 0),
            SIMD4(0, 0, 0, 0),
            SIMD4(0, 9, 0, 0),
        ]
        for v in c { u.append(SIMD4(v.x, v.y, v.z, 0)) }
        for v in spectrumColors { u.append(SIMD4(v.x, v.y, v.z, 0)) }
        enc.setRenderPipelineState(pipeline)
        u.withUnsafeBytes { enc.setFragmentBytes($0.baseAddress!, length: $0.count, index: 0) }
        enc.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        enc.endEncoding()
        buffer.present(drawable)
        buffer.commit()
    }

    // MARK: Заставка

    override func startAnimation() {
        super.startAnimation()
        stopped = false
        startedAt = CACurrentMediaTime()
        lastFrame = startedAt
        fetchNowPlaying()
        poll?.invalidate()
        poll = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in self?.fetchNowPlaying() }
    }

    override func stopAnimation() {
        super.stopAnimation()
        poll?.invalidate()
        poll = nil
    }

    @objc private func willStop() {
        stopped = true
        stopAnimation()
    }

    override func animateOneFrame() {
        guard !stopped else { return }
        let t = CACurrentMediaTime()
        let dt = Float(min(0.25, t - lastFrame))
        lastFrame = t
        if palT < 1 {
            palT = min(1, palT + dt / 1.5)
            let k = palT < 0.5 ? 4 * palT * palT * palT : 1 - powf(-2 * palT + 2, 3) / 2
            pal = Palette.lerp(palFrom, palTo, k)
        }
        energy += (pal.energy - energy) * (1 - expf(-dt * 1.2))
        time += dt * (0.16 + 0.62 * energy)
        metal?.draw()
    }

    override var hasConfigureSheet: Bool { false }
    override var configureSheet: NSWindow? { nil }

    // MARK: Что играет

    private func setUpCaption() {
        let preview = isPreview
        titleLabel.font = .systemFont(ofSize: preview ? 9 : 15, weight: .semibold)
        titleLabel.textColor = NSColor(white: 1, alpha: 0.88)
        artistLabel.font = .systemFont(ofSize: preview ? 8 : 13, weight: .regular)
        artistLabel.textColor = NSColor(white: 1, alpha: 0.55)
        for l in [titleLabel, artistLabel] {
            l.lineBreakMode = .byTruncatingTail
            l.maximumNumberOfLines = 1
        }
        caption.orientation = .vertical
        caption.alignment = .leading
        caption.spacing = preview ? 1 : 3
        caption.addArrangedSubview(titleLabel)
        caption.addArrangedSubview(artistLabel)
        caption.translatesAutoresizingMaskIntoConstraints = false
        caption.alphaValue = 0
        addSubview(caption)
        let inset: CGFloat = preview ? 10 : 48
        NSLayoutConstraint.activate([
            caption.leadingAnchor.constraint(equalTo: leadingAnchor, constant: inset),
            caption.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -inset),
            caption.widthAnchor.constraint(lessThanOrEqualTo: widthAnchor, multiplier: 0.4),
        ])
    }

    private func fetchNowPlaying() {
        var req = URLRequest(url: nowPlayingURL)
        req.timeoutInterval = 1.5
        req.cachePolicy = .reloadIgnoringLocalCacheData
        URLSession.shared.dataTask(with: req) { [weak self] data, _, _ in
            let now = data.flatMap { try? JSONDecoder().decode(NowPlaying.self, from: $0) }
            DispatchQueue.main.async { self?.apply(now?.playing == true && !(now?.title.isEmpty ?? true) ? now : nil) }
        }.resume()
    }

    private func apply(_ next: NowPlaying?) {
        guard next != now else { return }
        now = next
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.6
            caption.animator().alphaValue = next == nil ? 0 : 1
        }
        if let next {
            titleLabel.stringValue = next.title
            artistLabel.stringValue = next.artist
        }
        guard let url = next?.artworkUrl, let parsed = URL(string: url) else {
            artworkURL = nil
            retint(.calm)
            return
        }
        guard url != artworkURL else { return }
        artworkURL = url
        URLSession.shared.dataTask(with: parsed) { [weak self] data, _, _ in
            let colors = data.flatMap { NSImage(data: $0) }.flatMap(Self.colors)
            DispatchQueue.main.async {
                guard let self, self.artworkURL == url else { return }
                self.retint(colors.map { Palette.cover(vivid: $0.vivid, deep: $0.deep) } ?? .calm)
            }
        }.resume()
    }

    private func retint(_ to: Palette) {
        palFrom = pal
        palTo = to
        palT = 0
    }

    /// Самый яркий насыщенный цвет обложки и её тёмная основа — в линейном пространстве.
    private static func colors(_ image: NSImage) -> (vivid: Vec3, deep: Vec3)? {
        let side = 16
        var px = [UInt8](repeating: 0, count: side * side * 4)
        guard let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil),
              let ctx = CGContext(data: &px, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                  space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        ctx.interpolationQuality = .medium
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: side, height: side))
        var best: (score: Float, c: Vec3) = (-1, Vec3(0.7, 0.2, 0.9))
        var sum = Vec3(0, 0, 0)
        for i in stride(from: 0, to: px.count, by: 4) {
            let c = Vec3(Float(px[i]), Float(px[i + 1]), Float(px[i + 2])) / 255
            let hi = max(c.x, c.y, c.z)
            let lo = min(c.x, c.y, c.z)
            let sat = hi > 0 ? (hi - lo) / hi : 0
            let score = sat * hi * (hi > 0.22 ? 1 : 0.2)
            if score > best.score { best = (score, c) }
            sum += c
        }
        let avg = sum / Float(side * side)
        let toLin = { (c: Vec3) in Vec3(powf(c.x, 2.2), powf(c.y, 2.2), powf(c.z, 2.2)) }
        return (toLin(best.c), toLin(avg * 0.6))
    }
}
