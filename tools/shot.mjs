// Скриншоты страниц на разных размерах — для проверки интерфейса глазами.
//
//   bun tools/shot.mjs <url> [--routes home,player] [--sizes phone,desktop] [--out dir]
//                      [--theme dark|light] [--full] [--wait ms] [--dpr n] [--reduced-motion]
//                      [--eval "js"] [--init "js"] [--cookie name=value] [--name base]
//
// В url можно поставить {route}: http://localhost:3400{route} --routes /,/search
// Без {route} маршрут добавляется как #hash: http://localhost:4400/directions/a/ --routes home,player
// Печатает ошибки консоли и неудачные запросы — их тоже надо исправлять.
import { chromium } from "playwright-core";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

const IPHONE_UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 27_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148";
const IPAD_UA = "Mozilla/5.0 (iPad; CPU OS 27_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148";

const SIZES = {
  phone: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
  phoneSmall: { viewport: { width: 375, height: 667 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
  tablet: { viewport: { width: 1024, height: 1366 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
  // Узкое окно компьютера: боковая панель — полоса значков (rail)
  narrow: { viewport: { width: 900, height: 760 }, deviceScaleFactor: 2 },
  laptop: { viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2 },
  desktop: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 },
  wide: { viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 },
  // Телевизор 4K (сцена вечеринки)
  tv4k: { viewport: { width: 3840, height: 2160 }, deviceScaleFactor: 1 },
  // Размеры скриншотов App Store: iPhone 6,9″ — 1320×2868, iPad 13″ — 2064×2752
  iphone69: { viewport: { width: 440, height: 956 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true },
  ipad13: { viewport: { width: 1032, height: 1376 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
  ipad13l: { viewport: { width: 1376, height: 1032 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
  // Те же размеры App Store «как в приложении»: строка браузера iPhone/iPad (интерфейс включает
  // раскладку ios/ipados) и безопасные отступы устройства. iPhone 6,3″ — 1206×2622
  iphone69ios: { viewport: { width: 440, height: 956 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true, userAgent: IPHONE_UA, safe: { top: 62, right: 0, bottom: 34, left: 0 } },
  iphone63ios: { viewport: { width: 402, height: 874 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true, userAgent: IPHONE_UA, safe: { top: 62, right: 0, bottom: 34, left: 0 } },
  ipad13ios: { viewport: { width: 1032, height: 1376 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, userAgent: IPAD_UA, safe: { top: 24, right: 0, bottom: 20, left: 0 } },
  ipad13lios: { viewport: { width: 1376, height: 1032 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, userAgent: IPAD_UA, safe: { top: 24, right: 0, bottom: 20, left: 0 } },
  // Складной iPhone Duo (симулятор Xcode 27.2, iOS 27.2): сложенный — внешний экран 466×678 pt, строка
  // состояния — колонкой справа (отступ справа ≈83 pt); раскрытый — внутренний 951×669 pt, строка
  // состояния в правом верхнем углу (WKWebView отдаёт её отступом сверху ≈81 pt); 669×951 — раскрытый
  // и повёрнутый (отступы — как у раскрытого, не замерены). Строка браузера — iPhone: по ней интерфейс
  // узнаёт платформу ios. safe — env(safe-area-inset-*) WebView на симуляторе, по снимкам
  duoFolded: { viewport: { width: 466, height: 678 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true, userAgent: IPHONE_UA, safe: { top: 0, right: 83, bottom: 34, left: 0 } },
  duoOpen: { viewport: { width: 951, height: 669 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true, userAgent: IPHONE_UA, safe: { top: 81, right: 0, bottom: 36, left: 0 } },
  duoOpenPortrait: { viewport: { width: 669, height: 951 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true, userAgent: IPHONE_UA, safe: { top: 81, right: 0, bottom: 36, left: 0 } },
};

function parse(argv) {
  const opts = { urls: [], routes: [""], sizes: ["phone", "desktop"], out: "artifacts/shots", wait: 600, cookies: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => argv[++i];
    if (a === "--routes") opts.routes = next().split(",").map((s) => s.trim());
    else if (a === "--sizes") opts.sizes = next().split(",").map((s) => s.trim());
    else if (a === "--out") opts.out = next();
    else if (a === "--theme") opts.theme = next();
    else if (a === "--full") opts.full = true;
    else if (a === "--wait") opts.wait = Number(next());
    else if (a === "--dpr") opts.dpr = Number(next());
    else if (a === "--reduced-motion") opts.reducedMotion = true;
    else if (a === "--eval") opts.eval = next();
    // Скрипт до кода страницы (например, выбрать тему в localStorage)
    else if (a === "--init") opts.init = next();
    else if (a === "--cookie") opts.cookies.push(next());
    else if (a === "--name") opts.name = next();
    else if (a.startsWith("--")) throw new Error(`неизвестный флаг ${a}`);
    else opts.urls.push(a);
  }
  if (!opts.urls.length) throw new Error("нужен хотя бы один url");
  return opts;
}

const slug = (s) => s.replace(/^#/, "").replace(/[^a-z0-9а-яё]+/gi, "-").replace(/^-|-$/g, "") || "root";

const opts = parse(process.argv.slice(2));
mkdirSync(opts.out, { recursive: true });

const browser = await chromium.launch({
  channel: "chrome",
  headless: true,
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--autoplay-policy=no-user-gesture-required", "--mute-audio"],
});

let problems = 0;
for (const sizeName of opts.sizes) {
  const size = SIZES[sizeName];
  if (!size) throw new Error(`неизвестный размер ${sizeName}: ${Object.keys(SIZES).join(", ")}`);
  const { safe, ...device } = size;
  const context = await browser.newContext({
    ...device,
    deviceScaleFactor: opts.dpr ?? size.deviceScaleFactor,
    colorScheme: opts.theme === "light" ? "light" : "dark",
    reducedMotion: opts.reducedMotion ? "reduce" : "no-preference",
    locale: "ru-RU",
    timezoneId: "Europe/Berlin",
  });
  // Chrome без окна не знает вырезов экрана: отступы безопасной зоны устройства — переменными токенов
  if (safe) {
    await context.addInitScript((s) => {
      const css = `:root{--safe-top:${s.top}px!important;--safe-right:${s.right}px!important;--safe-bottom:${s.bottom}px!important;--safe-left:${s.left}px!important}`;
      document.addEventListener("DOMContentLoaded", () => {
        const el = document.createElement("style");
        el.textContent = css;
        document.head.append(el);
      });
    }, safe);
  }
  if (opts.init) await context.addInitScript(opts.init);
  for (const urlTemplate of opts.urls) {
    for (const cookie of opts.cookies) {
      const [name, ...rest] = cookie.split("=");
      await context.addCookies([{ name, value: rest.join("="), url: new URL(urlTemplate.replace("{route}", "/")).origin }]);
    }
    for (const route of opts.routes) {
      const url = urlTemplate.includes("{route}")
        ? urlTemplate.replace("{route}", route)
        : route ? `${urlTemplate.split("#")[0]}#${route}` : urlTemplate;
      const page = await context.newPage();
      const label = `${sizeName} ${url}`;
      page.on("console", (m) => {
        // Предупреждения программного WebGL в Chrome без окна — не ошибки страницы
        if (m.text().includes("GL Driver Message")) return;
        if (m.type() === "error" || m.type() === "warning") {
          problems++;
          console.log(`  [console.${m.type()}] ${label}: ${m.text()}`);
        }
      });
      page.on("pageerror", (e) => { problems++; console.log(`  [pageerror] ${label}: ${e.message}`); });
      page.on("requestfailed", (r) => { problems++; console.log(`  [requestfailed] ${label}: ${r.url()} ${r.failure()?.errorText}`); });
      page.on("response", (r) => {
        if (r.status() >= 400 && !r.url().endsWith("/favicon.ico")) { problems++; console.log(`  [http ${r.status()}] ${label}: ${r.url()}`); }
      });
      await page.goto(url, { waitUntil: "networkidle", timeout: 30000 }).catch((e) => console.log(`  [goto] ${label}: ${e.message}`));
      await page.evaluate(() => document.fonts?.ready).catch(() => {});
      if (opts.eval) await page.evaluate(opts.eval).catch((e) => console.log(`  [eval] ${label}: ${e.message}`));
      await page.waitForTimeout(opts.wait);
      const base = opts.name ?? slug(route || new URL(url).pathname);
      const file = join(opts.out, `${base}-${sizeName}${opts.theme === "light" ? "-light" : ""}.png`);
      await page.screenshot({ path: file, fullPage: !!opts.full });
      console.log(file);
      await page.close();
    }
  }
  await context.close();
}
await browser.close();
if (problems) console.log(`проблем в консоли и сети: ${problems}`);
