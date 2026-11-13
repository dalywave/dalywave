// Видео-история трека: вертикальное 1080×1920, 10, 15 или 30 с (момент выбирает человек), 30 к/с. Обложка, название и исполнитель, текущая
// строка текста, живая волна в цвет обложки (пульс и размах — от самого звука истории), водяной знак.
//
// Сборка на устройстве:
// - WebCodecs (Safari 16.4+, WKWebView iOS 16.4+ и macOS 13.3+, Chrome, Edge, Firefox): кадры кодируются
//   в H.264 быстрее реального времени и складываются в MP4 (mp4-muxer). Звук — AAC через AudioEncoder, если
//   браузер его умеет (Firefox — нет); иначе MP4 без звука уходит на сервер, и звук добавляет ffmpeg (story.rs).
//   Safari в режиме «качество» не отдаёт кадры до конца — тогда кодируем в «реальном времени»
//   (encodeVideoAdaptive); описание AAC от Safari чинит aac.ts.
// - Без WebCodecs (iOS 15–16.3) или если кодировщик не справился: MediaRecorder пишет холст в реальном
//   времени, звук — тоже сервер.
import { ArrayBufferTarget, Muxer } from 'mp4-muxer'
import { CARD_FONT, drawCardImage, fitText, loadCardImage } from '~/utils/rewind-share'
import { aacDescription } from './aac'
import { type FrameLevel, type StoryLine, frameLevels, lineAt } from './levels'
import { StoryWave, storyPalette } from './wave'

export { frameLevels, lineAt, type StoryLine }

export const STORY_W = 1080
export const STORY_H = 1920
export const STORY_FPS = 30
/** Длина по умолчанию */
export const STORY_MS = 15_000
const VIDEO_BITRATE = 8_000_000
const AUDIO_BITRATE = 192_000
/** Волна рисуется в половину размера: она мягкая, а телефону так вдвое легче */
const WAVE_SCALE = 0.5
/** Время волны на первом кадре: с него начинаются и превью, и видео */
const WAVE_T0 = 17.3

export interface StoryInput {
  title: string
  artist: string
  coverUrl: string | null
  /** Цвета обложки, самый заметный первым */
  palette: string[] | null
  /** Звук истории — выбранный момент */
  audio: AudioBuffer
  /** Длина истории, мс; нет — STORY_MS */
  durationMs?: number
  lines: StoryLine[]
  watermark: string
}

export type StoryEncoder = 'webcodecs' | 'webcodecs-server-audio' | 'recorder'

export interface StoryResult {
  /** MP4 со звуком — или видео без звука, если `needsAudio` (звук добавит сервер) */
  blob: Blob
  needsAudio: boolean
  encoder: StoryEncoder
}

// ---------- Кадр ----------

interface Scene {
  ctx: CanvasRenderingContext2D
  canvas: HTMLCanvasElement
  wave: StoryWave
  cover: HTMLImageElement | null
  levels: FrameLevel[]
  input: StoryInput
  time: number
  /** Длина, мс, и число кадров */
  ms: number
  frames: number
}

function wrap(ctx: CanvasRenderingContext2D, text: string, max: number, lines: number): string[] {
  const words = text.split(/\s+/).filter(Boolean)
  const out: string[] = []
  let cur = ''
  for (const w of words) {
    const next = cur ? `${cur} ${w}` : w
    if (ctx.measureText(next).width <= max || !cur) cur = next
    else {
      out.push(cur)
      cur = w
    }
  }
  if (cur) out.push(cur)
  if (out.length > lines) {
    const rest = out.slice(lines - 1).join(' ')
    return [...out.slice(0, lines - 1), fitText(ctx, rest, max)]
  }
  return out
}

const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v))
const easeOut = (k: number) => 1 - Math.pow(1 - clamp(k, 0, 1), 3)

function drawFrame(s: Scene, frame: number) {
  const { ctx, input } = s
  const t = frame / STORY_FPS
  const lv = s.levels[Math.min(frame, s.levels.length - 1)] ?? { energy: 0.3, beat: 0, bands: [0, 0, 0] as [number, number, number] }
  s.time += (1 / STORY_FPS) * (0.16 + 0.62 * lv.energy)
  s.wave.draw({ time: s.time, beat: lv.beat, energy: lv.energy, bands: lv.bands, expand: easeOut(t / 1.4), flash: Math.max(0, 0.4 - t) })
  ctx.drawImage(s.wave.canvas, 0, 0, STORY_W, STORY_H)
  // Мягкая тень под подписями: гребень волны не спорит с текстом
  const scrim = ctx.createRadialGradient(STORY_W / 2, 1250, 0, STORY_W / 2, 1250, 640)
  scrim.addColorStop(0, 'rgba(7, 5, 15, 0.55)')
  scrim.addColorStop(1, 'rgba(7, 5, 15, 0)')
  ctx.fillStyle = scrim
  ctx.fillRect(0, 600, STORY_W, 1300)

  // Обложка: входит за 0,5 с, чуть дышит на долях
  const enter = easeOut(t / 0.5)
  const side = 720
  const k = (0.94 + 0.06 * enter) * (1 + 0.012 * lv.beat)
  const w = side * k
  const x = (STORY_W - w) / 2
  const y = 250 + (side - w) / 2 + (1 - enter) * 24
  ctx.save()
  ctx.globalAlpha = enter
  ctx.shadowColor = 'rgba(0, 0, 0, 0.55)'
  ctx.shadowBlur = 90
  ctx.shadowOffsetY = 36
  ctx.fillStyle = '#111'
  ctx.beginPath()
  ctx.roundRect(x, y, w, w, 36)
  ctx.fill()
  ctx.restore()
  if (s.cover) {
    ctx.save()
    ctx.globalAlpha = enter
    drawCardImage(ctx, s.cover, x, y, w, w, 36)
    ctx.restore()
  }

  // Название и исполнитель
  ctx.save()
  ctx.globalAlpha = easeOut((t - 0.15) / 0.5)
  ctx.textAlign = 'center'
  ctx.fillStyle = '#ffffff'
  ctx.font = `700 64px ${CARD_FONT}`
  ctx.fillText(fitText(ctx, input.title, 900), STORY_W / 2, 1100)
  ctx.fillStyle = 'rgba(255, 255, 255, 0.66)'
  ctx.font = `500 44px ${CARD_FONT}`
  ctx.fillText(fitText(ctx, input.artist, 900), STORY_W / 2, 1168)
  ctx.restore()

  // Строка текста: появляется мягко при смене
  const ms = t * 1000
  const line = lineAt(input.lines, ms)
  if (line) {
    const since = ms - line.atMs
    ctx.save()
    ctx.globalAlpha = easeOut(since / 300)
    ctx.textAlign = 'center'
    ctx.fillStyle = '#ffffff'
    ctx.font = `650 54px ${CARD_FONT}`
    const rows = wrap(ctx, line.text, 900, 3)
    rows.forEach((r, i) => ctx.fillText(r, STORY_W / 2, 1310 + i * 68 + (1 - easeOut(since / 300)) * 12))
    ctx.restore()
  }

  // Водяной знак
  ctx.save()
  ctx.textAlign = 'center'
  ctx.fillStyle = 'rgba(255, 255, 255, 0.5)'
  ctx.font = `600 30px ${CARD_FONT}`
  ctx.fillText(input.watermark, STORY_W / 2, STORY_H - 84)
  ctx.restore()
}

async function scene(input: StoryInput): Promise<Scene> {
  await Promise.all([500, 600, 700].map(w => document.fonts?.load(`${w} 64px "Onest Variable"`).catch(() => null)))
  const canvas = document.createElement('canvas')
  canvas.width = STORY_W
  canvas.height = STORY_H
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('canvas')
  const wave = new StoryWave(Math.round(STORY_W * WAVE_SCALE), Math.round(STORY_H * WAVE_SCALE), storyPalette(input.palette))
  const ms = input.durationMs ?? STORY_MS
  return { ctx, canvas, wave, cover: await loadCardImage(input.coverUrl), levels: frameLevels(input.audio), input, time: WAVE_T0, ms, frames: Math.round((ms / 1000) * STORY_FPS) }
}

/** Кадр для предпросмотра (по умолчанию — на 6-й секунде) — тот же, что попадёт в видео */
export async function previewFrame(input: StoryInput, frame = 6 * STORY_FPS): Promise<HTMLCanvasElement> {
  const s = await scene(input)
  frame = Math.min(frame, s.frames - 1)
  for (let f = 0; f <= frame; f++) {
    // Время волны копится по кадрам — без отрисовки промежуточных
    const lv = s.levels[Math.min(f, s.levels.length - 1)]
    if (f < frame) s.time += (1 / STORY_FPS) * (0.16 + 0.62 * (lv?.energy ?? 0.3))
  }
  drawFrame(s, frame)
  s.wave.dispose()
  return s.canvas
}

// ---------- Кодирование ----------

const AVC_CODECS = ['avc1.640028', 'avc1.4d0028', 'avc1.42e028']
/** Сколько кадров держать в очереди кодировщика: память телефона */
const MAX_QUEUE = 6
/** Кодировщик в режиме «качество» за это время не взял ни кадра и ничего не отдал. Так делает Safari
 * (VideoToolbox): копит кадры до flush, очередь не убывает, и сборка вечно стоит на 1 %. Тогда — заново
 * в режиме «реальное время»: там Safari отдаёт кадры сразу */
const QUALITY_STALL_MS = 1500
/** Кодировщик молчит совсем — понятная ошибка вместо вечных процентов */
const STALL_MS = 10_000
/** Доля прогресса на звук: он кодируется первым и быстро */
const AUDIO_SHARE = 0.04

/** Сборка не удалась: `stall` — кодировщик замолчал, `unsupported` — устройство не умеет видео из холста */
export class StoryError extends Error {
  constructor(readonly reason: 'stall' | 'unsupported', message: string = reason) {
    super(message)
    this.name = 'StoryError'
  }
}

/** Обещание с пределом по времени: вместо вечного ожидания — StoryError('stall') */
export function withTimeout<T>(p: Promise<T>, ms: number, what = 'stall'): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined
  const limit = new Promise<never>((_, reject) => (timer = setTimeout(() => reject(new StoryError('stall', what)), ms)))
  return Promise.race([p, limit]).finally(() => clearTimeout(timer))
}

const videoConfig = (codec: string, latencyMode: LatencyMode): VideoEncoderConfig =>
  ({ codec, width: STORY_W, height: STORY_H, bitrate: VIDEO_BITRATE, framerate: STORY_FPS, latencyMode, avc: { format: 'avc' } })

async function videoCodec(): Promise<string | null> {
  if (typeof VideoEncoder === 'undefined' || typeof VideoFrame === 'undefined') return null
  for (const codec of AVC_CODECS) {
    try {
      const r = await VideoEncoder.isConfigSupported(videoConfig(codec, 'quality'))
      if (r.supported) return codec
    }
    catch {
      // Следующий профиль
    }
  }
  return null
}

const audioConfig = (audio: AudioBuffer): AudioEncoderConfig =>
  ({ codec: 'mp4a.40.2', sampleRate: audio.sampleRate, numberOfChannels: 2, bitrate: AUDIO_BITRATE })

async function aacSupported(audio: AudioBuffer): Promise<boolean> {
  if (typeof AudioEncoder === 'undefined' || typeof AudioData === 'undefined') return false
  try {
    return !!(await AudioEncoder.isConfigSupported(audioConfig(audio))).supported
  }
  catch {
    return false
  }
}

/** Как соберётся видео на этом устройстве */
export async function storyEncoder(audio: AudioBuffer): Promise<StoryEncoder> {
  if (await videoCodec()) return (await aacSupported(audio)) ? 'webcodecs' : 'webcodecs-server-audio'
  return 'recorder'
}

function aborted(signal?: AbortSignal) {
  if (signal?.aborted) throw new DOMException('cancelled', 'AbortError')
}
const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError'
const pause = (ms: number) => new Promise(r => setTimeout(r, ms))

interface Chunk<C, M> { chunk: C, meta?: M }
type VideoChunk = Chunk<EncodedVideoChunk, EncodedVideoChunkMetadata>
type AudioChunk = Chunk<EncodedAudioChunk, EncodedAudioChunkMetadata>

/** Звук истории → AAC. Не вышло — null: видео уйдёт без звука, звук добавит сервер */
async function encodeAudio(audio: AudioBuffer, ms: number, signal?: AbortSignal): Promise<AudioChunk[] | null> {
  if (!(await aacSupported(audio))) return null
  const chunks: AudioChunk[] = []
  let failure: unknown = null
  const ae = new AudioEncoder({ output: (chunk, meta) => chunks.push({ chunk, meta }), error: (e) => { failure = e } })
  try {
    ae.configure(audioConfig(audio))
    const left = audio.getChannelData(0)
    const right = audio.numberOfChannels > 1 ? audio.getChannelData(1) : left
    const len = Math.min(audio.length, Math.round((ms / 1000) * audio.sampleRate))
    const block = 4096
    for (let i = 0; i < len; i += block) {
      aborted(signal)
      if (failure) throw failure
      const n = Math.min(block, len - i)
      const data = new Float32Array(n * 2)
      data.set(left.subarray(i, i + n), 0)
      data.set(right.subarray(i, i + n), n)
      const ad = new AudioData({ format: 'f32-planar', sampleRate: audio.sampleRate, numberOfFrames: n, numberOfChannels: 2, timestamp: Math.round((i / audio.sampleRate) * 1_000_000), data })
      ae.encode(ad)
      ad.close()
    }
    await withTimeout(ae.flush(), STALL_MS, 'audio flush')
    if (failure) throw failure
    return chunks.length ? chunks : null
  }
  catch (e) {
    if (isAbort(e)) throw e
    console.warn('История: звук не закодировался, его добавит сервер', e)
    return null
  }
  finally {
    if (ae.state !== 'closed') ae.close()
  }
}

/** Кадры → H.264. Кодировщик, который перестал брать кадры, — StoryError('stall') */
async function encodeVideo(s: Scene, codec: string, latencyMode: LatencyMode, onProgress: (share: number) => void, signal?: AbortSignal): Promise<VideoChunk[]> {
  s.time = WAVE_T0
  const chunks: VideoChunk[] = []
  let failure: unknown = null
  let active = performance.now()
  const ve = new VideoEncoder({
    output: (chunk, meta) => {
      chunks.push({ chunk, meta })
      active = performance.now()
    },
    error: (e) => { failure = e },
  })
  ve.addEventListener?.('dequeue', () => (active = performance.now()))
  try {
    ve.configure(videoConfig(codec, latencyMode))
    const step = 1_000_000 / STORY_FPS
    for (let f = 0; f < s.frames; f++) {
      aborted(signal)
      if (failure) throw failure
      drawFrame(s, f)
      const frame = new VideoFrame(s.canvas, { timestamp: Math.round(f * step), duration: Math.round(step) })
      ve.encode(frame, { keyFrame: f % (STORY_FPS * 2) === 0 })
      frame.close()
      // Не копить кадры в очереди: память телефона. Но и не ждать вечно кодировщик, который замолчал
      while (ve.encodeQueueSize > MAX_QUEUE) {
        aborted(signal)
        if (failure) throw failure
        // Вкладка скрыта (или экран заблокирован): Safari усыпляет страницу — это не молчание кодировщика
        if (document.hidden) active = performance.now()
        const quiet = performance.now() - active
        if (quiet > STALL_MS || (latencyMode === 'quality' && !chunks.length && quiet > QUALITY_STALL_MS))
          throw new StoryError('stall', `video encoder (${latencyMode}) stuck at frame ${f}, queue ${ve.encodeQueueSize}`)
        await pause(4)
      }
      if (f % 6 === 0) {
        onProgress(f / s.frames)
        await pause(0)
      }
    }
    await withTimeout(ve.flush(), STALL_MS, 'video flush')
    if (failure) throw failure
    if (!chunks.length) throw new StoryError('stall', 'video encoder gave nothing')
    return chunks
  }
  finally {
    if (ve.state !== 'closed') ve.close()
  }
}

/** Сначала «качество», если кодировщик в нём молчит (Safari) — «реальное время» */
async function encodeVideoAdaptive(s: Scene, codec: string, onProgress: (share: number) => void, signal?: AbortSignal): Promise<VideoChunk[]> {
  // Повтор начинается с первого кадра, но полоса назад не прыгает
  let best = 0
  const report = (p: number) => onProgress((best = Math.max(best, p)))
  try {
    return await encodeVideo(s, codec, 'quality', report, signal)
  }
  catch (e) {
    if (!(e instanceof StoryError)) throw e
    console.warn('История: кодировщик молчит в режиме «качество», пробуем «реальное время»', e.message)
    const rt = await VideoEncoder.isConfigSupported(videoConfig(codec, 'realtime')).catch(() => null)
    if (!rt?.supported) throw e
    return await encodeVideo(s, codec, 'realtime', report, signal)
  }
}

function mux(video: VideoChunk[], audio: AudioChunk[] | null, sampleRate: number): Blob {
  const muxer = new Muxer({
    target: new ArrayBufferTarget(),
    video: { codec: 'avc', width: STORY_W, height: STORY_H, frameRate: STORY_FPS },
    audio: audio ? { codec: 'aac', numberOfChannels: 2, sampleRate } : undefined,
    fastStart: 'in-memory',
    firstTimestampBehavior: 'offset',
  })
  for (const c of video) muxer.addVideoChunk(c.chunk, c.meta)
  for (const c of audio ?? []) {
    const dc = c.meta?.decoderConfig
    // Safari кладёт в описание весь ES_Descriptor — в MP4 нужен только AudioSpecificConfig (aac.ts)
    const meta = dc ? { ...c.meta, decoderConfig: { ...dc, description: aacDescription(dc.description, dc.sampleRate || sampleRate, dc.numberOfChannels || 2) } } : c.meta
    muxer.addAudioChunk(c.chunk, meta)
  }
  muxer.finalize()
  return new Blob([muxer.target.buffer], { type: 'video/mp4' })
}

async function encodeRecorder(s: Scene, onProgress: (share: number) => void, signal?: AbortSignal): Promise<Blob> {
  const canvas = s.canvas as HTMLCanvasElement & { captureStream?: (fps?: number) => MediaStream }
  if (typeof MediaRecorder === 'undefined' || typeof canvas.captureStream !== 'function') throw new StoryError('unsupported')
  s.time = WAVE_T0
  const stream = canvas.captureStream(STORY_FPS)
  const mime = ['video/mp4;codecs=avc1', 'video/mp4', 'video/webm;codecs=vp9', 'video/webm'].find(m => MediaRecorder.isTypeSupported?.(m)) ?? ''
  const rec = new MediaRecorder(stream, mime ? { mimeType: mime, videoBitsPerSecond: VIDEO_BITRATE } : { videoBitsPerSecond: VIDEO_BITRATE })
  const parts: Blob[] = []
  rec.ondataavailable = e => e.data.size && parts.push(e.data)
  const done = new Promise<void>(resolve => (rec.onstop = () => resolve()))
  try {
    drawFrame(s, 0)
    rec.start(500)
    const start = performance.now()
    let f = 0
    await new Promise<void>((resolve, reject) => {
      rec.onerror = () => reject(new StoryError('stall', 'MediaRecorder error'))
      const tick = () => {
        if (signal?.aborted) {
          reject(new DOMException('cancelled', 'AbortError'))
          return
        }
        const want = Math.min(s.frames - 1, Math.floor(((performance.now() - start) / 1000) * STORY_FPS))
        while (f < want) drawFrame(s, ++f)
        onProgress(f / s.frames)
        if (performance.now() - start >= s.ms) resolve()
        else requestAnimationFrame(tick)
      }
      requestAnimationFrame(tick)
    })
  }
  finally {
    if (rec.state !== 'inactive') rec.stop()
  }
  await withTimeout(done, STALL_MS, 'recorder stop')
  stream.getTracks().forEach(t => t.stop())
  if (!parts.length) throw new StoryError('stall', 'MediaRecorder gave nothing')
  return new Blob(parts, { type: (mime || 'video/webm').split(';')[0] })
}

/** Собрать историю. `needsAudio` — видео без звука: его надо отправить на сервер (`catalog.storyVideo`).
 * Прогресс — по настоящим шагам: звук, затем кадры; кодировщик, который замолчал, — StoryError, а не вечные 1 % */
export async function renderStory(input: StoryInput, onProgress: (share: number) => void, signal?: AbortSignal): Promise<StoryResult> {
  const s = await scene(input)
  try {
    const codec = await videoCodec()
    if (codec) {
      try {
        const audio = await encodeAudio(input.audio, s.ms, signal)
        onProgress(AUDIO_SHARE)
        const video = await encodeVideoAdaptive(s, codec, p => onProgress(AUDIO_SHARE + p * (1 - AUDIO_SHARE)), signal)
        const blob = mux(video, audio, input.audio.sampleRate)
        onProgress(1)
        return { blob, needsAudio: !audio, encoder: audio ? 'webcodecs' : 'webcodecs-server-audio' }
      }
      catch (e) {
        if (isAbort(e)) throw e
        // Кодировщик не справился — пишем холст в реальном времени (15 с), звук добавит сервер
        console.warn('История: WebCodecs не справился, пишем холст в реальном времени', e)
        onProgress(0)
      }
    }
    return { blob: await encodeRecorder(s, onProgress, signal), needsAudio: true, encoder: 'recorder' }
  }
  finally {
    s.wave.dispose()
  }
}

/** WAV с сервера → AudioBuffer (без звука в колонках: OfflineAudioContext ничего не играет) */
export async function decodeStoryAudio(wav: ArrayBuffer): Promise<AudioBuffer> {
  const ctx = new OfflineAudioContext(2, 44_100, 44_100)
  return await ctx.decodeAudioData(wav.slice(0))
}
