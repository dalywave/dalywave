// Волна DalyWave, направление C «Сцена».
// Свои шейдеры WebGL без библиотек: ленты из тонких линий света, как след осциллографа
// или лучи прожекторов в дымке. Одно состояние (настроение, энергия, доля, реакции)
// на всё приложение и сколько угодно «видов» — холстов, которые его рисуют.
(() => {
  'use strict';

  // ---------- Цвет ----------
  // Работаем в OKLab: так цвет обложки можно привести к нужной светлоте,
  // не теряя оттенка, а смешивание палитр не «грязнит» середину.
  const toLin = (c) => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
  const toSrgb = (c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055);
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const lerp = (a, b, t) => a + (b - a) * t;

  function hexToRgb(hex) {
    const h = hex.replace('#', '');
    const n = parseInt(h.length === 3 ? h.split('').map((x) => x + x).join('') : h, 16);
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
  }
  function rgbToHex(rgb) {
    return '#' + rgb.map((v) => Math.round(clamp(v) * 255).toString(16).padStart(2, '0')).join('');
  }
  function rgbToOklab([r, g, b]) {
    r = toLin(r); g = toLin(g); b = toLin(b);
    const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    return [
      0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
      1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
      0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
    ];
  }
  function oklabToRgbRaw([L, a, b]) {
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
    const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
    const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    return [
      4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
      -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
      -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
    ];
  }
  // OKLCH → sRGB с уменьшением насыщенности, пока цвет не влезет в охват
  function oklchToRgb(L, C, h) {
    for (let c = C; c >= 0; c -= 0.005) {
      const lin = oklabToRgbRaw([L, c * Math.cos(h), c * Math.sin(h)]);
      if (lin.every((v) => v >= -0.0005 && v <= 1.0005)) return lin.map((v) => toSrgb(clamp(v)));
    }
    return oklabToRgbRaw([L, 0, 0]).map((v) => toSrgb(clamp(v)));
  }
  function toLch(rgb) {
    const [L, a, b] = rgbToOklab(rgb);
    return { L, C: Math.hypot(a, b), h: Math.atan2(b, a) };
  }
  // «Гель» — цвет света, снятый с обложки: та же гамма, но светлота и насыщенность
  // приведены к рабочим значениям, чтобы читаться на чёрном и не кричать.
  function gel(rgb, L = 0.78, minC = 0.09, maxC = 0.17) {
    const { C, h } = toLch(rgb);
    return oklchToRgb(L, clamp(C, minC, maxC), h);
  }

  // Средние «живые» цвета картинки: корзины по оттенку, вес — насыщенность.
  const sampleCache = new Map();
  function sample(url) {
    if (!url) return Promise.resolve(null);
    if (sampleCache.has(url)) return sampleCache.get(url);
    const p = new Promise((resolve) => {
      const img = new Image();
      img.decoding = 'async';
      img.onload = () => {
        try {
          const S = 28;
          const cv = document.createElement('canvas');
          cv.width = cv.height = S;
          const cx = cv.getContext('2d', { willReadFrequently: true });
          cx.drawImage(img, 0, 0, S, S);
          const data = cx.getImageData(0, 0, S, S).data;
          const bins = Array.from({ length: 18 }, () => ({ w: 0, r: 0, g: 0, b: 0 }));
          let ar = 0, ag = 0, ab = 0, chroma = 0;
          for (let i = 0; i < data.length; i += 4) {
            const r = data[i] / 255, g = data[i + 1] / 255, b = data[i + 2] / 255;
            ar += r; ag += g; ab += b;
            const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn, l = (mx + mn) / 2;
            chroma += d;
            if (d < 0.1 || l < 0.08 || l > 0.96) continue;
            let h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
            h = (h * 60 + 360) % 360;
            const sat = d / (1 - Math.abs(2 * l - 1) + 1e-6);
            const w = sat * sat * (0.35 + l);
            const bin = bins[Math.floor(h / 20) % 18];
            bin.w += w; bin.r += r * w; bin.g += g * w; bin.b += b * w;
          }
          const n = data.length / 4;
          const vivid = bins.filter((x) => x.w > 0.4).sort((x, y) => y.w - x.w).slice(0, 3)
            .map((x) => [x.r / x.w, x.g / x.w, x.b / x.w]);
          resolve({ vivid, avg: [ar / n, ag / n, ab / n], mono: chroma / n < 0.07 || !vivid.length });
        } catch {
          resolve(null);
        }
      };
      img.onerror = () => resolve(null);
      img.src = url;
    });
    sampleCache.set(url, p);
    return p;
  }

  // ---------- Настроения ----------
  // Спокойное — глубокие холодные; бодрое — тёплые и яркие; весёлое — гиперпоп;
  // грустное — приглушённые сине-сиреневые.
  const MOODS = {
    energetic: { name: 'Бодрое', c: ['#FF7A1A', '#FF3B5C', '#FFC53D'], beam: '#FFB061', bg: '#100503' },
    cheerful: { name: 'Весёлое', c: ['#FF4FCB', '#FFD94A', '#3FE6FF'], beam: '#FF9DE4', bg: '#0E040D' },
    calm: { name: 'Спокойное', c: ['#356BFF', '#2CD3F0', '#7A5CFF'], beam: '#5D9CFF', bg: '#02050C' },
    sad: { name: 'Грустное', c: ['#5F7DBA', '#9785C8', '#3E5E8E'], beam: '#6F84B0', bg: '#03040A' },
  };
  const MOOD_ORDER = ['energetic', 'cheerful', 'calm', 'sad'];

  // ---------- Шейдеры ----------
  const VERT = `
attribute vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`;

  const FRAG = `
precision highp float;
uniform vec2  uRes;
uniform float uTime;
uniform float uPulse;
uniform float uEnergy;
uniform vec3  uBands;
uniform float uExpand;
uniform float uFlash;
uniform vec2  uRipple;
uniform vec3  uC0;
uniform vec3  uC1;
uniform vec3  uC2;
uniform vec3  uBeam;
uniform vec3  uBg;
uniform vec4  uShape;   // x: волн по ширине, y: размах, z: центр по высоте, w: полуширина ленты
uniform vec4  uLook;    // x: линий в ленте, y: сила лучей, z: зерно, w: размер пикселя холста
uniform vec4  uEnv;     // x,y: где лентам ярче (центр и ширина по x), z: px холста на css px, w: смещение лучей
uniform float uRibbons; // 2 или 3 ленты

const float TAU = 6.2831853;

float hash12(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}
float vnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash12(i), hash12(i + vec2(1.0, 0.0)), u.x),
             mix(hash12(i + vec2(0.0, 1.0)), hash12(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p) {
  float s = 0.0;
  float a = 0.5;
  for (int i = 0; i < 4; i++) { s += a * vnoise(p); p = p * 2.03 + vec2(17.1, 9.2); a *= 0.5; }
  return s;
}

// Геометрия ленты: центр c и его наклон dc; полуширина w со знаком (лента перекручивается) и dw.
void geom(float xn, float id, float aspect, out float c, out float dc, out float w, out float dw) {
  float k = TAU * uShape.x;
  float t = uTime;
  float ph = id * 2.17;
  float amp = uShape.y * uExpand * (0.42 + 0.78 * uEnergy) * (1.0 + 0.2 * uPulse);
  float f1 = k;          float p1 = t * 0.31 + ph;
  float f2 = k * 1.87;   float p2 = -t * 0.47 + ph * 1.63;
  float f3 = k * 3.41;   float p3 = t * 0.83 + ph * 0.71;
  float a3 = 0.08 + 0.12 * uBands.z;
  c  = uShape.z + (id - 1.0) * 0.05 * uExpand
     + amp * (0.62 * sin(xn * f1 + p1) + 0.28 * sin(xn * f2 + p2) + a3 * sin(xn * f3 + p3));
  dc = amp * (0.62 * f1 * cos(xn * f1 + p1) + 0.28 * f2 * cos(xn * f2 + p2) + a3 * f3 * cos(xn * f3 + p3));

  // Пропуск: по лентам бежит рябь слева направо
  if (uRipple.x >= 0.0) {
    float front = mix(-0.8, 0.8, uRipple.x) - id * 0.06;
    float d = xn - front;
    float env = exp(-d * d * 26.0) * (1.0 - uRipple.x) * uRipple.y;
    c  += env * 0.075 * sin(d * 36.0);
    dc += env * 0.075 * 36.0 * cos(d * 36.0);
  }

  float kw = k * 0.74;
  float pw = -t * 0.37 + ph * 1.31;
  float W = uShape.w * (0.3 + 0.7 * uExpand) * (0.6 + 0.55 * uEnergy);
  w  = W * sin(xn * kw + pw);
  dw = W * kw * cos(xn * kw + pw);
  dc /= aspect;
  dw /= aspect;
}

vec3 ribbon(vec2 p, float xn, float aspect, float id, vec3 ca, vec3 cb, float N, float gain) {
  float c; float dc; float w; float dw;
  geom(xn, id, aspect, c, dc, w, dw);
  float px = uLook.w;
  float s = p.y - c;
  float aw = max(abs(w), 1e-5);
  float sg = w < 0.0 ? -1.0 : 1.0;
  float u = s / aw * sg;
  float f = (clamp(u, -1.0, 1.0) * 0.5 + 0.5) * (N - 1.0);
  float kk = floor(f + 0.5);
  float uk = kk / (N - 1.0) * 2.0 - 1.0;
  float sk = s - w * uk;
  float slope = dc + dw * uk;
  float nrm = inversesqrt(1.0 + slope * slope);
  float spx = sk * nrm / px;
  float dpx = abs(spx);
  float gap = aw * 2.0 / (N - 1.0) * nrm / px;

  float lw = (0.62 + 0.45 * uFlash + 0.18 * uPulse) * uEnv.z;
  float sh = (0.45 + 1.4 * uFlash + 0.35 * uPulse) * uEnv.z;
  vec3 dd = vec3(spx - sh, spx, spx + sh);
  vec3 core = exp(-(dd * dd) / (lw * lw));
  float halo = exp(-dpx / ((2.2 + 4.5 * uEnergy) * uEnv.z)) * 0.2;

  float var = 0.5 + 0.5 * sin(kk * 2.399 + id * 4.1 + uTime * 0.37);
  var = 0.3 + 0.7 * var * var;
  float edge = 1.0 - 0.6 * abs(uk);
  float travel = 0.72 + 0.28 * sin(xn * 8.0 - uTime * 1.5 + kk * 0.61 + id * 1.3);
  vec3 lines = (core + halo) * var * edge * travel;

  // Где линии сходятся ближе пикселя — ровная заливка той же средней яркости
  float merge = smoothstep(1.0, 3.2, gap);
  float inside = 1.0 - smoothstep(0.92, 1.08, abs(u));
  float fillAmt = clamp(1.6 * lw / max(gap, 0.25), 0.0, 1.6) * 0.42 * inside;
  lines = mix(vec3(fillAmt), lines, merge);

  // Свет пучка в дымке и узел перекрута
  float body = exp(-(s * s) / (aw * aw * 1.3 + 0.0004)) * 0.055;
  float node = exp(-aw / px * 0.2) * exp(-abs(s) / px * 0.07) * 0.55;

  vec3 col = mix(ca, cb, 0.5 + 0.5 * uk);
  return (col * lines + mix(ca, cb, 0.5) * (body + node)) * gain;
}

void main() {
  vec2 p = (gl_FragCoord.xy - 0.5 * uRes) / uRes.y;
  float aspect = uRes.x / uRes.y;
  float xn = p.x / aspect;

  // Сцена: тёмный цвет настроения и виньетка
  vec2 q = p * vec2(0.9 / max(aspect, 1.0), 1.15);
  vec3 col = uBg * max(0.0, 1.0 - 1.1 * dot(q, q));

  // Лучи прожекторов сверху, видны в дымке
  if (uLook.y > 0.001) {
    float haze = 0.2 + 1.1 * fbm(p * vec2(2.0, 1.3) + vec2(uTime * 0.025, -uTime * 0.04));
    vec3 beams = vec3(0.0);
    for (int i = 0; i < 3; i++) {
      float fi = float(i);
      vec2 apex = vec2((-0.36 + 0.36 * fi + uEnv.w) * aspect, 0.66);
      float ang = (0.26 - 0.26 * fi) + 0.08 * sin(uTime * 0.16 + fi * 2.1);
      vec2 dir = vec2(sin(ang), -cos(ang));
      vec2 v = p - apex;
      float len = length(v);
      float ca = dot(v, dir) / max(len, 1e-4);
      float a = acos(clamp(ca, -1.0, 1.0));
      float wd = 0.085 + 0.03 * fi;
      float cone = exp(-(a * a) / (wd * wd));
      float fall = exp(-len * 1.35) * smoothstep(-0.75, 0.25, p.y);
      vec3 bc = fi < 0.5 ? uC0 : (fi < 1.5 ? uBeam : uC2);
      beams += bc * cone * fall;
    }
    col += beams * haze * uLook.y * (0.085 + 0.08 * uEnergy + 0.09 * uPulse);
  }

  float ex = (xn - uEnv.x) / uEnv.y;
  float env = exp(-ex * ex);
  float N = uLook.x;
  vec3 rib = ribbon(p, xn, aspect, 0.0, uC0, uC1, N, 1.0);
  rib += ribbon(p, xn, aspect, 1.0, uC1, uC2, N, 0.8);
  if (uRibbons > 2.5) rib += ribbon(p, xn, aspect, 2.0, uC2, uC0, N, 0.62);
  col += rib * (0.4 + 0.6 * env) * (0.82 + 0.36 * uEnergy) * (1.0 + 0.55 * uFlash);

  // Лайк: короткая общая вспышка
  col += mix(uC0, vec3(1.0), 0.45) * uFlash * 0.05;

  col = 1.0 - exp(-col * 1.35);
  col = pow(col, vec3(0.4545));
  col += (hash12(gl_FragCoord.xy + fract(uTime * 7.0) * 113.0) - 0.5) * uLook.z;
  gl_FragColor = vec4(col, 1.0);
}`;

  // Пресеты видов: где лента, сколько линий, есть ли лучи
  const PRESETS = {
    hero: { cycles: 1.35, amp: 0.2, centerY: 0.5, width: 0.1, lines: 16, beams: 1, envX: 0.2, envW: 0.55, ribbons: 3, beamShift: 0.08 },
    heroPhone: { cycles: 0.9, amp: 0.12, centerY: 0.36, width: 0.075, lines: 13, beams: 0.9, envX: 0, envW: 0.9, ribbons: 3, beamShift: 0 },
    stage: { cycles: 1.05, amp: 0.15, centerY: 0.34, width: 0.08, lines: 16, beams: 1, envX: 0, envW: 0.7, ribbons: 3, beamShift: 0 },
    player: { cycles: 0.85, amp: 0.1, centerY: 0.31, width: 0.06, lines: 12, beams: 0.85, envX: 0, envW: 0.9, ribbons: 3, beamShift: 0 },
    full: { cycles: 1.25, amp: 0.2, centerY: 0.5, width: 0.1, lines: 18, beams: 1, envX: 0, envW: 0.8, ribbons: 3, beamShift: 0 },
    card: { cycles: 0.9, amp: 0.2, centerY: 0.52, width: 0.12, lines: 11, beams: 0.6, envX: 0, envW: 1, ribbons: 2, beamShift: 0 },
  };

  // ---------- Общее состояние ----------
  const now = () => performance.now();
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');

  const S = {
    mood: 'calm',
    bpm: 120,
    beatT0: now(),
    beatIndex: -1,
    energyManual: 0.55,
    energyAuto: true,
    energy: 0.32,
    bands: [0.2, 0.2, 0.2],
    playing: false,
    expand: 0.68,
    expandVel: 0,
    expandTarget: 0.68,
    pulse: 0,
    onset: 0,
    flash: 0,
    rippleT0: -1e9,
    time: 9.5,
    cover: null,
    coverBlend: 0.24,
    fps: 0,
    pal: null,
    palFrom: null,
    palTo: null,
    palT0: 0,
    palDur: 1200,
  };

  function moodPalette(key, cover) {
    const m = MOODS[key] || MOODS.calm;
    let cs = m.c.map(hexToRgb);
    let beam = hexToRgb(m.beam);
    const bg = hexToRgb(m.bg);
    if (cover && cover.vivid && cover.vivid.length && !cover.mono) {
      // Обложка подмешивается, но настроение главное: берём её оттенки на светлоте настроения
      cs = cs.map((c, i) => {
        const src = cover.vivid[i % cover.vivid.length];
        const a = rgbToOklab(c), b = rgbToOklab(gel(src, toLch(c).L, 0.08, 0.2));
        const t = S.coverBlend;
        return oklabToRgbRaw([lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]).map((v) => toSrgb(clamp(v)));
      });
    }
    return { c: cs, beam, bg };
  }
  // В шейдер — линейные значения: свет складывается физично
  const linPal = (pal) => ({ c: pal.c.map((c) => c.map(toLin)), beam: pal.beam.map(toLin), bg: pal.bg.map(toLin) });
  function mixPal(a, b, t) {
    const m = (x, y) => x.map((v, i) => lerp(v, y[i], t));
    return { c: a.c.map((c, i) => m(c, b.c[i])), beam: m(a.beam, b.beam), bg: m(a.bg, b.bg) };
  }
  S.pal = linPal(moodPalette(S.mood, null));

  function retargetPalette(instant) {
    const target = linPal(moodPalette(S.mood, S.cover));
    if (instant || reduced.matches) {
      S.pal = target; S.palTo = null;
      redrawStatic();
      return;
    }
    S.palFrom = S.pal; S.palTo = target; S.palT0 = now();
  }

  // ---------- События ----------
  const listeners = {};
  function on(name, fn) { (listeners[name] ||= new Set()).add(fn); return () => listeners[name].delete(fn); }
  function emit(name, data) { listeners[name]?.forEach((fn) => { try { fn(data); } catch (e) { console.error(e); } }); }

  // ---------- Звук: превью и анализ ----------
  const A = { el: null, ctx: null, an: null, freq: null, prevLow: 0, flux: [], lastOnset: 0, onsets: [], peak: 0.25, url: null, active: false };

  function ensureAudio() {
    if (A.el) return;
    A.el = new Audio();
    A.el.preload = 'auto';
    A.el.volume = A.volume ?? 0.8;
    A.el.addEventListener('ended', () => { A.active = false; emit('audio', { type: 'ended', url: A.url }); });
    A.el.addEventListener('pause', () => emit('audio', { type: 'pause', url: A.url }));
    A.el.addEventListener('play', () => emit('audio', { type: 'play', url: A.url }));
    A.el.addEventListener('timeupdate', () => emit('audio', { type: 'time', url: A.url, t: A.el.currentTime, d: A.el.duration }));
    A.el.addEventListener('error', () => { A.active = false; emit('audio', { type: 'error', url: A.url }); });
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    try {
      A.ctx = new AC();
      const src = A.ctx.createMediaElementSource(A.el);
      A.an = A.ctx.createAnalyser();
      A.an.fftSize = 2048;
      A.an.smoothingTimeConstant = 0.55;
      src.connect(A.an);
      A.an.connect(A.ctx.destination);
      A.freq = new Uint8Array(A.an.frequencyBinCount);
    } catch {
      A.an = null;
    }
  }

  function analyse(dt, t) {
    if (!A.an || !A.active || A.el.paused) return false;
    A.an.getByteFrequencyData(A.freq);
    const hz = A.ctx.sampleRate / A.an.fftSize;
    const band = (lo, hi) => {
      const a = Math.max(1, Math.floor(lo / hz)), b = Math.min(A.freq.length - 1, Math.ceil(hi / hz));
      let s = 0;
      for (let i = a; i <= b; i++) s += A.freq[i];
      return s / ((b - a + 1) * 255);
    };
    const low = band(35, 160), mid = band(160, 2200), high = band(2200, 9000);
    const level = 0.5 * low + 0.35 * mid + 0.15 * high;
    // Автоусиление: превью сведены по-разному, энергия должна ходить по всей шкале
    A.peak = Math.max(level, A.peak - dt * 0.04);
    const e = clamp((level / Math.max(0.16, A.peak) - 0.35) / 0.65);
    S.bands = [lerp(S.bands[0], low, 0.3), lerp(S.bands[1], mid, 0.3), lerp(S.bands[2], high * 2.2, 0.3)];

    // Удары: рост энергии в басу выше скользящего порога
    const flux = Math.max(0, low - A.prevLow);
    A.prevLow = low;
    A.flux.push(flux);
    if (A.flux.length > 40) A.flux.shift();
    const mean = A.flux.reduce((s, v) => s + v, 0) / A.flux.length;
    const sd = Math.sqrt(A.flux.reduce((s, v) => s + (v - mean) ** 2, 0) / A.flux.length);
    if (flux > mean + 1.6 * sd && flux > 0.018 && t - A.lastOnset > 190) {
      const ioi = (t - A.lastOnset) / 1000;
      A.lastOnset = t;
      S.onset = 1;
      emit('onset', { t });
      if (ioi > 0.25 && ioi < 1.6) {
        let bpm = 60 / ioi;
        while (bpm < 80) bpm *= 2;
        while (bpm > 165) bpm /= 2;
        A.onsets.push(bpm);
        if (A.onsets.length > 12) A.onsets.shift();
        const sorted = [...A.onsets].sort((x, y) => x - y);
        const median = sorted[sorted.length >> 1];
        if (A.onsets.length >= 4) S.bpm = lerp(S.bpm, median, 0.2);
      }
      // Подстройка фазы доли к ударам — пульс попадает в музыку
      const beatMs = 60000 / S.bpm;
      const ph = (((t - S.beatT0) / beatMs) % 1 + 1) % 1;
      const err = ph > 0.5 ? ph - 1 : ph;
      if (Math.abs(err) < 0.3) S.beatT0 += err * beatMs * 0.35;
    }
    return e;
  }

  // ---------- Цикл ----------
  const views = new Set();
  let raf = 0, last = now(), fpsAcc = 0, fpsN = 0, fpsT = now();

  function update(dt, t) {
    // Энергия: из звука, из ползунка или «дыхание» в паузе
    const audioE = analyse(dt, t);
    let target;
    if (audioE !== false && S.energyAuto) target = audioE;
    else if (!S.energyAuto) target = S.energyManual;
    else target = S.playing ? 0.55 + 0.12 * Math.sin(t / 4100) : 0.3;
    S.energy = lerp(S.energy, target, 1 - Math.exp(-dt * (audioE !== false ? 9 : 3)));

    // Доля по часам трека
    const beatMs = 60000 / S.bpm;
    const beats = (t - S.beatT0) / beatMs;
    const idx = Math.floor(beats);
    const ph = beats - idx;
    if (idx !== S.beatIndex) {
      S.beatIndex = idx;
      if (S.playing) emit('beat', { index: idx, downbeat: idx % 4 === 0 });
    }
    S.onset *= Math.exp(-dt * 7);
    let pulse;
    if (S.playing) {
      const beatPulse = Math.exp(-ph * 5.5) * (idx % 4 === 0 ? 1 : 0.62);
      pulse = audioE !== false ? Math.max(S.onset, beatPulse * 0.45) : beatPulse * 0.8;
    } else {
      pulse = 0.12 * (0.5 + 0.5 * Math.sin(t / 1300));
    }
    S.pulse = pulse;

    // Раскрытие — пружина с лёгким перелётом
    const k = 26, damp = 7.5;
    S.expandVel += ((S.expandTarget - S.expand) * k - S.expandVel * damp) * dt;
    S.expand += S.expandVel * dt;

    S.flash *= Math.exp(-dt * 3.2);
    S.time += dt * (S.playing ? 0.75 + 0.65 * S.energy : 0.38);

    if (S.palTo) {
      const p = clamp((t - S.palT0) / S.palDur);
      const e = p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2;
      S.pal = mixPal(S.palFrom, S.palTo, e);
      if (p >= 1) { S.pal = S.palTo; S.palTo = null; }
    }
  }

  function loop() {
    raf = requestAnimationFrame(loop);
    const t = now();
    const dt = Math.min(0.1, (t - last) / 1000);
    last = t;
    update(dt, t);
    let drew = 0;
    for (const v of views) {
      if (!v.visible || !v.ok) continue;
      if (t - v.lastDraw < 1000 / v.fpsCap - 3) continue;
      v.lastDraw = t;
      v.draw();
      v.adapt(t);
      drew++;
    }
    if (drew) {
      fpsN++;
      if (t - fpsT > 1000) { S.fps = Math.round((fpsN * 1000) / (t - fpsT)); fpsN = 0; fpsT = t; }
    }
    emit('frame', { pulse: S.pulse, energy: S.energy, bpm: S.bpm, fps: S.fps, expand: S.expand });
    if (!shouldRun()) stop();
  }
  function shouldRun() {
    if (document.hidden || reduced.matches) return false;
    for (const v of views) if (v.visible && v.ok) return true;
    return !!(A.active && listeners.frame?.size);
  }
  function start() {
    if (raf || !shouldRun()) return;
    last = now();
    raf = requestAnimationFrame(loop);
  }
  function stop() { cancelAnimationFrame(raf); raf = 0; }
  function redrawStatic() {
    if (!reduced.matches) return;
    for (const v of views) if (v.ok) v.draw(true);
  }

  document.addEventListener('visibilitychange', () => (document.hidden ? stop() : start()));
  reduced.addEventListener?.('change', () => { if (reduced.matches) { stop(); redrawStatic(); } else start(); });

  // ---------- Вид: один холст ----------
  class View {
    constructor(canvas, opts) {
      this.canvas = canvas;
      this.o = { ...PRESETS[opts.preset || 'hero'], ...opts };
      this.visible = false;
      this.lastDraw = 0;
      this.fpsCap = 60;
      // Адаптивное разрешение: если вид не успевает за своей частотой, рисуем меньше пикселей
      this.quality = 1;
      this.win = { t0: 0, n: 0 };
      this.ok = this.init();
      if (!this.ok) {
        canvas.closest('[data-wave]')?.classList.add('is-fallback');
        return;
      }
      this.io = new IntersectionObserver((es) => {
        this.visible = es[es.length - 1].isIntersecting;
        if (this.visible) { start(); if (reduced.matches) this.draw(true); }
      });
      this.io.observe(canvas);
      this.ro = new ResizeObserver(() => { this.resize(); if (reduced.matches) this.draw(true); });
      this.ro.observe(canvas);
      canvas.addEventListener('webglcontextlost', (e) => { e.preventDefault(); this.ok = false; });
      canvas.addEventListener('webglcontextrestored', () => { this.ok = this.init(); this.resize(); });
      this.resize();
    }
    init() {
      const o = { alpha: false, antialias: false, depth: false, stencil: false, premultipliedAlpha: false, powerPreference: 'high-performance' };
      const gl = this.canvas.getContext('webgl2', o) || this.canvas.getContext('webgl', o);
      if (!gl) return false;
      this.gl = gl;
      const sh = (type, src) => {
        const s = gl.createShader(type);
        gl.shaderSource(s, src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) { console.error(gl.getShaderInfoLog(s)); return null; }
        return s;
      };
      const vs = sh(gl.VERTEX_SHADER, VERT), fs = sh(gl.FRAGMENT_SHADER, FRAG);
      if (!vs || !fs) return false;
      const pr = gl.createProgram();
      gl.attachShader(pr, vs);
      gl.attachShader(pr, fs);
      gl.bindAttribLocation(pr, 0, 'aPos');
      gl.linkProgram(pr);
      if (!gl.getProgramParameter(pr, gl.LINK_STATUS)) { console.error(gl.getProgramInfoLog(pr)); return false; }
      this.pr = pr;
      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      this.u = {};
      ['uRes', 'uTime', 'uPulse', 'uEnergy', 'uBands', 'uExpand', 'uFlash', 'uRipple', 'uC0', 'uC1', 'uC2', 'uBeam', 'uBg', 'uShape', 'uLook', 'uEnv', 'uRibbons']
        .forEach((n) => (this.u[n] = gl.getUniformLocation(pr, n)));
      return true;
    }
    resize() {
      const r = this.canvas.getBoundingClientRect();
      if (!r.width || !r.height) return;
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      // Свет мягкий: ограничиваем число пикселей, иначе на 2× экранах шейдер греет батарею
      const budget = (this.o.budget || (r.width < 600 ? 7.5e5 : 1.5e6)) * this.quality;
      const raw = r.width * dpr * r.height * dpr;
      const s = Math.min(1, Math.sqrt(budget / raw));
      this.canvas.width = Math.max(2, Math.round(r.width * dpr * s));
      this.canvas.height = Math.max(2, Math.round(r.height * dpr * s));
      this.cssToCanvas = dpr * s;
      this.fpsCap = r.width < 600 ? 30 : 60;
      this.lastDraw = 0;
    }
    // Окно 2 с после прогрева: меньше 70 % от нужной частоты — бюджет пикселей ×0,7 (не ниже 0,35)
    adapt(t) {
      const w = this.win;
      if (!w.t0) { w.t0 = t + 1500; w.n = 0; return; }
      if (t < w.t0) return;
      w.n++;
      const span = t - w.t0;
      if (span < 2000) return;
      const fps = (w.n * 1000) / span;
      if (fps < this.fpsCap * 0.7 && this.quality > 0.35) {
        this.quality = Math.max(0.35, this.quality * 0.7);
        this.resize();
      }
      w.t0 = t; w.n = 0;
    }
    draw(isStatic) {
      const gl = this.gl, u = this.u, o = this.o, c = this.canvas;
      if (!gl || gl.isContextLost()) return;
      gl.viewport(0, 0, c.width, c.height);
      gl.useProgram(this.pr);
      const st = isStatic && reduced.matches;
      const pal = S.pal;
      gl.uniform2f(u.uRes, c.width, c.height);
      gl.uniform1f(u.uTime, st ? 12.4 : S.time + (o.timeOffset || 0));
      gl.uniform1f(u.uPulse, st ? 0 : S.pulse);
      gl.uniform1f(u.uEnergy, st ? 0.5 : S.energy);
      gl.uniform3f(u.uBands, S.bands[0], S.bands[1], S.bands[2]);
      gl.uniform1f(u.uExpand, st ? 1 : Math.max(0, S.expand));
      gl.uniform1f(u.uFlash, st ? 0 : S.flash);
      const rp = (now() - S.rippleT0) / 1150;
      gl.uniform2f(u.uRipple, !st && rp >= 0 && rp < 1 ? rp : -1, 1);
      gl.uniform3fv(u.uC0, pal.c[0]);
      gl.uniform3fv(u.uC1, pal.c[1]);
      gl.uniform3fv(u.uC2, pal.c[2]);
      gl.uniform3fv(u.uBeam, pal.beam);
      gl.uniform3fv(u.uBg, pal.bg);
      // Центр ленты задаётся долей высоты сверху, в шейдере ось y смотрит вверх
      gl.uniform4f(u.uShape, o.cycles, o.amp, 0.5 - o.centerY, o.width);
      gl.uniform4f(u.uLook, o.lines, o.beams, 1.6 / 255, 1 / c.height);
      gl.uniform4f(u.uEnv, o.envX, o.envW, this.cssToCanvas || 1, o.beamShift || 0);
      gl.uniform1f(u.uRibbons, o.ribbons);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
    set(opts) { Object.assign(this.o, opts); this.lastDraw = 0; if (reduced.matches) this.draw(true); }
    destroy() {
      this.io?.disconnect();
      this.ro?.disconnect();
      views.delete(this);
      this.gl?.getExtension('WEBGL_lose_context')?.loseContext();
      this.ok = false;
    }
  }

  // ---------- Публичный интерфейс ----------
  const Wave = {
    MOODS,
    MOOD_ORDER,
    get state() {
      return {
        mood: S.mood, bpm: Math.round(S.bpm), energy: S.energy, playing: S.playing, fps: S.fps,
        energyAuto: S.energyAuto, audio: A.active && A.el && !A.el.paused, url: A.url, reduced: reduced.matches,
      };
    },
    mount(canvas, opts = {}) {
      const v = new View(canvas, opts);
      if (v.ok) { views.add(v); start(); if (reduced.matches) v.draw(true); }
      return v;
    },
    setMood(key, instant) {
      if (!MOODS[key] || key === S.mood) return;
      S.mood = key;
      retargetPalette(instant);
      emit('mood', { mood: key });
    },
    async setCover(url) {
      S.cover = url ? await sample(url) : null;
      retargetPalette();
    },
    setEnergy(v) { S.energyManual = clamp(v); S.energyAuto = false; },
    setEnergyAuto(on) { S.energyAuto = !!on; },
    setBpm(v) { S.bpm = clamp(v, 50, 220); },
    // Реакции
    start() {
      S.playing = true;
      S.expand = 0.04; S.expandVel = 0; S.expandTarget = 1;
      S.beatT0 = now();
      S.flash = Math.max(S.flash, 0.35);
      start();
    },
    stop() { S.playing = false; S.expandTarget = 0.68; },
    setPlaying(on) { on ? (S.playing || Wave.start()) : Wave.stop(); },
    skip() { S.rippleT0 = now(); S.onset = 1; },
    like() { S.flash = 1; },
    // Превью: настоящий звук ведёт энергию и удары
    play(url) {
      ensureAudio();
      if (A.ctx?.state === 'suspended') A.ctx.resume();
      if (A.url !== url) { A.el.src = url; A.url = url; A.peak = 0.25; A.onsets = []; A.flux = []; }
      A.active = true;
      const p = A.el.play();
      if (!S.playing) Wave.start();
      start();
      return p?.catch?.(() => { A.active = false; emit('audio', { type: 'blocked', url }); });
    },
    pause() { if (A.el) A.el.pause(); A.active = false; Wave.stop(); },
    seek(sec) { if (A.el && isFinite(sec)) A.el.currentTime = sec; },
    // AudioContext создаётся только по жесту пользователя, громкость до этого просто запоминаем
    setVolume(v) { A.volume = clamp(v); if (A.el) A.el.volume = A.volume; },
    get audio() { return A.el; },
    on,
    colors: { sample, gel, hexToRgb, rgbToHex, toLch, oklchToRgb },
  };
  window.Wave = Wave;
})();
