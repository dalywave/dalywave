/* Тексты песен — общий движок прототипов.
   Данные — настоящие ответы /api/tracks/{id}/lyrics (design/fixtures/lyrics.json): только строки с временем
   начала (LRCLIB), без пословной разметки и без исполнителей. Всё, что тоньше строки, здесь ОЦЕНКА:
   - конец строки и начало проигрыша — по длине строки в слогах, если до следующей строки долгая пауза;
   - время каждого слова — делим время пения строки пропорционально слогам (гласным).
   Часы имитируют воспроизведение без звука. Параметры адреса (для скриншотов и ссылок):
   device=phone|desktop, theme=dark|light, track=ptichki|hochesh|onmyway, t=мс, paused=1, bare=1 (без стенда),
   open=0 (телефон: плеер до открытия текста), scroll=px (ручная прокрутка), share=номер строки, hide=1. */
window.LY = (() => {
  const qs = new URLSearchParams(location.search);
  const params = {
    device: qs.get("device") || (matchMedia("(max-width: 760px)").matches ? "phone" : "desktop"),
    theme: qs.get("theme") === "light" ? "light" : "dark",
    track: qs.get("track") || "ptichki",
    t: qs.has("t") ? Number(qs.get("t")) : null,
    paused: qs.get("paused") === "1",
    bare: qs.get("bare") === "1",
    open: qs.get("open") !== "0",
    scroll: Number(qs.get("scroll") || 0),
    share: qs.has("share") ? Number(qs.get("share")) : null,
    hide: qs.get("hide") === "1",
    peek: qs.get("peek") === "1",
  };
  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  /** Строку подсвечиваем чуть раньше — как в приложении */
  const LEAD = 250;
  /** Пауза длиннее этого после конца строки — проигрыш */
  const BREAK_MS = 5000;
  /** Ручная прокрутка: столько лента ждёт, потом сама возвращается к текущей строке */
  const RESUME_MS = 6000;

  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const fmt = (ms) => {
    const s = Math.max(0, Math.floor(ms / 1000));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const VOWELS = /[аеёиоуыэюяaeiouy]/gi;
  const syllables = (w) => Math.max(1, (w.match(VOWELS) || []).length);

  // ---------- Модель текста ----------
  /** Слова строки с оценкой времени: время пения делится пропорционально слогам */
  function words(text, start, sing) {
    const parts = text.split(/\s+/).filter(Boolean);
    const syl = parts.map(syllables);
    const total = syl.reduce((a, b) => a + b, 0) || 1;
    let acc = 0;
    return parts.map((w, i) => {
      const s = start + (sing * acc) / total;
      acc += syl[i];
      return { w, s, e: start + (sing * acc) / total };
    });
  }

  function build(track) {
    const ly = track.lyrics;
    const dur = track.durationMs;
    if (ly.kind !== "synced") {
      const stanzas = (ly.plain || "").split(/\n\s*\n/).map((s) => s.split("\n").map((l) => l.trim()).filter(Boolean)).filter((s) => s.length);
      return { kind: ly.kind, stanzas, items: [] };
    }
    const src = ly.lines;
    const items = [];
    const firstText = src.find((l) => l.text);
    if (firstText && firstText.tMs >= 4000) items.push({ type: "break", tMs: 0, endMs: firstText.tMs, intro: true });
    for (let k = 0; k < src.length; k++) {
      const l = src[k];
      const next = src[k + 1];
      const nextStart = next ? next.tMs : dur;
      if (!l.text) {
        // Пустая строка у LRCLIB — проигрыш; короткие паузы не показываем
        const after = src.slice(k + 1).find((x) => x.text);
        const end = after ? after.tMs : dur;
        if (end - l.tMs >= 3000) items.push({ type: after ? "break" : "end", tMs: l.tMs, endMs: end });
        continue;
      }
      // Подпевка в скобках — отдельной строкой помельче, как у Apple Music
      const backing = [...l.text.matchAll(/\(([^)]*)\)/g)].map((m) => m[1].trim()).filter(Boolean).join(" ");
      let main = l.text.replace(/\([^)]*\)/g, "").replace(/\s+/g, " ").trim();
      const onlyBacking = !main;
      if (onlyBacking) main = backing;
      const syl = main.split(/\s+/).reduce((a, w) => a + syllables(w), 0);
      const est = clamp(syl * 240 + 500, 1300, 7000);
      const slot = nextStart - l.tMs;
      let endMs = nextStart;
      let tail = null;
      if (slot > est + BREAK_MS && next && next.text) {
        endMs = l.tMs + est;
        tail = { type: "break", tMs: endMs, endMs: nextStart };
      } else if (!next && slot > est + 3000) {
        endMs = l.tMs + est;
        tail = { type: "end", tMs: endMs, endMs: dur };
      }
      const span = endMs - l.tMs;
      const sing = slot <= est ? span * 0.94 : Math.min(span * 0.92, est * 1.5);
      const it = { type: "line", tMs: l.tMs, endMs, sing, text: l.text, main, onlyBacking, backing: onlyBacking ? "" : backing };
      it.words = words(main, l.tMs, onlyBacking ? sing : sing * (backing ? 0.78 : 1));
      if (it.backing) it.backWords = words(backing, l.tMs + sing * 0.78, sing * 0.22 + 200);
      items.push(it);
      if (tail) items.push(tail);
    }
    items.forEach((it, i) => (it.i = i));
    return { kind: "synced", items, lines: items.filter((x) => x.type === "line") };
  }

  /** Индекс текущего пункта: последний, что начался к t (с опережением) */
  function activeAt(items, t) {
    let lo = 0, hi = items.length - 1, found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (items[mid].tMs <= t + LEAD) { found = mid; lo = mid + 1; } else hi = mid - 1;
    }
    return found;
  }
  const progress = (it, t) => clamp((t - it.tMs) / Math.max(1, it.endMs - it.tMs));

  // ---------- Цвета обложки ----------
  function hsl(r, g, b) {
    r /= 255; g /= 255; b /= 255;
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b), l = (mx + mn) / 2;
    if (mx === mn) return [0, 0, l];
    const d = mx - mn, s = l > 0.5 ? d / (2 - mx - mn) : d / (mx + mn);
    let h = mx === r ? (g - b) / d + (g < b ? 6 : 0) : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return [h * 60, s, l];
  }
  function rgbOf(h, s, l) {
    const k = (n) => (n + h / 30) % 12, a = s * Math.min(l, 1 - l);
    const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, 9 - k(n), 1));
    return [f(0) * 255, f(8) * 255, f(4) * 255].map(Math.round);
  }
  const lum = ([r, g, b]) => {
    const c = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
    return 0.2126 * c(r) + 0.7152 * c(g) + 0.0722 * c(b);
  };
  const css = ([r, g, b], a = 1) => (a === 1 ? `rgb(${r} ${g} ${b})` : `rgb(${r} ${g} ${b} / ${a})`);
  /** Цвет того же тона с заданной яркостью (относительной, по WCAG) */
  function withLum(h, s, target) {
    let lo = 0, hi = 1;
    for (let i = 0; i < 18; i++) {
      const m = (lo + hi) / 2;
      if (lum(rgbOf(h, s, m)) < target) lo = m; else hi = m;
    }
    return rgbOf(h, s, (lo + hi) / 2);
  }

  /** Палитра обложки: несколько заметных тонов по частоте и насыщенности; серая обложка — mono */
  function palette(img) {
    const c = document.createElement("canvas");
    c.width = c.height = 40;
    const x = c.getContext("2d", { willReadFrequently: true });
    x.drawImage(img, 0, 0, 40, 40);
    const d = x.getImageData(0, 0, 40, 40).data;
    const bins = Array.from({ length: 18 }, () => ({ w: 0, r: 0, g: 0, b: 0, n: 0 }));
    let ar = 0, ag = 0, ab = 0, n = 0;
    for (let i = 0; i < d.length; i += 4) {
      const [r, g, b] = [d[i], d[i + 1], d[i + 2]];
      ar += r; ag += g; ab += b; n++;
      const [h, s, l] = hsl(r, g, b);
      const w = s * (1 - Math.abs(l - 0.5) * 1.6);
      if (w <= 0.05) continue;
      const bin = bins[Math.floor(h / 20) % 18];
      bin.w += w; bin.r += r * w; bin.g += g * w; bin.b += b * w; bin.n += w;
    }
    const avg = [ar / n, ag / n, ab / n].map(Math.round);
    const top = bins.filter((b) => b.w > 0).sort((a, b) => b.w - a.w);
    const mono = !top.length || top[0].w < n * 0.06;
    const tones = mono ? [avg] : top.slice(0, 3).map((b) => [b.r / b.n, b.g / b.n, b.b / b.n].map(Math.round));
    while (tones.length < 3) tones.push(tones[tones.length - 1]);
    const [h, s] = hsl(...tones[0]);
    return {
      mono, avg, tones, h, s,
      /** Сплошной фон под белый текст (и чёрный «впереди») — как у Spotify */
      solid: withLum(h, mono ? 0 : Math.min(s, 0.62), 0.085),
      /** Светлый вариант того же тона — для светлой темы */
      pastel: withLum(h, mono ? 0 : Math.min(s, 0.55), 0.5),
      /** Акцент для тёмного фона: читается на чернилах */
      accent: mono ? [255, 126, 179] : withLum(h, Math.max(0.6, Math.min(s, 0.9)), 0.42),
      accentLight: mono ? [214, 42, 112] : withLum(h, Math.max(0.6, Math.min(s, 0.9)), 0.16),
    };
  }
  function loadImg(src) {
    return new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = rej; i.src = src; });
  }

  // ---------- Часы ----------
  const clock = { t: 0, playing: true, dur: 1, subs: new Set(), seekSubs: new Set() };
  clock.seek = (ms) => { clock.t = clamp(ms, 0, clock.dur - 1); for (const f of clock.seekSubs) f(clock.t); };
  clock.toggle = (v) => { clock.playing = v ?? !clock.playing; for (const f of clock.seekSubs) f(clock.t); };
  let last = performance.now();
  function loop(now) {
    const dt = Math.min(100, now - last);
    last = now;
    if (clock.playing) {
      clock.t += dt;
      if (clock.t >= clock.dur) clock.seek(0);
    }
    for (const f of clock.subs) f(clock.t);
    requestAnimationFrame(loop);
  }

  // ---------- Слежение за строкой при обычной прокрутке ----------
  /** Лента сама держит строку на `anchor` высоты; человек прокрутил — пауза, кнопка «к текущей», через RESUME_MS — назад */
  function follower({ scroller, anchor = 0.33, getEl, pill, onState }) {
    let following = true, timer = 0, anim = 0;
    const offsetIn = (el) => { let y = 0; for (let p = el; p && p !== scroller; p = p.offsetParent) y += p.offsetTop; return y; };
    function targetOf(el) { return Math.max(0, offsetIn(el) + el.offsetHeight / 2 - scroller.clientHeight * anchor); }
    function animateTo(top, smooth) {
      cancelAnimationFrame(anim);
      const from = scroller.scrollTop, dist = top - from;
      if (!smooth || reduced || Math.abs(dist) < 2) { scroller.scrollTop = top; return; }
      const t0 = performance.now(), D = Math.min(900, 420 + Math.abs(dist) * 0.6);
      const step = (now) => {
        const k = clamp((now - t0) / D);
        const e = 1 - (1 - k) ** 4;
        scroller.scrollTop = from + dist * e;
        if (k < 1) anim = requestAnimationFrame(step);
      };
      anim = requestAnimationFrame(step);
    }
    function sync(smooth = true) { const el = getEl(); if (following && el) animateTo(targetOf(el), smooth); }
    function updatePill() {
      if (!pill) return;
      pill.classList.toggle("is-hidden", following);
      const el = getEl();
      if (el) pill.dataset.dir = offsetIn(el) < scroller.scrollTop ? "up" : "down";
    }
    function pause() {
      cancelAnimationFrame(anim);
      following = false;
      clearTimeout(timer);
      timer = setTimeout(resume, RESUME_MS);
      updatePill();
      onState?.(false);
    }
    function resume() {
      clearTimeout(timer);
      following = true;
      updatePill();
      onState?.(true);
      sync(true);
    }
    scroller.addEventListener("wheel", pause, { passive: true });
    scroller.addEventListener("touchmove", pause, { passive: true });
    scroller.addEventListener("keydown", (e) => { if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", " "].includes(e.key)) pause(); });
    scroller.addEventListener("scroll", () => { if (!following) { updatePill(); clearTimeout(timer); timer = setTimeout(resume, RESUME_MS); } }, { passive: true });
    pill?.addEventListener("click", resume);
    return { sync, pause, resume, get following() { return following; }, manual(dy) { pause(); scroller.scrollTop += dy; updatePill(); } };
  }

  // ---------- Долгое нажатие ----------
  function press(el, { tap, long }) {
    let timer = 0, x = 0, y = 0, fired = false;
    el.addEventListener("pointerdown", (e) => {
      fired = false; x = e.clientX; y = e.clientY;
      timer = setTimeout(() => { fired = true; long?.(e); }, 520);
    });
    el.addEventListener("pointermove", (e) => { if (Math.hypot(e.clientX - x, e.clientY - y) > 8) clearTimeout(timer); });
    el.addEventListener("pointerup", () => clearTimeout(timer));
    el.addEventListener("pointercancel", () => clearTimeout(timer));
    el.addEventListener("click", (e) => { if (fired) { e.preventDefault(); return; } tap?.(e); });
    el.addEventListener("contextmenu", (e) => { e.preventDefault(); clearTimeout(timer); if (!fired) long?.(e); fired = true; setTimeout(() => (fired = false), 50); });
  }

  // ---------- Разметка-помощники ----------
  const icon = (name, fill) => `<i class="${fill ? "ph-fill" : "ph"} ph-${name}" aria-hidden="true"></i>`;
  const glyph = (w = 24) => `<svg width="${w}" height="${w}" viewBox="0 0 100 100" aria-hidden="true"><path d="M16 31C18.83 37.33 27.33 67.33 33 69C38.67 70.67 44.33 41 50 41C55.67 41 61.33 70.67 67 69C72.67 67.33 81.17 37.33 84 31" fill="none" stroke="currentColor" stroke-width="11.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  const statusbar = () => `<div class="sb"><span>9:41</span><span class="sb__island"></span><span class="sb__icons">${icon("cell-signal-full", true)}${icon("wifi-high", true)}<span class="sb__bat"><i></i></span></span></div>`;
  const homeInd = () => `<div class="home-ind"></div>`;
  const wordsHtml = (ws) => ws.map((w, k) => `<span class="w" data-k="${k}">${esc(w.w)}</span>`).join(" ");

  /** Оболочка приложения на компьютере: боковая колонка, нижняя панель; main — содержимое варианта */
  function shell(ctx, mainHtml, { lyricsOn = true } = {}) {
    const { track, all } = ctx;
    const pins = all.map((t) => `<div class="side__pin"><img src="${t.cover}" alt=""><div><b>${esc(t.title)}</b><small>${esc(t.artists.join(", "))}</small></div></div>`).join("");
    return `<div class="shell">
      <nav class="side">
        <div class="side__logo">${glyph(26)}DalyWave</div>
        <div class="side__item">${icon("house")}Главная</div>
        <div class="side__item">${icon("magnifying-glass")}Поиск</div>
        <div class="side__item">${icon("books")}Медиатека</div>
        <div class="side__cap">Недавнее</div>${pins}
      </nav>
      <main class="main">${mainHtml}</main>
      <footer class="bar">
        <div class="bar__now"><img src="${track.cover}" alt=""><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div><button class="ib ib--sm" type="button" aria-label="В любимые">${icon("heart")}</button></div>
        <div class="bar__mid">
          <div class="bar__ctl"><button class="ib" type="button" aria-label="Перемешать">${icon("shuffle")}</button><button class="ib" type="button" aria-label="Предыдущий" data-act="prev">${icon("skip-back", true)}</button><button class="ib ib--play" type="button" aria-label="Пауза" data-act="play">${icon("pause", true)}</button><button class="ib" type="button" aria-label="Следующий">${icon("skip-forward", true)}</button><button class="ib" type="button" aria-label="Повтор">${icon("repeat")}</button></div>
          <div class="bar__prog"><span data-bind="pos">0:00</span><div class="track" data-act="seekbar"><i data-bind="fill"></i></div><span>${fmt(track.durationMs)}</span></div>
        </div>
        <div class="bar__right"><button class="ib ${lyricsOn ? "is-on" : ""}" type="button" aria-label="Текст" data-act="lyrics">${icon("microphone-stage", lyricsOn)}</button><button class="ib" type="button" aria-label="Очередь">${icon("queue")}</button><button class="ib" type="button" aria-label="Устройства">${icon("devices")}</button><button class="ib" type="button" aria-label="Громкость">${icon("speaker-high")}</button></div>
      </footer>
    </div>`;
  }

  /** Общие кнопки: пауза, перемотка по полосе, «в начало строки»; позиция и полоса на кадре */
  function wireCommon(root, track) {
    root.addEventListener("click", (e) => {
      const a = e.target.closest("[data-act]");
      if (!a) return;
      if (a.dataset.act === "play") clock.toggle();
      if (a.dataset.act === "prev") clock.seek(0);
      if (a.dataset.act === "seekbar") {
        const r = a.getBoundingClientRect();
        clock.seek(((e.clientX - r.left) / r.width) * track.durationMs);
      }
    });
    const pos = root.querySelectorAll('[data-bind="pos"]');
    const left = root.querySelectorAll('[data-bind="left"]');
    const fill = root.querySelectorAll('[data-bind="fill"]');
    const plays = root.querySelectorAll('[data-act="play"]');
    let lastSec = -1, lastPlaying = null;
    clock.subs.add((t) => {
      const s = Math.floor(t / 1000);
      if (s !== lastSec) {
        lastSec = s;
        pos.forEach((el) => (el.textContent = fmt(t)));
        left.forEach((el) => (el.textContent = `−${fmt(track.durationMs - t)}`));
      }
      fill.forEach((el) => (el.style.width = `${(t / track.durationMs) * 100}%`));
      if (lastPlaying !== clock.playing) {
        lastPlaying = clock.playing;
        plays.forEach((el) => { el.innerHTML = icon(clock.playing ? "pause" : "play", true); el.setAttribute("aria-label", clock.playing ? "Пауза" : "Слушать"); });
      }
    });
  }

  // ---------- Лист «Поделиться» ----------
  function share(ctx, lines, cardStyle = "") {
    const { stage, track } = ctx;
    const el = document.createElement("div");
    el.className = "share";
    el.innerHTML = `<div class="share__sheet" role="dialog" aria-label="Поделиться строкой">
      <div class="share__grab"></div>
      <div class="card" style="${cardStyle}">
        <p class="card__lines">${lines.map((l) => `<span>${esc(l)}</span>`).join("")}</p>
        <div class="card__meta"><img src="${track.cover}" alt=""><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div></div>
        <span class="card__logo">${glyph(20)}</span>
      </div>
      <div class="share__actions">
        <button class="share__act" type="button">${icon("copy")}Скопировать</button>
        <button class="share__act" type="button">${icon("image")}Картинка</button>
        <button class="share__act" type="button">${icon("paper-plane-tilt")}Отправить</button>
      </div>
    </div>`;
    stage.appendChild(el);
    requestAnimationFrame(() => el.classList.add("is-in"));
    const close = () => { el.classList.remove("is-in"); setTimeout(() => el.remove(), 350); };
    el.addEventListener("click", (e) => { if (e.target === el || e.target.closest(".share__act")) close(); });
    addEventListener("keydown", function k(e) { if (e.key === "Escape") { close(); removeEventListener("keydown", k); } });
    return close;
  }

  // ---------- Стенд ----------
  function harness(variant, model, track, tracks) {
    const url = (patch) => {
      const q = new URLSearchParams(location.search);
      q.set("t", String(Math.round(clock.t)));
      for (const [k, v] of Object.entries(patch)) { if (v == null) q.delete(k); else q.set(k, v); }
      return `?${q}`;
    };
    const seg = (key, opts, cur) => `<span class="seg">${opts.map(([v, l]) => `<button type="button" data-k="${key}" data-v="${v}" aria-pressed="${v === cur}">${l}</button>`).join("")}</span>`;
    const jumps = [];
    if (model.kind === "synced") {
      const brk = model.items.find((x) => x.type === "break" && !x.intro) || model.items.find((x) => x.type === "break");
      const l = model.lines[Math.min(6, model.lines.length - 1)];
      jumps.push(["Строка", l.tMs - 400]);
      if (brk) jumps.push(["Проигрыш", brk.tMs + 600]);
      jumps.push(["Начало", 0]);
    }
    const h = document.createElement("header");
    h.className = "harness";
    h.innerHTML = `<a href="./">${icon("arrow-left")} Все варианты</a><b>${variant}</b>
      ${seg("device", [["phone", "Телефон"], ["desktop", "Компьютер"]], params.device)}
      ${seg("theme", [["dark", "Тёмная"], ["light", "Светлая"]], params.theme)}
      ${seg("track", tracks.map((t) => [t.slug, t.lyrics.kind === "synced" ? t.title.replace("Да что романтичного в птичках?", "Птички") : `${t.title} · без времени`]), track.slug)}
      <span class="sep"></span>
      ${jumps.map(([l, t]) => `<button class="hbtn" type="button" data-jump="${t}">${l}</button>`).join("")}
      <button class="hbtn" type="button" data-toggle>${icon("pause", true)}</button>`;
    document.body.prepend(h);
    h.addEventListener("click", (e) => {
      const b = e.target.closest("button");
      if (!b) return;
      if (b.dataset.k) {
        const patch = { [b.dataset.k]: b.dataset.v };
        if (b.dataset.k === "track") patch.t = null;
        location.search = url(patch);
      }
      if (b.dataset.jump) clock.seek(Number(b.dataset.jump));
      if ("toggle" in b.dataset) clock.toggle();
    });
    const tg = h.querySelector("[data-toggle]");
    clock.seekSubs.add(() => (tg.innerHTML = icon(clock.playing ? "pause" : "play", true)));
    // Ссылки «без стенда» сохраняют время
    h.querySelector("a").addEventListener("click", () => {});
  }

  function fit(stage) {
    if (params.bare) return;
    const wrap = stage.parentElement;
    const k = Math.min(1, (wrap.clientWidth - 48) / stage.offsetWidth, (wrap.clientHeight - 48) / stage.offsetHeight);
    stage.style.transform = `scale(${k})`;
  }

  /** Запуск варианта: variant.render(ctx) строит экран, ctx.onFrame — кадры */
  async function boot(name, variant) {
    document.documentElement.dataset.theme = params.theme;
    document.body.dataset.device = params.device;
    if (params.bare) document.body.classList.add("bare");
    const res = await fetch("../fixtures/lyrics.json");
    const data = await res.json();
    const all = data.tracks.map((t) => ({ ...t, cover: t.cover }));
    const track = all.find((t) => t.slug === params.track) || all[0];
    const model = build(track);
    const img = await loadImg(track.cover);
    const pal = palette(img);
    clock.dur = track.durationMs;
    const firstLine = model.lines?.[3];
    clock.t = params.t ?? (firstLine ? firstLine.tMs - 300 : 0);
    clock.playing = !params.paused;

    const wrap = document.createElement("div");
    wrap.className = "stagewrap";
    const stage = document.createElement("div");
    stage.className = `stage stage--${params.device}`;
    stage.dataset.theme = params.theme;
    wrap.appendChild(stage);
    document.body.appendChild(wrap);
    if (!params.bare) harness(name, model, track, all);
    await document.fonts.ready;
    const ctx = { stage, track, all, model, pal, params, clock };
    variant(ctx);
    wireCommon(stage, track);
    fit(stage);
    addEventListener("resize", () => fit(stage));
    requestAnimationFrame(loop);
  }

  return { params, reduced, clamp, fmt, esc, build, activeAt, progress, palette, css, lum, withLum, rgbOf, hsl, clock, follower, press, icon, glyph, statusbar, homeInd, wordsHtml, shell, share, boot, LEAD };
})();
