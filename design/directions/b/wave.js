/* Волна «Импульс» — DalyWave, направление B.
   Сфера или тор из колец светящихся точек на WebGL, без библиотек.
   Один движок состояния (такт, энергия, палитра, реакции) и сколько угодно холстов:
   на главной, в плеере и на отдельном экране отладки. Наружу — window.Wave. */
(() => {
  'use strict';

  const hex = (h) => {
    const n = parseInt(h.slice(1), 16);
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
  };
  const mix = (a, b, k) => a + (b - a) * k;
  const mix3 = (a, b, k) => [mix(a[0], b[0], k), mix(a[1], b[1], k), mix(a[2], b[2], k)];
  const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const easeInOut = (k) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
  const easeOutBack = (k) => 1 + 2.2 * Math.pow(k - 1, 3) + 1.2 * Math.pow(k - 1, 2);

  const ACCENT = hex('#cdff3f');

  // Настроение задаёт палитру (глубина → середина → блик) и характер формы.
  // torus — доля тора, squash — «поникшая» сфера, spike — шипы на сильных местах,
  // lobe — упругие лепестки, wobble — размах течения поверхности, speed — его темп.
  const MOODS = {
    energetic: { label: 'Бодрое', deep: '#2b0a04', mid: '#ff5a1f', hi: '#ffc94a', energy: 0.72,
      form: { torus: 1, squash: 0, spike: 1, lobe: 0, wobble: 1, speed: 1.35 } },
    happy: { label: 'Весёлое', deep: '#2a0828', mid: '#ff4fa0', hi: '#ffe36b', energy: 0.6,
      form: { torus: 0, squash: -0.15, spike: 0.25, lobe: 1, wobble: 0.8, speed: 1.1 } },
    calm: { label: 'Спокойное', deep: '#06123a', mid: '#2f6bff', hi: '#5cf2e0', energy: 0.42,
      form: { torus: 0, squash: 0, spike: 0.1, lobe: 0, wobble: 0.5, speed: 0.72 } },
    sad: { label: 'Грустное', deep: '#0b0d1c', mid: '#5b5fa8', hi: '#b8c2e8', energy: 0.3,
      form: { torus: 0, squash: 1, spike: 0, lobe: 0, wobble: 0.45, speed: 0.5 } },
  };
  const FORM_KEYS = ['torus', 'squash', 'spike', 'lobe', 'wobble', 'speed'];
  const PAL_KEYS = ['deep', 'mid', 'hi'];

  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)');

  // ——— Состояние, общее для всех холстов ———
  const S = {
    time: 0, flow: 0, lastNow: 0,
    playing: false, playAmt: 0,
    bpm: 120, beatPos: 0, lastBeat: -1, kick: 0, bar: 0, ringK: 1,
    energy: 0.3, manualEnergy: null, onset: 0, bands: [0, 0, 0],
    expand: 1, expandT: -1,
    ripple: 1, rippleT: -10, rippleDir: [0.6, 0.2, 0.77],
    flash: 0, likeT: -10,
    mood: 'calm',
    cover: null,
    pal: null, palFrom: null, palTo: null,
    form: null, formFrom: null, formTo: null,
    blendT: -10,
    draws: 0,
  };

  const listeners = {};
  const emit = (name, data) => (listeners[name] || []).forEach((fn) => fn(data));

  function moodPalette(key) {
    const m = MOODS[key];
    let p = { deep: hex(m.deep), mid: hex(m.mid), hi: hex(m.hi) };
    if (S.cover) {
      // Обложка подмешивается мягко: настроение остаётся узнаваемым
      p = {
        deep: mix3(p.deep, S.cover.dark, 0.3),
        mid: mix3(p.mid, S.cover.vivid, 0.24),
        hi: mix3(p.hi, mix3(S.cover.vivid, [1, 1, 1], 0.5), 0.14),
      };
    }
    return p;
  }

  S.pal = moodPalette('calm');
  S.form = { ...MOODS.calm.form };

  function blendTo(key) {
    S.palFrom = { ...S.pal };
    S.formFrom = { ...S.form };
    S.palTo = moodPalette(key);
    S.formTo = { ...MOODS[key].form };
    S.blendT = S.time;
    if (reduced.matches) {
      S.pal = S.palTo; S.form = S.formTo; S.blendT = -10;
      renderStaticAll();
    }
  }

  // ——— Звук: превью MP3 через WebAudio ———
  const audio = { ctx: null, analyser: null, gain: null, src: null, playing: false, startAt: 0, offset: 0, bpm: 120,
    url: null, td: null, fd: null, prevFd: null, flux: [], lastOnset: 0, peak: 0.12, cache: new Map() };

  function ensureAudio() {
    if (audio.ctx) return audio.ctx;
    const AC = window.AudioContext || window.webkitAudioContext;
    audio.ctx = new AC();
    audio.analyser = audio.ctx.createAnalyser();
    audio.analyser.fftSize = 2048;
    audio.analyser.smoothingTimeConstant = 0.6;
    audio.gain = audio.ctx.createGain();
    audio.gain.gain.value = 0.8;
    audio.analyser.connect(audio.gain).connect(audio.ctx.destination);
    audio.td = new Float32Array(audio.analyser.fftSize);
    audio.fd = new Uint8Array(audio.analyser.frequencyBinCount);
    audio.prevFd = new Uint8Array(audio.analyser.frequencyBinCount);
    return audio.ctx;
  }

  // Сетка битов по превью: огибающая атак → автокорреляция → темп и фаза.
  // В приложении сетку даёт сервер (Beat This!), здесь — честная замена на 30 секундах.
  function analyzeTempo(buf) {
    const sr = buf.sampleRate;
    const a = buf.getChannelData(0);
    const b = buf.numberOfChannels > 1 ? buf.getChannelData(1) : a;
    const n = buf.length;
    const fps = 100;
    const hop = Math.round(sr / fps);
    const win = hop * 2;
    const frames = Math.max(0, Math.floor((n - win) / hop));
    const env = new Float32Array(frames);
    const k = 1 - Math.exp((-2 * Math.PI * 150) / sr);
    let y = 0, prevL = 0, prevF = 0;
    const low = new Float32Array(n);
    for (let i = 0; i < n; i++) { y += k * ((a[i] + b[i]) * 0.5 - y); low[i] = y; }
    for (let f = 0; f < frames; f++) {
      let eL = 0, eF = 0;
      const s = f * hop;
      for (let i = s; i < s + win; i++) { const l = low[i]; eL += l * l; const x = (a[i] + b[i]) * 0.5; eF += x * x; }
      const lL = Math.log(1e-6 + eL), lF = Math.log(1e-6 + eF);
      env[f] = Math.max(0, lL - prevL) * 0.6 + Math.max(0, lF - prevF) * 0.4;
      prevL = lL; prevF = lF;
    }
    // Убираем медленный тренд, чтобы считать только атаки
    const avgWin = 25;
    const det = new Float32Array(frames);
    let acc = 0;
    for (let f = 0; f < frames; f++) {
      acc += env[f];
      if (f >= avgWin) acc -= env[f - avgWin];
      det[f] = Math.max(0, env[f] - acc / Math.min(f + 1, avgWin));
    }
    const minL = Math.floor((60 * fps) / 180), maxL = Math.ceil((60 * fps) / 68);
    const scores = [];
    for (let L = minL; L <= maxL; L++) {
      let s = 0;
      for (let i = 0; i + L < frames; i++) s += det[i] * det[i + L];
      s /= Math.max(1, frames - L);
      const bpm = (60 * fps) / L;
      const w = Math.exp(-0.5 * Math.pow(Math.log2(bpm / 120) / 0.85, 2));
      scores.push({ L, s: s * w });
    }
    let bi = 0;
    for (let i = 1; i < scores.length; i++) if (scores[i].s > scores[bi].s) bi = i;
    let L = scores[bi]?.L || 50;
    if (bi > 0 && bi < scores.length - 1) {
      const s0 = scores[bi - 1].s, s1 = scores[bi].s, s2 = scores[bi + 1].s;
      const d = s0 - 2 * s1 + s2;
      if (d !== 0) L += clamp((0.5 * (s0 - s2)) / d, -0.5, 0.5);
    }
    let bestO = 0, bestS = -1;
    for (let o = 0; o < Math.ceil(L); o++) {
      let s = 0;
      for (let m = 0; ; m++) { const idx = Math.round(o + m * L); if (idx >= frames) break; s += det[idx]; }
      if (s > bestS) { bestS = s; bestO = o; }
    }
    return { bpm: Math.round(((60 * fps) / L) * 10) / 10, offset: bestO / fps };
  }

  async function loadPreview(url) {
    if (audio.cache.has(url)) return audio.cache.get(url);
    const ctx = ensureAudio();
    const res = await fetch(url);
    if (!res.ok) throw new Error(`превью не загрузилось: ${res.status}`);
    const buf = await ctx.decodeAudioData(await res.arrayBuffer());
    const entry = { buf, tempo: analyzeTempo(buf) };
    audio.cache.set(url, entry);
    return entry;
  }

  async function playPreview(url) {
    const ctx = ensureAudio();
    if (ctx.state === 'suspended') await ctx.resume();
    stopPreview();
    const { buf, tempo } = await loadPreview(url);
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.connect(audio.analyser);
    // Событие — только про естественный конец превью; ручная остановка его не шлёт,
    // иначе приложение решило бы, что трек доиграл, и пропустило следующий
    src.onended = () => {
      if (audio.src !== src) return;
      audio.playing = false; audio.src = null; audio.url = null;
      emit('previewend', { url });
    };
    src.start();
    audio.src = src; audio.url = url; audio.playing = true;
    audio.startAt = ctx.currentTime; audio.bpm = tempo.bpm; audio.offset = tempo.offset; audio.peak = 0.12;
    S.bpm = tempo.bpm; S.playing = true; S.lastBeat = -1;
    emit('analysis', { url, ...tempo });
    return tempo;
  }

  function stopPreview() {
    if (audio.src) {
      const src = audio.src;
      audio.src = null;
      try { src.stop(); } catch { /* уже остановлен */ }
    }
    audio.playing = false; audio.url = null;
  }

  function previewTime() {
    return audio.playing ? audio.ctx.currentTime - audio.startAt : 0;
  }

  function readAudio(dt) {
    const an = audio.analyser;
    an.getFloatTimeDomainData(audio.td);
    let sum = 0;
    for (let i = 0; i < audio.td.length; i++) sum += audio.td[i] * audio.td[i];
    const rms = Math.sqrt(sum / audio.td.length);
    audio.peak = Math.max(rms, audio.peak * Math.exp(-dt * 0.25));
    an.getByteFrequencyData(audio.fd);
    // Спектральный поток по низам и середине — признак атаки
    let flux = 0, lo = 0, mi = 0, hi = 0;
    const N = audio.fd.length;
    for (let i = 1; i < N; i++) {
      const v = audio.fd[i];
      const d = v - audio.prevFd[i];
      if (d > 0 && i < 260) flux += d;
      if (i < 12) lo += v; else if (i < 120) mi += v; else if (i < 600) hi += v;
    }
    audio.prevFd.set(audio.fd);
    const h = audio.flux;
    h.push(flux);
    if (h.length > 43) h.shift();
    const mean = h.reduce((x, y) => x + y, 0) / h.length;
    const sd = Math.sqrt(h.reduce((x, y) => x + (y - mean) * (y - mean), 0) / h.length);
    if (flux > mean + 1.4 * sd && flux > 400 && S.time - audio.lastOnset > 0.12) {
      audio.lastOnset = S.time;
      S.onset = 1;
      emit('onset', { strength: clamp((flux - mean) / (sd * 4 + 1)) });
    }
    S.bands = [lo / (12 * 255), mi / (108 * 255), hi / (480 * 255)];
    return clamp(Math.pow(rms / Math.max(audio.peak, 0.08), 1.5) * 0.85 + rms * 0.8);
  }

  // ——— Обновление состояния раз в кадр ———
  function update(dt) {
    S.time += dt;
    S.flow += dt * S.form.speed;

    if (audio.playing) {
      S.beatPos = (previewTime() - audio.offset) * (audio.bpm / 60);
    } else if (S.playing) {
      S.beatPos += dt * (S.bpm / 60);
    }
    S.playAmt += ((S.playing ? 1 : 0) - S.playAmt) * Math.min(1, dt * 3);
    const bi = Math.floor(S.beatPos);
    const frac = S.beatPos - bi;
    if (S.playing && bi !== S.lastBeat && S.beatPos >= 0) {
      S.lastBeat = bi;
      emit('beat', { index: bi, downbeat: ((bi % 4) + 4) % 4 === 0 });
    }
    const down = ((bi % 4) + 4) % 4 === 0;
    S.kick = S.beatPos >= 0 ? Math.exp(-frac * 6.5) * (down ? 1 : 0.62) * S.playAmt : 0;
    S.bar = (((S.beatPos / 4) % 1) + 1) % 1;
    S.ringK = down ? frac : 1;

    const base = MOODS[S.mood].energy;
    let target = S.manualEnergy != null ? S.manualEnergy : S.playing ? base : base * 0.45;
    if (audio.playing) target = mix(target, readAudio(dt), 0.75);
    else S.bands = S.bands.map((v) => v * Math.exp(-dt * 4));
    S.energy += (target - S.energy) * Math.min(1, dt * 4);
    S.onset *= Math.exp(-dt * 7);

    if (S.expandT >= 0) {
      const k = (S.time - S.expandT) / 1.5;
      S.expand = k >= 1 ? 1 : clamp(easeOutBack(clamp(k)), 0, 1.2);
      if (k >= 1) S.expandT = -1;
    }
    S.ripple = clamp((S.time - S.rippleT) / 1.3);
    const lt = S.time - S.likeT;
    S.flash = lt < 1.4 ? Math.exp(-lt * 3.2) * clamp(lt * 14) : 0;

    if (S.blendT >= 0) {
      const k = clamp((S.time - S.blendT) / 1.2);
      const e = easeInOut(k);
      for (const key of PAL_KEYS) S.pal[key] = mix3(S.palFrom[key], S.palTo[key], e);
      for (const key of FORM_KEYS) S.form[key] = mix(S.formFrom[key], S.formTo[key], e);
      if (k >= 1) S.blendT = -10;
    }
    emit('frame', S);
  }

  // ——— Шейдеры ———
  // Симплекс-шум 3D: Ashima Arts / Stefan Gustavson, MIT
  const NOISE = `
vec3 mod289(vec3 x){return x-floor(x*(1.0/289.0))*289.0;}
vec4 mod289(vec4 x){return x-floor(x*(1.0/289.0))*289.0;}
vec4 permute(vec4 x){return mod289(((x*34.0)+1.0)*x);}
vec4 taylorInvSqrt(vec4 r){return 1.79284291400159-0.85373472095314*r;}
float snoise(vec3 v){
  const vec2 C=vec2(1.0/6.0,1.0/3.0);const vec4 D=vec4(0.0,0.5,1.0,2.0);
  vec3 i=floor(v+dot(v,C.yyy));vec3 x0=v-i+dot(i,C.xxx);
  vec3 g=step(x0.yzx,x0.xyz);vec3 l=1.0-g;vec3 i1=min(g.xyz,l.zxy);vec3 i2=max(g.xyz,l.zxy);
  vec3 x1=x0-i1+C.xxx;vec3 x2=x0-i2+C.yyy;vec3 x3=x0-D.yyy;
  i=mod289(i);
  vec4 p=permute(permute(permute(i.z+vec4(0.0,i1.z,i2.z,1.0))+i.y+vec4(0.0,i1.y,i2.y,1.0))+i.x+vec4(0.0,i1.x,i2.x,1.0));
  float n_=0.142857142857;vec3 ns=n_*D.wyz-D.xzx;
  vec4 j=p-49.0*floor(p*ns.z*ns.z);vec4 x_=floor(j*ns.z);vec4 y_=floor(j-7.0*x_);
  vec4 x=x_*ns.x+ns.yyyy;vec4 y=y_*ns.x+ns.yyyy;vec4 h=1.0-abs(x)-abs(y);
  vec4 b0=vec4(x.xy,y.xy);vec4 b1=vec4(x.zw,y.zw);
  vec4 s0=floor(b0)*2.0+1.0;vec4 s1=floor(b1)*2.0+1.0;vec4 sh=-step(h,vec4(0.0));
  vec4 a0=b0.xzyw+s0.xzyw*sh.xxyy;vec4 a1=b1.xzyw+s1.xzyw*sh.zzww;
  vec3 p0=vec3(a0.xy,h.x);vec3 p1=vec3(a0.zw,h.y);vec3 p2=vec3(a1.xy,h.z);vec3 p3=vec3(a1.zw,h.w);
  vec4 norm=taylorInvSqrt(vec4(dot(p0,p0),dot(p1,p1),dot(p2,p2),dot(p3,p3)));
  p0*=norm.x;p1*=norm.y;p2*=norm.z;p3*=norm.w;
  vec4 m=max(0.6-vec4(dot(x0,x0),dot(x1,x1),dot(x2,x2),dot(x3,x3)),0.0);m=m*m;
  return 42.0*dot(m*m,vec4(dot(p0,x0),dot(p1,x1),dot(p2,x2),dot(p3,x3)));
}`;

  const POINT_VS = `
precision highp float;
attribute vec4 aS;   // точка на сфере; w — широта 0..1 сверху вниз
attribute vec4 aT;   // точка на торе; w — случайное 0..1
attribute vec3 aM;   // x — слой (0 оболочка, 1 туман, 2 пыль), y — случайное, z — размер в css px
uniform mat3 uRot;
uniform vec2 uRes, uCenter;
uniform float uRadius, uDpr, uSize;
uniform float uTime, uKick, uBar, uEnergy, uOnset, uPlay, uExpand, uRipple, uFlash;
uniform float uTorus, uSquash, uSpike, uLobe, uWobble;
uniform vec3 uRipDir, uBands;
uniform vec3 uDeep, uMid, uHi, uAcc;
varying vec3 vCol;
varying float vA;
${NOISE}
void main(){
  float layer = aM.x;
  float shell = 1.0 - step(0.5, layer);
  float dust = step(1.5, layer);
  vec3 ps = aS.xyz;
  vec3 pt = aT.xyz;
  vec3 p = mix(ps, pt, uTorus);
  vec3 ringC = normalize(vec3(pt.x, 0.0, pt.z) + 1e-5) * 0.74;
  vec3 n = normalize(mix(normalize(ps + 1e-5), normalize(pt - ringC + 1e-5), uTorus));

  float t = uTime;
  float nz = snoise(p * 1.55 + vec3(0.0, t * 0.21, t * 0.15));
  float nd = snoise(p * 4.1 + vec3(t * 0.37, 0.0, -t * 0.29));
  float lon = atan(ps.z, ps.x);
  float latS = aS.w;
  float lonN = atan(pt.z, pt.x) / 6.28318 + 0.5;

  float amp = (0.035 + 0.13 * uEnergy + uBands.y * 0.05) * uWobble;
  float disp = nz * amp;
  disp += max(nd, 0.0) * uSpike * (0.025 + 0.11 * uEnergy);
  disp += nd * uOnset * 0.045;
  disp += uLobe * 0.11 * sin(3.0 * lon + t * 0.8) * pow(sin(latS * 3.14159), 2.0) * (0.6 + uEnergy);
  disp += uKick * (0.02 + 0.07 * uEnergy);
  disp += (1.0 - uPlay) * 0.014 * sin(t * 1.1);

  // Полоса импульса: на сфере идёт сверху вниз, на торе бежит по кругу — один раз за такт
  float bc = mix(latS, lonN, uTorus);
  float bd = abs(fract(bc - uBar + 0.5) - 0.5);
  bd = mix(abs(latS - uBar), bd, uTorus);
  float band = exp(-pow(bd * 11.0, 2.0)) * uPlay * shell;
  disp += band * (0.025 + 0.06 * uEnergy);

  // Рябь пропуска
  float dd = distance(normalize(ps + 1e-5), uRipDir);
  float fr = uRipple * 2.7;
  float rip = sin((dd - fr) * 15.0) * exp(-pow((dd - fr) * 3.0, 2.0)) * (1.0 - uRipple);
  disp += rip * 0.11 * (1.0 - dust);

  p += n * disp * mix(1.0, 0.45, step(0.5, layer) - dust) * (1.0 + dust * 1.6);
  p *= 1.0 + dust * (uKick * 0.06 + uOnset * 0.08 + uEnergy * 0.06);
  p.y *= 1.0 - 0.15 * uSquash;
  p.y -= 0.05 * uSquash * (1.0 - p.y * p.y);
  float ex = clamp(uExpand, 0.0, 1.2);
  p = p * mix(0.18, 1.0, ex) + n * (aM.y - 0.5) * (1.0 - min(ex, 1.0)) * 0.5;

  vec3 pr = uRot * p;
  vec3 nr = uRot * n;
  float persp = 4.4;
  float s = persp / (persp - pr.z);
  vec2 xy = uCenter + vec2(pr.x, -pr.y) * uRadius * s;
  vec2 clip = xy / uRes * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);

  float depth = clamp(pr.z * 0.5 + 0.5, 0.0, 1.0);
  float sz = aM.z * uSize * s * (1.0 + uKick * 0.3 * shell + band * 1.1 + uFlash * 0.5 + uOnset * 0.25 * dust);
  gl_PointSize = max(sz * uDpr, 1.0);

  vec3 L = normalize(vec3(-0.45, 0.62, 0.64));
  float lit = clamp(dot(nr, L), 0.0, 1.0);
  vec3 c = mix(uDeep * 2.2 + 0.02, uMid, smoothstep(-0.25, 0.85, lit + nz * 0.35));
  c = mix(c, uHi, pow(lit, 3.5) * 0.8);
  float rim = pow(1.0 - abs(nr.z), 2.5);
  c += uMid * rim * 0.32;
  float spark = step(0.955, aT.w) * shell;
  float tw = 0.5 + 0.5 * sin(t * 2.3 + aM.y * 31.0);
  c = mix(c, uAcc, spark * (0.4 + 0.35 * uKick + 0.25 * tw));
  c = mix(c, uAcc, band * 0.88);
  c = mix(c, mix(uAcc, vec3(1.0), 0.3), uFlash * 0.72 * shell);

  float a = mix(0.13, 1.0, pow(depth, 1.25));
  if (layer > 0.5 && layer < 1.5) { a *= 0.32; c = mix(c, uMid, 0.55); }
  if (dust > 0.5) { a *= (0.35 + 0.65 * tw) * (0.45 + 0.55 * uEnergy); c = mix(uHi, uAcc, step(0.7, aM.y) * 0.8); }
  vCol = c;
  vA = a * clamp(0.25 + 0.75 * ex, 0.0, 1.0);
}`;

  const POINT_FS = `
precision mediump float;
varying vec3 vCol;
varying float vA;
void main(){
  vec2 d = gl_PointCoord * 2.0 - 1.0;
  float r2 = dot(d, d);
  if (r2 > 1.0) discard;
  float a = smoothstep(1.0, 0.3, r2) * vA;
  gl_FragColor = vec4(vCol * a, a);
}`;

  const GLOW_VS = `
attribute vec2 aP;
varying vec2 vUv;
void main(){ vUv = aP * 0.5 + 0.5; gl_Position = vec4(aP, 0.0, 1.0); }`;

  const GLOW_FS = `
precision mediump float;
uniform vec2 uRes, uCenter;
uniform float uRadius, uEnergy, uKick, uFlash, uRing, uPlay, uTorus;
uniform vec3 uDeep, uMid, uHi, uAcc;
varying vec2 vUv;
float hash(vec2 p){ return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
void main(){
  vec2 px = vec2(vUv.x, 1.0 - vUv.y) * uRes;
  vec2 dv = px - uCenter;
  float d = length(dv) / uRadius;
  // Подсветка гаснет к краям холста — никаких видимых прямоугольников
  vec2 ed = min(vUv, 1.0 - vUv) * uRes;
  float fz = min(uRes.x, uRes.y) * 0.22;
  float edge = smoothstep(0.0, fz, ed.x) * smoothstep(0.0, fz, ed.y);
  float g1 = exp(-d * d * 0.85) * (0.2 + 0.18 * uEnergy + 0.08 * uKick);
  float g2 = exp(-d * d * 3.2) * (0.07 + 0.08 * uKick) * (1.0 - uTorus * 0.6);
  vec3 c = (uMid * g1 + uHi * g2 * 0.55 + uDeep * exp(-d * d * 0.45) * 0.3) * edge;
  // Кольцо импульса на сильной доле: тонкий пунктир из точек, как сама волна
  float rr = 1.05 + uRing * 0.72;
  float ang = atan(dv.y, dv.x);
  float dots = smoothstep(0.55, 0.95, 0.5 + 0.5 * cos(ang * 160.0));
  float ring = exp(-pow((d - rr) * uRadius / 2.2, 2.0)) * dots * pow(1.0 - uRing, 1.6) * uPlay * 0.7;
  c += uAcc * ring;
  c += uAcc * uFlash * exp(-d * d * 1.2) * 0.14 * edge;
  c += (hash(px) - 0.5) * (2.0 / 255.0);
  c = max(c, 0.0);
  float a = clamp(max(max(c.r, c.g), c.b), 0.0, 1.0);
  gl_FragColor = vec4(min(c, vec3(a)), a);
}`;

  function compile(gl, type, src) {
    const sh = gl.createShader(type);
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(sh);
      gl.deleteShader(sh);
      throw new Error('шейдер: ' + log);
    }
    return sh;
  }

  function program(gl, vs, fs) {
    const p = gl.createProgram();
    gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vs));
    gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error('программа: ' + gl.getProgramInfoLog(p));
    const u = {};
    const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
    for (let i = 0; i < n; i++) {
      const info = gl.getActiveUniform(p, i);
      u[info.name] = gl.getUniformLocation(p, info.name);
    }
    return { p, u };
  }

  // Детерминированный генератор: одинаковая картинка при каждом запуске
  function rng(seed) {
    let s = seed >>> 0;
    return () => {
      s = (s + 0x6d2b79f5) >>> 0;
      let t = s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // Кольца точек: на сфере — параллели, на торе — те же кольца, свёрнутые в трубку
  function buildParticles(rings) {
    const R = 0.74, r = 0.24;
    const rand = rng(20261111);
    const out = [];
    const push = (s, lat, t, rnd, layer, rnd2, size) => out.push(s[0], s[1], s[2], lat, t[0], t[1], t[2], rnd, layer, rnd2, size);
    let shellCount = 0;
    for (let i = 0; i < rings; i++) {
      const v = (i + 0.5) / rings;
      const lat = v * Math.PI;
      const sl = Math.sin(lat), cl = Math.cos(lat);
      const count = Math.max(6, Math.round(rings * 2 * sl));
      const off = (i % 2) * 0.5;
      const tv = (v + 0.5) * Math.PI * 2;
      for (let j = 0; j < count; j++) {
        const lon = ((j + off) / count) * Math.PI * 2;
        const s = [sl * Math.cos(lon), cl, sl * Math.sin(lon)];
        const t = [(R + r * Math.cos(tv)) * Math.cos(lon), r * Math.sin(tv), (R + r * Math.cos(tv)) * Math.sin(lon)];
        push(s, v, t, rand(), 0, rand(), 2.1);
        shellCount++;
      }
    }
    const randDir = () => {
      const z = rand() * 2 - 1, a = rand() * Math.PI * 2, q = Math.sqrt(1 - z * z);
      return [q * Math.cos(a), z, q * Math.sin(a)];
    };
    const fog = Math.round(shellCount * 0.22);
    for (let k = 0; k < fog; k++) {
      const d = randDir();
      const rad = 0.3 + 0.62 * Math.cbrt(rand());
      const u = rand() * Math.PI * 2, tv = rand() * Math.PI * 2, rr = r * Math.sqrt(rand()) * 0.95;
      const t = [(R + rr * Math.cos(tv)) * Math.cos(u), rr * Math.sin(tv), (R + rr * Math.cos(tv)) * Math.sin(u)];
      push([d[0] * rad, d[1] * rad, d[2] * rad], Math.acos(d[1]) / Math.PI, t, rand(), 1, rand(), 1.5);
    }
    const dust = Math.round(shellCount * 0.07);
    for (let k = 0; k < dust; k++) {
      const d = randDir();
      const rad = 1.16 + 0.5 * rand();
      const u = rand() * Math.PI * 2, tv = rand() * Math.PI * 2, rr = r * (1.5 + rand() * 1.4);
      const t = [(R + rr * Math.cos(tv)) * Math.cos(u), rr * Math.sin(tv) * 0.7, (R + rr * Math.cos(tv)) * Math.sin(u)];
      push([d[0] * rad, d[1] * rad, d[2] * rad], Math.acos(d[1]) / Math.PI, t, rand(), 2, rand(), 1.7);
    }
    return { data: new Float32Array(out), count: out.length / 11 };
  }

  function rotation(tilt, spin, roll) {
    const cx = Math.cos(tilt), sx = Math.sin(tilt);
    const cy = Math.cos(spin), sy = Math.sin(spin);
    const cz = Math.cos(roll), sz = Math.sin(roll);
    // R = Rz · Rx · Ry, по столбцам для WebGL
    const ry = [cy, 0, -sy, 0, 1, 0, sy, 0, cy];
    const rx = [1, 0, 0, 0, cx, sx, 0, -sx, cx];
    const rz = [cz, sz, 0, -sz, cz, 0, 0, 0, 1];
    const mul = (a, b) => {
      const o = new Array(9);
      for (let c = 0; c < 3; c++) for (let rr = 0; rr < 3; rr++)
        o[c * 3 + rr] = a[rr] * b[c * 3] + a[3 + rr] * b[c * 3 + 1] + a[6 + rr] * b[c * 3 + 2];
      return o;
    };
    return new Float32Array(mul(rz, mul(rx, ry)));
  }

  // ——— Холст ———
  const views = new Set();

  class View {
    constructor(canvas, opts = {}) {
      this.canvas = canvas;
      this.layout = opts.layout || canvas.dataset.layout || 'sphere';
      this.visible = false;
      this.lastDraw = 0;
      this.ok = false;
      const gl = canvas.getContext('webgl2', { alpha: true, antialias: false, premultipliedAlpha: true })
        || canvas.getContext('webgl', { alpha: true, antialias: false, premultipliedAlpha: true });
      if (!gl) { canvas.classList.add('wave-fallback'); return; }
      this.gl = gl;
      try {
        this.pts = program(gl, POINT_VS, POINT_FS);
        this.glow = program(gl, GLOW_VS, GLOW_FS);
      } catch (e) {
        console.error(e);
        canvas.classList.add('wave-fallback');
        return;
      }
      this.quad = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
      this.buf = gl.createBuffer();
      this.rings = 0;
      this.ok = true;
      canvas.addEventListener('webglcontextlost', (e) => { e.preventDefault(); this.ok = false; });
      this.ro = new ResizeObserver(() => this.resize());
      this.ro.observe(canvas);
      this.io = new IntersectionObserver((es) => {
        this.visible = es[es.length - 1].isIntersecting;
        if (this.visible && reduced.matches) this.draw();
      });
      this.io.observe(canvas);
      this.resize();
    }

    resize() {
      if (!this.ok) return;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const w = Math.max(1, this.canvas.clientWidth), h = Math.max(1, this.canvas.clientHeight);
      this.w = w; this.h = h; this.dpr = dpr;
      this.canvas.width = Math.round(w * dpr);
      this.canvas.height = Math.round(h * dpr);
      const cs = getComputedStyle(this.canvas);
      const num = (name, d) => { const v = parseFloat(cs.getPropertyValue(name)); return Number.isFinite(v) ? v : d; };
      this.cx = num('--wave-cx', 0.5) * w;
      this.cy = num('--wave-cy', 0.5) * h;
      this.radius = num('--wave-r', 0.34) * Math.min(w, h);
      // Телефон и маленькие холсты — 30 кадров в секунду и меньше точек
      this.interval = w < 700 ? 1000 / 30 : 1000 / 60;
      const rings = Math.round(clamp((Math.PI * this.radius) / 8.6, 34, 72));
      if (rings !== this.rings) {
        this.rings = rings;
        const { data, count } = buildParticles(rings);
        this.count = count;
        const gl = this.gl;
        gl.bindBuffer(gl.ARRAY_BUFFER, this.buf);
        gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
      }
      if (reduced.matches || !running) this.draw();
    }

    draw() {
      if (!this.ok || !this.count) return;
      S.draws++;
      const gl = this.gl;
      const W = this.canvas.width, H = this.canvas.height, dpr = this.dpr;
      gl.viewport(0, 0, W, H);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);

      const ring = this.layout === 'ring';
      const torus = ring ? 1 : S.form.torus;
      const center = [this.cx * dpr, this.cy * dpr];
      const radius = this.radius * dpr;
      const pal = S.pal;

      // Подсветка и кольцо импульса
      const g = this.glow;
      gl.useProgram(g.p);
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
      const ap = gl.getAttribLocation(g.p, 'aP');
      gl.enableVertexAttribArray(ap);
      gl.vertexAttribPointer(ap, 2, gl.FLOAT, false, 0, 0);
      gl.uniform2f(g.u.uRes, W, H);
      gl.uniform2f(g.u.uCenter, center[0], center[1]);
      gl.uniform1f(g.u.uRadius, radius);
      gl.uniform1f(g.u.uEnergy, S.energy);
      gl.uniform1f(g.u.uKick, S.kick);
      gl.uniform1f(g.u.uFlash, S.flash);
      gl.uniform1f(g.u.uRing, S.ringK);
      gl.uniform1f(g.u.uPlay, S.playAmt);
      gl.uniform1f(g.u.uTorus, torus);
      gl.uniform3fv(g.u.uDeep, pal.deep);
      gl.uniform3fv(g.u.uMid, pal.mid);
      gl.uniform3fv(g.u.uHi, pal.hi);
      gl.uniform3fv(g.u.uAcc, ACCENT);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      gl.disableVertexAttribArray(ap);

      // Точки — сложением, чтобы пересечения светились
      const p = this.pts;
      gl.useProgram(p.p);
      gl.blendFunc(gl.ONE, gl.ONE);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.buf);
      const stride = 11 * 4;
      const locs = [['aS', 4, 0], ['aT', 4, 16], ['aM', 3, 32]].map(([name, size, off]) => {
        const l = gl.getAttribLocation(p.p, name);
        gl.enableVertexAttribArray(l);
        gl.vertexAttribPointer(l, size, gl.FLOAT, false, stride, off);
        return l;
      });
      const tilt = ring ? 1.2 + 0.05 * Math.sin(S.time * 0.21) : mix(0.36, 1.02, torus);
      const spin = S.time * (ring ? 0.16 : 0.07) * mix(0.8, 1.25, S.form.speed - 0.5) + 0.6;
      const roll = ring ? 0.06 * Math.sin(S.time * 0.17) : 0.05 * Math.sin(S.time * 0.13) - 0.08;
      gl.uniformMatrix3fv(p.u.uRot, false, rotation(tilt, spin, roll));
      gl.uniform2f(p.u.uRes, W, H);
      gl.uniform2f(p.u.uCenter, center[0], center[1]);
      gl.uniform1f(p.u.uRadius, radius / 1.12);
      gl.uniform1f(p.u.uDpr, dpr);
      gl.uniform1f(p.u.uSize, clamp(this.radius / 190, 0.8, 1.3));
      gl.uniform1f(p.u.uTime, S.flow);
      gl.uniform1f(p.u.uKick, S.kick);
      gl.uniform1f(p.u.uBar, S.bar);
      gl.uniform1f(p.u.uEnergy, S.energy);
      gl.uniform1f(p.u.uOnset, S.onset);
      gl.uniform1f(p.u.uPlay, S.playAmt);
      gl.uniform1f(p.u.uExpand, S.expand);
      gl.uniform1f(p.u.uRipple, S.ripple);
      gl.uniform1f(p.u.uFlash, S.flash);
      gl.uniform1f(p.u.uTorus, torus);
      gl.uniform1f(p.u.uSquash, ring ? 0 : S.form.squash);
      gl.uniform1f(p.u.uSpike, S.form.spike);
      gl.uniform1f(p.u.uLobe, ring ? 0 : S.form.lobe);
      gl.uniform1f(p.u.uWobble, S.form.wobble * (ring ? 0.6 : 1));
      gl.uniform3fv(p.u.uRipDir, S.rippleDir);
      gl.uniform3fv(p.u.uBands, S.bands);
      gl.uniform3fv(p.u.uDeep, pal.deep);
      gl.uniform3fv(p.u.uMid, pal.mid);
      gl.uniform3fv(p.u.uHi, pal.hi);
      gl.uniform3fv(p.u.uAcc, ACCENT);
      gl.drawArrays(gl.POINTS, 0, this.count);
      locs.forEach((l) => gl.disableVertexAttribArray(l));
    }

    destroy() {
      this.ro?.disconnect();
      this.io?.disconnect();
      views.delete(this);
      this.gl?.getExtension('WEBGL_lose_context')?.loseContext();
      this.ok = false;
    }
  }

  // ——— Цикл ———
  let running = false;
  let raf = 0;

  function frame(now) {
    raf = requestAnimationFrame(frame);
    const dt = Math.min(0.1, (now - (S.lastNow || now)) / 1000);
    S.lastNow = now;
    update(dt);
    for (const v of views) {
      if (!v.visible || !v.ok) continue;
      if (now - v.lastDraw >= v.interval - 2) { v.lastDraw = now; v.draw(); }
    }
  }

  function startLoop() {
    if (running || reduced.matches || document.hidden) return;
    running = true;
    S.lastNow = 0;
    raf = requestAnimationFrame(frame);
  }

  function stopLoop() {
    running = false;
    cancelAnimationFrame(raf);
  }

  // Статичный кадр для «уменьшения движения»: выразительная, но неподвижная поза
  function renderStaticAll() {
    const keep = { time: S.time, flow: S.flow, kick: S.kick, bar: S.bar, ringK: S.ringK, playAmt: S.playAmt };
    S.time = 7.3; S.flow = 7.3; S.kick = 0.25; S.bar = 0.32; S.ringK = 1; S.playAmt = S.playing ? 1 : 0.4;
    if (S.blendT >= 0) { S.pal = S.palTo; S.form = S.formTo; S.blendT = -10; }
    views.forEach((v) => v.visible && v.draw());
    Object.assign(S, keep);
  }

  document.addEventListener('visibilitychange', () => (document.hidden ? stopLoop() : startLoop()));
  reduced.addEventListener?.('change', () => (reduced.matches ? (stopLoop(), renderStaticAll()) : startLoop()));

  // ——— Цвет обложки ———
  function coverColors(img) {
    const c = document.createElement('canvas');
    c.width = c.height = 24;
    const x = c.getContext('2d', { willReadFrequently: true });
    x.drawImage(img, 0, 0, 24, 24);
    const d = x.getImageData(0, 0, 24, 24).data;
    const vivid = [];
    let dark = [0, 0, 0], dn = 0;
    for (let i = 0; i < d.length; i += 4) {
      const r = d[i] / 255, g = d[i + 1] / 255, b = d[i + 2] / 255;
      const max = Math.max(r, g, b), min = Math.min(r, g, b);
      const sat = max === 0 ? 0 : (max - min) / max;
      vivid.push({ c: [r, g, b], score: sat * sat * max * (max > 0.25 ? 1 : 0.2) });
      if (max < 0.4) { dark = [dark[0] + r, dark[1] + g, dark[2] + b]; dn++; }
    }
    vivid.sort((a, b) => b.score - a.score);
    const top = vivid.slice(0, 40);
    const avg = top.reduce((acc, v) => [acc[0] + v.c[0], acc[1] + v.c[1], acc[2] + v.c[2]], [0, 0, 0]).map((v) => v / top.length);
    const m = Math.max(...avg, 0.01);
    return {
      vivid: avg.map((v) => clamp(v / m)),
      dark: dn ? dark.map((v) => (v / dn) * 0.8) : [0.03, 0.03, 0.05],
    };
  }

  // ——— Наружу ———
  const Wave = {
    MOODS,
    ACCENT: '#cdff3f',
    get state() { return S; },
    get audio() { return { playing: audio.playing, url: audio.url, bpm: audio.bpm, time: previewTime() }; },
    mount(canvas, opts) {
      if (canvas.__wave) return canvas.__wave;
      const v = new View(canvas, opts);
      canvas.__wave = v;
      if (v.ok) views.add(v);
      startLoop();
      return v;
    },
    unmountAll() { [...views].forEach((v) => { v.canvas.__wave = null; v.destroy(); }); },
    refresh() { views.forEach((v) => v.resize()); },
    setMood(key) {
      if (!MOODS[key] || key === S.mood) return;
      S.mood = key;
      blendTo(key);
      emit('mood', key);
    },
    setEnergy(v) { S.manualEnergy = v == null ? null : clamp(v); if (reduced.matches) renderStaticAll(); },
    setBpm(bpm) { S.bpm = clamp(bpm, 40, 220); },
    setPlaying(on) {
      S.playing = !!on;
      if (on && S.beatPos < 0) S.beatPos = 0;
      if (reduced.matches) renderStaticAll();
    },
    start() {
      S.expandT = S.time; S.expand = 0;
      S.likeT = S.time - 0.35;
      if (reduced.matches) { S.expand = 1; S.expandT = -1; renderStaticAll(); }
    },
    skip() {
      const a = Math.random() * Math.PI * 2;
      S.rippleDir = [Math.cos(a) * 0.62, Math.sin(a) * 0.5, 0.6];
      const l = Math.hypot(...S.rippleDir);
      S.rippleDir = S.rippleDir.map((v) => v / l);
      S.rippleT = S.time;
    },
    like() { S.likeT = S.time; },
    setCover(url) {
      if (!url) { S.cover = null; blendTo(S.mood); return; }
      const img = new Image();
      img.onload = () => {
        try { S.cover = coverColors(img); } catch { S.cover = null; }
        blendTo(S.mood);
      };
      img.src = url;
    },
    playPreview,
    stopPreview,
    analyzeTempo,
    on(name, fn) { (listeners[name] ||= []).push(fn); return () => { listeners[name] = listeners[name].filter((f) => f !== fn); }; },
  };

  window.Wave = Wave;
})();
