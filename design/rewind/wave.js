// Волна DalyWave для макетов «Итогов»: тот же шейдер, что в design/directions/a/wave.js
// (и apps/ui/app/lib/wave), но по холсту на историю и с двумя добавками:
//  - uChart: лента превращается в график минут по месяцам (12 значений);
//  - uBoxK: свечение вокруг обложки появляется и гаснет плавно.
// Параметры раскладки и палитра перетекают между слайдами сами.
(function () {
  "use strict";

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
// График месяцев: сглаженная кривая через 12 точек (веса — колокол, индексы постоянные)
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
}`;

  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const lerp = (a, b, k) => a + (b - a) * k;
  const easeOutExpo = (k) => (k >= 1 ? 1 : 1 - Math.pow(2, -10 * k));

  function hexToLin(hex) {
    const n = parseInt(hex.replace("#", ""), 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => Math.pow(c / 255, 2.2));
  }

  // Палитры настроений — те же, что у волны приложения
  const MOODS = {
    energetic: ["#0e0602", "#7a2208", "#ff6a1a", "#ffd27a"],
    happy: ["#0c0410", "#5c0f5e", "#ff4fa8", "#ffe9a8"],
    calm: ["#020509", "#0a2a52", "#1d8cbc", "#aef0ff"],
    sad: ["#05040b", "#1c1946", "#5b50c4", "#d3ccff"],
  };

  // Палитра из обложки: фон почти чёрный в тон, тело — глубокий цвет, гребень — живой, свет — светлый
  function coverPalette(c) {
    const { toHsl, hsl } = window.RW;
    const v = toHsl(c.vivid);
    const d = toHsl(c.second || c.vivid);
    const grey = v[1] < 0.18;
    const s = grey ? 0.2 : clamp(v[1], 0.6, 0.95);
    return [
      hsl(v[0], grey ? 0.1 : 0.5, 0.025),
      hsl(lerp(v[0], d[0], 0.3), s * 0.85, 0.2),
      hsl(v[0], s, 0.55),
      hsl(v[0] + 0.03, Math.min(1, s + 0.1), 0.86),
    ];
  }

  const DEFAULTS = { tilt: -0.42, offset: 0.1, amp: 1, freq: 2.2, fadeL: 0, fadeR: 0, energy: 0.5, chart: 0, boxK: 0, vig: 0.6 };

  class Wave {
    constructor(container) {
      this.container = container;
      this.canvas = document.createElement("canvas");
      this.canvas.className = "wave-canvas";
      this.canvas.setAttribute("aria-hidden", "true");
      container.prepend(this.canvas);
      this.reduced = matchMedia("(prefers-reduced-motion: reduce)");
      this.cur = { ...DEFAULTS };
      this.target = { ...DEFAULTS };
      this.pal = MOODS.calm.map(hexToLin);
      this.palTo = this.pal;
      this.months = new Array(12).fill(0.5);
      this.chartX = [-0.2, 0.2];
      this.chartY = [-0.3, 0.05];
      this.time = 17.3;
      this.beat = 0;
      this.bpm = 120;
      this.playing = true;
      this.expand = 1;
      this.expandAt = -1;
      this.flash = 0;
      this.ripple = [0, 9, 0];
      this.boxEl = null;
      this.box = null;
      this.initGL();
      new ResizeObserver(() => this.resize()).observe(container);
      this.resize();
      this.last = performance.now();
      this.frame = this.frame.bind(this);
      requestAnimationFrame(this.frame);
    }

    initGL() {
      const opts = { antialias: false, alpha: false, depth: false, stencil: false, premultipliedAlpha: false, preserveDrawingBuffer: true };
      const gl = this.canvas.getContext("webgl2", opts) || this.canvas.getContext("webgl", opts);
      this.gl = gl;
      if (!gl) return;
      const sh = (type, src) => {
        const s = gl.createShader(type);
        gl.shaderSource(s, src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
          console.error(gl.getShaderInfoLog(s));
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
      gl.useProgram(prog);
      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(0);
      gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
      this.u = {};
      for (const name of ["uRes", "uTime", "uBeat", "uEnergy", "uBands", "uExpand", "uFlash", "uRipple", "uC0", "uC1", "uC2", "uC3", "uLayout", "uFade", "uFreq", "uBox", "uBoxR", "uBoxK", "uChart", "uMonths", "uChartX", "uChartY", "uVig"]) {
        this.u[name] = gl.getUniformLocation(prog, name);
      }
    }

    resize() {
      const c = this.container;
      const w = Math.max(1, c.clientWidth);
      const h = Math.max(1, c.clientHeight);
      let scale = Math.min(window.devicePixelRatio || 1, 2);
      const budget = 2.4e6;
      if (w * h * scale * scale > budget) scale = Math.sqrt(budget / (w * h));
      this.canvas.width = Math.round(w * scale);
      this.canvas.height = Math.round(h * scale);
      this.cssW = w;
      this.cssH = h;
    }

    // Координаты шейдера: центр холста — 0, высота — 1
    toShader(px, py) {
      return [(px - this.cssW / 2) / this.cssH, -(py - this.cssH / 2) / this.cssH];
    }

    set(params, opts = {}) {
      Object.assign(this.target, params);
      if (opts.instant || this.reduced.matches) Object.assign(this.cur, this.target);
    }

    setPalette(hexes, opts = {}) {
      this.palTo = hexes.map(hexToLin);
      if (opts.instant || this.reduced.matches) this.pal = this.palTo;
    }

    setMonths(values) {
      const max = Math.max(...values);
      this.months = values.map((v) => v / max);
    }

    setBox(el) {
      this.boxEl = el || null;
    }

    measureBox() {
      this.box = null;
      if (!this.boxEl) return;
      const r = this.boxEl.getBoundingClientRect();
      const c = this.container.getBoundingClientRect();
      if (!r.width || !c.height) return;
      const cx = (r.left + r.width / 2 - c.left - c.width / 2) / c.height;
      const cy = -(r.top + r.height / 2 - c.top - c.height / 2) / c.height;
      const radius = parseFloat(getComputedStyle(this.boxEl).borderTopLeftRadius) || 0;
      this.box = [cx, cy, r.width / 2 / c.height, r.height / 2 / c.height, Math.min(radius, r.width / 2) / c.height];
    }

    start() {
      this.expand = 0;
      this.expandAt = performance.now();
      this.flash = 0.55;
    }

    skip() {
      this.ripple = [(Math.random() - 0.5) * 0.3, 0, 1];
    }

    frame(now) {
      requestAnimationFrame(this.frame);
      const dt = Math.min(0.1, (now - this.last) / 1000);
      this.last = now;
      const still = this.reduced.matches;
      // Раскладка и палитра перетекают за ~1,2 с
      const k = still ? 1 : 1 - Math.exp(-dt * 2.6);
      for (const key in this.target) this.cur[key] = lerp(this.cur[key], this.target[key], k);
      this.pal = this.pal.map((c, i) => c.map((v, j) => lerp(v, this.palTo[i][j], still ? 1 : 1 - Math.exp(-dt * 2.2))));
      if (this.expandAt >= 0) {
        const e = clamp((now - this.expandAt) / 1600, 0, 1);
        this.expand = easeOutExpo(e);
        if (e >= 1) this.expandAt = -1;
      }
      this.flash *= Math.exp(-dt * 2.6);
      this.ripple[1] += dt;
      if (this.playing && !still) {
        const pos = (now / 1000) * (this.bpm / 60);
        const ph = pos - Math.floor(pos);
        const accent = Math.floor(pos) % 4 === 0 ? 1 : 0.62;
        this.beat = Math.min(1, ph / 0.035) * Math.exp(-ph * 5.2) * accent;
      } else this.beat = 0;
      if (!still) this.time += dt * (0.16 + 0.62 * this.cur.energy);
      this.measureBox();
      this.draw(still);
    }

    draw(still) {
      const gl = this.gl;
      if (!gl) return;
      const u = this.u;
      const L = this.cur;
      gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      gl.uniform2f(u.uRes, this.canvas.width, this.canvas.height);
      gl.uniform1f(u.uTime, this.time);
      gl.uniform1f(u.uBeat, still ? 0 : this.beat * 0.8);
      gl.uniform1f(u.uEnergy, L.energy);
      gl.uniform3f(u.uBands, 0.25 * this.beat, 0.2, 0.15);
      gl.uniform1f(u.uExpand, still ? 1 : this.expand);
      gl.uniform1f(u.uFlash, still ? 0 : this.flash);
      gl.uniform3f(u.uRipple, this.ripple[0], this.ripple[1], this.ripple[2]);
      const [c0, c1, c2, c3] = this.pal;
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
      gl.uniform1f(u.uBoxK, L.boxK);
      gl.uniform1f(u.uChart, L.chart);
      gl.uniform1fv(u.uMonths, new Float32Array(this.months));
      gl.uniform2f(u.uChartX, this.chartX[0], this.chartX[1]);
      gl.uniform2f(u.uChartY, this.chartY[0], this.chartY[1]);
      gl.uniform1f(u.uVig, L.vig);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
  }

  // Та же кривая, что в шейдере, — чтобы подписи вставали точно на ленту
  function chartValue(months, u) {
    let s = 0;
    let w = 0;
    for (let i = 0; i < 12; i++) {
      const d = u - i;
      const k = Math.exp(-d * d * 2.2);
      s += months[i] * k;
      w += k;
    }
    return s / w;
  }

  window.RWave = { Wave, MOODS, coverPalette, chartValue };
})();
