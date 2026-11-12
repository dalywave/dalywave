/* Б · «Карточка» — по мотивам Spotify.
   Плоский цвет обложки вместо картинки; спетое — белым, впереди — тёмным, текущая строка заливается
   белым слева направо (оценка по буквам внутри настоящего времени строки). Проигрыш — нота, которая
   наполняется снизу. Телефон: под плеером карточка «Текст» с живыми строками — нажатие раскрывает её
   на весь экран, стрелка вниз (или смахивание) сворачивает обратно в карточку. Поделиться: долгое
   нажатие — режим выбора строк (до пяти), затем карточка для отправки.
   Компьютер: текст занимает центральную колонку приложения, кнопка «Текст» в нижней панели. */
(() => {
  const { icon, esc, clamp, css, clock, params, statusbar, homeInd, withLum, reduced, fmt } = LY;

  /** Время слов по буквам: заливка идёт равномерно по строке */
  function charWords(it) {
    const ws = it.main.split(/\s+/).filter(Boolean);
    const total = ws.reduce((a, w) => a + w.length + 1, 0);
    let acc = 0;
    return ws.map((w) => {
      const s = it.tMs + (it.sing * acc) / total;
      acc += w.length + 1;
      return { w, s, e: it.tMs + (it.sing * acc) / total };
    });
  }

  LY.boot("Б · Карточка", (ctx) => {
    const { stage, track, model, pal } = ctx;
    const phone = params.device === "phone";
    const light = params.theme === "light";
    const s = pal.mono ? 0 : Math.min(pal.s, 0.7);
    const bc = css(withLum(pal.h, s, light ? 0.42 : 0.11));
    const bp = css(withLum(pal.h, pal.mono ? 0 : Math.min(pal.s, 0.6), light ? 0.8 : 0.028));
    stage.style.cssText += `;--bc:${bc};--bp:${bp}`;
    const synced = model.kind === "synced";
    const items = model.items;
    items.forEach((it) => { if (it.type === "line") it.cw = charWords(it); });

    const lineHtml = (it) => {
      if (it.type === "break") return `<div class="b-line b-brk" data-i="${it.i}" role="button" aria-label="Проигрыш"><span class="b-note">♪</span></div>`;
      if (it.type === "end") return `<div class="b-line b-end" data-i="${it.i}" aria-hidden="true"></div>`;
      const back = it.backing ? ` <span class="b-backing">(${esc(it.backing)})</span>` : "";
      return `<div class="b-line ${it.onlyBacking ? "is-back" : ""}" data-i="${it.i}" role="button" tabindex="0">${it.cw.map((w) => `<span class="w">${esc(w.w)}</span>`).join(" ")}${back}</div>`;
    };
    const listHtml = synced ? items.map(lineHtml).join("") : model.stanzas.map((st) => `<p class="b-stanza">${st.map(esc).join("<br>")}</p>`).join("");
    const cardHtml = synced ? items.map(lineHtml).join("") : model.stanzas.slice(0, 2).map((st) => `<p class="b-stanza">${st.map(esc).join("<br>")}</p>`).join("");
    const selbarHtml = `<div class="b-selbar is-hidden"><button class="b-selbtn" type="button" data-sel="cancel">Отмена</button><button class="b-selbtn b-selbtn--main" type="button" data-sel="share">${icon("share-fat", true)}Поделиться</button></div>`;

    if (phone) {
      stage.innerHTML = `<div class="b b--phone">
        ${statusbar()}
        <div class="b-player">
          <header class="b-head"><button class="ib" type="button" aria-label="Свернуть">${icon("caret-down")}</button><div class="b-head__ctx"><small>Играет из альбома</small><b>${esc(track.album)}</b></div><button class="ib" type="button" aria-label="Ещё">${icon("dots-three", true)}</button></header>
          <img class="b-cover" src="${track.cover}" alt="">
          <div class="b-meta"><div><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div><button class="ib" type="button" aria-label="В любимые">${icon("plus-circle")}</button></div>
          <div class="b-prog"><div class="track" data-act="seekbar"><i data-bind="fill"></i></div><div class="b-times"><span data-bind="pos"></span><span>${fmt(track.durationMs)}</span></div></div>
          <div class="b-transport"><button class="ib" type="button" aria-label="Перемешать">${icon("shuffle")}</button><button class="ib ib--lg" type="button" aria-label="Предыдущий" data-act="prev">${icon("skip-back", true)}</button><button class="ib ib--play" type="button" data-act="play"></button><button class="ib ib--lg" type="button" aria-label="Следующий">${icon("skip-forward", true)}</button><button class="ib" type="button" aria-label="Повтор">${icon("repeat")}</button></div>
          <div class="b-row"><button class="ib ib--sm" type="button" aria-label="Устройства">${icon("devices")}</button><span></span><button class="ib ib--sm" type="button" aria-label="Поделиться">${icon("share-network")}</button><button class="ib ib--sm" type="button" aria-label="Очередь">${icon("queue")}</button></div>
          <section class="b-card" role="button" tabindex="0" aria-label="Открыть текст">
            <header class="b-card__head"><b>Текст</b><span></span><span class="b-card__btn" aria-hidden="true">${icon("arrows-out-simple")}</span></header>
            <div class="b-card__body"><div class="b-card__list">${cardHtml}</div></div>
          </section>
          <div class="b-gap"></div>
        </div>
        <div class="b-full" aria-hidden="true">
          <header class="b-fhead"><button class="ib" type="button" aria-label="Свернуть текст" data-close>${icon("caret-down")}</button><div class="b-fhead__t"><b>${esc(track.title)}</b><small>${esc(track.artists.join(", "))}</small></div><button class="ib" type="button" aria-label="Поделиться строками" data-select>${icon("share-fat")}</button></header>
          <div class="b-scroll" tabindex="0"><div class="b-list">${listHtml}</div></div>
          <button class="pill is-hidden" type="button">${icon("arrow-down")}Вернуться к текущей</button>
          <footer class="b-ffoot"><div class="b-prog"><div class="track" data-act="seekbar"><i data-bind="fill"></i></div><div class="b-times"><span data-bind="pos"></span><span data-bind="left"></span></div></div><button class="ib ib--play" type="button" data-act="play"></button></footer>
          ${selbarHtml}
        </div>
        ${homeInd()}
      </div>`;
    } else {
      stage.innerHTML = LY.shell(ctx, `<div class="b b--desk">
          <div class="b-panel">
            <div class="b-scroll" tabindex="0"><div class="b-list">${listHtml}</div></div>
            <button class="pill is-hidden" type="button">${icon("arrow-down")}Вернуться к текущей</button>
            ${selbarHtml}
          </div>
        </div>`);
    }

    const root = stage.querySelector(".b");
    const full = phone ? root.querySelector(".b-full") : root;
    const scroller = full.querySelector(".b-scroll");
    const list = full.querySelector(".b-list");
    const pill = full.querySelector(".pill");
    const els = [...list.querySelectorAll(".b-line")];
    const card = root.querySelector(".b-card");
    const cardList = root.querySelector(".b-card__list");
    const cardEls = cardList ? [...cardList.querySelectorAll(".b-line")] : [];

    let follow = null;
    // ---------- Карточка ↔ весь экран ----------
    const player = root.querySelector(".b-player");
    function cardClip() {
      const r = card.getBoundingClientRect(), s = stage.getBoundingClientRect(), k = s.height / stage.offsetHeight || 1;
      const top = (r.top - s.top) / k, left = (r.left - s.left) / k, w = r.width / k, h = r.height / k;
      return `inset(${top}px ${stage.offsetWidth - left - w}px ${stage.offsetHeight - top - h}px ${left}px round 16px)`;
    }
    function open(animate = true) {
      full.style.transition = "none";
      full.style.clipPath = cardClip();
      full.offsetWidth;
      full.style.transition = animate && !reduced ? "" : "none";
      root.classList.add("is-open");
      full.style.clipPath = "inset(0px 0px 0px 0px round 0px)";
      full.setAttribute("aria-hidden", "false");
      follow?.sync(false);
    }
    function close() {
      if (selecting) endSelect();
      full.style.clipPath = cardClip();
      root.classList.remove("is-open");
      full.setAttribute("aria-hidden", "true");
    }
    if (phone) {
      card.addEventListener("click", () => open());
      full.querySelector("[data-close]").addEventListener("click", close);
      // Смахнуть вниз по шапке — свернуть
      let sy = null;
      full.querySelector(".b-fhead").addEventListener("pointerdown", (e) => (sy = e.clientY));
      addEventListener("pointerup", (e) => { if (sy != null && e.clientY - sy > 60) close(); sy = null; });
      if (params.peek) player.scrollTop = card.offsetTop - 150;
    }

    // ---------- Выбор строк, чтобы поделиться ----------
    let selecting = false;
    const sel = new Set();
    const selbar = full.querySelector(".b-selbar");
    function startSelect(i) {
      selecting = true;
      root.classList.add("is-selecting");
      selbar.classList.remove("is-hidden");
      follow?.pause();
      toggleSel(i);
    }
    function toggleSel(i) {
      if (sel.has(i)) sel.delete(i);
      else if (sel.size < 5) sel.add(i);
      els.forEach((el) => el.classList.toggle("is-sel", sel.has(Number(el.dataset.i))));
      selbar.querySelector(".b-selbtn--main").disabled = !sel.size;
    }
    function endSelect() {
      selecting = false;
      sel.clear();
      root.classList.remove("is-selecting");
      selbar.classList.add("is-hidden");
      els.forEach((el) => el.classList.remove("is-sel"));
    }
    selbar.addEventListener("click", (e) => {
      const b = e.target.closest("[data-sel]");
      if (!b) return;
      if (b.dataset.sel === "share") {
        const lines = [...sel].sort((a, b) => a - b).map((i) => items[i].main);
        LY.share(ctx, lines, `background:${bc};color:${light ? "#0b0c10" : "#fff"}`);
      }
      endSelect();
      follow?.resume();
    });
    full.querySelector("[data-select]")?.addEventListener("click", () => {
      if (selecting) { endSelect(); return; }
      const a = LY.activeAt(items, clock.t);
      startSelect(items[a]?.type === "line" ? a : model.lines[0].i);
    });

    if (!synced) {
      root.classList.add("is-plain");
      if (phone && params.open) requestAnimationFrame(() => open(false));
      return;
    }

    // ---------- Слежение ----------
    let active = -2;
    follow = LY.follower({ scroller, anchor: phone ? 0.3 : 0.32, getEl: () => els[Math.max(0, active)], pill });
    els.forEach((el) => {
      const it = items[Number(el.dataset.i)];
      if (it.type === "end") return;
      LY.press(el, {
        tap: () => { if (selecting) { if (it.type === "line") toggleSel(it.i); return; } clock.seek(it.tMs); follow.resume(); },
        long: () => { if (it.type === "line" && !selecting) startSelect(it.i); },
      });
    });

    function cardPlace() {
      if (!cardList) return;
      const el = cardEls[Math.max(0, active)];
      if (!el) return;
      cardList.style.transform = `translateY(${-Math.max(0, el.offsetTop - 8)}px)`;
    }

    function paint(set, a) {
      set.forEach((el, i) => {
        el.classList.toggle("is-active", i === a);
        el.classList.toggle("is-past", i < a);
      });
    }
    clock.subs.add((t) => {
      const a = LY.activeAt(items, t);
      if (a !== active) {
        const jump = Math.abs(a - active) > 4;
        active = a;
        paint(els, a);
        paint(cardEls, a);
        follow.sync(!jump);
        cardPlace();
      }
      const it = items[a];
      if (!it) return;
      for (const set of [els, cardEls]) {
        const el = set[a];
        if (!el) continue;
        if (it.type === "line") {
          const ws = el.querySelectorAll(".w");
          it.cw.forEach((w, k) => ws[k].style.setProperty("--p", clamp((t - w.s) / Math.max(80, w.e - w.s)).toFixed(3)));
        } else if (it.type === "break") {
          el.style.setProperty("--bp", LY.progress(it, t).toFixed(3));
        }
      }
    });

    requestAnimationFrame(() => {
      active = LY.activeAt(items, clock.t);
      paint(els, active);
      paint(cardEls, active);
      cardList && cardList.classList.add("is-instant");
      cardPlace();
      requestAnimationFrame(() => cardList?.classList.remove("is-instant"));
      if (phone && params.open) open(false);
      follow.sync(false);
      if (params.scroll) setTimeout(() => follow.manual(params.scroll), 300);
      if (params.share != null) setTimeout(() => { const l = model.lines[params.share]; startSelect(l.i); if (items[l.i + 1]?.type === "line") toggleSel(l.i + 1); }, 300);
    });
  });
})();
