// Волна DalyWave для историй «Итогов года» в стиле «Волна года»: тот же шейдер, что у волны
// приложения (lib/wave), с двумя добавками из макета design/rewind/wave.js:
//  - uChart: лента ложится графиком минут по месяцам (12 значений);
//  - uBoxK: свечение вокруг фото или обложки появляется и гаснет плавно.
// Раскладка и палитра перетекают между слайдами сами; холст один на все истории.

const VERT = 'attribute vec2 aPos;void main(){gl_Position=vec4(aPos,0.0,1.0);}'

const FRAG = `
precision highp float;
uniform vec2 uRes;
uniform float uTime;
uniform float uBeat;
uniform float uEnergy;
uniform vec3 uBands;
uniform float uExpand;
uniform float uFlash;
uniform vec3 uRipple;
uniform vec3 uC0;
uniform vec3 uC1;
uniform vec3 uC2;
uniform vec3 uC3;
uniform vec4 uLayout;
uniform vec2 uFade;
uniform float uFreq;
uniform vec4 uBox;
uniform float uBoxR;
uniform float uBoxK;
uniform float uChart;
uniform float uMonths[12];
uniform vec2 uChartX;
uniform vec2 uChartY;
uniform float uVig;

float hash(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}
float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
float fbm(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  for (int i = 0; i < 4; i++) {
    v += a * noise(p);
    p = mat2(1.6, 1.2, -1.2, 1.6) * p + 7.3;
    a *= 0.5;
  }
  return v;
}
float sdRoundBox(vec2 p, vec2 b, float r) {
  vec2 q = abs(p) - b + r;
  return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}
float caustic(vec2 p, float t) {
  vec2 a = p * vec2(9.0, 5.0);
  float f = sin(a.x + 1.7 * sin(a.y * 0.8 + t) + t * 0.6) + sin(a.y * 1.3 + 1.9 * sin(a.x * 0.7 - t * 0.8) - t * 0.4);
  float lines = 1.0 - smoothstep(0.0, 0.32, abs(f));
  return lines * lines;
}
float chartAt(float x) {
  float u = clamp((x - uChartX.x) / (uChartX.y - uChartX.x), 0.0, 1.0) * 11.0;
  float s = 0.0;
  float w = 0.0;
  for (int i = 0; i < 12; i++) {
    float d = u - float(i);
    float k = exp(-d * d * 2.2);
    s += uMonths[i] * k;
    w += k;
  }
  return mix(uChartY.x, uChartY.y, s / w);
}
float ribbon(float x, float t, float amp) {
  float wav = amp * (sin(x * 1.15 * uFreq + t * 0.35) + 0.55 * sin(x * 2.4 * uFreq - t * 0.42 + 3.6));
  if (uChart < 0.001) return wav;
  return mix(wav, chartAt(x) + wav * 0.08, uChart);
}

void main() {
  vec2 frag = gl_FragCoord.xy;
  vec2 p = (frag - 0.5 * uRes) / uRes.y;
  float ex = uExpand;
  float e = uEnergy;
  float t = uTime;

  float cs = cos(uLayout.x);
  float sn = sin(uLayout.x);
  vec2 q = vec2(cs * p.x + sn * p.y, -sn * p.x + cs * p.y);
  q.y -= uLayout.y;

  vec2 w = vec2(fbm(q * 1.3 + vec2(0.0, t * 0.1)), fbm(q * 1.3 + vec2(4.7, -t * 0.08)));
  q += (w - 0.5) * (0.07 + 0.12 * e + 0.06 * uBands.y) * (0.35 + 0.65 * ex) * (1.0 - 0.8 * uChart);

  float rd = abs(q.x - uRipple.x);
  float front = uRipple.y * 0.95;
  float ripEnv = exp(-abs(rd - front) * 4.5) * exp(-uRipple.y * 1.25) * uRipple.z;
  float rip = sin((rd - front) * 24.0) * ripEnv * 0.045;

  float amp = (0.055 + 0.1 * e) * uLayout.w * mix(0.1, 1.0, ex);
  float thick = (0.085 + 0.05 * e + 0.035 * uBands.x) * (1.0 + 0.18 * uBeat) * mix(0.12, 1.0, ex);

  float y0 = ribbon(q.x, t, amp) + rip;
  float dy = q.y - y0;
  float above = max(dy, 0.0);
  float below = max(-dy, 0.0);

  float tw = q.x * 1.7 * uFreq + t * 0.33 + 1.2 * sin(q.x * 0.7 * uFreq - t * 0.15);
  float faceH = thick * (1.5 + 1.2 * (0.5 + 0.5 * sin(tw))) * (1.0 + 1.6 * uChart);
  float fz = below / faceH;
  float body = exp(-pow(fz, 1.5)) * exp(-above * above / (0.00035 + 0.0005 * e));

  float m = fbm(vec2(q.x * 1.9 * uFreq - t * 0.2, fz * 2.6 + t * 0.05));
  float ca = caustic(vec2(q.x * 0.42 * uFreq - t * 0.02, fz * 0.32 + 0.13), t * 0.45 + e);
  vec3 faceCol = mix(uC1, uC2, smoothstep(0.15, 0.9, m) * (1.0 - 0.55 * min(fz, 1.0)));

  vec3 col = uC0;
  col += faceCol * body * (0.7 + 0.35 * uBeat + 0.15 * uBands.x);
  col += mix(uC2, uC3, 0.6) * ca * body * smoothstep(0.05, 0.4, fz) * (0.18 + 0.22 * e + 0.15 * uBeat);

  float sheen = 0.55 + 0.45 * sin(q.x * 3.1 * uFreq + t * 0.8 + 1.5 * sin(t * 0.3));
  float core = exp(-abs(dy) / (0.0042 + 0.005 * e + 0.004 * uBeat));
  col += uC3 * core * (0.3 + 0.5 * sheen + 0.5 * uBeat + 0.2 * uBands.z) * ex;
  float glow = exp(-abs(dy) * (15.0 - 4.0 * e - 3.0 * uBeat));
  col += mix(uC2, uC3, 0.35) * glow * (0.1 + 0.12 * e + 0.2 * uBeat) * ex;

  for (int i = 1; i < 3; i++) {
    float fi = float(i);
    float yi = ribbon(q.x * (1.0 + 0.09 * fi * (1.0 - uChart)), t - 0.7 * fi, amp * (0.8 - 0.15 * fi)) - (0.075 + 0.02 * e) * fi * ex + rip * 0.5;
    float di = q.y - yi;
    col += mix(uC2, uC3, 0.3) * exp(-abs(di) / (0.0035 + 0.0025 * fi)) * (0.2 / fi) * ex * body;
    col += uC2 * exp(-abs(di) * 16.0) * (0.06 / fi) * ex;
  }

  float patch = smoothstep(0.3, 0.75, noise(vec2(q.x * 1.6 * uFreq - t * 0.12, 3.0)));
  float rn = noise(vec2(q.x * 26.0 * uFreq + w.x * 3.0, t * 0.3));
  float rays = smoothstep(0.45, 1.0, rn) * patch;
  float rayMask = smoothstep(0.0, 0.025, above) * exp(-above * (15.0 - 5.0 * e - 3.0 * uBeat));
  col += mix(uC2, uC3, 0.3) * rays * rayMask * (0.16 + 0.3 * e + 0.3 * uBeat) * ex;
  col += uC1 * exp(-above * 5.0) * 0.1 * ex;

  col += uC3 * ripEnv * 0.45 * exp(-abs(dy) * 8.0);

  if (uBox.z > 0.0 && uBoxK > 0.001) {
    float sd = sdRoundBox(p - uBox.xy, uBox.zw, uBoxR);
    float halo = exp(-max(sd, 0.0) * 13.0) * (0.5 + 0.5 * uBeat);
    col += mix(uC2, uC3, 0.3) * halo * (0.25 + 0.75 * min(body + glow, 1.0)) * 0.8 * uBoxK;
    col += uC1 * exp(-max(sd, 0.0) * 4.0) * 0.35 * uBoxK;
  }

  float fx = frag.x / uRes.x;
  float fade = 1.0;
  if (uFade.x > 0.0) fade *= mix(0.4, 1.0, smoothstep(0.0, uFade.x, fx));
  if (uFade.y > 0.0) fade *= mix(0.6, 1.0, smoothstep(1.0, 1.0 - uFade.y, fx));
  col = uC0 + (col - uC0) * fade;

  col += uC3 * uFlash * (0.35 * min(body + core, 1.4) + 0.05 + 0.15 * glow);

  vec2 vp = frag / uRes - 0.5;
  col *= 1.0 - uVig * dot(vp, vp);

  col = 1.0 - exp(-col * 1.45);
  col = pow(max(col, 0.0), vec3(1.0 / 2.2));
  col += (hash(frag + fract(t * 7.0) * 61.0) - 0.5) / 180.0;
  gl_FragColor = vec4(col, 1.0);
}`

const UNIFORMS = ['uRes', 'uTime', 'uBeat', 'uEnergy', 'uBands', 'uExpand', 'uFlash', 'uRipple', 'uC0', 'uC1', 'uC2', 'uC3', 'uLayout', 'uFade', 'uFreq', 'uBox', 'uBoxR', 'uBoxK', 'uChart', 'uMonths', 'uChartX', 'uChartY', 'uVig'] as const
type Uniform = typeof UNIFORMS[number]

const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v))
const lerp = (a: number, b: number, k: number) => a + (b - a) * k
const easeOutExpo = (k: number) => (k >= 1 ? 1 : 1 - 2 ** (-10 * k))

type Rgb = [number, number, number]
function hexToLin(hex: string): Rgb {
  const n = Number.parseInt(hex.replace('#', ''), 16) || 0
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(c => (c / 255) ** 2.2) as Rgb
}

/** Раскладка волны на слайде */
export interface WaveLayout {
  /** Наклон ленты, радианы */
  tilt: number
  /** Сдвиг ленты по высоте, доли высоты (вверх — плюс) */
  offset: number
  amp: number
  freq: number
  fadeL: number
  fadeR: number
  /** Энергия: размах, толщина, скорость */
  energy: number
  /** 0..1 — лента ложится графиком месяцев */
  chart: number
  /** 0..1 — свечение вокруг фото или обложки */
  boxK: number
  vig: number
}

export const WAVE_DEFAULTS: WaveLayout = { tilt: -0.42, offset: 0.1, amp: 1, freq: 2.2, fadeL: 0, fadeR: 0, energy: 0.5, chart: 0, boxK: 0, vig: 0.6 }

/** Палитра «спокойно» — пока цвета обложки не посчитаны */
export const WAVE_CALM = ['#020509', '#0a2a52', '#1d8cbc', '#aef0ff']

/** Та же кривая месяцев, что в шейдере, — чтобы подписи вставали точно на ленту. `u` — 0..11 */
export function chartValue(months: number[], u: number): number {
  let s = 0
  let w = 0
  for (let i = 0; i < 12; i++) {
    const d = u - i
    const k = Math.exp(-d * d * 2.2)
    s += (months[i] ?? 0) * k
    w += k
  }
  return s / w
}

interface Gl {
  gl: WebGLRenderingContext
  u: Record<Uniform, WebGLUniformLocation | null>
}

function createGl(canvas: HTMLCanvasElement, preserve: boolean): Gl | null {
  const opts: WebGLContextAttributes = { antialias: false, alpha: false, depth: false, stencil: false, premultipliedAlpha: false, preserveDrawingBuffer: preserve }
  const gl = (canvas.getContext('webgl2', opts) ?? canvas.getContext('webgl', opts)) as WebGLRenderingContext | null
  if (!gl) return null
  const sh = (type: number, src: string) => {
    const s = gl.createShader(type)
    if (!s) return null
    gl.shaderSource(s, src)
    gl.compileShader(s)
    return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null
  }
  const vs = sh(gl.VERTEX_SHADER, VERT)
  const fs = sh(gl.FRAGMENT_SHADER, FRAG)
  const prog = gl.createProgram()
  if (!vs || !fs || !prog) return null
  gl.attachShader(prog, vs)
  gl.attachShader(prog, fs)
  gl.bindAttribLocation(prog, 0, 'aPos')
  gl.linkProgram(prog)
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return null
  gl.useProgram(prog)
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer())
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW)
  gl.enableVertexAttribArray(0)
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0)
  const u = {} as Record<Uniform, WebGLUniformLocation | null>
  for (const name of UNIFORMS) u[name] = gl.getUniformLocation(prog, name)
  return { gl, u }
}

interface FrameState {
  time: number
  beat: number
  expand: number
  flash: number
  ripple: [number, number, number]
  pal: Rgb[]
  layout: WaveLayout
  box: [number, number, number, number, number] | null
  months: number[]
  chartX: [number, number]
  chartY: [number, number]
}

function draw({ gl, u }: Gl, w: number, h: number, s: FrameState) {
  const L = s.layout
  gl.viewport(0, 0, w, h)
  gl.uniform2f(u.uRes, w, h)
  gl.uniform1f(u.uTime, s.time)
  gl.uniform1f(u.uBeat, s.beat * 0.8)
  gl.uniform1f(u.uEnergy, L.energy)
  gl.uniform3f(u.uBands, 0.25 * s.beat, 0.2, 0.15)
  gl.uniform1f(u.uExpand, s.expand)
  gl.uniform1f(u.uFlash, s.flash)
  gl.uniform3f(u.uRipple, s.ripple[0], s.ripple[1], s.ripple[2])
  const [c0, c1, c2, c3] = s.pal as [Rgb, Rgb, Rgb, Rgb]
  gl.uniform3f(u.uC0, c0[0], c0[1], c0[2])
  gl.uniform3f(u.uC1, c1[0], c1[1], c1[2])
  gl.uniform3f(u.uC2, c2[0], c2[1], c2[2])
  gl.uniform3f(u.uC3, c3[0], c3[1], c3[2])
  gl.uniform4f(u.uLayout, L.tilt, L.offset, 0, L.amp)
  gl.uniform2f(u.uFade, L.fadeL, L.fadeR)
  gl.uniform1f(u.uFreq, L.freq)
  const b = s.box
  gl.uniform4f(u.uBox, b?.[0] ?? 0, b?.[1] ?? 0, b?.[2] ?? 0, b?.[3] ?? 0)
  gl.uniform1f(u.uBoxR, b?.[4] ?? 0)
  gl.uniform1f(u.uBoxK, L.boxK)
  gl.uniform1f(u.uChart, L.chart)
  gl.uniform1fv(u.uMonths, new Float32Array(s.months))
  gl.uniform2f(u.uChartX, s.chartX[0], s.chartX[1])
  gl.uniform2f(u.uChartY, s.chartY[0], s.chartY[1])
  gl.uniform1f(u.uVig, L.vig)
  gl.drawArrays(gl.TRIANGLES, 0, 3)
}

const normMonths = (values: number[]) => {
  const max = Math.max(1, ...values)
  return Array.from({ length: 12 }, (_, i) => (values[i] ?? 0) / max)
}

export class RewindWave {
  readonly canvas: HTMLCanvasElement
  private ctx: Gl | null
  private readonly container: HTMLElement
  private readonly reduced: MediaQueryList
  private cur: WaveLayout = { ...WAVE_DEFAULTS }
  private target: WaveLayout = { ...WAVE_DEFAULTS }
  private pal: Rgb[] = WAVE_CALM.map(hexToLin)
  private palTo: Rgb[] = this.pal
  private months = new Array<number>(12).fill(0.5)
  /** Где лежит график по ширине и высоте — в координатах шейдера */
  chartX: [number, number] = [-0.2, 0.2]
  chartY: [number, number] = [-0.3, 0.05]
  bpm = 112
  private time = 17.3
  private beat = 0
  private expand = 1
  private expandAt = -1
  private flash = 0
  private ripple: [number, number, number] = [0, 9, 0]
  private boxEl: HTMLElement | null = null
  private box: FrameState['box'] = null
  private cssW = 1
  private cssH = 1
  private last = 0
  private raf = 0
  private alive = true
  private readonly ro: ResizeObserver

  constructor(container: HTMLElement) {
    this.container = container
    this.canvas = document.createElement('canvas')
    this.canvas.className = 'rw-wave-canvas'
    this.canvas.setAttribute('aria-hidden', 'true')
    container.prepend(this.canvas)
    this.reduced = window.matchMedia('(prefers-reduced-motion: reduce)')
    this.ctx = createGl(this.canvas, false)
    this.ro = new ResizeObserver(() => this.resize())
    this.ro.observe(container)
    this.resize()
    this.last = performance.now()
    this.raf = requestAnimationFrame(this.frame)
  }

  /** Есть ли WebGL: без него страница рисует плоский фон */
  get ok(): boolean {
    return !!this.ctx
  }

  private resize() {
    const w = Math.max(1, this.container.clientWidth)
    const h = Math.max(1, this.container.clientHeight)
    let scale = Math.min(window.devicePixelRatio || 1, 2)
    // Бюджет пикселей: шейдер тяжёлый, на большом экране — чуть мягче, зато ровно
    const budget = 2.4e6
    if (w * h * scale * scale > budget) scale = Math.sqrt(budget / (w * h))
    this.canvas.width = Math.round(w * scale)
    this.canvas.height = Math.round(h * scale)
    this.cssW = w
    this.cssH = h
  }

  /** Координаты шейдера: центр холста — 0, высота — 1, вверх — плюс */
  toShader(px: number, py: number): [number, number] {
    return [(px - this.cssW / 2) / this.cssH, -(py - this.cssH / 2) / this.cssH]
  }

  set(params: Partial<WaveLayout>, opts: { instant?: boolean } = {}) {
    Object.assign(this.target, params)
    if (opts.instant || this.reduced.matches) Object.assign(this.cur, this.target)
  }

  setPalette(hexes: string[], opts: { instant?: boolean } = {}) {
    this.palTo = hexes.slice(0, 4).map(hexToLin)
    if (opts.instant || this.reduced.matches) this.pal = this.palTo
  }

  setMonths(values: number[]) {
    this.months = normMonths(values)
  }

  setBox(el: HTMLElement | null) {
    this.boxEl = el
  }

  /** Волна раскрывается со вспышкой — начало историй */
  start() {
    this.expand = 0
    this.expandAt = performance.now()
    this.flash = 0.55
  }

  /** Рябь по ленте — переход на следующий слайд */
  skip() {
    this.ripple = [(Math.random() - 0.5) * 0.3, 0, 1]
  }

  destroy() {
    this.alive = false
    cancelAnimationFrame(this.raf)
    this.ro.disconnect()
    this.ctx?.gl.getExtension('WEBGL_lose_context')?.loseContext()
    this.ctx = null
    this.canvas.remove()
  }

  private measureBox() {
    this.box = null
    if (!this.boxEl) return
    const r = this.boxEl.getBoundingClientRect()
    const c = this.container.getBoundingClientRect()
    if (!r.width || !c.height) return
    const cx = (r.left + r.width / 2 - c.left - c.width / 2) / c.height
    const cy = -(r.top + r.height / 2 - c.top - c.height / 2) / c.height
    const radius = Number.parseFloat(getComputedStyle(this.boxEl).borderTopLeftRadius) || 0
    this.box = [cx, cy, r.width / 2 / c.height, r.height / 2 / c.height, Math.min(radius, r.width / 2) / c.height]
  }

  private frame = (now: number) => {
    if (!this.alive) return
    this.raf = requestAnimationFrame(this.frame)
    // Вкладка скрыта — не рисуем
    if (document.hidden) {
      this.last = now
      return
    }
    const dt = Math.min(0.1, (now - this.last) / 1000)
    this.last = now
    const still = this.reduced.matches
    // Раскладка и палитра перетекают за ~1,2 с
    const k = still ? 1 : 1 - Math.exp(-dt * 2.6)
    for (const key of Object.keys(this.target) as (keyof WaveLayout)[]) this.cur[key] = lerp(this.cur[key], this.target[key], k)
    const kp = still ? 1 : 1 - Math.exp(-dt * 2.2)
    this.pal = this.pal.map((c, i) => c.map((v, j) => lerp(v, this.palTo[i]?.[j] ?? v, kp)) as Rgb)
    if (this.expandAt >= 0) {
      const e = clamp((now - this.expandAt) / 1600, 0, 1)
      this.expand = easeOutExpo(e)
      if (e >= 1) this.expandAt = -1
    }
    this.flash *= Math.exp(-dt * 2.6)
    this.ripple[1] += dt
    if (!still) {
      const pos = (now / 1000) * (this.bpm / 60)
      const ph = pos - Math.floor(pos)
      const accent = Math.floor(pos) % 4 === 0 ? 1 : 0.62
      this.beat = Math.min(1, ph / 0.035) * Math.exp(-ph * 5.2) * accent
      this.time += dt * (0.16 + 0.62 * this.cur.energy)
    }
    else {
      this.beat = 0
    }
    this.measureBox()
    if (!this.ctx) return
    draw(this.ctx, this.canvas.width, this.canvas.height, {
      time: this.time,
      beat: still ? 0 : this.beat,
      expand: still ? 1 : this.expand,
      flash: still ? 0 : this.flash,
      ripple: this.ripple,
      pal: this.pal,
      layout: this.cur,
      box: this.box,
      months: this.months,
      chartX: this.chartX,
      chartY: this.chartY,
    })
  }
}

/**
 * Один неподвижный кадр волны заданного размера — для картинки «Поделиться».
 * `null` — WebGL недоступен
 */
export function renderWaveStill(width: number, height: number, layout: Partial<WaveLayout>, palette: string[], opts: { time?: number, months?: number[] } = {}): HTMLCanvasElement | null {
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const ctx = createGl(canvas, true)
  if (!ctx) return null
  draw(ctx, width, height, {
    time: opts.time ?? 23.1,
    beat: 0.35,
    expand: 1,
    flash: 0,
    ripple: [0, 9, 0],
    pal: palette.slice(0, 4).map(hexToLin),
    layout: { ...WAVE_DEFAULTS, ...layout },
    box: null,
    months: normMonths(opts.months ?? []),
    chartX: [-0.2, 0.2],
    chartY: [-0.3, 0.05],
  })
  // Кадр — в 2D: контекст WebGL можно отпустить сразу
  const out = document.createElement('canvas')
  out.width = width
  out.height = height
  out.getContext('2d')?.drawImage(canvas, 0, 0)
  ctx.gl.getExtension('WEBGL_lose_context')?.loseContext()
  return out
}
