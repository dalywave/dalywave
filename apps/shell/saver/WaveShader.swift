// Шейдер волны «Полночь» на Metal — перенос apps/ui/app/lib/wave/shader.ts (GLSL) без светлой темы и
// без рамки обложки. Меняя один, меняйте и другой. Исходник собирается при запуске
// (`makeLibrary(source:)`): заставке не нужен Metal-компилятор в сборке.
//
// Униформы — 14 векторов float4 (см. `WaveUniforms` в DalyWaveSaver.swift):
//  0 res.x, res.y, time, beat     1 energy, expand, flash, spectrum
//  2 tilt, offset, 0, amp         3 fadeL, fadeR, freq, 0
//  4 bands.xyz                    5 ripple.xyz
//  6..9 C0..C3                    10..13 спектр S0..S3

let waveShaderSource = """
#include <metal_stdlib>
using namespace metal;

struct VOut { float4 pos [[position]]; };

vertex VOut wave_vertex(uint vid [[vertex_id]]) {
  float2 p[3] = { float2(-1.0, -1.0), float2(3.0, -1.0), float2(-1.0, 3.0) };
  VOut o;
  o.pos = float4(p[vid], 0.0, 1.0);
  return o;
}

static float hash(float2 p) {
  float3 p3 = fract(float3(p.x, p.y, p.x) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}
static float noise(float2 p) {
  float2 i = floor(p);
  float2 f = fract(p);
  float2 u = f * f * (3.0 - 2.0 * f);
  float a = hash(i);
  float b = hash(i + float2(1.0, 0.0));
  float c = hash(i + float2(0.0, 1.0));
  float d = hash(i + float2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
static float fbm(float2 p) {
  float v = 0.0;
  float a = 0.5;
  float2x2 m = float2x2(float2(1.6, 1.2), float2(-1.2, 1.6));
  for (int i = 0; i < 4; i++) {
    v += a * noise(p);
    p = m * p + 7.3;
    a *= 0.5;
  }
  return v;
}
static float caustic(float2 p, float t) {
  float2 a = p * float2(9.0, 5.0);
  float f = sin(a.x + 1.7 * sin(a.y * 0.8 + t) + t * 0.6) + sin(a.y * 1.3 + 1.9 * sin(a.x * 0.7 - t * 0.8) - t * 0.4);
  float lines = 1.0 - smoothstep(0.0, 0.32, abs(f));
  return lines * lines;
}
static float3 spectrum(float k, constant float4 *u) {
  k = clamp(k, 0.0, 1.0);
  float3 c = mix(u[10].xyz, u[11].xyz, smoothstep(0.0, 0.4, k));
  c = mix(c, u[12].xyz, smoothstep(0.4, 0.74, k));
  return mix(c, u[13].xyz, smoothstep(0.74, 1.0, k));
}
static float ribbon(float x, float t, float amp, float freq) {
  return amp * (sin(x * 1.15 * freq + t * 0.35) + 0.55 * sin(x * 2.4 * freq - t * 0.42 + 3.6));
}

fragment float4 wave_fragment(VOut in [[stage_in]], constant float4 *u [[buffer(0)]]) {
  float2 res = u[0].xy;
  float t = u[0].z;
  float beat = u[0].w;
  float e = u[1].x;
  float ex = u[1].y;
  float flash = u[1].z;
  float spec = u[1].w;
  float4 lay = u[2];
  float2 fadeLR = u[3].xy;
  float freq = u[3].z;
  float3 bands = u[4].xyz;
  float3 ripple = u[5].xyz;
  float3 C0 = u[6].xyz;

  // Metal считает пиксели сверху, GLSL — снизу
  float2 frag = float2(in.pos.x, res.y - in.pos.y);
  float2 p = (frag - 0.5 * res) / res.y;

  float cs = cos(lay.x);
  float sn = sin(lay.x);
  float2 q = float2(cs * p.x + sn * p.y, -sn * p.x + cs * p.y);
  q.y -= lay.y;

  float2 w = float2(fbm(q * 1.3 + float2(0.0, t * 0.1)), fbm(q * 1.3 + float2(4.7, -t * 0.08)));
  q += (w - 0.5) * (0.07 + 0.12 * e + 0.06 * bands.y) * (0.35 + 0.65 * ex);

  float3 C1 = u[7].xyz;
  float3 C2 = u[8].xyz;
  float3 C3 = u[9].xyz;
  if (spec > 0.001) {
    float kx = frag.x / res.x * 0.92 + 0.04 + 0.06 * sin(t * 0.12 + q.y * 2.0);
    float3 sp = spectrum(kx, u);
    C1 = mix(C1, sp * 0.32, spec);
    C2 = mix(C2, sp, spec);
    C3 = mix(C3, mix(sp, float3(1.0, 0.94, 0.97), 0.62), spec);
  }

  float rd = abs(q.x - ripple.x);
  float front = ripple.y * 0.95;
  float ripEnv = exp(-abs(rd - front) * 4.5) * exp(-ripple.y * 1.25) * ripple.z;
  float rip = sin((rd - front) * 24.0) * ripEnv * 0.045;

  float amp = (0.055 + 0.1 * e) * lay.w * mix(0.1, 1.0, ex);
  float thick = (0.085 + 0.05 * e + 0.035 * bands.x) * (1.0 + 0.18 * beat) * mix(0.12, 1.0, ex);

  float y0 = ribbon(q.x, t, amp, freq) + rip;
  float dy = q.y - y0;
  float above = max(dy, 0.0);
  float below = max(-dy, 0.0);

  float tw = q.x * 1.7 * freq + t * 0.33 + 1.2 * sin(q.x * 0.7 * freq - t * 0.15);
  float faceH = thick * (1.5 + 1.2 * (0.5 + 0.5 * sin(tw)));
  float fz = below / faceH;
  float body = exp(-pow(fz, 1.5)) * exp(-above * above / (0.00035 + 0.0005 * e));

  float m = fbm(float2(q.x * 1.9 * freq - t * 0.2, fz * 2.6 + t * 0.05));
  float ca = caustic(float2(q.x * 0.42 * freq - t * 0.02, fz * 0.32 + 0.13), t * 0.45 + e);
  float3 faceCol = mix(C1, C2, smoothstep(0.15, 0.9, m) * (1.0 - 0.55 * min(fz, 1.0)));

  float3 base = C0;
  float3 col = base;
  col += faceCol * body * (0.7 + 0.35 * beat + 0.15 * bands.x);
  col += mix(C2, C3, 0.6) * ca * body * smoothstep(0.05, 0.4, fz) * (0.18 + 0.22 * e + 0.15 * beat);

  float sheen = 0.55 + 0.45 * sin(q.x * 3.1 * freq + t * 0.8 + 1.5 * sin(t * 0.3));
  float core = exp(-abs(dy) / (0.0042 + 0.005 * e + 0.004 * beat));
  col += C3 * core * (0.3 + 0.5 * sheen + 0.5 * beat + 0.2 * bands.z) * ex;
  float glow = exp(-abs(dy) * (15.0 - 4.0 * e - 3.0 * beat));
  col += mix(C2, C3, 0.35) * glow * (0.1 + 0.12 * e + 0.2 * beat) * ex;

  for (int i = 1; i < 3; i++) {
    float fi = float(i);
    float yi = ribbon(q.x * (1.0 + 0.09 * fi), t - 0.7 * fi, amp * (0.8 - 0.15 * fi), freq) - (0.075 + 0.02 * e) * fi * ex + rip * 0.5;
    float di = q.y - yi;
    col += mix(C2, C3, 0.3) * exp(-abs(di) / (0.0035 + 0.0025 * fi)) * (0.2 / fi) * ex * body;
    col += C2 * exp(-abs(di) * 16.0) * (0.06 / fi) * ex;
  }

  float patchv = smoothstep(0.3, 0.75, noise(float2(q.x * 1.6 * freq - t * 0.12, 3.0)));
  float rn = noise(float2(q.x * 26.0 * freq + w.x * 3.0, t * 0.3));
  float rays = smoothstep(0.45, 1.0, rn) * patchv;
  float rayMask = smoothstep(0.0, 0.025, above) * exp(-above * (15.0 - 5.0 * e - 3.0 * beat));
  col += mix(C2, C3, 0.3) * rays * rayMask * (0.16 + 0.3 * e + 0.3 * beat) * ex;
  col += C1 * exp(-above * 5.0) * 0.1 * ex;

  col += C3 * ripEnv * 0.45 * exp(-abs(dy) * 8.0);

  float fx = frag.x / res.x;
  float fade = 1.0;
  if (fadeLR.x > 0.0) fade *= mix(0.4, 1.0, smoothstep(0.0, fadeLR.x, fx));
  if (fadeLR.y > 0.0) fade *= mix(0.6, 1.0, smoothstep(1.0, 1.0 - fadeLR.y, fx));
  col = base + (col - base) * fade;

  col += C3 * flash * (0.35 * min(body + core, 1.4) + 0.05 + 0.15 * glow);

  float2 vp = frag / res - 0.5;
  col *= 1.0 - 0.6 * dot(vp, vp);

  col = 1.0 - exp(-col * 1.45);
  col = pow(max(col, 0.0), float3(1.0 / 2.2));
  col += (hash(frag + fract(t * 7.0) * 61.0) - 0.5) / 180.0;
  return float4(col, 1.0);
}
"""
