<script setup lang="ts">
import type { WaveExplain, WhyKind } from '~/api/types'

// «Как подобран» — карта вкуса (прототип А, design/why/a.html). Плоскость — карта звучания
// (UMAP по векторам звучания, сервер `reco::map`). Облака — грани вкуса, линия — что играло в
// сессии (точка — дослушано, кружок — пропущено), точки — варианты этого подбора. От последнего
// трека расходится волна, варианты загораются по оценке, отсеянное гаснет, выбранный трек
// вспыхивает, карта чуть приближается к нему, рядом — карточка и причины. ~7 с, потом покой;
// без движения (prefers-reduced-motion) — сразу покой. Цвета — из токенов темы.
const props = withDefaults(defineProps<{
  data: WaveExplain | null
  /** Телефон: меньше подписей, причины строкой внизу */
  compact?: boolean
  /** Видно ли карту: спрятанная не крутит кадры */
  active?: boolean
}>(), { compact: false, active: true })

const bridge = useBridge()
const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })

const T = 7200
const ICONS: Partial<Record<WhyKind, string>> = {
  artist: 'ph:user-sound-fill',
  recent: 'ph:waveform-fill',
  facet: 'ph:circles-three-fill',
  familiar: 'ph:heart-fill',
  new: 'ph:compass-fill',
  moment: 'ph:sun-horizon-fill',
  seed: 'ph:radio-fill',
  release: 'ph:bell-ringing-fill',
  explore: 'ph:shuffle-fill',
  learned: 'ph:check-circle-fill',
}

const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const size = reactive({ w: 0, h: 0 })
/** Момент анимации (мс): подписи появляются по нему */
const now = ref(T)

const clamp = (x: number, a = 0, b = 1) => Math.min(b, Math.max(a, x))
const lerp = (a: number, b: number, k: number) => a + (b - a) * k
const seg = (t: number, a: number, b: number) => clamp((t - a) / (b - a))
const ease = (k: number) => 1 - (1 - k) ** 3
const easeIO = (k: number) => (k < 0.5 ? 4 * k * k * k : 1 - (-2 * k + 2) ** 3 / 2)

type P = [number, number]
interface Cand { id: string, p: P, q: number, out: boolean, top: boolean, color: string | null }

/** Раскладка в пикселях холста: пересчитывается при смене данных и размера */
const scene = computed(() => {
  const d = props.data
  const { w, h } = size
  if (!d || !w || !h) return null
  const pad = props.compact ? 22 : 44
  const bottom = props.compact ? 64 : 0
  const pts: P[] = [[d.x, d.y], ...d.candidates.map(c => [c.x, c.y] as P), ...d.session.map(s => [s.x, s.y] as P)]
  const xs = pts.map(p => p[0])
  const ys = pts.map(p => p[1])
  let [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)]
  // Мало точек — не растягивать соседей на весь экран
  const span = Math.max(x1 - x0, y1 - y0, 0.12)
  const cx = (x0 + x1) / 2
  const cy = (y0 + y1) / 2
  x0 = Math.min(x0, cx - span * 0.3)
  x1 = Math.max(x1, cx + span * 0.3)
  y0 = Math.min(y0, cy - span * 0.3)
  y1 = Math.max(y1, cy + span * 0.3)
  let sx = (w - 2 * pad) / (x1 - x0)
  let sy = (h - 2 * pad - bottom) / (y1 - y0)
  // Пропорции карты — не больше чем в полтора раза: иначе облака сплющиваются
  sx = Math.min(sx, sy * 1.6)
  sy = Math.min(sy, sx * 1.6)
  const ox = (w - (x1 - x0) * sx) / 2
  const oy = (h - bottom - (y1 - y0) * sy) / 2
  const at = (x: number, y: number): P => [ox + (x - x0) * sx, oy + (y - y0) * sy]
  const unit = Math.min(sx, sy)

  const ranked = d.candidates.filter(c => c.score != null)
  const vmax = Math.max(...ranked.map(c => c.score!), 0)
  const vmin = Math.min(...ranked.map(c => c.score!), 0)
  const top = new Set([...ranked].sort((a, b) => b.score! - a.score!).slice(0, 12).map(c => c.trackId))
  const cands: Cand[] = d.candidates.map(c => ({
    id: c.trackId,
    p: at(c.x, c.y),
    q: c.score != null ? ((c.score - vmin) / (vmax - vmin || 1)) ** 2.2 : 0,
    out: c.stage === 'dropped',
    top: c.stage === 'picked' || top.has(c.trackId),
    color: c.facet != null ? `--chart-${(c.facet % 6) + 1}` : null,
  }))
  const dust: P[] = []
  for (let i = 0; i + 1 < d.dust.length; i += 2) dust.push(at(d.dust[i]!, d.dust[i + 1]!))
  const trail = d.session.slice(props.compact ? -5 : -6).map(s => ({ ...s, p: at(s.x, s.y) }))
  const chosen = at(d.x, d.y)
  const origin = trail.length ? trail[trail.length - 1]!.p : chosen
  const maxR = Math.max(60, ...cands.map(c => Math.hypot(c.p[0] - origin[0], c.p[1] - origin[1]))) + 10
  const facets = d.facets.map((f, i) => ({ ...f, i, p: at(f.x, f.y), r: Math.max(18, f.r * unit), color: `--chart-${(i % 6) + 1}` }))
  const rivals = d.rivals.map(r => ({ ...r, p: at(r.x, r.y) }))
  return { w, h, cands, dust, trail, chosen, origin, maxR, facets, rivals, bottom }
})

/** Камера: в конце карта чуть приближается к выбранному и сдвигает его к центру */
function cam(t: number) {
  const s = scene.value!
  const k = easeIO(seg(t, 5600, 6900))
  const zoom = lerp(1, props.compact ? 1.1 : 1.18, k)
  const [px, py] = s.chosen
  const cx = s.w / 2
  const cy = (s.h - s.bottom) / 2
  return { zoom, tx: px * (1 - zoom) + (cx - px) * 0.3 * k, ty: py * (1 - zoom) + (cy - py) * 0.12 * k }
}
const camNow = computed(() => (scene.value ? cam(now.value) : { zoom: 1, tx: 0, ty: 0 }))
const view = (p: P, c = camNow.value): P => [p[0] * c.zoom + c.tx, p[1] * c.zoom + c.ty]

// ---------- Цвета темы ----------
const probe = import.meta.client ? document.createElement('canvas').getContext('2d') : null
/** Цвет токена → «r, g, b» (холст понимает любые записи цвета и отдаёт #rrggbb или rgba(…)) */
function rgb(name: string): string {
  const raw = box.value ? getComputedStyle(box.value).getPropertyValue(name).trim() : ''
  if (!probe || !raw) return '128, 128, 128'
  probe.fillStyle = '#808080'
  probe.fillStyle = raw
  const v = String(probe.fillStyle)
  if (v.startsWith('#')) {
    const n = Number.parseInt(v.slice(1), 16)
    return `${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}`
  }
  const m = v.match(/[\d.]+/g) ?? ['128', '128', '128']
  return `${m[0]}, ${m[1]}, ${m[2]}`
}
let palette: Record<string, string> = {}
let dark = true
function readPalette() {
  palette = Object.fromEntries(['--text-1', '--accent', ...Array.from({ length: 6 }, (_, i) => `--chart-${i + 1}`)].map(n => [n, rgb(n)]))
  dark = document.documentElement.dataset.theme !== 'light'
}

// ---------- Кадр ----------
function draw(t: number) {
  const s = scene.value
  const el = canvas.value
  const g = el?.getContext('2d')
  if (!s || !el || !g) return
  const dpr = Math.min(2, window.devicePixelRatio || 1)
  if (el.width !== Math.round(s.w * dpr)) {
    el.width = Math.round(s.w * dpr)
    el.height = Math.round(s.h * dpr)
  }
  g.setTransform(dpr, 0, 0, dpr, 0, 0)
  g.clearRect(0, 0, s.w, s.h)
  const F = (a: number) => `rgba(${palette['--text-1']}, ${a})`
  const A = (a: number) => `rgba(${palette['--accent']}, ${a})`
  const C = (name: string, a: number) => `rgba(${palette[name]}, ${a})`
  const c = cam(t)
  const V = (p: P) => view(p, c)
  const appear = ease(seg(t, 0, 900))
  const small = props.compact ? 0.8 : 1

  // Каталог вокруг — пыль
  g.fillStyle = F((dark ? 0.14 : 0.16) * appear)
  for (const p of s.dust) {
    const [x, y] = V(p)
    if (x < -4 || y < -4 || x > s.w + 4 || y > s.h + 4) continue
    g.fillRect(x - 0.6, y - 0.6, 1.2, 1.2)
  }
  // Облака граней: пятно у центра грани и мягкие пятна у её вариантов
  g.save()
  g.globalCompositeOperation = dark ? 'lighter' : 'multiply'
  for (const f of s.facets) {
    const [px, py] = V(f.p)
    const r = f.r * c.zoom * 1.4
    const gr = g.createRadialGradient(px, py, 0, px, py, r)
    gr.addColorStop(0, C(f.color, (dark ? 0.2 : 0.12) * appear * (0.6 + f.weight)))
    gr.addColorStop(1, C(f.color, 0))
    g.fillStyle = gr
    g.fillRect(px - r, py - r, r * 2, r * 2)
  }
  const blob = (props.compact ? 18 : 28) * c.zoom
  for (const x of s.cands) {
    if (!x.color) continue
    const [px, py] = V(x.p)
    const gr = g.createRadialGradient(px, py, 0, px, py, blob)
    gr.addColorStop(0, C(x.color, (dark ? 0.08 : 0.08) * appear))
    gr.addColorStop(1, C(x.color, 0))
    g.fillStyle = gr
    g.fillRect(px - blob, py - blob, blob * 2, blob * 2)
  }
  g.restore()

  // Сессия: линия того, что играло, проявляется по порядку
  const n = s.trail.length
  if (n) {
    const shown = seg(t, 100, 1000) * (n - 1)
    g.lineWidth = 1.25
    g.strokeStyle = F(0.22)
    g.beginPath()
    for (let i = 0; i <= Math.min(n - 1, Math.ceil(shown)); i++) {
      let [x, y] = V(s.trail[i]!.p)
      if (i > shown) {
        const [ax, ay] = V(s.trail[i - 1]!.p)
        const k = shown - (i - 1)
        x = lerp(ax, x, k)
        y = lerp(ay, y, k)
      }
      if (i === 0) g.moveTo(x, y)
      else g.lineTo(x, y)
    }
    g.stroke()
    s.trail.forEach((st, i) => {
      if (i > shown + 0.01) return
      const [x, y] = V(st.p)
      g.beginPath()
      g.arc(x, y, (i === n - 1 ? 4.5 : 3.5) * small, 0, 7)
      if (st.outcome === 'skip' || st.outcome === 'dislike' || st.outcome === 'not_now') {
        g.strokeStyle = F(0.55)
        g.lineWidth = 1.2
        g.stroke()
      }
      else {
        g.fillStyle = st.outcome === 'like' || st.outcome === 'more_like_this' ? A(0.9) : F(0.7)
        g.fill()
      }
    })
  }

  // Волна поиска от последнего трека
  const O = V(s.origin)
  const wave = seg(t, 900, 3300)
  const R = s.maxR * ease(wave) * c.zoom
  if (wave > 0 && wave < 1) {
    g.lineWidth = 1.5
    g.strokeStyle = A(0.35 * (1 - wave))
    g.beginPath()
    g.arc(O[0], O[1], R, 0, 7)
    g.stroke()
    g.strokeStyle = A(0.15 * (1 - wave))
    g.beginPath()
    g.arc(O[0], O[1], R * 0.82, 0, 7)
    g.stroke()
  }

  // Варианты: загораются, когда до них доходит волна; отсеянное гаснет; лучшие — ярче
  const sort = seg(t, 3300, 4600)
  const narrow = seg(t, 4600, 5600)
  for (const x of s.cands) {
    const [px, py] = V(x.p)
    const lit = t >= 3300 ? 1 : clamp((R - Math.hypot(px - O[0], py - O[1])) / 60)
    if (lit <= 0) continue
    let a = 0.42 * lit
    let r = 1.8
    if (x.out) {
      a = lerp(a, 0.1, sort)
    }
    else {
      a = lerp(a, 0.25 + 0.65 * x.q, sort)
      r = lerp(r, 1.6 + 3.2 * x.q, sort)
      a = x.top ? lerp(a, 0.55, narrow) : lerp(a, 0.18 + 0.3 * x.q, narrow)
    }
    g.beginPath()
    g.arc(px, py, r * Math.sqrt(c.zoom) * small, 0, 7)
    g.fillStyle = x.top && narrow > 0 ? A(a * 0.75) : F(a)
    g.fill()
    if (x.out && sort > 0.5) {
      // Отсеянное: крестик едва заметен
      g.strokeStyle = F(0.14 * (sort - 0.5) * 2)
      g.lineWidth = 1
      g.beginPath()
      g.moveTo(px - 2, py - 2)
      g.lineTo(px + 2, py + 2)
      g.moveTo(px + 2, py - 2)
      g.lineTo(px - 2, py + 2)
      g.stroke()
    }
  }

  // Соперники и выбор: тонкие линии от последнего трека
  const P0 = V(s.chosen)
  if (narrow > 0) {
    g.setLineDash([3, 4])
    g.lineWidth = 1
    for (const r of s.rivals) {
      const [rx, ry] = V(r.p)
      g.strokeStyle = F(0.22 * narrow * (1 - 0.6 * seg(t, 5600, 6400)))
      g.beginPath()
      g.moveTo(O[0], O[1])
      g.lineTo(rx, ry)
      g.stroke()
      g.beginPath()
      g.arc(rx, ry, 6 * small, 0, 7)
      g.strokeStyle = F(0.45 * narrow)
      g.stroke()
    }
    g.setLineDash([])
    if (n) {
      g.strokeStyle = A(0.7 * narrow)
      g.lineWidth = 1.5
      g.beginPath()
      g.moveTo(O[0], O[1])
      g.quadraticCurveTo((O[0] + P0[0]) / 2 + (P0[1] - O[1]) * 0.2, (O[1] + P0[1]) / 2 - (P0[0] - O[0]) * 0.2, P0[0], P0[1])
      g.stroke()
    }
  }
  // Вспышка выбора
  const flash = seg(t, 5300, 6300)
  if (flash > 0 && flash < 1) {
    for (const k of [0, 0.25]) {
      const f = clamp(flash - k)
      g.strokeStyle = A(0.6 * (1 - f))
      g.lineWidth = 2
      g.beginPath()
      g.arc(P0[0], P0[1], 8 + 60 * ease(f), 0, 7)
      g.stroke()
    }
  }
  if (t >= 3300) {
    const k = seg(t, 5300, 5600)
    g.beginPath()
    g.arc(P0[0], P0[1], lerp(3.5, 7, k) * small, 0, 7)
    g.fillStyle = A(lerp(0.7, 1, k))
    g.fill()
    if (k > 0) {
      g.beginPath()
      g.arc(P0[0], P0[1], 12 * small, 0, 7)
      g.strokeStyle = A(0.35 * k)
      g.lineWidth = 1
      g.stroke()
    }
  }
}

// ---------- Подписи поверх холста ----------
const chosenFacet = computed(() => props.data?.facet ?? null)
const facetLabels = computed(() => {
  const s = scene.value
  if (!s) return []
  const list = props.compact ? s.facets.filter(f => f.i < 2 || f.i === chosenFacet.value) : s.facets
  return list.map((f) => {
    const [x, y] = view(f.p)
    // Подпись целиком в рамке (с запасом на мягкий край), иначе не показывать обрезанной
    const half = Math.min(110, f.label.length * 3.3 + 3) + s.w * 0.04
    const inside = x - half > 0 && x + half < s.w && y > 14 && y < s.h - s.bottom - 10
    return { id: f.id, label: f.label, x, y, on: now.value >= 500 && inside, quiet: now.value > 6000 && f.i !== chosenFacet.value }
  })
})
const lastLabel = computed(() => {
  const s = scene.value
  const last = s?.trail[s.trail.length - 1]
  if (!s || !last) return null
  const [x, y] = view(last.p)
  return { text: `«${last.title}»`, x, y: y + 16, on: now.value >= 700 && now.value < 5900 }
})
const rivalLabels = computed(() => {
  const s = scene.value
  if (!s || props.compact) return []
  return s.rivals.map((r) => {
    const [x, y] = view(r.p)
    const edge = x < 70 || x > s.w - 70 || y < 30 || y > s.h - 30
    return { id: r.trackId, text: r.title, x, y: y - 15, on: now.value >= 4800 && !edge, quiet: now.value > 6300 }
  })
})
/** Карточка выбранного — сбоку от точки, где больше места; на телефоне — под точкой */
const card = computed(() => {
  const s = scene.value
  if (!s) return null
  const [x, y] = view(s.chosen)
  const right = x < s.w * 0.6
  if (props.compact) return { x: clamp(x, 120, s.w - 120), y: clamp(y + 38, 30, s.h - s.bottom - 30), right: null, on: now.value >= 5900 }
  return { x: x + (right ? 26 : -26), y: clamp(y, 30, s.h - 150), right, on: now.value >= 5900 }
})
const cover = computed(() => {
  const img = props.data?.track.album?.image
  return img?.small ? bridge.resolveUrl(img.small) : null
})
const reasons = computed(() => (props.data?.reasons ?? []).slice(0, props.compact ? 3 : 4).map((r, i) => ({ ...r, icon: ICONS[r.kind] ?? 'ph:sparkle-fill', on: now.value >= 6300 + i * 180 })))

/** Подписи не налезают друг на друга и на карточку с причинами: прячется та, что ниже в списке
 *  (грани — по весу, затем последний трек, затем соперники) */
type Box = [number, number, number, number]
const hits = (a: Box, b: Box) => a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
const hidden = computed(() => {
  const out = new Set<string>()
  // Подпись внизу слева (дослушано / пропущено / варианты) — тоже занятое место
  const taken: Box[] = props.compact ? [] : [[size.w * 0.08 - 4, size.h - 32, size.w * 0.08 + 420, size.h]]
  const c = card.value
  if (c?.on) {
    const n = reasons.value.length
    if (props.compact) {
      taken.push([c.x - 130, c.y - 28, c.x + 130, c.y + 28])
      taken.push([0, size.h - 14 - n * 34, size.w, size.h])
    }
    else {
      const [l, r] = c.right ? [c.x - 4, c.x + 340] : [c.x - 340, c.x + 4]
      taken.push([l, c.y - 28, r, c.y + 46 + n * 36])
    }
  }
  const box = (text: string, x: number, y: number): Box => {
    const w = Math.min(220, text.length * 6.6 + 6)
    return [x - w / 2, y - 9, x + w / 2, y + 9]
  }
  const consider = (key: string, text: string, x: number, y: number, on: boolean) => {
    if (!on) return
    const b = box(text, x, y)
    if (taken.some(t => hits(t, b))) out.add(key)
    else taken.push(b)
  }
  for (const f of facetLabels.value) consider(`f${f.id}`, f.label, f.x, f.y - (props.compact ? 0 : 6), f.on)
  const l = lastLabel.value
  if (l) consider('last', l.text, l.x, l.y, l.on)
  for (const r of rivalLabels.value) consider(`r${r.id}`, r.text, r.x, r.y, r.on)
  return out
})

// ---------- Время ----------
let raf = 0
let start = 0
function frame(t: number) {
  now.value = t
  draw(t)
}
function loop(ts: number) {
  const t = ts - start
  frame(Math.min(t, T))
  if (t < T && props.active) raf = requestAnimationFrame(loop)
}
function play() {
  cancelAnimationFrame(raf)
  if (!scene.value) return
  readPalette()
  if (ui.reducedMotion || !props.active) return frame(T)
  start = performance.now()
  raf = requestAnimationFrame(loop)
}

// Новый трек — заново; смена размера или темы в покое — перерисовать кадр
watch(() => props.data?.track.id, () => void nextTick(play))
watch(() => props.active, v => (v ? void nextTick(play) : cancelAnimationFrame(raf)))
watch(() => [size.w, size.h, props.compact], () => {
  if (now.value >= T || ui.reducedMotion) void nextTick(() => {
    readPalette()
    draw(T)
  })
})
let themeObs: MutationObserver | null = null
let resizeObs: ResizeObserver | null = null
onMounted(() => {
  resizeObs = new ResizeObserver(([e]) => {
    size.w = Math.round(e!.contentRect.width)
    size.h = Math.round(e!.contentRect.height)
  })
  if (box.value) resizeObs.observe(box.value)
  themeObs = new MutationObserver(() => {
    readPalette()
    if (now.value >= T) draw(T)
  })
  themeObs.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'data-mood'] })
  void nextTick(play)
})
// Размер стал известен уже после данных — запустить, если ещё не шли
watch(scene, (s, old) => {
  if (s && !old) void nextTick(play)
})
onBeforeUnmount(() => {
  cancelAnimationFrame(raf)
  themeObs?.disconnect()
  resizeObs?.disconnect()
})

const settled = computed(() => now.value >= T)
</script>

<template>
  <div ref="box" class="tm" :class="{ 'tm--compact': compact }" role="img" :aria-label="t('aria')">
    <canvas ref="canvas" class="tm__canvas" aria-hidden="true" />
    <div v-if="!data" class="tm__wait" aria-hidden="true"><span /><span /><span /></div>
    <div v-else class="tm__ov" aria-hidden="true">
      <span v-for="f in facetLabels" :key="f.id" class="tm__lbl tm__lbl--facet" :class="{ on: f.on && !hidden.has(`f${f.id}`), quiet: f.quiet }" :style="{ left: `${f.x}px`, top: `${f.y - (compact ? 0 : 6)}px` }">{{ f.label }}</span>
      <span v-if="lastLabel" class="tm__lbl" :class="{ on: lastLabel.on && !hidden.has('last') }" :style="{ left: `${lastLabel.x}px`, top: `${lastLabel.y}px` }">{{ lastLabel.text }}</span>
      <span v-for="r in rivalLabels" :key="r.id" class="tm__lbl" :class="{ on: r.on && !hidden.has(`r${r.id}`), quiet: r.quiet }" :style="{ left: `${r.x}px`, top: `${r.y}px` }">{{ r.text }}</span>
      <div
        v-if="card"
        class="tm__card"
        :class="{ on: card.on, 'is-left': card.right === false, 'is-right': card.right === true }"
        :style="{ left: `${card.x}px`, top: `${card.y}px` }"
      >
        <img v-if="cover" :src="cover" alt="">
        <div class="tm__card-text">
          <b class="ellipsis">{{ data.track.title }}</b>
          <span class="ellipsis">{{ data.track.artists.map(a => a.name).join(', ') }}</span>
        </div>
      </div>
      <div v-if="card" class="tm__reasons" :class="{ 'is-left': card.right === false }" :style="compact ? undefined : { left: `${card.x}px`, top: `${card.y + 46}px` }">
        <span v-for="(r, i) in reasons" :key="i" class="tm__chip" :class="{ on: r.on }"><Icon :name="r.icon" />{{ r.text }}</span>
      </div>
      <div v-if="!compact" class="tm__legend" :class="{ on: now >= 800 }">
        <span><i class="tm__dot" />{{ t('done') }}</span>
        <span><i class="tm__dot tm__dot--ring" />{{ t('skipped') }}</span>
        <span><i class="tm__dot tm__dot--small" />{{ t('options') }}</span>
      </div>
    </div>
    <span class="sr-only">{{ data ? [data.track.title, ...data.reasons.map(r => r.text)].join('. ') : '' }}</span>
    <button v-if="data && settled && !ui.reducedMotion" type="button" class="tm__replay" :aria-label="t('replay')" @click="play">
      <Icon name="ph:arrow-counter-clockwise" />
    </button>
  </div>
</template>

<style scoped>
.tm { position: relative; width: 100%; height: 100%; overflow: hidden; }
.tm__canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  /* Карта уходит за край мягко, без рамки */
  -webkit-mask-image: linear-gradient(to right, transparent, #000 7%, #000 93%, transparent), linear-gradient(to bottom, transparent, #000 8%, #000 92%, transparent);
  -webkit-mask-composite: source-in;
  mask-image: linear-gradient(to right, transparent, #000 7%, #000 93%, transparent), linear-gradient(to bottom, transparent, #000 8%, #000 92%, transparent);
  mask-composite: intersect;
}
.tm__ov { position: absolute; inset: 0; pointer-events: none; }
.tm__lbl {
  position: absolute;
  max-width: 220px;
  overflow: hidden;
  font-size: var(--fs-caption);
  color: var(--text-2);
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0;
  transform: translate(-50%, -50%);
  transition: opacity 0.5s var(--ease-out);
}
.tm__lbl--facet { font-weight: 500; color: var(--text-3); }
.tm__lbl.on { opacity: 1; }
.tm__lbl.on.quiet { opacity: 0.5; }
.tm__card {
  position: absolute;
  display: flex;
  align-items: center;
  gap: 10px;
  max-width: 280px;
  padding: 6px 12px 6px 6px;
  border-radius: var(--r-3);
  background: var(--ink-3);
  box-shadow: var(--shadow-cover), inset 0 0 0 1px var(--line);
  opacity: 0;
  transform: translate(-50%, -50%);
  transition: opacity 0.5s var(--ease-out);
}
.tm__card.is-right { transform: translate(0, -50%); }
.tm__card.is-left { transform: translate(-100%, -50%); }
.tm__card.on { opacity: 1; }
.tm__card img { width: 36px; height: 36px; flex: none; border-radius: 5px; object-fit: cover; }
.tm__card-text { display: flex; flex-direction: column; min-width: 0; }
.tm__card b { font-size: var(--fs-small); font-weight: 600; color: var(--text-1); }
.tm__card span { font-size: var(--fs-caption); color: var(--text-3); }
.tm__reasons { position: absolute; display: flex; flex-direction: column; align-items: flex-start; gap: 8px; }
.tm__reasons.is-left { align-items: flex-end; transform: translateX(-100%); }
.tm--compact .tm__reasons { left: 12px; right: 12px; bottom: 10px; align-items: center; gap: 6px; }
.tm__chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  max-width: 100%;
  padding: 0 11px;
  overflow: hidden;
  border-radius: var(--r-pill);
  background: var(--ink-3);
  box-shadow: inset 0 0 0 1px var(--line);
  font-size: var(--fs-caption);
  font-weight: 500;
  color: var(--text-1);
  white-space: nowrap;
  opacity: 0;
  transform: translateY(4px);
  transition: opacity 0.5s var(--ease-out), transform 0.5s var(--ease-out);
}
.tm__chip.on { opacity: 1; transform: none; }
.tm__chip :deep(svg) { flex: none; font-size: 14px; color: var(--accent); }
.tm__legend { position: absolute; left: 8%; bottom: 10px; display: flex; gap: 14px; font-size: var(--fs-caption); color: var(--text-3); opacity: 0; transition: opacity 0.5s var(--ease-out); }
.tm__legend.on { opacity: 1; }
.tm__legend span { display: inline-flex; align-items: center; gap: 6px; }
.tm__dot { width: 7px; height: 7px; border-radius: 50%; background: var(--text-2); }
.tm__dot--ring { background: none; box-shadow: inset 0 0 0 1.2px var(--text-2); }
.tm__dot--small { width: 3px; height: 3px; }
.tm__replay {
  position: absolute;
  right: 6px;
  bottom: 6px;
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: none;
  color: var(--text-3);
  font-size: 16px;
  cursor: pointer;
  transition: color var(--dur-2) var(--ease-out), background var(--dur-2) var(--ease-out);
}
.tm__replay:hover { background: var(--fill-2); color: var(--text-1); }
.tm--compact .tm__replay { top: 0; bottom: auto; width: 44px; height: 44px; }
/* Пока карта грузится — тихие точки, без «загрузки» словами */
.tm__wait { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; gap: 8px; }
.tm__wait span { width: 6px; height: 6px; border-radius: 50%; background: var(--text-3); animation: tm-wait 1.2s var(--ease-out) infinite; }
.tm__wait span:nth-child(2) { animation-delay: 0.15s; }
.tm__wait span:nth-child(3) { animation-delay: 0.3s; }
@keyframes tm-wait { 0%, 100% { opacity: 0.2; } 50% { opacity: 0.8; } }
@media (prefers-reduced-motion: reduce) {
  .tm__wait span { animation: none; opacity: 0.5; }
  .tm__lbl, .tm__card, .tm__chip, .tm__legend { transition: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "aria": "Как подобран трек: карта вкуса",
    "done": "дослушано",
    "skipped": "пропущено",
    "options": "варианты волны",
    "replay": "Показать ещё раз"
  },
  "en": {
    "aria": "How this track was picked: taste map",
    "done": "played through",
    "skipped": "skipped",
    "options": "wave options",
    "replay": "Show again"
  }
}
</i18n>
