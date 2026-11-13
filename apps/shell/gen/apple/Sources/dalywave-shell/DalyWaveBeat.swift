import AVFAudio
import CoreHaptics
import Foundation

// «Почувствуй бит»: короткие удары Taptic Engine на сильных долях играющего трека. Когда и с какой силой
// бить, решает Rust (apps/shell/src/beat.rs) по сетке долей из анализа и позиции движка; здесь — только
// Core Haptics: движок, транзиентные события с точным временем и задержка вывода звука.
//
// Вибрация работает только на экране: в фоне iOS её не даёт, и beat.rs там не зовёт сюда.

@objc(DalyWaveBeat)
final class DalyWaveBeatBridge: NSObject {
    private static let queue = DispatchQueue(label: "rest.daly.wave.beat", qos: .userInteractive)
    private static var engine: CHHapticEngine?
    /// Удары, поставленные на будущее: перемотка и пауза их снимают
    private static var pending: [CHHapticPatternPlayer] = []

    /// Есть ли Taptic Engine с Core Haptics (iPhone 8 и новее; у iPad и симулятора — нет).
    @objc static var supported: Bool {
        CHHapticEngine.capabilitiesForHardware().supportsHaptics
    }

    /// Сколько звук идёт от движка до ушей, с: задержка маршрута (AirPods — до 0,2 с) и буфер вывода.
    @objc static var outputLatency: Double {
        let session = AVAudioSession.sharedInstance()
        return session.outputLatency + session.ioBufferDuration
    }

    @objc static func start() {
        queue.async {
            guard supported else { return }
            if engine == nil {
                guard let e = try? CHHapticEngine() else { return }
                // Только вибрация: звук играет наш движок, свой аудиоканал Core Haptics не нужен
                e.playsHapticsOnly = true
                e.isAutoShutdownEnabled = false
                e.resetHandler = {
                    queue.async {
                        pending.removeAll()
                        try? engine?.start()
                    }
                }
                e.stoppedHandler = { _ in
                    queue.async { pending.removeAll() }
                }
                engine = e
            }
            try? engine?.start()
        }
    }

    @objc static func stop() {
        queue.async {
            cancelPending()
            engine?.stop(completionHandler: nil)
            engine = nil
        }
    }

    /// Снять поставленные удары (перемотка, смена трека).
    @objc static func reset() {
        queue.async { cancelPending() }
    }

    /// Удар через `delay` секунд: `intensity` и `sharpness` — 0…1.
    @objc static func tap(_ intensity: Double, sharpness: Double, delay: Double) {
        queue.async {
            guard let engine else { return }
            let event = CHHapticEvent(
                eventType: .hapticTransient,
                parameters: [
                    CHHapticEventParameter(parameterID: .hapticIntensity, value: Float(min(max(intensity, 0), 1))),
                    CHHapticEventParameter(parameterID: .hapticSharpness, value: Float(min(max(sharpness, 0), 1))),
                ],
                relativeTime: 0
            )
            guard let pattern = try? CHHapticPattern(events: [event], parameters: []),
                  let player = try? engine.makePlayer(with: pattern)
            else { return }
            do {
                try player.start(atTime: engine.currentTime + max(delay, 0))
                pending.append(player)
                if pending.count > 4 { pending.removeFirst(pending.count - 4) }
            } catch {
                // Движок остановила система (звонок, Siri): следующий start() поднимет его
                try? engine.start()
            }
        }
    }

    private static func cancelPending() {
        for p in pending { try? p.stop(atTime: CHHapticTimeImmediate) }
        pending.removeAll()
    }
}
