// Мок разделов, которых сервер ещё не умеет: волна (рекомендации), звук (этап 1b) и устройства
// (управление между устройствами). ТОЛЬКО РАЗРАБОТКА: модуль подключается динамическим импортом
// за `import.meta.dev` и в сборку не попадает.
//
// Треки — настоящие, с сервера: превью из design/fixtures/audio сопоставляются по deezerId,
// поэтому лайки, события и переходы работают с настоящими id.
import type { Track } from '~/api/types'
import { resamplePeaks, seededRandom, syntheticPeaks } from '~/utils/peaks'
import type { PlaybackDevice, SourceResult, WaveSettings, WaveTrack } from '../types'

// Темп, фаза первой доли и огибающая пяти превью — посчитаны по MP3 заранее
const ANALYSIS: Record<number, { bpm: number, phase: number, peaks: number[] }> = {
  3107728361: { bpm: 90, phase: 0, peaks: [0.58, 0.72, 0.77, 0.88, 0.7, 0.92, 0.68, 0.65, 0.6, 0.56, 0.58, 0.85, 0.73, 0.77, 0.75, 0.73, 1, 0.69, 0.69, 0.81, 0.75, 0.55, 0.46, 0.49, 0.43, 0.91, 0.56, 0.88, 0.74, 0.69, 0.71, 0.94, 0.66, 0.67, 0.97, 0.66, 0.62, 0.54, 0.49, 0.9, 0.5, 0.91, 0.72, 0.68, 0.88, 0.87, 0.67, 0.7, 0.94, 0.34, 0.64, 0.58, 0.52, 0.86, 0.58, 0.9, 0.62, 0.76, 0.74, 0.76, 0.87, 0.72, 0.96, 0.64, 0.67, 0.56, 0.55, 0.68, 0.75, 0.81, 0.73, 0.73, 0.75, 0.95, 0.71, 0.68, 0.86, 0.69, 0.55, 0.47] },
  3580793301: { bpm: 161.5, phase: 0.052, peaks: [0.73, 0.84, 0.78, 0.88, 0.89, 0.83, 0.85, 0.79, 0.69, 0.84, 0.81, 0.76, 0.93, 0.95, 0.89, 0.81, 0.82, 0.68, 0.75, 0.92, 0.81, 0.81, 0.79, 0.84, 0.83, 0.82, 0.82, 0.92, 0.94, 0.98, 0.77, 0.81, 0.85, 0.75, 0.75, 0.81, 0.81, 0.86, 0.82, 0.79, 0.86, 0.71, 0.74, 0.81, 0.86, 0.91, 0.81, 0.86, 0.81, 0.8, 0.87, 0.86, 0.7, 0.84, 0.79, 0.63, 0.84, 0.76, 0.8, 0.81, 0.9, 0.82, 0.82, 0.81, 0.88, 0.78, 0.89, 0.94, 0.79, 0.85, 0.79, 0.62, 0.93, 0.92, 0.85, 0.92, 0.94, 0.82, 0.66, 0.73] },
  3661025372: { bpm: 110, phase: 0.068, peaks: [0.42, 0.41, 0.4, 0.35, 0.11, 0.27, 0.57, 0.6, 0.49, 0.61, 0.49, 0.52, 0.29, 0.1, 0.04, 0.12, 0.2, 0.27, 0.52, 0.51, 0.5, 0.49, 0.56, 0.48, 0.16, 0.1, 0.12, 0.25, 0.38, 0.47, 0.62, 0.59, 0.59, 0.6, 0.61, 0.49, 0.25, 0.23, 0.28, 0.32, 0.24, 0.44, 0.48, 0.49, 0.57, 0.47, 0.65, 0.32, 0.11, 0.05, 0.44, 0.52, 0.6, 0.64, 0.55, 0.55, 0.5, 0.43, 0.58, 0.12, 0.08, 0.08, 0.13, 0.27, 0.41, 0.57, 0.48, 0.64, 0.49, 0.56, 0.3, 0.12, 0.04, 0.2, 0.26, 0.33, 0.51, 0.66, 0.61, 0.57] },
  3898084851: { bpm: 141.5, phase: 0.013, peaks: [0.23, 0.21, 0.17, 0.25, 0.37, 0.4, 0.31, 0.33, 0.36, 0.38, 0.34, 0.41, 0.5, 0.38, 0.39, 0.34, 0.26, 0.25, 0.4, 0.44, 0.34, 0.4, 0.39, 0.31, 0.28, 0.28, 0.27, 0.34, 0.44, 0.3, 0.36, 0.39, 0.29, 0.34, 0.29, 0.36, 0.35, 0.37, 0.26, 0.32, 0.53, 0.66, 0.86, 0.92, 0.77, 0.72, 0.65, 0.99, 0.59, 0.68, 0.62, 0.86, 0.77, 0.61, 0.6, 0.67, 1, 0.73, 0.76, 0.63, 0.79, 0.93, 0.86, 0.74, 0.68, 0.99, 0.75, 0.74, 0.69, 0.83, 0.83, 0.69, 0.7, 0.68, 0.97, 0.75, 0.93, 0.72, 0.68, 0.88] },
  4059700721: { bpm: 104.5, phase: 0.314, peaks: [0.78, 0.85, 0.72, 0.81, 0.85, 0.62, 0.79, 0.88, 0.68, 0.74, 0.91, 0.72, 0.68, 0.92, 0.64, 0.67, 0.92, 0.6, 0.62, 0.97, 0.59, 0.64, 0.97, 0.58, 0.48, 0.95, 0.61, 0.67, 0.97, 0.65, 0.67, 1, 0.47, 0.65, 0.96, 0.68, 0.69, 0.97, 0.68, 0.69, 0.94, 0.65, 0.63, 0.93, 0.71, 0.68, 0.93, 0.69, 0.7, 0.88, 0.8, 0.7, 0.88, 0.77, 0.63, 0.85, 0.79, 0.69, 0.8, 0.85, 0.7, 0.72, 0.85, 0.65, 0.75, 0.86, 0.61, 0.67, 0.92, 0.65, 0.68, 0.9, 0.6, 0.5, 0.9, 0.56, 0.59, 0.9, 0.57, 0.59] },
}

const PREVIEW_IDS = [4059700721, 3107728361, 3898084851, 3661025372, 3580793301]
const PREVIEW_REASONS: Record<number, string> = {
  4059700721: 'Похоже на Rushex',
  3107728361: 'Похоже на Mzlff',
  3898084851: 'Из любимого',
  3661025372: 'Вы часто слушаете',
  3580793301: 'Похоже на Lowborn',
}

export interface MockDeps {
  /** Трек каталога по id Deezer — сервер заводит его у себя */
  deezerTrack(deezerId: number): Promise<Track | null>
  /** Любимые треки с сервера */
  liked(): Promise<Track[]>
  /** Имена артистов, на которых подписан человек */
  followedNames(): Promise<string[]>
}

export interface DevMock {
  waveNext(settings: WaveSettings, count: number, exclude: string[]): Promise<WaveTrack[]>
  source(track: Track): SourceResult
  waveform(track: Track, bars: number): number[]
  devices(): PlaybackDevice[]
}

const iso = (daysAgo: number) => new Date(Date.now() - daysAgo * 86_400_000).toISOString()

export function createMock(deps: MockDeps): DevMock {
  let previews: Promise<Track[]> | null = null
  const loadPreviews = () => (previews ??= Promise.all(PREVIEW_IDS.map(id => deps.deezerTrack(id).catch(() => null)))
    .then(list => list.filter((t): t is Track => !!t)))

  return {
    async waveNext(settings, count, exclude) {
      const skip = new Set(exclude)
      const [first, liked, names] = await Promise.all([loadPreviews(), deps.liked().catch(() => []), deps.followedNames().catch(() => [])])
      const r = seededRandom(`${settings.mood}${settings.activity}${settings.character}${settings.language}${exclude.length}`)
      const pool = liked.slice().sort(() => r() - 0.5)
      const out: WaveTrack[] = []
      for (const t of [...first, ...pool]) {
        if (out.length >= count) break
        if (skip.has(t.id) || out.some(w => w.track.id === t.id)) continue
        const fixed = t.deezerId ? PREVIEW_REASONS[t.deezerId] : undefined
        const k = seededRandom(t.id)()
        const like = names.length ? names[Math.floor(k * names.length)] : null
        const reason = fixed ?? (k < 0.55 || !like ? 'Из любимого' : `Похоже на ${like}`)
        out.push({ track: t, reason })
      }
      return out
    },

    source(track) {
      const id = track.deezerId
      const an = id ? ANALYSIS[id] : undefined
      if (!id || !an) {
        // Как ответит сервер этапа 1b. Готовый файл в разработке — тишина со временем;
        // остальное — 202: скачивается (с местом в очереди) или ждёт загрузчик на Mac, который локально не запущен
        if (track.audioStatus === 'ready') return { kind: 'ready', source: null, peaks: null }
        if (track.audioStatus === 'unavailable' || track.audioStatus === 'failed') return { kind: 'unavailable' }
        if (track.audioStatus === 'queued' || track.audioStatus === 'acquiring') {
          return { kind: 'pending', status: 'acquiring', acquisitionId: null, queuePosition: 1 + Math.floor(seededRandom(track.id)() * 9) }
        }
        return { kind: 'pending', status: 'waiting_for_downloader', acquisitionId: null, queuePosition: null }
      }
      const bpm = an.bpm > 140 ? an.bpm / 2 : an.bpm
      return {
        kind: 'ready',
        source: { url: `/_fixtures/audio/${id}.mp3`, durationMs: 30_000, bpm, beatPhase: an.phase, preview: true },
        peaks: an.peaks,
      }
    },

    waveform(track, bars) {
      const an = track.deezerId ? ANALYSIS[track.deezerId] : undefined
      return an ? resamplePeaks(an.peaks, bars) : syntheticPeaks(track.id, bars)
    },

    devices() {
      return [
        { id: 'this', name: 'Этот браузер', kind: 'web', current: true, online: true, lastSeenAt: new Date().toISOString() },
        { id: 'iphone', name: 'iPhone Захара', kind: 'phone', current: false, online: true, lastSeenAt: iso(0.01) },
        { id: 'pc', name: 'ПК в комнате', kind: 'desktop', current: false, online: false, lastSeenAt: iso(0.09) },
        { id: 'ipad', name: 'iPad', kind: 'tablet', current: false, online: false, lastSeenAt: iso(3) },
      ]
    },
  }
}
