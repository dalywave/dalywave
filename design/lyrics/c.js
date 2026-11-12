/* В · «Полночь» — своё направление.
   Чёрный экран «Полночи», цвет — только у волны (тон берётся из обложки). Текст по центру, текущая строка
   крупная, остальные мельче и тише. Под текущей строкой волна DalyWave прорисовывается слева направо ровно
   за время строки — это честные данные (начало следующей строки), без угадывания слов. Проигрыш — живая
   волна, которая сходит на нет к началу строки; следующая строка уже видна, чтобы успеть вступить.
   Справа — «ритм песни»: строки на оси времени (густо — речитатив, пусто — проигрыш); по ней можно
   водить пальцем. Простой текст — как стихи: строфы по центру, повторы припева свёрнуты.
   Телефон: текущая строка живёт под обложкой; нажатие — обложка уходит в шапку, текст на весь экран;
   закрыть — шапкой или стрелкой вниз. Компьютер: текст в центральной колонке приложения. */
(() => {
  const { icon, esc, clamp, css, clock, params, statusbar, homeInd, reduced, fmt } = LY;

  /** Волна-подчёркивание: синус постоянной длины волны под текст любой ширины */
  function wavePath(w, h = 10, len = 18, amp = 2.6, phase = 0) {
    let d = `M0 ${h / 2 + Math.sin(phase) * amp}`;
    for (let x = 2; x <= w; x += 2) d += ` L${x} ${(h / 2 + Math.sin((x / len) * Math.PI * 2 + phase) * amp).toFixed(2)}`;
    return d;
  }
  const waveSvg = (w, cls = "c-wave") => `<svg class="${cls}" width="${w}" height="10" viewBox="0 0 ${w} 10" aria-hidden="true"><path class="c-wave__track" d="${wavePath(w)}" pathLength="1"/><path class="c-wave__fill" d="${wavePath(w)}" pathLength="1"/></svg>`;

  LY.boot("В · Полночь", (ctx) => {
    const { stage, track, model, pal } = ctx;
    const phone = params.device === "phone";
    const light = params.theme === "light";
    stage.style.cssText += `;--acc:${css(light ? pal.accentLight : pal.accent)}`;
    const synced = model.kind === "synced";
    const items = model.items;

    const lineHtml = (it) => {
      if (it.type === "break") return `<div class="c-line c-brk" data-i="${it.i}" role="button" aria-label="Проигрыш"><svg class="c-bw" width="132" height="24" viewBox="0 0 132 24" aria-hidden="true"><path d="${wavePath(132, 24, 26, 2)}" pathLength="1"/></svg></div>`;
      if (it.type === "end") return `<div class="c-line c-end" data-i="${it.i}" aria-hidden="true"></div>`;
      return `<div class="c-line ${it.onlyBacking ? "is-back" : ""}" data-i="${it.i}" role="button" tabindex="0"><span class="c-txt">${esc(it.main)}${it.backing ? `<small class="c-backing">${esc(it.backing)}</small>` : ""}</span>${phone ? "" : `<button class="c-share" type="button" aria-label="Поделиться строкой">${icon("share-fat")}</button>`}</div>`;
    };

    // Простой текст: строфы; повтор строфы (припев) свёрнут до заголовка
    let plainHtml = "";
    if (!synced) {
      const key = (s) => s.join("\n").toLowerCase();
      const counts = new Map();
      model.stanzas.forEach((s) => counts.set(key(s), (counts.get(key(s)) || 0) + 1));
      const seen = new Set();
      plainHtml = model.stanzas.map((s) => {
        const k = key(s);
        const chorus = counts.get(k) > 1;
        const body = `<p class="c-stanza">${s.map(esc).join("<br>")}</p>`;
        if (!chorus) return body;
        if (!seen.has(k)) { seen.add(k); return `<div class="c-chorus"><span class="c-label">Припев</span>${body}</div>`; }
        return `<details class="c-repeat"><summary><span class="c-label">Припев</span>${icon("caret-down")}</summary>${body}</details>`;
      }).join("");
    }
    const listHtml = synced ? items.map(lineHtml).join("") : plainHtml;
    const rhythm = synced
      ? `<div class="c-rhythm" role="slider" aria-label="Ритм песни: перемотка по строкам" aria-valuemin="0" aria-valuemax="${track.durationMs}" tabindex="0">
          ${items.filter((x) => x.type === "line").map((x) => `<i data-i="${x.i}" style="top:${(x.tMs / track.durationMs) * 100}%;width:${Math.round(5 + Math.min(13, x.main.length / 3.2))}px"></i>`).join("")}
          <b class="c-window"></b><span class="c-head"></span></div>`
      : "";

    if (phone) {
      const firstPlain = model.stanzas?.[0]?.[0] ?? "";
      stage.innerHTML = `<div class="c c--phone ${params.open ? "is-open" : ""} ${synced ? "" : "is-plain"}">
        ${statusbar()}
        <header class="c-top"><button class="ib" type="button" aria-label="Свернуть плеер">${icon("caret-down")}</button><div class="c-top__ctx"><small>Играет из альбома</small><b>${esc(track.album)}</b></div><button class="ib" type="button" aria-label="Ещё">${icon("dots-three", true)}</button></header>
        <img class="c-cover" src="${track.cover}" alt="">
        <div class="c-meta c-meta--big"><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div><button class="ib" type="button" aria-label="В любимые">${icon("heart")}</button></div>
        <button class="c-strip" type="button" aria-label="Открыть текст"><span class="c-strip__t">${synced ? "" : esc(firstPlain)}</span>${synced ? waveSvg(200, "c-wave c-strip__w") : ""}<span class="c-strip__go">${icon("caret-up")}</span></button>
        <div class="c-meta c-meta--small"><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div><button class="ib" type="button" aria-label="Закрыть текст" data-close>${icon("caret-down")}</button></div>
        <section class="c-lyr" aria-label="Текст песни"><div class="c-scroll" tabindex="0"><div class="c-list">${listHtml}</div></div>${rhythm}</section>
        <button class="pill is-hidden" type="button">${icon("arrow-down")}К текущей строке</button>
        <div class="c-ctl c-ctl--big">
          <div class="c-prog"><div class="track" data-act="seekbar"><i data-bind="fill"></i></div><div class="c-times"><span data-bind="pos"></span><span data-bind="left"></span></div></div>
          <div class="c-transport"><button class="ib" type="button" aria-label="Перемешать">${icon("shuffle")}</button><button class="ib ib--lg" type="button" aria-label="Предыдущий" data-act="prev">${icon("skip-back", true)}</button><button class="ib ib--play" type="button" data-act="play"></button><button class="ib ib--lg" type="button" aria-label="Следующий">${icon("skip-forward", true)}</button><button class="ib" type="button" aria-label="Повтор">${icon("repeat")}</button></div>
          <div class="c-row"><button class="ib ib--sm" type="button" aria-label="Устройства">${icon("devices")}</button><button class="ib ib--sm" type="button" aria-label="Волна">${icon("sparkle")}</button><button class="ib ib--sm" type="button" aria-label="Очередь">${icon("queue")}</button></div>
        </div>
        <div class="c-ctl c-ctl--mini">
          <div class="track" data-act="seekbar"><i data-bind="fill"></i></div>
          <div class="c-mini"><span data-bind="pos"></span><div><button class="ib" type="button" aria-label="Предыдущий" data-act="prev">${icon("skip-back", true)}</button><button class="ib ib--play ib--mid" type="button" data-act="play"></button><button class="ib" type="button" aria-label="Следующий">${icon("skip-forward", true)}</button></div><span data-bind="left"></span></div>
        </div>
        ${homeInd()}
      </div>`;
    } else {
      stage.innerHTML = LY.shell(ctx, `<div class="c c--desk ${synced ? "" : "is-plain"}">
          <header class="c-dhead"><img src="${track.cover}" alt=""><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))} · ${esc(track.album)}</small></div></header>
          <section class="c-lyr" aria-label="Текст песни"><div class="c-scroll" tabindex="0"><div class="c-list">${listHtml}</div></div>${rhythm}</section>
          <button class="pill is-hidden" type="button">${icon("arrow-down")}К текущей строке</button>
        </div>`);
    }

    const root = stage.querySelector(".c");
    const scroller = root.querySelector(".c-scroll");
    const list = root.querySelector(".c-list");
    const pill = root.querySelector(".pill");
    const els = [...list.querySelectorAll(".c-line")];

    if (phone) {
      const strip = root.querySelector(".c-strip");
      const setOpen = (v) => {
        root.classList.toggle("is-open", v);
        if (v && follow) setTimeout(() => follow.sync(false), 20);
      };
      strip.addEventListener("click", () => setOpen(true));
      root.querySelector("[data-close]").addEventListener("click", () => setOpen(false));
      root.querySelector(".c-cover").addEventListener("click", () => root.classList.contains("is-open") && setOpen(false));
    }
    let follow = null;
    if (!synced) return;

    // ---------- Слежение и перемотка ----------
    let active = -2;
    const strip = root.querySelector(".c-strip");
    const stripT = strip?.querySelector(".c-strip__t");
    const stripW = strip?.querySelector(".c-strip__w");
    const rh = root.querySelector(".c-rhythm");
    const ticks = rh ? [...rh.querySelectorAll("i")] : [];
    const head = rh?.querySelector(".c-head");
    const win = rh?.querySelector(".c-window");
    follow = LY.follower({ scroller, anchor: phone ? 0.36 : 0.4, getEl: () => els[Math.max(0, active)], pill, onState: (on) => { root.classList.toggle("is-manual", !on); showWindow(); } });

    function share(it) {
      LY.share(ctx, [it.main], light ? "background:#fff;color:#0b0c10;box-shadow:0 0 0 1px rgba(10,12,18,.08)" : "background:#0e1015;color:#f3f5f9;box-shadow:0 0 0 1px rgba(255,255,255,.08)");
      const card = stage.querySelector(".share .card");
      card.querySelector(".card__lines").insertAdjacentHTML("afterend", `<svg class="card__wave" viewBox="0 0 296 10" preserveAspectRatio="none" aria-hidden="true"><path d="${wavePath(296)}"/></svg>`);
    }
    els.forEach((el) => {
      const it = items[Number(el.dataset.i)];
      if (it.type === "end") return;
      LY.press(el, {
        tap: (e) => { if (e.target.closest(".c-share")) return; clock.seek(it.tMs); follow.resume(); },
        long: () => it.type === "line" && share(it),
      });
      el.querySelector(".c-share")?.addEventListener("click", (e) => { e.stopPropagation(); share(it); });
    });

    // «Ритм песни»: водишь — текст едет к строке, отпускаешь — перемотка туда
    if (rh) {
      let drag = false;
      const at = (e) => {
        const r = rh.getBoundingClientRect();
        const t = clamp((e.clientY - r.top) / r.height) * track.durationMs;
        const k = Math.max(0, LY.activeAt(items, t - LY.LEAD));
        return items[k];
      };
      rh.addEventListener("pointerdown", (e) => { drag = true; rh.setPointerCapture(e.pointerId); const it = at(e); if (it) { clock.seek(it.tMs); follow.resume(); } });
      rh.addEventListener("pointermove", (e) => { if (!drag) return; const it = at(e); if (it && Math.abs(it.tMs - clock.t) > 500) { clock.seek(it.tMs); follow.resume(); } });
      rh.addEventListener("pointerup", () => (drag = false));
    }
    function showWindow() {
      if (!win) return;
      if (follow.following) { win.style.opacity = "0"; return; }
      // Что сейчас видно в ленте — рамкой на оси времени
      const top = scroller.scrollTop, bottom = top + scroller.clientHeight;
      let a = null, b = null;
      els.forEach((el, i) => {
        const y = el.offsetTop;
        if (y + el.offsetHeight > top && y < bottom) { if (a == null) a = i; b = i; }
      });
      if (a == null) return;
      const dur = track.durationMs;
      const t0 = items[a].tMs, t1 = items[b].endMs;
      win.style.opacity = "1";
      win.style.top = `${(t0 / dur) * 100}%`;
      win.style.height = `${((t1 - t0) / dur) * 100}%`;
    }
    scroller.addEventListener("scroll", () => !follow.following && showWindow(), { passive: true });

    function activate(a) {
      els.forEach((el, i) => {
        el.classList.toggle("is-active", i === a);
        el.classList.toggle("is-past", i < a);
        el.classList.toggle("is-next", i === a + 1 && items[a]?.type === "break");
      });
      ticks.forEach((tk) => { const i = Number(tk.dataset.i); tk.classList.toggle("is-on", i === a); tk.classList.toggle("is-past", i < a); });
      const it = items[a];
      const el = els[a];
      list.querySelectorAll(".c-wave").forEach((w) => w.remove());
      if (it?.type === "line") {
        // Ширина волны — по самой длинной строке текста, а не по всей колонке
        const txt = el.querySelector(".c-txt");
        const r = document.createRange();
        r.selectNodeContents(txt.firstChild);
        const k = el.getBoundingClientRect().width / el.offsetWidth || 1;
        const w = Math.max(24, Math.round(Math.max(...[...r.getClientRects()].map((x) => x.width)) / k));
        txt.insertAdjacentHTML("beforeend", waveSvg(w));
        txt.lastElementChild.style.left = `${(txt.offsetWidth - w) / 2}px`;
      }
      if (stripT) {
        const brk = it?.type === "break";
        strip.classList.toggle("is-break", brk || !it);
        stripT.textContent = it?.type === "line" ? it.main : brk ? items[a + 1]?.main ?? "" : model.lines[0].main;
        stripT.classList.remove("is-in");
        stripT.offsetWidth;
        stripT.classList.add("is-in");
        if (stripW) {
          const w = Math.max(24, Math.min(stripT.offsetWidth, 300));
          stripW.setAttribute("width", w);
          stripW.setAttribute("viewBox", `0 0 ${w} 10`);
          stripW.querySelectorAll("path").forEach((p) => p.setAttribute("d", wavePath(w)));
        }
      }
    }

    clock.subs.add((t) => {
      const a = LY.activeAt(items, t);
      if (a !== active) {
        const jump = Math.abs(a - active) > 4;
        active = a;
        activate(a);
        follow.sync(!jump);
      }
      if (head) head.style.top = `${(t / track.durationMs) * 100}%`;
      const it = items[a];
      if (!it) return;
      const el = els[a];
      if (it.type === "line") {
        const p = LY.progress({ tMs: it.tMs, endMs: it.tMs + Math.max(it.sing, Math.min(it.endMs - it.tMs, it.sing * 1.2)) }, t);
        const f = el.querySelector(".c-wave__fill");
        if (f) f.style.strokeDashoffset = String(1 - p);
        if (stripW) stripW.querySelector(".c-wave__fill").style.strokeDashoffset = String(1 - p);
      } else if (it.type === "break") {
        // Волна живёт и укорачивается к центру — сколько осталось до строки
        const p = LY.progress(it, t);
        const path = el.querySelector(".c-bw path");
        const amp = 3 + 4 * Math.sin(Math.min(1, p * 4) * Math.PI / 2) * (1 - p * 0.6);
        path.setAttribute("d", wavePath(132, 24, 26, amp, (t / 1000) * 3.2));
        path.style.strokeDasharray = `${1 - p} 1`;
        path.style.strokeDashoffset = String(-p / 2);
        if (stripW) stripW.querySelector(".c-wave__fill").style.strokeDashoffset = "1";
      }
    });

    requestAnimationFrame(() => {
      active = LY.activeAt(items, clock.t);
      activate(active);
      follow.sync(false);
      if (params.scroll) setTimeout(() => { follow.manual(params.scroll); showWindow(); }, 300);
      if (params.share != null) setTimeout(() => share(model.lines[params.share]), 300);
    });
  });
})();
