// Волна для видео-истории: тот же шейдер «Полночь» (lib/wave/shader.ts), но свой холст и кадры по
// команде — не по requestAnimationFrame: время кадра задаёт кодировщик, а не часы.
import { FRAG, VERT } from '../wave/shader'

type Vec3 = [number, number, number]

const UNIFORMS = ['uRes', 'uTime', 'uBeat', 'uEnergy', 'uBands', 'uExpand', 'uFlash', 'uRipple', 'uC0', 'uC1', 'uC2', 'uC3', 'uS0', 'uS1', 'uS2', 'uS3', 'uSpectrum', 'uLayout', 'uFade', 'uFreq', 'uBox', 'uBoxR', 'uLight'] as const

export function hexToLin(hex: string): Vec3 {
  const n = parseInt(hex.replace('#', ''), 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(c => Math.pow(c / 255, 2.2)) as Vec3
}
const mix = (a: Vec3, b: Vec3, k: number): Vec3 => [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
const scale = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k]

/** Основа, толща, тело, гребень — в линейном цвете; spectrum — доля спектра «Сумерек» */
export interface StoryPalette {
  c: [Vec3, Vec3, Vec3, Vec3]
  spectrum: number
}

const BRAND: [string, string, string, string] = ['#07050f', '#1d1048', '#b01cf0', '#ffd8ea']
const SPECTRUM = ['#4b21ff', '#b01cf0', '#ff3d7a', '#ff9a3c'].map(hexToLin) as [Vec3, Vec3, Vec3, Vec3]

/** Палитра из цветов обложки (`palette` альбома: самый заметный первым); без них — цвета бренда.
 * Та же формула, что у заставки для Mac (apps/shell/saver): основа тёмная, тело и гребень — краска обложки */
export function storyPalette(cover: string[] | null | undefined): StoryPalette {
  const base = BRAND.map(hexToLin) as [Vec3, Vec3, Vec3, Vec3]
  if (!cover?.length) return { c: base, spectrum: 1 }
  const vivid = hexToLin(cover[0]!)
  const deep = scale(hexToLin(cover[1] ?? cover[0]!), 0.6)
  return {
    c: [mix(base[0], deep, 0.1), mix(base[1], scale(vivid, 0.3), 0.8), mix(base[2], vivid, 0.92), mix(vivid, [1, 0.96, 0.94], 0.6)],
    spectrum: 0,
  }
}

export interface WaveFrame {
  time: number
  beat: number
  energy: number
  bands: Vec3
  expand: number
  flash: number
}

export class StoryWave {
  readonly canvas: HTMLCanvasElement
  private gl: WebGLRenderingContext
  private u = {} as Record<(typeof UNIFORMS)[number], WebGLUniformLocation | null>

  constructor(width: number, height: number, private palette: StoryPalette) {
    this.canvas = document.createElement('canvas')
    this.canvas.width = width
    this.canvas.height = height
    const opts: WebGLContextAttributes = { antialias: false, alpha: false, depth: false, stencil: false, preserveDrawingBuffer: true }
    const gl = (this.canvas.getContext('webgl2', opts) ?? this.canvas.getContext('webgl', opts)) as WebGLRenderingContext | null
    if (!gl) throw new Error('webgl')
    this.gl = gl
    const compile = (type: number, src: string) => {
      const s = gl.createShader(type)!
      gl.shaderSource(s, src)
      gl.compileShader(s)
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? 'shader')
      return s
    }
    const prog = gl.createProgram()!
    gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT))
    gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG))
    gl.bindAttribLocation(prog, 0, 'aPos')
    gl.linkProgram(prog)
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog) ?? 'program')
    gl.useProgram(prog)
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer())
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW)
    gl.enableVertexAttribArray(0)
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0)
    for (const name of UNIFORMS) this.u[name] = gl.getUniformLocation(prog, name)
  }

  draw(f: WaveFrame) {
    const { gl, u } = this
    const [c0, c1, c2, c3] = this.palette.c
    gl.viewport(0, 0, this.canvas.width, this.canvas.height)
    gl.uniform2f(u.uRes, this.canvas.width, this.canvas.height)
    gl.uniform1f(u.uTime, f.time)
    gl.uniform1f(u.uBeat, f.beat)
    gl.uniform1f(u.uEnergy, f.energy)
    gl.uniform3f(u.uBands, f.bands[0], f.bands[1], f.bands[2])
    gl.uniform1f(u.uExpand, f.expand)
    gl.uniform1f(u.uFlash, f.flash)
    gl.uniform3f(u.uRipple, 0, 9, 0)
    gl.uniform3f(u.uC0, ...c0)
    gl.uniform3f(u.uC1, ...c1)
    gl.uniform3f(u.uC2, ...c2)
    gl.uniform3f(u.uC3, ...c3)
    gl.uniform3f(u.uS0, ...SPECTRUM[0])
    gl.uniform3f(u.uS1, ...SPECTRUM[1])
    gl.uniform3f(u.uS2, ...SPECTRUM[2])
    gl.uniform3f(u.uS3, ...SPECTRUM[3])
    gl.uniform1f(u.uSpectrum, this.palette.spectrum)
    // Вертикальный кадр: лента в нижней трети, с наклоном и частотой «телефонной» раскладки
    gl.uniform4f(u.uLayout, -0.22, -0.34, 0, 1.1)
    gl.uniform2f(u.uFade, 0, 0)
    gl.uniform1f(u.uFreq, 2.0)
    gl.uniform4f(u.uBox, 0, 0, 0, 0)
    gl.uniform1f(u.uBoxR, 0)
    gl.uniform1f(u.uLight, 0)
    gl.drawArrays(gl.TRIANGLES, 0, 3)
  }

  dispose() {
    this.gl.getExtension('WEBGL_lose_context')?.loseContext()
  }
}
