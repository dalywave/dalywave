// Шейдер волны «Полночь»: гребень ловит свет, под ним — толща с течением и бликами,
// над ним — короткое сияние. При «любом настроении» вдоль волны идёт спектр «Сумерек».
// GLSL ES 1.00 — работает и в WebGL2, и в WebGL1.

export const VERT = 'attribute vec2 aPos;void main(){gl_Position=vec4(aPos,0.0,1.0);}'

export const FRAG = `
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
uniform vec3 uS0;
uniform vec3 uS1;
uniform vec3 uS2;
uniform vec3 uS3;
uniform float uSpectrum;
uniform vec4 uLayout;
uniform vec2 uFade;
uniform float uFreq;
uniform vec4 uBox;
uniform float uBoxR;
// 1 — светлая тема: цвета C1..C3 и спектр — поглощение краски, uC0 — бумага
uniform float uLight;

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
// Блики в толще: тонкие линии там, где два искривлённых поля проходят через ноль
float caustic(vec2 p, float t) {
  vec2 a = p * vec2(9.0, 5.0);
  float f = sin(a.x + 1.7 * sin(a.y * 0.8 + t) + t * 0.6) + sin(a.y * 1.3 + 1.9 * sin(a.x * 0.7 - t * 0.8) - t * 0.4);
  float lines = 1.0 - smoothstep(0.0, 0.32, abs(f));
  return lines * lines;
}
// Спектр «Сумерек»: фиолетовый → пурпурный → розовый → оранжевый
vec3 spectrum(float k) {
  k = clamp(k, 0.0, 1.0);
  vec3 c = mix(uS0, uS1, smoothstep(0.0, 0.4, k));
  c = mix(c, uS2, smoothstep(0.4, 0.74, k));
  return mix(c, uS3, smoothstep(0.74, 1.0, k));
}
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

  vec2 w = vec2(fbm(q * 1.3 + vec2(0.0, t * 0.1)), fbm(q * 1.3 + vec2(4.7, -t * 0.08)));
  q += (w - 0.5) * (0.07 + 0.12 * e + 0.06 * uBands.y) * (0.35 + 0.65 * ex);

  // Цвета ленты: при «любом настроении» меняются вдоль неё
  vec3 C1 = uC1;
  vec3 C2 = uC2;
  vec3 C3 = uC3;
  if (uSpectrum > 0.001) {
    float kx = frag.x / uRes.x * 0.92 + 0.04 + 0.06 * sin(t * 0.12 + q.y * 2.0);
    vec3 sp = spectrum(kx);
    C1 = mix(C1, sp * 0.32, uSpectrum);
    C2 = mix(C2, sp, uSpectrum);
    // Гребень: в темноте — почти белый свет, на бумаге — самая густая краска
    C3 = mix(C3, uLight > 0.5 ? sp * 1.35 : mix(sp, vec3(1.0, 0.94, 0.97), 0.62), uSpectrum);
  }

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
  float faceH = thick * (1.5 + 1.2 * (0.5 + 0.5 * sin(tw)));
  float fz = below / faceH;
  float body = exp(-pow(fz, 1.5)) * exp(-above * above / (0.00035 + 0.0005 * e));

  float m = fbm(vec2(q.x * 1.9 * uFreq - t * 0.2, fz * 2.6 + t * 0.05));
  float ca = caustic(vec2(q.x * 0.42 * uFreq - t * 0.02, fz * 0.32 + 0.13), t * 0.45 + e);
  vec3 faceCol = mix(C1, C2, smoothstep(0.15, 0.9, m) * (1.0 - 0.55 * min(fz, 1.0)));

  // В темноте свет копится поверх основы; на светлом копится поглощение поверх чистой бумаги
  vec3 base = uLight > 0.5 ? vec3(0.0) : uC0;
  vec3 col = base;
  col += faceCol * body * (0.7 + 0.35 * uBeat + 0.15 * uBands.x);
  col += mix(C2, C3, 0.6) * ca * body * smoothstep(0.05, 0.4, fz) * (0.18 + 0.22 * e + 0.15 * uBeat);

  float sheen = 0.55 + 0.45 * sin(q.x * 3.1 * uFreq + t * 0.8 + 1.5 * sin(t * 0.3));
  float core = exp(-abs(dy) / (0.0042 + 0.005 * e + 0.004 * uBeat));
  col += C3 * core * (0.3 + 0.5 * sheen + 0.5 * uBeat + 0.2 * uBands.z) * ex;
  float glow = exp(-abs(dy) * (15.0 - 4.0 * e - 3.0 * uBeat));
  col += mix(C2, C3, 0.35) * glow * (0.1 + 0.12 * e + 0.2 * uBeat) * ex;

  for (int i = 1; i < 3; i++) {
    float fi = float(i);
    float yi = ribbon(q.x * (1.0 + 0.09 * fi), t - 0.7 * fi, amp * (0.8 - 0.15 * fi)) - (0.075 + 0.02 * e) * fi * ex + rip * 0.5;
    float di = q.y - yi;
    col += mix(C2, C3, 0.3) * exp(-abs(di) / (0.0035 + 0.0025 * fi)) * (0.2 / fi) * ex * body;
    col += C2 * exp(-abs(di) * 16.0) * (0.06 / fi) * ex;
  }

  float patch = smoothstep(0.3, 0.75, noise(vec2(q.x * 1.6 * uFreq - t * 0.12, 3.0)));
  float rn = noise(vec2(q.x * 26.0 * uFreq + w.x * 3.0, t * 0.3));
  float rays = smoothstep(0.45, 1.0, rn) * patch;
  float rayMask = smoothstep(0.0, 0.025, above) * exp(-above * (15.0 - 5.0 * e - 3.0 * uBeat));
  col += mix(C2, C3, 0.3) * rays * rayMask * (0.16 + 0.3 * e + 0.3 * uBeat) * ex;
  col += C1 * exp(-above * 5.0) * 0.1 * ex;

  col += C3 * ripEnv * 0.45 * exp(-abs(dy) * 8.0);

  if (uBox.z > 0.0) {
    float sd = sdRoundBox(p - uBox.xy, uBox.zw, uBoxR);
    float halo = exp(-max(sd, 0.0) * 13.0) * (0.5 + 0.5 * uBeat);
    col += mix(C2, C3, 0.3) * halo * (0.25 + 0.75 * min(body + glow, 1.0)) * 0.8;
    col += C1 * exp(-max(sd, 0.0) * 4.0) * 0.35;
  }

  float fx = frag.x / uRes.x;
  float fade = 1.0;
  if (uFade.x > 0.0) fade *= mix(0.4, 1.0, smoothstep(0.0, uFade.x, fx));
  if (uFade.y > 0.0) fade *= mix(0.6, 1.0, smoothstep(1.0, 1.0 - uFade.y, fx));
  col = base + (col - base) * fade;

  col += C3 * uFlash * (0.35 * min(body + core, 1.4) + 0.05 + 0.15 * glow);

  vec2 vp = frag / uRes - 0.5;
  col *= 1.0 - 0.6 * dot(vp, vp);

  col = uLight > 0.5 ? uC0 * exp(-col) : 1.0 - exp(-col * 1.45);
  col = pow(max(col, 0.0), vec3(1.0 / 2.2));
  col += (hash(frag + fract(t * 7.0) * 61.0) - 0.5) / 180.0;
  gl_FragColor = vec4(col, 1.0);
}`
