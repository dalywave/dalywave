// Фирменные файлы DalyWave из одного описания знака «Волна‑W».
//   bun tools/brand.mjs
// Пишет SVG и PNG в design/brand/, веб-иконки — в apps/ui/public/.
import { chromium } from "playwright-core";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const BRAND = "design/brand";
const PUBLIC = "apps/ui/public";
mkdirSync(BRAND, { recursive: true });
mkdirSync(PUBLIC, { recursive: true });

// Знак: одна плавная линия через пять точек — волна и буква W
const POINTS = [[16, 31], [33, 69], [50, 41], [67, 69], [84, 31]];
const STROKE = 11.5;
// Фон иконки: «Сумерки» — основной, «Ночь» и «Закат» — альтернативные иконки в настройках
const PALETTES = {
  twilight: [["#4B21FF", 0], ["#B01CF0", 0.4], ["#FF3D7A", 0.74], ["#FF9A3C", 1]],
  night: [["#4B21FF", 0], ["#A21CFF", 0.55], ["#FF3D8E", 1]],
  sunset: [["#E0215F", 0], ["#FF4D3A", 0.55], ["#FFAE3A", 1]],
};
const DEFAULT = "twilight";

function smoothPath(pts) {
  const p = [pts[0], ...pts, pts[pts.length - 1]];
  let d = `M${pts[0][0]} ${pts[0][1]}`;
  for (let i = 1; i < p.length - 2; i++) {
    const [x0, y0] = p[i - 1], [x1, y1] = p[i], [x2, y2] = p[i + 1], [x3, y3] = p[i + 2];
    const c1 = [x1 + (x2 - x0) / 6, y1 + (y2 - y0) / 6];
    const c2 = [x2 - (x3 - x1) / 6, y2 - (y3 - y1) / 6];
    d += `C${c1[0].toFixed(2)} ${c1[1].toFixed(2)} ${c2[0].toFixed(2)} ${c2[1].toFixed(2)} ${x2} ${y2}`;
  }
  return d;
}
const WAVE = smoothPath(POINTS);

// Суперэллипс — форма иконок iOS и macOS
function squircle(x, y, size, n = 5, steps = 240) {
  const a = size / 2;
  let d = "";
  for (let i = 0; i <= steps; i++) {
    const t = (2 * Math.PI * i) / steps;
    const c = Math.cos(t), s = Math.sin(t);
    const px = x + a + a * Math.sign(c) * Math.abs(c) ** (2 / n);
    const py = y + a + a * Math.sign(s) * Math.abs(s) ** (2 / n);
    d += (i ? "L" : "M") + px.toFixed(2) + " " + py.toFixed(2);
  }
  return d + "Z";
}

const defs = (id, x, y, size, stops) => `
  <linearGradient id="${id}-g" x1="${x}" y1="${y}" x2="${x + size}" y2="${y + size}" gradientUnits="userSpaceOnUse">
    ${stops.map(([color, offset]) => `<stop offset="${offset}" stop-color="${color}"/>`).join("")}
  </linearGradient>
  <radialGradient id="${id}-h" cx="${x + size * 0.22}" cy="${y + size * 0.12}" r="${size * 0.9}" gradientUnits="userSpaceOnUse">
    <stop offset="0" stop-color="#fff" stop-opacity=".28"/><stop offset=".6" stop-color="#fff" stop-opacity="0"/>
  </radialGradient>`;

// Знак внутри поля: 68 % от стороны, по центру
const glyphIn = (x, y, size, color = "#fff") =>
  `<g transform="translate(${x + size / 2} ${y + size / 2}) scale(${(size / 100) * 0.68}) translate(-50 -50)">
    <path d="${WAVE}" fill="none" stroke="${color}" stroke-width="${STROKE}" stroke-linecap="round" stroke-linejoin="round"/>
  </g>`;

const svg = (w, h, body) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>\n`;

// Сам знак, цвет — currentColor: для боковой панели, кнопок, заставки
const files = {
  "glyph.svg": svg(100, 100, `<path d="${WAVE}" fill="none" stroke="currentColor" stroke-width="${STROKE}" stroke-linecap="round" stroke-linejoin="round"/>`),
};
const renders = [];

for (const [name, stops] of Object.entries(PALETTES)) {
  const suffix = name === DEFAULT ? "" : `-${name}`;
  const mid = stops[Math.floor(stops.length / 2)][0];
  // Квадрат во весь кадр: iOS и Android сами накладывают маску; исходник для cargo tauri icon
  files[`icon-square${suffix}.svg`] = svg(1024, 1024, `<defs>${defs("s", 0, 0, 1024, stops)}</defs>
    <rect width="1024" height="1024" fill="url(#s-g)"/><rect width="1024" height="1024" fill="url(#s-h)"/>${glyphIn(0, 0, 1024)}`);
  // Суперэллипс с прозрачными углами: фавикон, сайт
  files[`icon-rounded${suffix}.svg`] = svg(1024, 1024, `<defs>${defs("r", 0, 0, 1024, stops)}<clipPath id="r-c"><path d="${squircle(0, 0, 1024)}"/></clipPath></defs>
    <g clip-path="url(#r-c)"><rect width="1024" height="1024" fill="url(#r-g)"/><rect width="1024" height="1024" fill="url(#r-h)"/></g>${glyphIn(0, 0, 1024)}`);
  // macOS: тело 824 по сетке Apple, поля и мягкая тень
  files[`icon-macos${suffix}.svg`] = svg(1024, 1024, `<defs>${defs("m", 100, 100, 824, stops)}
      <clipPath id="m-c"><path d="${squircle(100, 100, 824)}"/></clipPath>
      <filter id="m-s" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000" flood-opacity=".32"/></filter>
    </defs>
    <path d="${squircle(100, 100, 824)}" fill="${mid}" filter="url(#m-s)"/>
    <g clip-path="url(#m-c)"><rect x="100" y="100" width="824" height="824" fill="url(#m-g)"/><rect x="100" y="100" width="824" height="824" fill="url(#m-h)"/></g>${glyphIn(100, 100, 824)}`);
  renders.push(
    [`icon-square${suffix}.svg`, 1024, join(BRAND, `icon-1024${suffix}.png`)],
    [`icon-macos${suffix}.svg`, 1024, join(BRAND, `icon-macos-1024${suffix}.png`)],
    [`icon-rounded${suffix}.svg`, 512, join(BRAND, `icon-rounded-512${suffix}.png`)],
  );
}

// Темы значка iOS и macOS (26+): светлая — обычная иконка; тёмная — знак в цветах бренда на тёмном;
// монохромная — серый знак, который система тонирует цветом пользователя; прозрачную («жидкое стекло»)
// система собирает сама из слоёв Icon Composer: фон и знак отдельно.
const night = PALETTES[DEFAULT];
// Градиент — по габаритам самого знака: путь лежит в своей системе координат 0–100 внутри transform
const glyphGradient = (id) => `<linearGradient id="${id}" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#8E6BFF"/><stop offset=".45" stop-color="#D45BEF"/><stop offset=".75" stop-color="#FF6F95"/><stop offset="1" stop-color="#FFA25C"/>
  </linearGradient>`;
// Для тёмной темы знак светлее основного градиента: на чёрном насыщенные тёмные цвета проваливаются
files["icon-dark.svg"] = svg(1024, 1024, `<defs>${glyphGradient("d-g")}
    <linearGradient id="d-bg" x1="0" y1="0" x2="0" y2="1024" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#2A2A2E"/><stop offset="1" stop-color="#0E0E10"/></linearGradient></defs>
  <rect width="1024" height="1024" fill="url(#d-bg)"/>${glyphIn(0, 0, 1024, "url(#d-g)")}`);
// Слоты каталога ресурсов Xcode: тёмная — без фона (его даёт система), монохромная — оттенки серого без фона
files["icon-ios-dark.svg"] = svg(1024, 1024, `<defs>${glyphGradient("dt-g")}</defs>${glyphIn(0, 0, 1024, "url(#dt-g)")}`);
files["icon-ios-tinted.svg"] = svg(1024, 1024, glyphIn(0, 0, 1024, "#fff"));
// Слои для Icon Composer
files["layers/background.svg"] = svg(1024, 1024, `<defs>${defs("l", 0, 0, 1024, night)}</defs><rect width="1024" height="1024" fill="url(#l-g)"/>`);
files["layers/glyph.svg"] = svg(1024, 1024, glyphIn(0, 0, 1024, "#fff"));

mkdirSync(join(BRAND, "layers"), { recursive: true });
for (const [name, content] of Object.entries(files)) writeFileSync(join(BRAND, name), content);
writeFileSync(join(PUBLIC, "favicon.svg"), files["icon-rounded.svg"]);
renders.push(
  ["icon-dark.svg", 1024, join(BRAND, "icon-1024-dark.png")],
  ["icon-ios-dark.svg", 1024, join(BRAND, "icon-ios-dark-1024.png")],
  ["icon-ios-tinted.svg", 1024, join(BRAND, "icon-ios-tinted-1024.png")],
);
renders.push(
  ["icon-square.svg", 180, join(PUBLIC, "apple-touch-icon.png")],
  ["icon-rounded.svg", 512, join(PUBLIC, "icon-512.png")],
  ["icon-rounded.svg", 32, join(PUBLIC, "favicon-32.png")],
);

const browser = await chromium.launch({ channel: "chrome", headless: true });
const page = await browser.newPage({ deviceScaleFactor: 1 });
for (const [src, size, out] of renders) {
  await page.setViewportSize({ width: size, height: size });
  const body = files[src].replace(/width="1024" height="1024"/, `width="${size}" height="${size}"`);
  await page.setContent(`<html><body style="margin:0;background:transparent">${body}</body></html>`);
  await page.screenshot({ path: out, omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
  console.log(out);
}
await browser.close();
