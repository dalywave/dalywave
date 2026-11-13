// Проверка «где заиграет музыка, выбранная на другом устройстве» (crates/server/src/choice.rs) в двух
// контекстах браузера: A — компьютер у колонок играет, B — ноутбук выбирает альбом. Простой A
// имитируется в базе (last_input_at), звук — в «пустое» устройство браузера. Снимки тостов и настроек —
// в --out.
//
//   bun tools/device-choice-check.mjs --base http://localhost:3452 --cookie <файл name=value>
//        --db <DATABASE_URL> [--tracks id,id,id,id] [--out dir]
//
// Шаги: A используют — выбор с B играет на A; A давно не трогали — играет на B, A на паузе; A —
// медиаплеер — играет на A, несмотря на простой; две поправки «Включить на A» — «Запомнил», и третий
// выбор сразу на A.
import { chromium } from "playwright-core";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { execFileSync } from "node:child_process";

const args = process.argv.slice(2);
const flag = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : def;
};
const base = flag("--base", "http://localhost:3400");
const out = flag("--out", new URL("../design/shots/device-choice", import.meta.url).pathname);
const db = flag("--db");
if (!db) throw new Error("нужен --db");
mkdirSync(out, { recursive: true });
const [cookieName, ...rest] = readFileSync(flag("--cookie"), "utf8").trim().split("=");
const cookieValue = rest.join("=");

const sql = (q) => execFileSync("psql", [db, "-Atc", q], { encoding: "utf8" }).trim();
const api = async (path, init = {}) => {
  const res = await fetch(`${base}${path}`, { ...init, headers: { Cookie: `${cookieName}=${cookieValue}`, Origin: base, "Content-Type": "application/json", ...init.headers } });
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res.json();
};

// Треки: --tracks id,id,… или лайкнутые, у которых уже есть звук
const ids = flag("--tracks", "").split(",").filter(Boolean);
const list = ids.length
  ? await api("/api/tracks/lookup", { method: "POST", body: JSON.stringify({ ids }) }).then((r) => r.items ?? r)
  : (await api("/api/library/tracks?limit=100")).items.map((i) => i.track).filter((t) => t.audioStatus === "ready");
if (list.length < 4) throw new Error("нужно хотя бы четыре готовых трека");
const album = (n) => list.slice(n, n + 2);

const UA = {
  windows: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
  mac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
  iphone: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
};
const SIZES = {
  phone: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
  desktop: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 },
};

const browser = await chromium.launch({
  channel: "chrome",
  headless: true,
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--autoplay-policy=no-user-gesture-required", "--mute-audio", "--disable-audio-output"],
});

const problems = [];
async function device(label, size, ua, theme = "dark") {
  const context = await browser.newContext({ ...SIZES[size], userAgent: UA[ua], colorScheme: theme, locale: "ru-RU", timezoneId: "Europe/Berlin" });
  await context.addCookies([{ name: cookieName, value: cookieValue, url: base }]);
  const page = await context.newPage();
  page.on("pageerror", (e) => problems.push(`${label}: ${e.message}`));
  await page.goto(`${base}/`, { waitUntil: "networkidle" });
  await waitFor(page, "connect.started && connect.ready", `${label}: на связи`, 40000).catch(async (e) => {
    console.log(problems.join("\n"), await inPage(page, "JSON.stringify({ s: connect.started, r: connect.ready, id: connect.deviceId, url: location.href })").catch(String));
    throw e;
  });
  return { label, context, page, id: await inPage(page, "connect.deviceId") };
}

function inPage(page, expr, arg) {
  return page.evaluate(
    ([expr, arg]) => {
      const pinia = document.querySelector("#__nuxt").__vue_app__.config.globalProperties.$pinia;
      const player = pinia._s.get("player");
      const connect = pinia._s.get("connect");
      const ui = pinia._s.get("ui");
      return new Function("player", "connect", "ui", "arg", `return (${expr})`)(player, connect, ui, arg);
    },
    [expr, arg],
  );
}

async function waitFor(page, expr, what, timeout = 12000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    if (await inPage(page, expr).catch(() => false)) return;
    await page.waitForTimeout(150);
  }
  throw new Error(`не дождались: ${what}`);
}

const results = [];
async function step(name, fn) {
  try {
    await fn();
    results.push(`✓ ${name}`);
  } catch (e) {
    results.push(`✗ ${name}: ${e.message}`);
  }
}
const shot = async (d, name, wait = 700) => {
  await d.page.waitForTimeout(wait);
  await d.page.screenshot({ path: join(out, `${name}.png`), timeout: 120000 });
};
const toast = (d) => d.page.locator(".toast").last();

const a = await device("A (ПК)", "desktop", "windows");
const b = await device("B (ноутбук)", "desktop", "mac");
// Чистый старт: без запомненных поправок и настроек
sql(`DELETE FROM connect_choices WHERE from_id IN ('${a.id}','${b.id}')`);
sql(`UPDATE connect_devices SET choice='auto', media_player=false WHERE id IN ('${a.id}','${b.id}')`);

/** A играет; idleSec — сколько его не трогали (null — только что) */
async function aPlays(n, idleSec) {
  if (!(await inPage(a.page, "connect.isActive && player.playing"))) {
    await inPage(a.page, "player.playTracks(arg, 0, { type: 'queue', id: null, title: 'A' })", album(n));
    await waitFor(a.page, "player.playing && connect.isActive", "A играет", 20000);
  }
  await waitFor(b.page, "connect.remote", "B — пульт");
  // Присутствие A дошло до сервера (оно уходит, как только A заиграл)
  for (let i = 0; i < 40 && !sql(`SELECT presence_at FROM connect_devices WHERE id='${a.id}'`); i++) await a.page.waitForTimeout(150);
  if (idleSec != null) sql(`UPDATE connect_devices SET last_input_at = now() - interval '${idleSec} seconds' WHERE id='${a.id}'`);
}
const bPicks = (n) => inPage(b.page, "player.playTracks(arg, 0, { type: 'album', id: null, title: 'Альбом' })", album(n));
const lastPlaced = () => sql(`SELECT decided FROM connect_choices WHERE from_id='${b.id}' AND to_id='${a.id}'`);

await step("A используют — выбор на B играет на A", async () => {
  await aPlays(0, null);
  await bPicks(2);
  await waitFor(a.page, `player.current?.track.id === '${album(2)[0].id}' && player.playing`, "A играет выбранное");
  if (!(await inPage(b.page, "connect.remote"))) throw new Error("B перестал быть пультом");
  await toast(b).waitFor({ timeout: 4000 });
  await shot(b, "desktop-toast-there");
});

await step("A давно не трогали — играет на B, A на паузе", async () => {
  await aPlays(0, 300);
  await bPicks(0);
  await waitFor(b.page, `connect.isActive && !player.mirrored && player.current?.track.id === '${album(0)[0].id}'`, "B играет сам", 20000);
  await waitFor(a.page, "!player.playing && connect.remote", "A на паузе и пульт");
  await toast(b).waitFor({ timeout: 4000 });
  await shot(b, "desktop-toast-here");
  if (lastPlaced() !== "here") throw new Error(`решение: ${lastPlaced()}`);
});

await step("медиаплеер: настройки в «Где играть» и выбор идёт на него", async () => {
  // Обратно на A кнопкой «Играть здесь» на A — это не поправка (её просит не B)
  await inPage(a.page, "connect.continueHere(true)");
  await waitFor(a.page, "connect.isActive && player.playing", "A снова играет", 20000);
  await inPage(b.page, "ui.openPanel('devices')");
  await b.page.waitForTimeout(500);
  await b.page.locator(`.devwrap:has-text("Windows") .dev__more`).first().click();
  await b.page.waitForTimeout(300);
  await b.page.locator(".devwrap.is-editing [role=switch]").first().click();
  await waitFor(b.page, `connect.devices.find(d => d.id === '${a.id}')?.mediaPlayer`, "метка «Медиаплеер»");
  await shot(b, "desktop-device-settings");
  await inPage(b.page, "(ui.panelOpen = false)");
  await aPlays(0, 3600);
  await bPicks(2);
  await waitFor(a.page, `player.current?.track.id === '${album(2)[0].id}' && player.playing`, "A (медиаплеер) играет выбранное");
  await api(`/api/devices/${a.id}`, { method: "PATCH", body: JSON.stringify({ mediaPlayer: false }) });
});

async function correctToA() {
  await toast(b).getByRole("button", { name: /Включить на/ }).click();
  await waitFor(a.page, "connect.isActive && player.playing", "A играет после поправки", 20000);
}

await step("две поправки «Включить на A» — запомнил", async () => {
  // «Играть здесь» на A после решения «на B» — тоже поправка: серия с чистого листа
  sql(`DELETE FROM connect_choices WHERE from_id = '${b.id}'`);
  await aPlays(0, 300);
  await bPicks(0);
  await waitFor(b.page, "connect.isActive && !player.mirrored", "B играет (1)", 20000);
  await correctToA();
  await aPlays(0, 300);
  await bPicks(2);
  await waitFor(b.page, "connect.isActive && !player.mirrored", "B играет (2)", 20000);
  await correctToA();
  await b.page.getByText(/Запомнил/).waitFor({ timeout: 4000 });
  await shot(b, "desktop-toast-learned", 300);
  const prefer = sql(`SELECT prefer FROM connect_choices WHERE from_id='${b.id}' AND to_id='${a.id}' AND prefer_until > now()`);
  if (prefer !== "there") throw new Error(`запомнено: ${prefer}`);
});

await step("после запоминания выбор сразу на A, хотя его не трогают", async () => {
  await aPlays(0, 3600);
  await bPicks(0);
  await waitFor(a.page, `player.current?.track.id === '${album(0)[0].id}' && player.playing`, "A играет выбранное");
});

// Телефон: «Где играть» с раскрытыми настройками ПК, тёмная и светлая
for (const theme of ["dark", "light"]) {
  await step(`снимок листа на телефоне (${theme})`, async () => {
    const p = await device(`телефон ${theme}`, "phone", "iphone", theme);
    await p.page.evaluate((t) => document.documentElement.setAttribute("data-theme", t), theme);
    await waitFor(p.page, "connect.remote", "телефон — пульт");
    await inPage(p.page, "ui.openPanel('devices')");
    await p.page.waitForTimeout(600);
    await p.page.locator(".dwrap .drow__more").first().click();
    await shot(p, `phone-devices-${theme}`);
    await p.page.goto(`${base}/settings`, { waitUntil: "networkidle" });
    await p.page.evaluate((t) => document.documentElement.setAttribute("data-theme", t), theme);
    await p.page.getByText("Когда выбираю музыку здесь").first().scrollIntoViewIfNeeded();
    await shot(p, `phone-settings-${theme}`);
    await p.context.close();
  });
}
await step("настройки на компьютере (светлая)", async () => {
  await b.page.goto(`${base}/settings`, { waitUntil: "networkidle" });
  await b.page.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  await b.page.getByText("Когда выбираю музыку здесь").first().scrollIntoViewIfNeeded();
  await shot(b, "desktop-settings-light");
});

await inPage(a.page, "player.pause?.()").catch(() => {});
await browser.close();
console.log(results.join("\n"));
if (problems.length) console.log("Ошибки страниц:\n" + problems.join("\n"));
