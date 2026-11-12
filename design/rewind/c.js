// Направление В «Пластинка»: год как вещи, которые можно взять в руки. Конверт с пластинкой,
// кассета со счётчиком плёнки, стопка полароидов, пластинка трека года под иглой, значок
// характера и чек итогов, который выползает из принтера. Фон — тёмный, в тон обложки, с зерном.
// Переход — плёнка протягивается на кадр вверх, по краям мелькает перфорация.
(function () {
  "use strict";
  const { fmt, esc, typo, NB, MONTHS, MONTH_LETTERS, ICONS, coverColors, toHsl, hsl, reduced } = window.RW;

  const slides = [
    { id: "intro", ms: 6000 },
    { id: "minutes", ms: 8000 },
    { id: "artist", ms: 8000 },
    { id: "toy", ms: 8000 },
    { id: "persona", ms: 7000 },
    { id: "summary", ms: 12000 },
  ];

  const disc = (img) => `<div class="c-disc"><i class="c-disc__grooves"></i><i class="c-disc__shine"></i><div class="c-label"><img src="${img}" alt=""></div></div>`;

  // Штрихкод чека — из названия года, всегда одинаковый
  function barcode(seed) {
    let x = 0;
    let h = 2166136261;
    const bars = [];
    while (x < 200) {
      h = Math.imul(h ^ seed.charCodeAt(x % seed.length), 16777619) >>> 0;
      const w = 1 + (h % 3);
      if ((h >> 3) & 1) bars.push(`<rect x="${x}" y="0" width="${w}" height="36"/>`);
      x += w + 1;
    }
    return `<svg class="c-bar" viewBox="0 0 200 36" preserveAspectRatio="none" aria-hidden="true">${bars.join("")}</svg>`;
  }

  function build(stage, D, story) {
    const t = D.toy.track;
    const max = Math.max(...D.months);
    const peak = D.months.indexOf(max);
    const days = Math.floor(D.minutes / 1440);
    const hours = Math.floor((D.minutes % 1440) / 60);
    const digits = String(D.minutes).split("");
    const row = (l, r) => `<div class="c-row"><span>${l}</span><i></i><span>${r}</span></div>`;
    const html = {
      intro: `
        <p class="c-kicker c-in" style="--d:0">Сторона А</p>
        <div class="c-sleeve c-obj">
          ${disc(D.topTracks[2].cover)}
          <div class="c-sleeve__cover"><img src="${t.cover}" alt=""></div>
        </div>
        <div class="c-foot">
          <h1 class="c-title c-in" style="--d:3">Итоги ${D.year}</h1>
          <p class="c-sub c-in" style="--d:4">${typo(D.name)}, твой год на${NB}одной пластинке</p>
        </div>`,
      minutes: `
        <p class="c-kicker c-in" style="--d:0">Минуты музыки</p>
        <div class="c-cass c-obj">
          <i class="c-screw" style="left:3%;top:5%"></i><i class="c-screw" style="right:3%;top:5%"></i><i class="c-screw" style="left:3%;bottom:6%"></i><i class="c-screw" style="right:3%;bottom:6%"></i>
          <div class="c-cass__label">
            <div class="c-cass__title"><b>MIX ${D.year}</b><span>${esc(D.name)}</span></div>
            <div class="c-cass__window"><i class="c-reel c-reel--full"></i><i class="c-reel"></i></div>
            <div class="c-cass__stripes"><i></i><i></i></div>
          </div>
          <div class="c-cass__foot"><i></i><i></i><i></i><i></i></div>
        </div>
        <div class="c-counter c-in" style="--d:2" aria-label="${fmt(D.minutes)} минут">
          ${digits.map((d, i) => `<span class="c-dg${i === digits.length - 3 ? " c-dg--gap" : ""}"><span class="c-dg__strip" style="--n:${d};--d:${i}">${"0123456789".split("").map((x) => `<b>${x}</b>`).join("")}</span></span>`).join("")}
        </div>
        <p class="c-sub c-in" style="--d:3">минут на плёнке — ${days}${NB}дней и ${hours}${NB}часов</p>
        <div class="c-vu c-in" style="--d:4" aria-hidden="true">
          ${D.months.map((m, i) => {
            const lit = Math.max(1, Math.round((m / max) * 9));
            return `<div class="c-vu__col${i === peak ? " is-peak" : ""}">${Array.from({ length: 9 }, (_, k) => `<i class="${k < lit ? (k === lit - 1 ? "is-top" : "is-lit") : ""}" style="--k:${k}"></i>`).reverse().join("")}<span>${MONTH_LETTERS[i]}</span></div>`;
          }).join("")}
        </div>`,
      artist: `
        <p class="c-kicker c-in" style="--d:0">Артист года</p>
        <div class="c-polas c-obj">
          <figure class="c-pola c-pola--3"><img src="${D.topArtists[2].img}" alt=""></figure>
          <figure class="c-pola c-pola--2"><img src="${D.topArtists[1].img}" alt=""></figure>
          <figure class="c-pola c-pola--1"><i class="c-tape"></i><img src="${D.topArtists[0].img}" alt=""><figcaption>${esc(D.topArtists[0].name)}<small>${fmt(D.topArtists[0].minutes)}${NB}минут · ${fmt(D.topArtists[0].plays)} прослушиваний</small></figcaption></figure>
        </div>
        <ol class="c-list c-in" style="--d:4">${D.topArtists.slice(1, 4).map((a, i) => `<li><b>${i + 2}</b><span>${esc(a.name)}</span><em>${fmt(a.minutes)}${NB}мин</em></li>`).join("")}</ol>`,
      toy: `
        <p class="c-kicker c-in" style="--d:0">Трек года</p>
        <div class="c-turn c-obj">
          ${disc(t.cover)}
          <svg class="c-arm" viewBox="0 0 100 220" aria-hidden="true">
            <circle cx="70" cy="22" r="16" class="c-arm__base"/>
            <circle cx="70" cy="22" r="6" class="c-arm__pin"/>
            <path d="M70 22 L74 150 L58 196" class="c-arm__rod"/>
            <rect x="46" y="190" width="20" height="26" rx="3" transform="rotate(24 56 203)" class="c-arm__head"/>
          </svg>
        </div>
        <div class="c-foot">
          <h2 class="c-name c-in" style="--d:3">${typo(t.title)}</h2>
          <p class="c-sub c-in" style="--d:4">${esc(t.artist)}</p>
          <div class="c-stamps c-in" style="--d:5"><span><b>${t.plays}</b> раз</span><span>с${NB}${D.toy.first}</span><span>в${NB}${D.toy.peak}е — чаще всего</span></div>
        </div>`,
      persona: `
        <p class="c-kicker c-in" style="--d:0">Твой характер</p>
        <div class="c-pin c-obj">
          <svg viewBox="0 0 200 200" aria-hidden="true">
            <defs><path id="c-ring" d="M100,100 m-71,0 a71,71 0 1,1 142,0 a71,71 0 1,1 -142,0"/></defs>
            <circle cx="100" cy="100" r="97" class="c-pin__rim"/>
            <circle cx="100" cy="100" r="90" class="c-pin__enamel"/>
            <circle cx="100" cy="100" r="58" class="c-pin__core"/>
            <text class="c-pin__ring"><textPath href="#c-ring" textLength="440">НОЧНОЙ СЛУШАТЕЛЬ · ${D.year} · НОЧНОЙ СЛУШАТЕЛЬ · ${D.year} ·</textPath></text>
            <path d="M88 64 a28 28 0 1 0 30 40 a22 22 0 1 1 -30 -40z" class="c-pin__moon"/>
            <text x="100" y="140" class="c-pin__time">01:00</text>
          </svg>
        </div>
        <div class="c-foot">
          <h2 class="c-name c-in" style="--d:3">Ночной слушатель</h2>
          <p class="c-sub c-in" style="--d:4">${D.persona.share}${NB}% музыки — после полуночи, чаще всего в${NB}01:00</p>
        </div>`,
      summary: `
        <div class="c-printer">
          <i class="c-slot"></i>
          <div class="c-receipt-clip">
            <div class="c-receipt">
              <p class="c-r-head"><b>DalyWave</b>Итоги ${D.year}<br>${esc(D.name)}</p>
              <p class="c-r-sec">Артисты</p>
              ${D.topArtists.map((a, i) => row(`${i + 1}. ${esc(a.name)}`, `${fmt(a.minutes)}`)).join("")}
              <p class="c-r-sec">Треки</p>
              ${D.topTracks.map((x, i) => row(`${i + 1}. ${esc(x.title)}`, `${x.plays}×`)).join("")}
              <div class="c-r-total">${row("Минут всего", fmt(D.minutes))}${row("Жанр", esc(D.genre))}${row("Характер", esc(D.persona.title))}</div>
              ${barcode("dalywave-" + D.year)}
              <p class="c-r-thanks">Спасибо, что слушал</p>
            </div>
          </div>
        </div>
        <div class="c-actions c-in" style="--d:8">
          <button type="button" class="c-btn c-btn--primary">${ICONS.share}Поделиться</button>
          <button type="button" class="c-btn">${ICONS.plus}В плейлист</button>
        </div>`,
    };
    const els = slides.map((s) => {
      const el = document.createElement("section");
      el.className = `slide s-${s.id}`;
      el.innerHTML = `<i class="c-holes c-holes--l"></i><i class="c-holes c-holes--r"></i><div class="c-frame">${html[s.id]}</div>`;
      stage.append(el);
      return el;
    });
    // Тёмный фон в тон обложки и цвет предметов из неё
    const sources = {
      intro: t.cover,
      minutes: D.topTracks[4].cover,
      artist: D.topArtists[0].img,
      toy: t.cover,
      persona: D.topArtists[4].img,
      summary: D.topArtists[0].img,
    };
    slides.forEach((s, i) => {
      coverColors(sources[s.id]).then((c) => {
        const v = toHsl(c.vivid);
        const el = els[i];
        el.style.setProperty("--bg", hsl(v[0], Math.min(v[1], 0.32), 0.085));
        el.style.setProperty("--obj", hsl(v[0], Math.max(0.5, Math.min(v[1], 0.8)), 0.52));
        el.style.setProperty("--obj-deep", hsl(v[0], Math.max(0.45, Math.min(v[1], 0.7)), 0.3));
      });
    });
    return els;
  }

  window.RW.variants.c = { id: "c", name: "Пластинка", slides, build };
})();
