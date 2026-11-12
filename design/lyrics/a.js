/* А · «Сцена» — по мотивам Apple Music.
   Живой фон из цветов обложки; крупные жирные строки; текущая — яркая, остальные приглушены и чуть размыты;
   лента едет пружиной, и строки догоняют друг друга лесенкой; слова «загораются» по оценке слогов;
   проигрыш — три дышащие точки, которые наполняются к началу строки; подпевка в скобках — отдельной строкой.
   Телефон: кнопка «Текст» внизу плеера — обложка сжимается в шапку, текст занимает экран, управление
   само прячется, пока человек слушает. Компьютер: полноэкранный плеер, справа текст. */
(() => {
  const { icon, esc, clamp, css, clock, params, statusbar, homeInd, wordsHtml, withLum, hsl, reduced, fmt } = LY;

  /** Пружина как CSS linear(): мягкая, с маленьким перелётом */
  function spring(k = 140, c = 19) {
    let x = 0, v = 0, t = 0;
    const dt = 1 / 120, pts = [];
    while (t < 2.5) {
      v += (k * (1 - x) - c * v) * dt; x += v * dt; t += dt; pts.push(x);
      if (t > 0.3 && Math.abs(1 - x) < 0.0015 && Math.abs(v) < 0.02) break;
    }
    const step = Math.max(1, Math.round(pts.length / 48));
    const s = pts.filter((_, i) => i % step === 0).map((p) => p.toFixed(3));
    return { ease: `linear(0, ${s.join(", ")}, 1)`, ms: Math.round(t * 1000) };
  }

  LY.boot("А · Сцена", (ctx) => {
    const { stage, track, model, pal } = ctx;
    const phone = params.device === "phone";
    const light = params.theme === "light";
    const sp = spring();
    // Фон: три тона обложки. Тёмная тема — глубокие, светлая — пастель
    const tones = pal.tones.map((c) => {
      const [h, s] = hsl(...c);
      return css(withLum(h, pal.mono ? 0 : Math.min(s * 1.1, 0.85), light ? 0.42 : 0.1));
    });
    const base = (() => { const [h, s] = hsl(...pal.avg); return css(withLum(h, pal.mono ? 0 : Math.min(s, 0.5), light ? 0.6 : 0.03)); })();
    stage.style.cssText += `;--c1:${tones[0]};--c2:${tones[1]};--c3:${tones[2]};--base:${base};--ease-spring:${sp.ease};--spring-ms:${sp.ms}ms`;

    const synced = model.kind === "synced";
    const items = model.items;
    const linesHtml = synced
      ? items.map((it) => {
          if (it.type === "break") return `<div class="a-line a-brk" data-i="${it.i}" role="button" aria-label="Проигрыш"><span class="a-dots"><i></i><i></i><i></i></span></div>`;
          if (it.type === "end") return `<div class="a-line a-end" data-i="${it.i}" aria-hidden="true"></div>`;
          return `<div class="a-line ${it.onlyBacking ? "is-back" : ""}" data-i="${it.i}" role="button" tabindex="0"><div class="a-main">${wordsHtml(it.words)}</div>${it.backing ? `<div class="a-back">${wordsHtml(it.backWords)}</div>` : ""}</div>`;
        }).join("")
      : model.stanzas.map((s) => `<p class="a-stanza">${s.map(esc).join("<br>")}</p>`).join("");

    const transport = `<div class="a-prog"><div class="track" data-act="seekbar"><i data-bind="fill"></i></div><div class="a-times"><span data-bind="pos">0:00</span><span data-bind="left"></span></div></div>
      <div class="a-transport"><button class="ib ib--lg" type="button" aria-label="Предыдущий" data-act="prev">${icon("rewind", true)}</button><button class="ib ib--xl" type="button" data-act="play">${icon("pause", true)}</button><button class="ib ib--lg" type="button" aria-label="Следующий">${icon("fast-forward", true)}</button></div>`;

    if (phone) {
      stage.innerHTML = `<div class="a a--phone ${params.open ? "is-open" : ""} ${synced ? "" : "is-plain"}">
        <div class="a-bg" aria-hidden="true"><i></i><i></i><i></i></div>
        ${statusbar()}
        <div class="a-grab"></div>
        <img class="a-cover" src="${track.cover}" alt="">
        <div class="a-meta a-meta--big"><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div><button class="ib ib--glass" type="button" aria-label="Ещё">${icon("dots-three", true)}</button></div>
        <div class="a-meta a-meta--small"><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div><button class="ib ib--glass ib--sm" type="button" aria-label="Ещё">${icon("dots-three", true)}</button></div>
        <section class="a-lyr" aria-label="Текст песни"><div class="a-list">${linesHtml}</div></section>
        <button class="pill is-hidden" type="button">${icon("arrow-down")}К текущей строке</button>
        <div class="a-ctl">${transport}
          <div class="a-row"><button class="ib a-lyrbtn" type="button" aria-label="Текст" aria-pressed="${params.open}">${icon("quotes", true)}</button><button class="ib" type="button" aria-label="Устройства">${icon("airplay")}</button><button class="ib" type="button" aria-label="Очередь">${icon("list-bullets")}</button></div>
        </div>
        ${homeInd()}
      </div>`;
    } else {
      stage.innerHTML = `<div class="a a--desk ${synced ? "" : "is-plain"}">
        <div class="a-bg" aria-hidden="true"><i></i><i></i><i></i></div>
        <header class="a-dtop"><button class="ib ib--glass" type="button" aria-label="Свернуть плеер">${icon("caret-down")}</button><span></span>
          <button class="ib ib--glass is-on" type="button" aria-label="Текст">${icon("quotes", true)}</button><button class="ib ib--glass" type="button" aria-label="Очередь">${icon("list-bullets")}</button><button class="ib ib--glass" type="button" aria-label="Ещё">${icon("dots-three", true)}</button></header>
        <div class="a-left">
          <img class="a-dcover" src="${track.cover}" alt="">
          <div class="a-dmeta"><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))} — ${esc(track.album)}</small></div><button class="ib ib--glass" type="button" aria-label="В любимые">${icon("heart")}</button></div>
          ${transport}
          <div class="a-vol">${icon("speaker-none", true)}<div class="track"><i style="width:62%"></i></div>${icon("speaker-high", true)}</div>
        </div>
        <section class="a-lyr" aria-label="Текст песни"><div class="a-list">${linesHtml}</div>
          <button class="pill is-hidden" type="button">${icon("arrow-down")}К текущей строке</button></section>
      </div>`;
    }

    const root = stage.querySelector(".a");
    const lyr = root.querySelector(".a-lyr");
    const list = root.querySelector(".a-list");
    const pill = root.querySelector(".pill");
    const els = [...list.querySelectorAll(".a-line")];

    // ---------- Открыть / закрыть текст на телефоне ----------
    let calmTimer = 0;
    function wake() {
      root.classList.remove("is-calm");
      clearTimeout(calmTimer);
      if (!phone || !synced) return;
      calmTimer = setTimeout(() => { if (root.classList.contains("is-open") && following && clock.playing) root.classList.add("is-calm"); }, 4000);
    }
    if (phone) {
      const btn = root.querySelector(".a-lyrbtn");
      const toggle = () => {
        const open = !root.classList.contains("is-open");
        root.classList.toggle("is-open", open);
        btn.setAttribute("aria-pressed", String(open));
        if (open) setTimeout(() => place(false), 30);
        wake();
      };
      btn.addEventListener("click", toggle);
      // Маленькая обложка в шапке — тоже «закрыть текст»
      root.querySelector(".a-cover").addEventListener("click", () => root.classList.contains("is-open") && toggle());
      root.addEventListener("pointerdown", wake);
      root.addEventListener("pointermove", (e) => e.pointerType === "mouse" && wake());
      if (params.hide) setTimeout(() => root.classList.add("is-calm"), 50);
      else wake();
    }

    if (!synced) {
      // Простой текст: обычная прокрутка, без слежения
      lyr.classList.add("is-scroll");
      return;
    }

    // ---------- Лента на пружине ----------
    const anchor = phone ? 0.26 : 0.36;
    let off = 0, following = true, active = -2, resumeT = 0, dragged = false;
    const vh = () => lyr.clientHeight;
    // На телефоне текущая строка стоит на одной высоте, даже когда управление прячется и окно ленты растёт
    const anchorPx = () => (phone ? 96 : vh() * anchor);
    function targetOff(i) {
      const el = els[Math.max(0, i)];
      return el ? el.offsetTop - anchorPx() : 0;
    }
    function setOff(v) {
      off = v;
      list.style.setProperty("--off", `${-off}px`);
    }
    function place(smooth = true) {
      if (!following) return;
      list.classList.toggle("is-instant", !smooth || reduced);
      setOff(targetOff(active));
      if (!smooth) requestAnimationFrame(() => list.classList.remove("is-instant"));
    }
    function stagger() {
      // Строки ниже текущей догоняют её с задержкой — «лесенка», как у Apple
      els.forEach((el, i) => {
        const d = i - active;
        el.style.setProperty("--d", `${d < -1 ? 0 : Math.min(8, Math.max(0, d + 1)) * 42}ms`);
        const dist = Math.abs(d);
        el.style.setProperty("--blur", following && dist > 0 ? `${Math.min(2.8, 0.5 + dist * 0.55).toFixed(2)}px` : "0px");
      });
    }
    function setPill() {
      pill.classList.toggle("is-hidden", following);
      const el = els[Math.max(0, active)];
      if (el) pill.dataset.dir = el.offsetTop - off < 0 ? "up" : "down";
    }
    function manual(dy) {
      following = false;
      list.classList.add("is-manual");
      root.classList.add("is-manual");
      const max = list.scrollHeight - vh() * 0.5, min = -anchorPx();
      setOff(clamp(off + dy, min, max));
      stagger();
      setPill();
      wake();
      clearTimeout(resumeT);
      resumeT = setTimeout(resume, 6000);
    }
    function resume() {
      clearTimeout(resumeT);
      following = true;
      list.classList.remove("is-manual");
      root.classList.remove("is-manual");
      stagger();
      place(true);
      setPill();
      wake();
    }
    pill.addEventListener("click", resume);
    lyr.addEventListener("wheel", (e) => { e.preventDefault(); manual(e.deltaY); }, { passive: false });
    let y0 = 0, o0 = 0, down = false;
    lyr.addEventListener("pointerdown", (e) => { down = true; dragged = false; y0 = e.clientY; o0 = off; });
    addEventListener("pointermove", (e) => {
      if (!down) return;
      const k = stage.getBoundingClientRect().height / stage.offsetHeight || 1;
      const dy = (e.clientY - y0) / k;
      if (!dragged && Math.abs(dy) > 6) dragged = true;
      if (dragged) manual(o0 - dy - off);
    });
    addEventListener("pointerup", () => { down = false; setTimeout(() => (dragged = false), 0); });

    // Нажатие — перемотка к строке; долгое — поделиться
    els.forEach((el) => {
      const it = items[Number(el.dataset.i)];
      if (it.type === "end") return;
      LY.press(el, {
        tap: () => { if (dragged) return; clock.seek(it.tMs); following = true; resume(); },
        long: () => { if (it.type === "line") openShare(it); },
      });
    });
    function openShare(it) {
      const next = items[it.i + 1];
      const lines = [it.main];
      if (next && next.type === "line") lines.push(next.main);
      LY.share(ctx, lines, `background: linear-gradient(150deg, ${tones[0]}, ${tones[1]} 55%, ${tones[2]}); color: ${light ? "#0b0c10" : "#fff"}`);
    }

    // ---------- Кадр ----------
    let lastWordsKey = "";
    clock.seekSubs.add(() => { lastWordsKey = ""; });
    clock.subs.add((t) => {
      const a = LY.activeAt(items, t);
      if (a !== active) {
        const prev = active;
        active = a;
        els.forEach((el, i) => {
          el.classList.toggle("is-active", i === a);
          el.classList.toggle("is-past", i < a);
        });
        // Ушедшая строка: слова остаются «загоревшимися», пока она гаснет
        if (prev >= 0 && els[prev]) els[prev].querySelectorAll(".w").forEach((w) => w.style.setProperty("--p", "1"));
        stagger();
        place(Math.abs(a - prev) < 6);
        setPill();
      }
      const it = items[a];
      if (!it) return;
      const el = els[a];
      if (it.type === "line") {
        const ws = el.querySelectorAll(".a-main .w");
        let key = "";
        it.words.forEach((w, k) => {
          const p = clamp((t + LY.LEAD * 0.4 - w.s) / Math.max(120, w.e - w.s));
          key += p.toFixed(2);
          ws[k].style.setProperty("--p", p.toFixed(3));
        });
        if (it.backWords) {
          const bs = el.querySelectorAll(".a-back .w");
          it.backWords.forEach((w, k) => bs[k].style.setProperty("--p", clamp((t - w.s) / Math.max(120, w.e - w.s)).toFixed(3)));
        }
        lastWordsKey = key;
      } else if (it.type === "break") {
        const p = LY.progress(it, t);
        el.style.setProperty("--bp", p.toFixed(3));
        el.classList.toggle("is-ending", it.endMs - t < 650);
        el.querySelectorAll("i").forEach((d, k) => d.style.setProperty("--f", clamp(p * 3 - k).toFixed(3)));
      }
    });

    // Первая расстановка без проезда; сцены для скриншотов
    requestAnimationFrame(() => {
      active = LY.activeAt(items, clock.t);
      els.forEach((el, i) => { el.classList.toggle("is-active", i === active); el.classList.toggle("is-past", i < active); });
      stagger();
      place(false);
      if (params.scroll) setTimeout(() => manual(params.scroll), 300);
      if (params.share != null) setTimeout(() => openShare(model.lines[params.share]), 300);
    });
  });
})();
