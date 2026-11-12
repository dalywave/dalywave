// Направление Б «Разворот»: история как журнальный постер. Плоское поле цвета обложки,
// огромные цифры Onest, кадрированные обложки и фото, волоски-разделители с подписями.
// Переход — следующая страница открывается шторкой справа, строки выезжают из-под маски.
(function () {
  "use strict";
  const { fmt, esc, typo, countUp, NB, MONTHS, MONTH_LETTERS, ICONS, slideColors, coverColors } = window.RW;

  const slides = [
    { id: "intro", ms: 6000 },
    { id: "minutes", ms: 8000 },
    { id: "artist", ms: 8000 },
    { id: "toy", ms: 8000 },
    { id: "persona", ms: 7000 },
    { id: "summary", ms: 12000 },
  ];

  // Строка, которая выезжает из-под маски
  const ln = (text, d, cls = "") => `<span class="b-ln ${cls}"><span style="--d:${d}">${text}</span></span>`;
  const rule = (left, right, d = 0) => `<div class="b-rule" style="--d:${d}"><span>${left}</span><span>${right}</span></div>`;

  function build(stage, D, story) {
    const t = D.toy.track;
    const max = Math.max(...D.months);
    const peak = D.months.indexOf(max);
    const days = Math.floor(D.minutes / 1440);
    const hours = Math.floor((D.minutes % 1440) / 60);
    const [y1, y2] = [String(D.year).slice(0, 2), String(D.year).slice(2)];
    const html = {
      intro: `
        ${rule("DalyWave", `Итоги${NB}года`)}
        <h1 class="b-year">${ln(y1, 1)}${ln(y2, 2)}</h1>
        <p class="b-lead">${ln(`${esc(D.name)}, вот как`, 3)}${ln("звучал твой год", 4)}</p>
        <div class="b-strip">${D.topTracks.map((x, i) => `<i style="--d:${i}"><img src="${x.cover}" alt=""></i>`).join("")}</div>`,
      minutes: `
        ${rule("02", "Минуты")}
        <div class="b-num">${ln(`<span data-count="${D.minutes}">${fmt(D.minutes)}</span>`, 1)}</div>
        <p class="b-cap">${ln("минут музыки", 2)}</p>
        <p class="b-note">${ln(`Это ${days}${NB}дней и ${hours}${NB}часов без перерыва`, 3)}</p>
        <div class="b-bars">
          ${D.months.map((m, i) => `<div class="b-col${i === peak ? " is-peak" : ""}" style="--h:${(m / max).toFixed(3)};--d:${i}">${i === peak ? `<em>${fmt(m)}</em>` : ""}<i></i><span>${MONTH_LETTERS[i]}</span></div>`).join("")}
        </div>`,
      artist: `
        ${rule("03", `Артист${NB}года`)}
        <figure class="b-photo"><img src="${D.topArtists[0].img}" alt=""><figcaption>№1</figcaption></figure>
        <h2 class="b-name">${ln(esc(D.topArtists[0].name), 1)}</h2>
        <div class="b-facts"><span>${fmt(D.topArtists[0].minutes)}${NB}минут</span><span>${fmt(D.topArtists[0].plays)} прослушиваний</span></div>
        <ol class="b-list" start="2">${D.topArtists.slice(1).map((a, i) => `<li style="--d:${i + 3}"><b>${i + 2}</b><span>${esc(a.name)}</span><em>${fmt(a.minutes)}</em></li>`).join("")}</ol>`,
      toy: `
        ${rule("04", `Трек${NB}года`)}
        <div class="b-times">${ln(`<span data-count="${t.plays}">${t.plays}</span>`, 1)}<span class="b-times__u">${ln("раз", 2)}</span></div>
        <div class="b-crop"><img src="${t.cover}" alt=""></div>
        <div class="b-track">
          <h2>${ln(typo("Даже моя бэйби"), 3)}${ln("не знает", 4)}</h2>
          <p>${ln(esc(t.artist), 5)}</p>
          <div class="b-facts"><span>с${NB}${D.toy.first}</span><span>чаще всего в${NB}${D.toy.peak}е</span></div>
        </div>`,
      persona: `
        ${rule("05", "Характер")}
        <div class="b-clock">${ln("01:00", 1)}</div>
        <h2 class="b-statement">${ln("Ночной", 2)}${ln("слушатель", 3)}</h2>
        <p class="b-note">${ln(`${D.persona.share}${NB}% музыки — после полуночи`, 4)}</p>
        <div class="b-day" aria-hidden="true">
          <div class="b-day__line"><i class="b-day__band" style="left:${(22 / 24) * 100}%;right:0"></i><i class="b-day__band" style="left:0;right:${(1 - 3 / 24) * 100}%"></i></div>
          <div class="b-day__ticks"><span>00</span><span>06</span><span>12</span><span>18</span><span>24</span></div>
        </div>`,
      summary: `
        ${rule("DalyWave", `Итоги${NB}${D.year}`)}
        <div class="b-sum">
          <div class="b-sum__col"><h3>Артисты</h3><ol>${D.topArtists.map((a, i) => `<li style="--d:${i}"><b>${i + 1}</b><span>${esc(a.name)}</span></li>`).join("")}</ol></div>
          <div class="b-sum__col"><h3>Треки</h3><ol>${D.topTracks.map((x, i) => `<li style="--d:${i + 1}"><b>${i + 1}</b><span>${esc(x.title)}</span></li>`).join("")}</ol></div>
        </div>
        <div class="b-sum__big">${ln(fmt(D.minutes), 2)}<span>минут</span></div>
        <dl class="b-sum__facts"><div><dt>Жанр</dt><dd>${esc(D.genre)}</dd></div><div><dt>Характер</dt><dd>${esc(D.persona.title)}</dd></div></dl>
        <div class="b-actions">
          <button type="button" class="b-btn b-btn--primary">${ICONS.share}Поделиться</button>
          <button type="button" class="b-btn">${ICONS.plus}В плейлист</button>
        </div>`,
    };
    const els = slides.map((s) => {
      const el = document.createElement("section");
      el.className = `slide s-${s.id}`;
      el.innerHTML = html[s.id];
      stage.append(el);
      return el;
    });
    // Поле каждого слайда — из своей обложки; соседние с одной обложкой — со сдвигом
    const sources = {
      intro: [t.cover, 0],
      minutes: [D.topArtists[4].img, 1],
      artist: [D.topArtists[0].img, 0],
      toy: [t.cover, 1],
      summary: [D.topArtists[2].img, 0],
    };
    story.colors = { persona: { bg: "#1b1a44", accent: "#c9c2ff", ink: "#ffffff", soft: "rgba(255, 255, 255, 0.74)" } };
    paint(els[4], story.colors.persona);
    slides.forEach((s, i) => {
      if (!sources[s.id]) return;
      const [url, shift] = sources[s.id];
      coverColors(url).then((c) => {
        story.colors[s.id] = slideColors([c.vivid, c.second, c.deep], shift);
        paint(els[i], story.colors[s.id]);
        if (story.i === i) story.root.style.setProperty("--ink", story.colors[s.id].ink);
      });
    });
    return els;
  }

  function paint(el, c) {
    el.style.setProperty("--bg", c.bg);
    el.style.setProperty("--ink", c.ink);
    el.style.setProperty("--soft", c.soft);
    el.style.setProperty("--acc", c.accent);
  }

  function show(i, prev, dir, story) {
    const id = slides[i].id;
    const c = story.colors[id];
    if (c) story.root.style.setProperty("--ink", c.ink);
    const el = story.slideEls[i];
    if (id === "minutes") countUp(el.querySelector("[data-count]"), window.RW.data.minutes, 1500, 350);
    if (id === "toy") countUp(el.querySelector("[data-count]"), window.RW.data.toy.track.plays, 1200, 350);
  }

  window.RW.variants.b = { id: "b", name: "Разворот", slides, build, show };
})();
