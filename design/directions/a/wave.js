// Волна DalyWave, направление A «Полночь».
// Один шейдер WebGL: светящаяся жидкая лента в темноте. Пульс — в долю трека,
// размах — от энергии, мелкие детали — от спектра. Один холст на страницу:
// он переезжает между экранами (главная, плеер, отдельный экран волны).
(function () {
  "use strict";

  // Палитры настроений: фон, глубина, тело ленты, свет гребня
  const MOODS = {
    energetic: { label: "Бодрое", pal: ["#0e0602", "#7a2208", "#ff6a1a", "#ffd27a"], energy: 0.78 },
    happy: { label: "Весёлое", pal: ["#0c0410", "#5c0f5e", "#ff4fa8", "#ffe9a8"], energy: 0.66 },
    calm: { label: "Спокойное", pal: ["#020509", "#0a2a52", "#1d8cbc", "#aef0ff"], energy: 0.42 },
    sad: { label: "Грустное", pal: ["#05040b", "#1c1946", "#5b50c4", "#d3ccff"], energy: 0.3 },
  };

  // Раскладки: как лента ложится в конкретный контейнер
  const LAYOUTS = {
    hero: { tilt: 0.09, offset: -0.06, fadeL: 0.42, fadeR: 0, amp: 1.0, freq: 1.0 },
    "hero-phone": { tilt: -0.42, offset: 0.1, fadeL: 0, fadeR: 0, amp: 1.0, freq: 2.2 },
    stage: { tilt: -0.06, offset: 0.0, fadeL: 0, fadeR: 0, amp: 0.85, freq: 1.0 },
    "stage-phone": { tilt: -0.18, offset: 0.0, fadeL: 0, fadeR: 0, amp: 0.9, freq: 2.0 },
    full: { tilt: -0.12, offset: 0.02, fadeL: 0, fadeR: 0, amp: 1.25, freq: 1.0 },
    "full-phone": { tilt: -0.5, offset: 0.06, fadeL: 0, fadeR: 0, amp: 1.1, freq: 2.0 },
  };

  const VERT = "attribute vec2 aPos;void main(){gl_Position=vec4(aPos,0.0,1.0);}";

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
// Блики в толще воды: тонкие линии там, где два искривлённых поля проходят через ноль
float caustic(vec2 p, float t) {
  vec2 a = p * vec2(9.0, 5.0);
  float f = sin(a.x + 1.7 * sin(a.y * 0.8 + t) + t * 0.6) + sin(a.y * 1.3 + 1.9 * sin(a.x * 0.7 - t * 0.8) - t * 0.4);
  float lines = 1.0 - smoothstep(0.0, 0.32, abs(f));
  return lines * lines;
}
// Линия ленты: две синусоиды разной длины — медленное «дыхание» формы
float ribbon(float x, float t, float amp) {
  return amp * (sin(x * 1.15 * uFreq + t * 0.35) + 0.55 * sin(x * 2.4 * uFreq - t * 0.42 + 3.6));
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

  // Текучесть: пространство слегка искажено шумом, сильнее на энергичной музыке
  vec2 w = vec2(fbm(q * 1.3 + vec2(0.0, t * 0.1)), fbm(q * 1.3 + vec2(4.7, -t * 0.08)));
  q += (w - 0.5) * (0.07 + 0.12 * e + 0.06 * uBands.y) * (0.35 + 0.65 * ex);

  // Рябь при пропуске: бежит от точки в обе стороны и гаснет
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

  // Склон волны под гребнем: его высота плавно меняется вдоль волны
  float tw = q.x * 1.7 * uFreq + t * 0.33 + 1.2 * sin(q.x * 0.7 * uFreq - t * 0.15);
  float faceH = thick * (1.5 + 1.2 * (0.5 + 0.5 * sin(tw)));
  float fz = below / faceH;
  float body = exp(-pow(fz, 1.5)) * exp(-above * above / (0.00035 + 0.0005 * e));

  // Внутри — струи вдоль волны и слабые блики, как свет в толще воды
  float m = fbm(vec2(q.x * 1.9 * uFreq - t * 0.2, fz * 2.6 + t * 0.05));
  float ca = caustic(vec2(q.x * 0.42 * uFreq - t * 0.02, fz * 0.32 + 0.13), t * 0.45 + e);
  vec3 faceCol = mix(uC1, uC2, smoothstep(0.15, 0.9, m) * (1.0 - 0.55 * min(fz, 1.0)));

  vec3 col = uC0;
  col += faceCol * body * (0.7 + 0.35 * uBeat + 0.15 * uBands.x);
  col += mix(uC2, uC3, 0.6) * ca * body * smoothstep(0.05, 0.4, fz) * (0.18 + 0.22 * e + 0.15 * uBeat);

  // Гребень ловит свет: тонкая линия, по ней бежит блик
  float sheen = 0.55 + 0.45 * sin(q.x * 3.1 * uFreq + t * 0.8 + 1.5 * sin(t * 0.3));
  float core = exp(-abs(dy) / (0.0042 + 0.005 * e + 0.004 * uBeat));
  col += uC3 * core * (0.3 + 0.5 * sheen + 0.5 * uBeat + 0.2 * uBands.z) * ex;
  float glow = exp(-abs(dy) * (15.0 - 4.0 * e - 3.0 * uBeat));
  col += mix(uC2, uC3, 0.35) * glow * (0.1 + 0.12 * e + 0.2 * uBeat) * ex;

  // Дальние гребни: глубина, как у ночного моря
  for (int i = 1; i < 3; i++) {
    float fi = float(i);
    float yi = ribbon(q.x * (1.0 + 0.09 * fi), t - 0.7 * fi, amp * (0.8 - 0.15 * fi)) - (0.075 + 0.02 * e) * fi * ex + rip * 0.5;
    float di = q.y - yi;
    col += mix(uC2, uC3, 0.3) * exp(-abs(di) / (0.0035 + 0.0025 * fi)) * (0.2 / fi) * ex * body;
    col += uC2 * exp(-abs(di) * 16.0) * (0.06 / fi) * ex;
  }

  // Бахрома сияния над гребнем: короткие лучи пятнами
  float patch = smoothstep(0.3, 0.75, noise(vec2(q.x * 1.6 * uFreq - t * 0.12, 3.0)));
  float rn = noise(vec2(q.x * 26.0 * uFreq + w.x * 3.0, t * 0.3));
  float rays = smoothstep(0.45, 1.0, rn) * patch;
  float rayMask = smoothstep(0.0, 0.025, above) * exp(-above * (15.0 - 5.0 * e - 3.0 * uBeat));
  col += mix(uC2, uC3, 0.3) * rays * rayMask * (0.16 + 0.3 * e + 0.3 * uBeat) * ex;
  col += uC1 * exp(-above * 5.0) * 0.1 * ex;

  // Свет бежит по фронту ряби
  col += uC3 * ripEnv * 0.45 * exp(-abs(dy) * 8.0);

  // Обложка, которую обнимает волна: свечение по контуру
  if (uBox.z > 0.0) {
    float sd = sdRoundBox(p - uBox.xy, uBox.zw, uBoxR);
    float halo = exp(-max(sd, 0.0) * 13.0) * (0.5 + 0.5 * uBeat);
    col += mix(uC2, uC3, 0.3) * halo * (0.25 + 0.75 * min(body + glow, 1.0)) * 0.8;
    col += uC1 * exp(-max(sd, 0.0) * 4.0) * 0.35;
  }

  // Под текстом лента тише
  float fx = frag.x / uRes.x;
  float fade = 1.0;
  if (uFade.x > 0.0) fade *= mix(0.4, 1.0, smoothstep(0.0, uFade.x, fx));
  if (uFade.y > 0.0) fade *= mix(0.6, 1.0, smoothstep(1.0, 1.0 - uFade.y, fx));
  col = uC0 + (col - uC0) * fade;

  // Вспышка от лайка
  col += uC3 * uFlash * (0.35 * min(body + core, 1.4) + 0.05 + 0.15 * glow);

  // Виньетка
  vec2 vp = frag / uRes - 0.5;
  col *= 1.0 - 0.6 * dot(vp, vp);

  // Мягкая тональная кривая и перевод в sRGB
  col = 1.0 - exp(-col * 1.45);
  col = pow(max(col, 0.0), vec3(1.0 / 2.2));
  // Дизеринг против полос на тёмных градиентах
  col += (hash(frag + fract(t * 7.0) * 61.0) - 0.5) / 180.0;
  gl_FragColor = vec4(col, 1.0);
}`;

  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const lerp = (a, b, k) => a + (b - a) * k;
  const easeInOut = (k) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
  const easeOutExpo = (k) => (k >= 1 ? 1 : 1 - Math.pow(2, -10 * k));

  function hexToLin(hex) {
    const n = parseInt(hex.replace("#", ""), 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => Math.pow(c / 255, 2.2));
  }
  const mixLin = (a, b, k) => a.map((v, i) => lerp(v, b[i], k));

  // Палитра настроения, слегка окрашенная цветом обложки. Обложка — только оттенок:
  // противоположный цвет в больших долях делает тёплые настроения грязными
  function buildPalette(moodKey, cover) {
    const base = MOODS[moodKey].pal.map(hexToLin);
    if (!cover || !cover.length) return base;
    const vivid = hexToLin(cover[0]);
    const deep = hexToLin(cover[1] || cover[0]);
    return [
      mixLin(base[0], deep, 0.05),
      mixLin(base[1], deep, 0.12),
      mixLin(base[2], vivid, 0.08),
      base[3],
    ];
  }

  class Engine {
    constructor() {
      this.canvas = document.createElement("canvas");
      this.canvas.className = "wave-canvas";
      this.canvas.setAttribute("aria-hidden", "true");
      this.listeners = new Set();
      this.reduced = matchMedia("(prefers-reduced-motion: reduce)");
      this.s = {
        mood: "calm",
        cover: null,
        pal: buildPalette("calm", null),
        palFrom: null,
        palTo: null,
        palT: 1,
        energy: 0.3,
        energyTarget: 0.3,
        manualEnergy: null,
        bpm: 120,
        beatOrigin: performance.now(),
        playing: false,
        beat: 0,
        expand: 1,
        expandFrom: 1,
        expandAt: -1,
        flash: 0,
        ripple: [0, 9, 0],
        time: 17.3,
        bands: [0, 0, 0],
        onset: 0,
        source: "clock",
        fps: 0,
      };
      this.layout = LAYOUTS.hero;
      this.box = null;
      this.visible = false;
      this.lastT = 0;
      this.lastDraw = 0;
      this.raf = 0;
      this.frame = this.frame.bind(this);
      this.initGL();
      this.ro = new ResizeObserver(() => this.resize());
      this.io = new IntersectionObserver((entries) => {
        this.visible = entries.some((en) => en.isIntersecting);
        this.kick();
      });
      document.addEventListener("visibilitychange", () => this.kick());
      this.reduced.addEventListener?.("change", () => this.kick());
      this.canvas.addEventListener("webglcontextlost", (e) => {
        e.preventDefault();
        cancelAnimationFrame(this.raf);
        this.raf = 0;
      });
      this.canvas.addEventListener("webglcontextrestored", () => {
        this.initGL();
        this.resize();
        this.kick();
      });
    }

    initGL() {
      const opts = { antialias: false, alpha: false, depth: false, stencil: false, premultipliedAlpha: false, powerPreference: "high-performance" };
      const gl = this.canvas.getContext("webgl2", opts) || this.canvas.getContext("webgl", opts);
      this.gl = gl;
      if (!gl) return;
      const sh = (type, src) => {
        const s = gl.createShader(type);
        gl.shaderSource(s, src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
          this.glError = gl.getShaderInfoLog(s);
          return null;
        }
        return s;
      };
      const vs = sh(gl.VERTEX_SHADER, VERT);
      const fs = sh(gl.FRAGMENT_SHADER, FRAG);
      if (!vs || !fs) {
        this.gl = null;
        return;
      }
      const prog = gl.createProgram();
      gl.attachShader(prog, vs);
      gl.attachShader(prog, fs);
      gl.bindAttribLocation(prog, 0, "aPos");
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
        this.glError = gl.getProgramInfoLog(prog);
        this.gl = null;
        return;
      }
      gl.useProgram(prog);
      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      this.u = {};
      for (const name of ["uRes", "uTime", "uBeat", "uEnergy", "uBands", "uExpand", "uFlash", "uRipple", "uC0", "uC1", "uC2", "uC3", "uLayout", "uFade", "uFreq", "uBox", "uBoxR"]) {
        this.u[name] = gl.getUniformLocation(prog, name);
      }
    }

    // Холст переезжает в новый контейнер; контекст WebGL при этом сохраняется
    mount(container, opts = {}) {
      if (!container) return;
      this.layout = LAYOUTS[opts.layout] || LAYOUTS.hero;
      this.layoutName = opts.layout || "hero";
      this.boxEl = opts.box || null;
      if (this.container !== container) {
        if (this.container) {
          this.ro.unobserve(this.container);
          this.io.unobserve(this.container);
        }
        this.container = container;
        container.prepend(this.canvas);
        this.ro.observe(container);
        this.io.observe(container);
      }
      container.classList.toggle("wave-fallback", !this.gl);
      this.resize();
      this.kick();
    }

    resize() {
      const c = this.container;
      if (!c) return;
      const w = Math.max(1, c.clientWidth);
      const h = Math.max(1, c.clientHeight);
      let scale = Math.min(window.devicePixelRatio || 1, 2);
      // Мягкая картинка не требует полного разрешения на больших холстах
      const budget = 2.4e6;
      if (w * h * scale * scale > budget) scale = Math.sqrt(budget / (w * h));
      const W = Math.round(w * scale);
      const H = Math.round(h * scale);
      if (this.canvas.width !== W || this.canvas.height !== H) {
        this.canvas.width = W;
        this.canvas.height = H;
      }
      this.cssW = w;
      this.cssH = h;
      this.lowPower = w < 600;
      this.measureBox();
      if (this.isStatic()) this.draw();
    }

    measureBox() {
      this.box = null;
      if (!this.boxEl || !this.container) return;
      const r = this.boxEl.getBoundingClientRect();
      const c = this.container.getBoundingClientRect();
      if (!r.width || !c.height) return;
      const cx = (r.left + r.width / 2 - c.left - c.width / 2) / c.height;
      const cy = -(r.top + r.height / 2 - c.top - c.height / 2) / c.height;
      const radius = parseFloat(getComputedStyle(this.boxEl).borderTopLeftRadius) || 0;
      this.box = [cx, cy, r.width / 2 / c.height, r.height / 2 / c.height, radius / c.height];
    }

    isStatic() {
      return this.reduced.matches;
    }

    active() {
      return !!(this.gl && this.container && this.visible && !document.hidden && !this.isStatic());
    }

    kick() {
      if (this.isStatic()) {
        cancelAnimationFrame(this.raf);
        this.raf = 0;
        this.s.beat = 0;
        this.s.expand = 1;
        this.s.flash = 0;
        this.s.ripple = [0, 9, 0];
        if (this.s.palT < 1 && this.s.palTo) {
          this.s.pal = this.s.palTo;
          this.s.palT = 1;
        }
        this.draw();
        return;
      }
      if (this.active() && !this.raf) {
        this.lastT = performance.now();
        this.raf = requestAnimationFrame(this.frame);
      }
    }

    frame(now) {
      this.raf = 0;
      if (!this.active()) return;
      this.raf = requestAnimationFrame(this.frame);
      // На телефоне 30 кадров в секунду — бережём батарею
      if (this.lowPower && now - this.lastDraw < 31) return;
      // Медленный кадр не должен замедлять время анимации; скрытая вкладка и так на паузе
      const dt = Math.min(0.25, (now - this.lastT) / 1000);
      this.lastT = now;
      if (this.lastDraw) this.s.fps = lerp(this.s.fps || 30, 1000 / Math.max(1, now - this.lastDraw), 0.1);
      this.lastDraw = now;
      this.update(dt, now);
      this.draw();
      for (const fn of this.listeners) fn(this.s);
    }

    update(dt, now) {
      const s = this.s;
      // Палитра перетекает при смене настроения
      if (s.palT < 1) {
        s.palT = Math.min(1, s.palT + dt / 1.2);
        const k = easeInOut(s.palT);
        s.pal = s.palFrom.map((c, i) => mixLin(c, s.palTo[i], k));
      }
      // Раскрытие при старте
      if (s.expandAt >= 0) {
        const k = clamp((now - s.expandAt) / 1600, 0, 1);
        s.expand = lerp(s.expandFrom, 1, easeOutExpo(k));
        if (k >= 1) s.expandAt = -1;
      }
      s.flash *= Math.exp(-dt * 2.6);
      s.ripple[1] += dt;

      const audio = this.audio && !this.audio.paused && this.analyser;
      if (audio) {
        this.readAudio(dt, now);
        s.source = "audio";
      } else {
        s.source = s.playing ? "clock" : "idle";
        s.bands = s.bands.map((v) => v * Math.exp(-dt * 3));
        if (s.playing) {
          const beatPos = ((now - s.beatOrigin) / 1000) * (s.bpm / 60);
          s.beat = pulse(beatPos);
        } else {
          // Без музыки форма медленно дышит
          s.beat = 0.12 * (0.5 + 0.5 * Math.sin(now / 1000 * 0.9));
        }
        const target = s.manualEnergy != null ? s.manualEnergy : s.playing ? s.energyTarget : 0.16;
        s.energy = lerp(s.energy, target, 1 - Math.exp(-dt * 2.2));
      }
      // Поток ускоряется вместе с энергией
      s.time += dt * (0.16 + 0.62 * s.energy);
    }

    readAudio(dt, now) {
      const s = this.s;
      const an = this.analyser;
      an.getByteFrequencyData(this.freq);
      an.getFloatTimeDomainData(this.wave);
      const hz = this.actx.sampleRate / an.fftSize;
      const band = (lo, hi) => {
        const a = Math.max(1, Math.floor(lo / hz));
        const b = Math.min(this.freq.length - 1, Math.ceil(hi / hz));
        let sum = 0;
        for (let i = a; i <= b; i++) sum += this.freq[i];
        return sum / ((b - a + 1) * 255);
      };
      const raw = [band(30, 150), band(150, 2000), band(2000, 9000)];
      s.bands = s.bands.map((v, i) => lerp(v, raw[i], 1 - Math.exp(-dt * 14)));
      // Спектральный поток — для онсетов
      let flux = 0;
      const lim = Math.min(this.freq.length, Math.ceil(7000 / hz));
      for (let i = 1; i < lim; i++) {
        const d = this.freq[i] - this.prev[i];
        if (d > 0) flux += d;
        this.prev[i] = this.freq[i];
      }
      flux /= lim * 255;
      this.fMean = lerp(this.fMean, flux, 0.06);
      this.fDev = lerp(this.fDev, Math.abs(flux - this.fMean), 0.06);
      if (flux > this.fMean + 1.7 * this.fDev + 0.004 && now - this.lastOnset > 170) {
        this.lastOnset = now;
        s.onset = 1;
      }
      s.onset *= Math.exp(-dt * 7);
      // Сетка долей из анализа трека
      let grid = 0;
      if (this.meta && this.meta.bpm) {
        const pos = (this.audio.currentTime - (this.meta.phase || 0)) * (this.meta.bpm / 60);
        grid = pos >= 0 ? pulse(pos) : 0;
        s.bpm = this.meta.bpm;
      }
      s.beat = Math.max(grid * 0.8, s.onset * 0.95);
      // Энергия — громкость относительно недавнего пика
      let sum = 0;
      for (let i = 0; i < this.wave.length; i++) sum += this.wave[i] * this.wave[i];
      const rms = Math.sqrt(sum / this.wave.length);
      this.peak = Math.max(this.peak * Math.exp(-dt * 0.15), rms, 0.02);
      const target = clamp(Math.pow(rms / this.peak, 1.4) * 0.95 + raw[0] * 0.25, 0, 1);
      s.energy = lerp(s.energy, target, 1 - Math.exp(-dt * 4));
    }

    draw() {
      const gl = this.gl;
      if (!gl || !this.container) return;
      const s = this.s;
      const u = this.u;
      const L = this.layout;
      const still = this.isStatic();
      gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      gl.uniform2f(u.uRes, this.canvas.width, this.canvas.height);
      gl.uniform1f(u.uTime, s.time);
      gl.uniform1f(u.uBeat, still ? 0 : s.beat);
      gl.uniform1f(u.uEnergy, still ? Math.max(s.energy, 0.42) : s.energy);
      gl.uniform3f(u.uBands, s.bands[0], s.bands[1], s.bands[2]);
      gl.uniform1f(u.uExpand, still ? 1 : s.expand);
      gl.uniform1f(u.uFlash, still ? 0 : s.flash);
      gl.uniform3f(u.uRipple, s.ripple[0], s.ripple[1], s.ripple[2]);
      const [c0, c1, c2, c3] = s.pal;
      gl.uniform3f(u.uC0, c0[0], c0[1], c0[2]);
      gl.uniform3f(u.uC1, c1[0], c1[1], c1[2]);
      gl.uniform3f(u.uC2, c2[0], c2[1], c2[2]);
      gl.uniform3f(u.uC3, c3[0], c3[1], c3[2]);
      gl.uniform4f(u.uLayout, L.tilt, L.offset, 0, L.amp);
      gl.uniform2f(u.uFade, L.fadeL, L.fadeR);
      gl.uniform1f(u.uFreq, L.freq);
      if (this.box) {
        gl.uniform4f(u.uBox, this.box[0], this.box[1], this.box[2], this.box[3]);
        gl.uniform1f(u.uBoxR, this.box[4]);
      } else {
        gl.uniform4f(u.uBox, 0, 0, 0, 0);
        gl.uniform1f(u.uBoxR, 0);
      }
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }

    // --- Управление ---

    setMood(key, opts = {}) {
      if (!MOODS[key]) return;
      const s = this.s;
      s.mood = key;
      const next = buildPalette(key, s.cover);
      if (opts.instant || this.isStatic()) {
        s.pal = next;
        s.palT = 1;
        this.draw();
      } else {
        s.palFrom = s.pal.map((c) => c.slice());
        s.palTo = next;
        s.palT = 0;
        this.kick();
      }
      if (s.manualEnergy == null) s.energyTarget = MOODS[key].energy;
    }

    setCover(colors, opts = {}) {
      this.s.cover = colors && colors.length ? colors : null;
      this.setMood(this.s.mood, opts);
    }

    setEnergy(v) {
      this.s.manualEnergy = v == null ? null : clamp(v, 0, 1);
      this.kick();
    }

    setTrackEnergy(v) {
      this.s.energyTarget = clamp(v, 0, 1);
    }

    setBpm(bpm) {
      const s = this.s;
      const now = performance.now();
      // Сохраняем фазу, чтобы пульс не дёрнулся
      const pos = ((now - s.beatOrigin) / 1000) * (s.bpm / 60);
      s.bpm = clamp(bpm || 120, 40, 220);
      s.beatOrigin = now - ((pos % 1) / (s.bpm / 60)) * 1000;
    }

    setPlaying(on) {
      const s = this.s;
      if (on && !s.playing) s.beatOrigin = performance.now();
      s.playing = !!on;
      this.kick();
    }

    // Старт — форма раскрывается из тонкой линии
    start() {
      const s = this.s;
      s.expandFrom = 0;
      s.expand = 0;
      s.expandAt = performance.now();
      s.flash = Math.max(s.flash, 0.55);
      this.setPlaying(true);
    }

    // Пропуск — рябь
    skip() {
      const s = this.s;
      s.ripple = [(Math.random() - 0.5) * 0.6, 0, 1];
      s.energy *= 0.7;
      s.beatOrigin = performance.now();
      this.kick();
    }

    // Лайк — вспышка
    like() {
      this.s.flash = 1;
      this.kick();
    }

    onFrame(fn) {
      this.listeners.add(fn);
      return () => this.listeners.delete(fn);
    }

    // --- Живой звук: превью MP3 через AnalyserNode ---

    ensureAudio() {
      if (this.audio) return;
      const Ctx = window.AudioContext || window.webkitAudioContext;
      this.actx = new Ctx();
      this.audio = new Audio();
      this.audio.preload = "auto";
      const src = this.actx.createMediaElementSource(this.audio);
      this.analyser = this.actx.createAnalyser();
      this.analyser.fftSize = 2048;
      this.analyser.smoothingTimeConstant = 0.55;
      this.gain = this.actx.createGain();
      src.connect(this.analyser);
      this.analyser.connect(this.gain);
      this.gain.connect(this.actx.destination);
      this.freq = new Uint8Array(this.analyser.frequencyBinCount);
      this.prev = new Uint8Array(this.analyser.frequencyBinCount);
      this.wave = new Float32Array(this.analyser.fftSize);
      this.fMean = 0;
      this.fDev = 0;
      this.lastOnset = 0;
      this.peak = 0.05;
      this.audio.addEventListener("ended", () => {
        this.s.playing = false;
        this.emitAudio();
      });
      this.audio.addEventListener("play", () => this.emitAudio());
      this.audio.addEventListener("pause", () => this.emitAudio());
    }

    emitAudio() {
      window.dispatchEvent(new CustomEvent("wave:audio", { detail: { playing: this.audio && !this.audio.paused, url: this.audioUrl } }));
    }

    async playAudio(url, meta = {}) {
      this.ensureAudio();
      if (this.actx.state === "suspended") await this.actx.resume();
      if (this.audioUrl !== url) {
        this.audioUrl = url;
        this.audio.src = url;
        this.peak = 0.05;
      }
      this.meta = meta;
      if (meta.bpm) this.setBpm(meta.bpm);
      this.s.manualEnergy = null;
      this.s.playing = true;
      try {
        await this.audio.play();
      } catch (e) {
        this.s.playing = false;
      }
      this.kick();
    }

    pauseAudio() {
      if (this.audio && !this.audio.paused) this.audio.pause();
      this.s.playing = false;
      this.kick();
    }

    get audioState() {
      if (!this.audio) return { playing: false, url: null, time: 0, duration: 30 };
      return { playing: !this.audio.paused, url: this.audioUrl, time: this.audio.currentTime, duration: this.audio.duration || 30 };
    }
  }

  // Огибающая доли: мягкая атака, экспоненциальный спад, сильная доля — ярче
  function pulse(beatPos) {
    const phase = beatPos - Math.floor(beatPos);
    const idx = Math.floor(beatPos);
    const accent = idx % 4 === 0 ? 1 : 0.62;
    const attack = Math.min(1, phase / 0.035);
    return attack * Math.exp(-phase * 5.2) * accent;
  }

  let engine = null;
  const get = () => (engine ||= new Engine());

  window.Wave = {
    MOODS,
    mount: (el, opts) => get().mount(el, opts),
    remeasure: () => engine && engine.measureBox(),
    setMood: (k, o) => get().setMood(k, o),
    setCover: (c, o) => get().setCover(c, o),
    setEnergy: (v) => get().setEnergy(v),
    setTrackEnergy: (v) => get().setTrackEnergy(v),
    setBpm: (b) => get().setBpm(b),
    setPlaying: (on) => get().setPlaying(on),
    start: () => get().start(),
    skip: () => get().skip(),
    like: () => get().like(),
    onFrame: (fn) => get().onFrame(fn),
    playAudio: (url, meta) => get().playAudio(url, meta),
    pauseAudio: () => get().pauseAudio(),
    get audio() {
      return get().audioState;
    },
    get state() {
      return get().s;
    },
    get supported() {
      return !!get().gl;
    },
    get error() {
      return get().glError || null;
    },
  };
})();
