// Проверка управления между устройствами в двух контекстах браузера (два устройства одного человека):
// вкладка A на компьютере играет (без звука: браузер выводит его в «пустое» устройство, и время идёт,
// даже если системный вывод занят), телефон B — пульт: видит трек и
// идущую позицию, ставит паузу, переключает, перематывает, переносит воспроизведение к себе; A
// становится пультом, а когда B пропадает — предлагает «Продолжить здесь». Снимки пульта на
// телефоне (390) и компьютере (1440) — в design/shots/connect/.
//
//   bun tools/connect-check.mjs [--base http://localhost:3400] [--tracks id,id,…] [--out dir]
//
// Вход — аварийным паролем из DALYWAVE_ADMIN_PASSWORD (корневой .env); пароль не печатается.
// Треки: --tracks, иначе — лайкнутые, сначала те, что уже на сервере.
import { chromium } from "playwright-core";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const flag = (name, def) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : def;
};
const base = flag("--base", "http://localhost:3400");
const out = flag("--out", new URL("../design/shots/connect", import.meta.url).pathname);
mkdirSync(out, { recursive: true });

const env = readFileSync(new URL("../.env", import.meta.url), "utf8");
const password = env.match(/^DALYWAVE_ADMIN_PASSWORD=(.*)$/m)?.[1]?.trim().replace(/^["']|["']$/g, "");
if (!password) throw new Error("нет DALYWAVE_ADMIN_PASSWORD в .env");
const login = await fetch(`${base}/api/auth/password`, {
  method: "POST",
  headers: { "Content-Type": "application/json", Origin: base },
  body: JSON.stringify({ password }),
});
if (!login.ok) throw new Error(`вход: HTTP ${login.status}`);
const session = login.headers.getSetCookie().find((c) => c.startsWith("dw_session="))?.split(";")[0].slice("dw_session=".length);
if (!session) throw new Error("вход: нет cookie dw_session");
const api = async (path, init = {}) => {
  const res = await fetch(`${base}${path}`, { ...init, headers: { Cookie: `dw_session=${session}`, Origin: base, "Content-Type": "application/json", ...init.headers } });
  if (!res.ok) throw new Error(`${path}: HTTP ${res.status}`);
  return res.json();
};

let trackIds = flag("--tracks", "")?.split(",").filter(Boolean) ?? [];
if (!trackIds.length) {
  const liked = await api("/api/library/tracks?limit=100");
  const list = liked.items.map((i) => i.track);
  trackIds = [...list.filter((t) => t.audioStatus === "ready"), ...list.filter((t) => t.audioStatus !== "ready")].slice(0, 8).map((t) => t.id);
}
const found = await api("/api/tracks/lookup", { method: "POST", body: JSON.stringify({ ids: trackIds }) });
const tracks = trackIds.map((id) => (found.items ?? found).find((t) => t.id === id)).filter(Boolean);
if (tracks.length < 2) throw new Error("нужно хотя бы два трека");

// Разные браузеры — разные имена в списке устройств («Chrome на Windows», «Safari на iPhone»)
const UA = {
  windows: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
  iphone: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
  mac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36",
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
async function device(label, size, ua) {
  const context = await browser.newContext({ ...SIZES[size], userAgent: UA[ua], colorScheme: "dark", locale: "ru-RU", timezoneId: "Europe/Berlin" });
  await context.addCookies([{ name: "dw_session", value: session, url: base }]);
  const page = await context.newPage();
  page.on("console", (m) => {
    if (m.type() === "error") problems.push(`${label}: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`${label}: ${e.message}`));
  await page.goto(`${base}/`, { waitUntil: "networkidle" });
  await waitFor(page, "connect.started && connect.ready", `${label}: устройство на связи`);
  return { label, context, page };
}

/** Выражение над хранилищами страницы: player, connect, ui */
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

async function waitFor(page, expr, what, timeout = 10000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    if (await inPage(page, expr).catch(() => false)) return;
    await page.waitForTimeout(150);
  }
  throw new Error(`не дождались: ${what} (${expr})`);
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

const shot = async (d, name) => {
  // Анимации появления плеера и листов успевают закончиться
  await d.page.waitForTimeout(1500);
  // Машина бывает загружена сборками: снимок с WebGL-волной тогда идёт долго
  await d.page.screenshot({ path: join(out, `${name}.png`), timeout: 120000 });
};

const a = await device("A (компьютер)", "desktop", "windows");
const b = await device("B (телефон)", "phone", "iphone");

await step("A играет очередь", async () => {
  await inPage(a.page, "player.playTracks(arg, 0, { type: 'queue', id: null, title: 'Проверка пульта' })", tracks);
  await waitFor(a.page, "player.playing && connect.isActive", "A играет и активно");
  // Звук пошёл (файл загрузился), а не только нажата кнопка
  await waitFor(a.page, "player.positionMs > 1500", "позиция A идёт", 20000);
});

await step("B видит трек A и идущую позицию", async () => {
  await waitFor(b.page, `connect.remote && player.current?.track.id === '${tracks[0].id}'`, "трек на пульте");
  const p1 = await inPage(b.page, "player.positionMs");
  await b.page.waitForTimeout(1600);
  const p2 = await inPage(b.page, "player.positionMs");
  if (!(p2 - p1 > 800)) throw new Error(`позиция не идёт: ${p1} → ${p2}`);
  const pa = await inPage(a.page, "player.positionMs");
  if (Math.abs(pa - p2) > 1500) throw new Error(`позиция расходится: A ${pa}, B ${p2}`);
});

await step("снимки пульта на телефоне", async () => {
  await shot(b, "phone-mini");
  await inPage(b.page, "(ui.playerOpen = true)");
  await shot(b, "phone-full");
  await inPage(b.page, "ui.openPanel('devices')");
  await shot(b, "phone-devices");
  await inPage(b.page, "(ui.sheetTab = null)");
});

await step("B: пауза", async () => {
  await b.page.getByRole("dialog").getByRole("button", { name: "Пауза" }).first().click();
  await waitFor(a.page, "!player.playing", "A на паузе");
  await waitFor(b.page, "!player.playing", "B видит паузу");
});

await step("B: играть", async () => {
  await inPage(b.page, "player.toggle()");
  await waitFor(a.page, "player.playing", "A снова играет");
});

await step("B: следующий трек", async () => {
  await inPage(b.page, "player.next()");
  await waitFor(a.page, `player.current?.track.id === '${tracks[1].id}'`, "A на втором треке");
  await waitFor(b.page, `player.current?.track.id === '${tracks[1].id}'`, "B видит второй трек");
});

await step("B: перемотка на 0:40", async () => {
  await waitFor(a.page, "player.durationMs > 45000 && !player.pending", "второй трек загружен", 15000);
  await inPage(b.page, "player.seek(40000)");
  await waitFor(a.page, "Math.abs(player.positionMs - 40000) < 2500", "A перемотан");
  await waitFor(b.page, "Math.abs(player.positionMs - 40000) < 2500", "B видит перемотку");
});

const c = await device("C (компьютер-пульт)", "desktop", "mac");
await step("снимки пульта на компьютере", async () => {
  await waitFor(c.page, "connect.remote && !!player.current", "C — пульт");
  await c.page.waitForTimeout(600);
  const [pa, pc] = [await inPage(a.page, "player.positionMs"), await inPage(c.page, "player.positionMs")];
  if (Math.abs(pa - pc) > 1500) throw new Error(`позиция у нового пульта: A ${pa}, C ${pc}`);
  await shot(c, "desktop-bar");
  await inPage(c.page, "ui.openPanel('devices')");
  await shot(c, "desktop-devices");
  await inPage(c.page, "(ui.panelOpen = false, ui.playerOpen = true)");
  await shot(c, "desktop-full");
  await inPage(c.page, "(ui.playerOpen = false)");
});

let before = 0;
await step("перенос на B из списка устройств", async () => {
  await inPage(b.page, "(ui.playerOpen = false, ui.openPanel('devices'))");
  await b.page.waitForTimeout(400);
  const here = b.page.getByRole("button", { name: /Играть здесь/ });
  await here.waitFor();
  before = await inPage(a.page, "player.positionMs");
  const t0 = Date.now();
  await here.click();
  await waitFor(b.page, "connect.isActive && !player.mirrored && player.playing", "B играет сам", 15000);
  await waitFor(a.page, "player.mirrored && connect.remote && player.playing", "A стал пультом");
  // Продолжил с того же места: позиция A в момент нажатия плюс прошедшее время
  const pos = await inPage(b.page, "player.positionMs");
  const expected = before + (Date.now() - t0);
  if (Math.abs(pos - expected) > 3000) throw new Error(`позиция после переноса ${Math.round(pos)}, ожидали ≈${Math.round(expected)}`);
  const queue = await inPage(b.page, "player.upcoming.length");
  if (queue !== tracks.length - 2) throw new Error(`очередь после переноса: ${queue}`);
  await inPage(b.page, "(ui.sheetTab = null)");
  await shot(a, "desktop-after-transfer");
});

await step("B пропал — A предлагает «Продолжить здесь»", async () => {
  await b.context.close();
  await waitFor(a.page, "connect.orphaned", "A видит, что B не в сети", 25000);
  await shot(a, "desktop-orphaned");
  await a.page.getByRole("button", { name: "Продолжить здесь" }).first().click();
  await waitFor(a.page, "connect.isActive && !player.mirrored && player.playing", "A продолжил", 15000);
});

await browser.close();
console.log(results.join("\n"));
if (problems.length) console.log(`\nОшибки консоли:\n${[...new Set(problems)].join("\n")}`);
process.exit(results.some((r) => r.startsWith("✗")) ? 1 : 0);
