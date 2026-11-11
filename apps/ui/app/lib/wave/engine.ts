// Двигатель волны: один холст WebGL на приложение, переезжает между экранами (главная, плеер).
// Пульс — в долю трека по часам или по звуку, размах — от энергии, детали — от спектра.
// Звук на сайте слушает анализатор WebAudio; в приложении звук играет движок на Rust — от него
// приходит уровень (RMS, пик, три полосы) ~20 раз в секунду.
// Бережёт батарею: DPR не выше 2, 30 кадров на узких экранах, пауза вне экрана и во фоне,
// при «уменьшении движения» — один статичный кадр.
import { FRAG, VERT } from './shader'

export type WaveMood = 'any' | 'energetic' | 'happy' | 'calm' | 'sad'
export type WaveLayout = 'hero' | 'hero-phone' | 'stage' | 'stage-phone' | 'full' | 'full-phone' | 'card'

type Vec3 = [number, number, number]

interface MoodSpec {
  pal: [string, string, string, string]
  energy: number
  spectrum: number
}

export const WAVE_MOODS: Record<WaveMood, MoodSpec> = {
  any: { pal: ['#07050f', '#1d1048', '#b01cf0', '#ffd8ea'], energy: 0.5, spectrum: 1 },
  energetic: { pal: ['#0e0602', '#7a2208', '#ff6a1a', '#ffd27a'], energy: 0.78, spectrum: 0 },
  happy: { pal: ['#0c0410', '#5c0f5e', '#ff4fa8', '#ffe9a8'], energy: 0.66, spectrum: 0 },
  calm: { pal: ['#020509', '#0a2a52', '#1d8cbc', '#aef0ff'], energy: 0.42, spectrum: 0 },
  sad: { pal: ['#05040b', '#1c1946', '#5b50c4', '#d3ccff'], energy: 0.3, spectrum: 0 },
}

const SPECTRUM: [string, string, string, string] = ['#4b21ff', '#b01cf0', '#ff3d7a', '#ff9a3c']

const LAYOUTS: Record<WaveLayout, { tilt: number, offset: number, fadeL: number, fadeR: number, amp: number, freq: number }> = {
  'hero': { tilt: 0.09, offset: -0.06, fadeL: 0.42, fadeR: 0, amp: 1.0, freq: 1.0 },
  'hero-phone': { tilt: -0.42, offset: 0.1, fadeL: 0, fadeR: 0, amp: 1.0, freq: 2.2 },
  'stage': { tilt: -0.06, offset: 0, fadeL: 0, fadeR: 0, amp: 0.85, freq: 1.0 },
  'stage-phone': { tilt: -0.18, offset: 0, fadeL: 0, fadeR: 0, amp: 0.9, freq: 2.0 },
  'full': { tilt: -0.12, offset: 0.02, fadeL: 0, fadeR: 0, amp: 1.25, freq: 1.0 },
  'full-phone': { tilt: -0.5, offset: 0.06, fadeL: 0, fadeR: 0, amp: 1.1, freq: 2.0 },
  'card': { tilt: -0.2, offset: 0, fadeL: 0, fadeR: 0, amp: 0.9, freq: 1.6 },
}

export interface WaveState {
  mood: WaveMood
  energy: number
  beat: number
  /** Пульс от настоящего звука: огибающая с быстрой атакой и медленным спадом по всплескам
   * громкости и найденным долям. Без звука (пауза, тишина, нет анализатора) — 0, по часам не бьётся */
  pulse: number
  bands: Vec3
  bpm: number
  playing: boolean
  source: 'idle' | 'clock' | 'audio'
  fps: number
}

/** Уровень звука от движка приложения: RMS, пик, полосы до 150 Гц, 150–2000 Гц, выше 2 кГц */
export interface TapLevels {
  rms: number
  peak: number
  bands: [number, number, number]
}

export interface AudioTap {
  /** Сайт: анализатор WebAudio */
  analyser?: AnalyserNode
  /** Приложение: последний уровень от движка; null — ещё нет */
  levels?: () => TapLevels | null
  /** Позиция трека в секундах — для сетки долей */
  time: () => number
  bpm?: number
  phase?: number
}

const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v))
const lerp = (a: number, b: number, k: number) => a + (b - a) * k
const easeInOut = (k: number) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2)
const easeOutExpo = (k: number) => (k >= 1 ? 1 : 1 - Math.pow(2, -10 * k))

function hexToLin(hex: string): Vec3 {
  const n = parseInt(hex.replace('#', ''), 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(c => Math.pow(c / 255, 2.2)) as Vec3
}
const mixLin = (a: Vec3, b: Vec3, k: number): Vec3 => [lerp(a[0], b[0], k), lerp(a[1], b[1], k), lerp(a[2], b[2], k)]

interface Palette {
  c: [Vec3, Vec3, Vec3, Vec3]
  spectrum: number
}

// Обложка — только оттенок: противоположный цвет в больших долях делает тёплые настроения грязными
function buildPalette(mood: WaveMood, cover: string[] | null): Palette {
  const spec = WAVE_MOODS[mood]
  const base = spec.pal.map(hexToLin) as [Vec3, Vec3, Vec3, Vec3]
  if (!cover?.length || spec.spectrum > 0) return { c: base, spectrum: spec.spectrum }
  const vivid = hexToLin(cover[0]!)
  const deep = hexToLin(cover[1] ?? cover[0]!)
  return {
    c: [mixLin(base[0], deep, 0.05), mixLin(base[1], deep, 0.12), mixLin(base[2], vivid, 0.08), base[3]],
    spectrum: spec.spectrum,
  }
}

// Огибающая доли: мягкая атака, экспоненциальный спад, сильная доля ярче
function pulse(beatPos: number): number {
  const phase = beatPos - Math.floor(beatPos)
  const accent = Math.floor(beatPos) % 4 === 0 ? 1 : 0.62
  return Math.min(1, phase / 0.035) * Math.exp(-phase * 5.2) * accent
}

const UNIFORMS = ['uRes', 'uTime', 'uBeat', 'uEnergy', 'uBands', 'uExpand', 'uFlash', 'uRipple', 'uC0', 'uC1', 'uC2', 'uC3', 'uS0', 'uS1', 'uS2', 'uS3', 'uSpectrum', 'uLayout', 'uFade', 'uFreq', 'uBox', 'uBoxR'] as const
type UniformName = (typeof UNIFORMS)[number]

export class WaveEngine {
  readonly canvas: HTMLCanvasElement
  private gl: WebGLRenderingContext | WebGL2RenderingContext | null = null
  private u = {} as Record<UniformName, WebGLUniformLocation | null>
  private container: HTMLElement | null = null
  private boxEl: HTMLElement | null = null
  private box: [number, number, number, number, number] | null = null
  private layout = LAYOUTS.hero
  private visible = false
  private lowPower = false
  private raf = 0
  private lastT = 0
  private lastDraw = 0
  private ro: ResizeObserver
  private io: IntersectionObserver
  private reduced: MediaQueryList
  private listeners = new Set<(s: WaveState) => void>()
  private tap: AudioTap | null = null
  private freq: Uint8Array<ArrayBuffer> | null = null
  private prev: Uint8Array<ArrayBuffer> | null = null
  private wave: Float32Array<ArrayBuffer> | null = null
  private fMean = 0
  private fDev = 0
  private lastOnset = 0
  private peak = 0.05
  private onset = 0
  private lastLevels: TapLevels | null = null
  private prevBands: Vec3 = [0, 0, 0]
  private envFast = 0
  private envSlow = 0
  readonly spectrumLin = SPECTRUM.map(hexToLin) as [Vec3, Vec3, Vec3, Vec3]
  glError: string | null = null

  // Состояние анимации
  private pal: Palette = buildPalette('any', null)
  private palFrom: Palette | null = null
  private palTo: Palette | null = null
  private palT = 1
  private cover: string[] | null = null
  private manualEnergy: number | null = null
  private energyTarget = WAVE_MOODS.any.energy
  private beatOrigin = 0
  private expand = 1
  private expandFrom = 1
  private expandAt = -1
  private flash = 0
  private ripple: Vec3 = [0, 9, 0]
  private time = 17.3

  readonly state: WaveState = { mood: 'any', energy: 0.3, beat: 0, pulse: 0, bands: [0, 0, 0], bpm: 120, playing: false, source: 'idle', fps: 0 }

  constructor() {
    this.canvas = document.createElement('canvas')
    this.canvas.className = 'wave-canvas'
    this.canvas.setAttribute('aria-hidden', 'true')
    this.reduced = matchMedia('(prefers-reduced-motion: reduce)')
    this.beatOrigin = performance.now()
    this.initGL()
    this.ro = new ResizeObserver(() => this.resize())
    this.io = new IntersectionObserver((entries) => {
      this.visible = entries.some(e => e.isIntersecting)
      this.kick()
    })
    document.addEventListener('visibilitychange', () => this.kick())
    this.reduced.addEventListener('change', () => this.kick())
    this.canvas.addEventListener('webglcontextlost', (e) => {
      e.preventDefault()
      cancelAnimationFrame(this.raf)
      this.raf = 0
    })
    this.canvas.addEventListener('webglcontextrestored', () => {
      this.initGL()
      this.resize()
      this.kick()
    })
  }

  get supported(): boolean {
    return !!this.gl
  }

  private initGL() {
    const opts: WebGLContextAttributes = { antialias: false, alpha: false, depth: false, stencil: false, premultipliedAlpha: false, powerPreference: 'high-performance' }
    const gl = (this.canvas.getContext('webgl2', opts) ?? this.canvas.getContext('webgl', opts)) as WebGLRenderingContext | WebGL2RenderingContext | null
    this.gl = gl
    if (!gl) return
    const compile = (type: number, src: string) => {
      const s = gl.createShader(type)
      if (!s) return null
      gl.shaderSource(s, src)
      gl.compileShader(s)
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
        this.glError = gl.getShaderInfoLog(s)
        return null
      }
      return s
    }
    const vs = compile(gl.VERTEX_SHADER, VERT)
    const fs = compile(gl.FRAGMENT_SHADER, FRAG)
    const prog = gl.createProgram()
    if (!vs || !fs || !prog) {
      this.gl = null
      return
    }
    gl.attachShader(prog, vs)
    gl.attachShader(prog, fs)
    gl.bindAttribLocation(prog, 0, 'aPos')
    gl.linkProgram(prog)
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      this.glError = gl.getProgramInfoLog(prog)
      this.gl = null
      return
    }
    gl.useProgram(prog)
    const buf = gl.createBuffer()
    gl.bindBuffer(gl.ARRAY_BUFFER, buf)
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW)
    gl.enableVertexAttribArray(0)
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0)
    for (const name of UNIFORMS) this.u[name] = gl.getUniformLocation(prog, name)
  }

  /** Перенести холст в контейнер. Контекст WebGL при этом сохраняется */
  mount(container: HTMLElement, opts: { layout?: WaveLayout, box?: HTMLElement | null } = {}) {
    this.layout = LAYOUTS[opts.layout ?? 'hero']
    this.boxEl = opts.box ?? null
    if (this.container !== container) {
      if (this.container) {
        this.ro.unobserve(this.container)
        this.io.unobserve(this.container)
      }
      this.container = container
      container.prepend(this.canvas)
      this.ro.observe(container)
      this.io.observe(container)
    }
    container.classList.toggle('wave-fallback', !this.gl)
    this.resize()
    this.kick()
  }

  unmount(container: HTMLElement) {
    if (this.container !== container) return
    this.ro.unobserve(container)
    this.io.unobserve(container)
    this.canvas.remove()
    this.container = null
    this.visible = false
    this.kick()
  }

  remeasure() {
    this.measureBox()
    if (this.isStatic()) this.draw()
  }

  private resize() {
    const c = this.container
    if (!c) return
    const w = Math.max(1, c.clientWidth)
    const h = Math.max(1, c.clientHeight)
    let scale = Math.min(window.devicePixelRatio || 1, 2)
    // Мягкая картинка не требует полного разрешения на больших холстах
    const budget = 2.4e6
    if (w * h * scale * scale > budget) scale = Math.sqrt(budget / (w * h))
    const W = Math.round(w * scale)
    const H = Math.round(h * scale)
    if (this.canvas.width !== W || this.canvas.height !== H) {
      this.canvas.width = W
      this.canvas.height = H
    }
    this.lowPower = w < 600
    this.measureBox()
    if (this.isStatic()) this.draw()
  }

  private measureBox() {
    this.box = null
    if (!this.boxEl || !this.container) return
    const r = this.boxEl.getBoundingClientRect()
    const c = this.container.getBoundingClientRect()
    if (!r.width || !c.height) return
    const cx = (r.left + r.width / 2 - c.left - c.width / 2) / c.height
    const cy = -(r.top + r.height / 2 - c.top - c.height / 2) / c.height
    const radius = parseFloat(getComputedStyle(this.boxEl).borderTopLeftRadius) || 0
    this.box = [cx, cy, r.width / 2 / c.height, r.height / 2 / c.height, radius / c.height]
  }

  private isStatic() {
    return this.reduced.matches
  }

  private active() {
    return !!(this.gl && this.container && this.visible && !document.hidden && !this.isStatic())
  }

  private kick() {
    if (this.isStatic()) {
      cancelAnimationFrame(this.raf)
      this.raf = 0
      this.state.beat = 0
      this.state.pulse = 0
      this.expand = 1
      this.flash = 0
      this.ripple = [0, 9, 0]
      if (this.palT < 1 && this.palTo) {
        this.pal = this.palTo
        this.palT = 1
      }
      this.draw()
      return
    }
    if (this.active() && !this.raf) {
      this.lastT = performance.now()
      this.raf = requestAnimationFrame(this.frame)
    }
  }

  private frame = (now: number) => {
    this.raf = 0
    if (!this.active()) return
    this.raf = requestAnimationFrame(this.frame)
    if (this.lowPower && now - this.lastDraw < 31) return
    // Медленный кадр не должен замедлять время анимации; скрытая вкладка и так на паузе
    const dt = Math.min(0.25, (now - this.lastT) / 1000)
    this.lastT = now
    if (this.lastDraw) this.state.fps = lerp(this.state.fps || 30, 1000 / Math.max(1, now - this.lastDraw), 0.1)
    this.lastDraw = now
    this.update(dt, now)
    this.draw()
    for (const fn of this.listeners) fn(this.state)
  }

  private update(dt: number, now: number) {
    const s = this.state
    if (this.palT < 1 && this.palFrom && this.palTo) {
      this.palT = Math.min(1, this.palT + dt / 1.2)
      const k = easeInOut(this.palT)
      const from = this.palFrom
      const to = this.palTo
      this.pal = {
        c: from.c.map((c, i) => mixLin(c, to.c[i]!, k)) as Palette['c'],
        spectrum: lerp(from.spectrum, to.spectrum, k),
      }
    }
    if (this.expandAt >= 0) {
      const k = clamp((now - this.expandAt) / 1600, 0, 1)
      this.expand = lerp(this.expandFrom, 1, easeOutExpo(k))
      if (k >= 1) this.expandAt = -1
    }
    this.flash *= Math.exp(-dt * 2.6)
    this.ripple[1] += dt

    if (this.tap && s.playing) {
      this.readAudio(dt, now)
      s.source = 'audio'
    }
    else {
      s.source = s.playing ? 'clock' : 'idle'
      s.bands = s.bands.map(v => v * Math.exp(-dt * 3)) as Vec3
      // Пульс кнопок и обложки — только от звука: по угаданному темпу он расходится с музыкой
      s.pulse *= Math.exp(-dt * 6)
      if (s.playing) s.beat = pulse(((now - this.beatOrigin) / 1000) * (s.bpm / 60))
      else s.beat = 0.12 * (0.5 + 0.5 * Math.sin((now / 1000) * 0.9))
      const target = this.manualEnergy ?? (s.playing ? this.energyTarget : 0.16)
      s.energy = lerp(s.energy, target, 1 - Math.exp(-dt * 2.2))
    }
    this.time += dt * (0.16 + 0.62 * s.energy)
  }

  private readAudio(dt: number, now: number) {
    const tap = this.tap!
    if (tap.analyser) this.readAnalyser(tap.analyser, dt, now)
    else this.readLevels(dt, now)
  }

  /** Сетка долей трека: пульс точно в долю, если анализ сервера знает темп */
  private grid(): number {
    const tap = this.tap!
    if (!tap.bpm) return 0
    const pos = (tap.time() - (tap.phase ?? 0)) * (tap.bpm / 60)
    this.state.bpm = tap.bpm
    return pos >= 0 ? pulse(pos) : 0
  }

  /** Энергия — от громкости относительно недавнего максимума, с добавкой баса */
  private follow(rms: number, bass: number, dt: number) {
    const s = this.state
    this.peak = Math.max(this.peak * Math.exp(-dt * 0.15), rms, 0.02)
    const target = clamp(Math.pow(rms / this.peak, 1.4) * 0.95 + bass * 0.25, 0, 1)
    s.energy = lerp(s.energy, this.manualEnergy ?? target, 1 - Math.exp(-dt * 4))
    this.followPulse(rms, dt)
  }

  /** Пульс: всплеск громкости над её медленным средним и найденные удары; сетка долей —
   * только если её знает анализ трека, и только пока есть звук */
  private followPulse(rms: number, dt: number) {
    const s = this.state
    this.envFast = lerp(this.envFast, rms, 1 - Math.exp(-dt * (rms > this.envFast ? 40 : 10)))
    this.envSlow = lerp(this.envSlow, rms, 1 - Math.exp(-dt * 1.5))
    const loud = clamp(rms / this.peak, 0, 1)
    const audible = rms > 0.004 ? 1 : 0
    const swell = clamp((this.envFast - this.envSlow) / (this.peak * 0.3), 0, 1)
    const target = audible * Math.max(swell, this.onset * 0.9 * loud, this.grid() * 0.6 * loud)
    s.pulse = lerp(s.pulse, target, 1 - Math.exp(-dt * (target > s.pulse ? 35 : 5)))
  }

  /** Приложение: уровень от движка вместо спектра — те же полосы, рывки и энергия */
  private readLevels(dt: number, now: number) {
    const s = this.state
    const l = this.tap!.levels?.() ?? null
    // RMS полосы в дБ → 0…1: от −66 дБ (тишина) до −6 дБ (громко), как байты анализатора
    const norm = (x: number) => clamp((20 * Math.log10(Math.max(x, 1e-6)) + 66) / 60, 0, 1)
    const raw: Vec3 = l ? [norm(l.bands[0]), norm(l.bands[1]), norm(l.bands[2])] : [0, 0, 0]
    s.bands = s.bands.map((v, i) => lerp(v, raw[i]!, 1 - Math.exp(-dt * 14))) as Vec3
    // Новое окно уровня: рост полос — «поток», как разница спектров на сайте
    if (l && l !== this.lastLevels) {
      this.lastLevels = l
      let flux = 0
      for (let i = 0; i < 3; i++) flux += Math.max(0, raw[i]! - this.prevBands[i]!)
      flux /= 3
      this.prevBands = raw
      this.fMean = lerp(this.fMean, flux, 0.06)
      this.fDev = lerp(this.fDev, Math.abs(flux - this.fMean), 0.06)
      if (flux > this.fMean + 1.7 * this.fDev + 0.01 && now - this.lastOnset > 170) {
        this.lastOnset = now
        this.onset = 1
      }
    }
    this.onset *= Math.exp(-dt * 7)
    s.beat = Math.max(this.grid() * 0.8, this.onset * 0.95)
    this.follow(l?.rms ?? 0, raw[0], dt)
  }

  private readAnalyser(an: AnalyserNode, dt: number, now: number) {
    const s = this.state
    if (!this.freq || this.freq.length !== an.frequencyBinCount) {
      this.freq = new Uint8Array(an.frequencyBinCount)
      this.prev = new Uint8Array(an.frequencyBinCount)
      this.wave = new Float32Array(an.fftSize)
    }
    const freq = this.freq
    const prev = this.prev!
    const wave = this.wave!
    an.getByteFrequencyData(freq)
    an.getFloatTimeDomainData(wave)
    const hz = an.context.sampleRate / an.fftSize
    const band = (lo: number, hi: number) => {
      const a = Math.max(1, Math.floor(lo / hz))
      const b = Math.min(freq.length - 1, Math.ceil(hi / hz))
      let sum = 0
      for (let i = a; i <= b; i++) sum += freq[i]!
      return sum / ((b - a + 1) * 255)
    }
    const raw: Vec3 = [band(30, 150), band(150, 2000), band(2000, 9000)]
    s.bands = s.bands.map((v, i) => lerp(v, raw[i]!, 1 - Math.exp(-dt * 14))) as Vec3
    let flux = 0
    const lim = Math.min(freq.length, Math.ceil(7000 / hz))
    for (let i = 1; i < lim; i++) {
      const d = freq[i]! - prev[i]!
      if (d > 0) flux += d
      prev[i] = freq[i]!
    }
    flux /= lim * 255
    this.fMean = lerp(this.fMean, flux, 0.06)
    this.fDev = lerp(this.fDev, Math.abs(flux - this.fMean), 0.06)
    if (flux > this.fMean + 1.7 * this.fDev + 0.004 && now - this.lastOnset > 170) {
      this.lastOnset = now
      this.onset = 1
    }
    this.onset *= Math.exp(-dt * 7)
    s.beat = Math.max(this.grid() * 0.8, this.onset * 0.95)
    let sum = 0
    for (let i = 0; i < wave.length; i++) sum += wave[i]! * wave[i]!
    this.follow(Math.sqrt(sum / wave.length), raw[0], dt)
  }

  private draw() {
    const gl = this.gl
    if (!gl || !this.container) return
    const s = this.state
    const u = this.u
    const L = this.layout
    const still = this.isStatic()
    gl.viewport(0, 0, this.canvas.width, this.canvas.height)
    gl.uniform2f(u.uRes, this.canvas.width, this.canvas.height)
    gl.uniform1f(u.uTime, this.time)
    gl.uniform1f(u.uBeat, still ? 0 : s.beat)
    gl.uniform1f(u.uEnergy, still ? Math.max(s.energy, 0.42) : s.energy)
    gl.uniform3f(u.uBands, s.bands[0], s.bands[1], s.bands[2])
    gl.uniform1f(u.uExpand, still ? 1 : this.expand)
    gl.uniform1f(u.uFlash, still ? 0 : this.flash)
    gl.uniform3f(u.uRipple, this.ripple[0], this.ripple[1], this.ripple[2])
    const [c0, c1, c2, c3] = this.pal.c
    gl.uniform3f(u.uC0, c0[0], c0[1], c0[2])
    gl.uniform3f(u.uC1, c1[0], c1[1], c1[2])
    gl.uniform3f(u.uC2, c2[0], c2[1], c2[2])
    gl.uniform3f(u.uC3, c3[0], c3[1], c3[2])
    const [s0, s1, s2, s3] = this.spectrumLin
    gl.uniform3f(u.uS0, s0[0], s0[1], s0[2])
    gl.uniform3f(u.uS1, s1[0], s1[1], s1[2])
    gl.uniform3f(u.uS2, s2[0], s2[1], s2[2])
    gl.uniform3f(u.uS3, s3[0], s3[1], s3[2])
    gl.uniform1f(u.uSpectrum, this.pal.spectrum)
    gl.uniform4f(u.uLayout, L.tilt, L.offset, 0, L.amp)
    gl.uniform2f(u.uFade, L.fadeL, L.fadeR)
    gl.uniform1f(u.uFreq, L.freq)
    if (this.box) {
      gl.uniform4f(u.uBox, this.box[0], this.box[1], this.box[2], this.box[3])
      gl.uniform1f(u.uBoxR, this.box[4])
    }
    else {
      gl.uniform4f(u.uBox, 0, 0, 0, 0)
      gl.uniform1f(u.uBoxR, 0)
    }
    gl.drawArrays(gl.TRIANGLES, 0, 3)
  }

  // ---------- Управление ----------

  setMood(mood: WaveMood, opts: { instant?: boolean } = {}) {
    this.state.mood = mood
    const next = buildPalette(mood, this.cover)
    if (opts.instant || this.isStatic()) {
      this.pal = next
      this.palT = 1
      this.draw()
    }
    else {
      this.palFrom = { c: this.pal.c.map(c => [...c] as Vec3) as Palette['c'], spectrum: this.pal.spectrum }
      this.palTo = next
      this.palT = 0
      this.kick()
    }
    if (this.manualEnergy == null) this.energyTarget = WAVE_MOODS[mood].energy
  }

  setCover(colors: string[] | null, opts: { instant?: boolean } = {}) {
    this.cover = colors?.length ? colors : null
    this.setMood(this.state.mood, opts)
  }

  setEnergy(v: number | null) {
    this.manualEnergy = v == null ? null : clamp(v, 0, 1)
    this.kick()
  }

  setTrackEnergy(v: number) {
    this.energyTarget = clamp(v, 0, 1)
  }

  setBpm(bpm: number) {
    const s = this.state
    const now = performance.now()
    const pos = ((now - this.beatOrigin) / 1000) * (s.bpm / 60)
    s.bpm = clamp(bpm || 120, 40, 220)
    this.beatOrigin = now - ((pos % 1) / (s.bpm / 60)) * 1000
  }

  setPlaying(on: boolean) {
    const s = this.state
    if (on && !s.playing) this.beatOrigin = performance.now()
    s.playing = on
    this.kick()
  }

  /** Звук, который слушает волна. null — пульс по часам */
  setAudioTap(tap: AudioTap | null) {
    this.tap = tap
    this.envFast = 0
    this.envSlow = 0
    this.peak = 0.05
    this.lastLevels = null
  }

  start() {
    this.expandFrom = 0
    this.expand = 0
    this.expandAt = performance.now()
    this.flash = Math.max(this.flash, 0.55)
    this.setPlaying(true)
  }

  skip() {
    this.ripple = [(Math.random() - 0.5) * 0.6, 0, 1]
    this.state.energy *= 0.7
    this.beatOrigin = performance.now()
    this.kick()
  }

  like() {
    this.flash = 1
    this.kick()
  }

  onFrame(fn: (s: WaveState) => void): () => void {
    this.listeners.add(fn)
    return () => this.listeners.delete(fn)
  }
}

let engine: WaveEngine | null = null

/** Один двигатель на приложение: контекстов WebGL у браузера немного */
export function getWaveEngine(): WaveEngine {
  engine ??= new WaveEngine()
  return engine
}
