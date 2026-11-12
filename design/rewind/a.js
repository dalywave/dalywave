// Направление А «Волна года»: одна живая волна на все истории. Между слайдами она не
// сменяется, а перетекает: палитра — из обложки слайда, размах — из энергии, на «Минутах»
// лента ложится графиком по месяцам, на артисте и треке года — обнимает фото и обложку.
// Переход — рябь по волне и текст, который всплывает строками.
(function () {
  "use strict";
  const { fmt, esc, typo, countUp, NB, MONTHS, MONTH_LETTERS, ICONS } = window.RW;
  const { Wave, MOODS, coverPalette, chartValue } = window.RWave;

  const slides = [
    { id: "intro", ms: 6000 },
    { id: "minutes", ms: 8000 },
    { id: "artist", ms: 8000 },
    { id: "toy", ms: 8000 },
    { id: "persona", ms: 7000 },
    { id: "summary", ms: 12000 },
  ];

  // Раскладка волны по слайдам
  const LAYOUT = {
    intro: { tilt: -0.5, offset: 0.06, amp: 1.15, freq: 2.0, energy: 0.62, chart: 0, boxK: 0, vig: 0.6 },
    minutes: { tilt: 0, offset: 0, amp: 0.5, freq: 1.6, energy: 0.45, chart: 1, boxK: 0, vig: 0.4 },
    artist: { tilt: -0.14, offset: 0, amp: 0.85, freq: 2.0, energy: 0.55, chart: 0, boxK: 1, vig: 0.6 },
    toy: { tilt: -0.2, offset: 0, amp: 0.9, freq: 2.0, energy: 0.78, chart: 0, boxK: 1, vig: 0.6 },
    persona: { tilt: -0.06, offset: -0.3, amp: 0.55, freq: 1.4, energy: 0.2, chart: 0, boxK: 0, vig: 0.7 },
    summary: { tilt: -0.1, offset: 0.27, amp: 0.7, freq: 1.8, energy: 0.5, chart: 0, boxK: 0, vig: 0.5 },
  };

  const r = (d) => `style="--d:${d}"`;

  function build(stage, D, story) {
    const wrap = document.createElement("div");
    wrap.className = "a-wave";
    stage.append(wrap);
    const wave = new Wave(wrap);
    wave.setMonths(D.months);
    story.wave = wave;
    const t = D.toy.track;
    const peak = D.months.indexOf(Math.max(...D.months));
    const days = Math.floor(D.minutes / 1440);
    const hours = Math.floor((D.minutes % 1440) / 60);
    const maxH = Math.max(...D.persona.hours);
    const html = {
      intro: `
        <p class="a-kicker a-in" ${r(0)}>Итоги года</p>
        <div class="a-intro__foot">
          <h1 class="a-year"><span class="a-in" ${r(1)}>20</span><span class="a-in" ${r(2)}>26</span></h1>
          <p class="a-lead a-in" ${r(3)}>${typo(D.name)}, вот как звучал твой год</p>
        </div>`,
      minutes: `
        <p class="a-kicker a-in" ${r(0)}>Минуты музыки</p>
        <div class="a-big a-in" ${r(1)}><span data-count="${D.minutes}">${fmt(D.minutes)}</span></div>
        <p class="a-sub a-in" ${r(2)}>Это ${days}${NB}дней и ${hours}${NB}часов без перерыва</p>
        <div class="a-peak a-in" ${r(6)}><b>${fmt(D.months[peak])}</b> ${MONTHS[peak]}</div>
        <div class="a-months a-in" ${r(4)}>${MONTH_LETTERS.map((m, i) => `<span class="${i === peak ? "is-peak" : ""}">${m}</span>`).join("")}</div>`,
      artist: `
        <p class="a-kicker a-in" ${r(0)}>Артист года</p>
        <div class="a-photo a-in" ${r(1)}><img src="${D.topArtists[0].img}" alt=""></div>
        <div class="a-foot">
          <h2 class="a-title a-in" ${r(2)}>${esc(D.topArtists[0].name)}</h2>
          <p class="a-meta a-in" ${r(3)}>${fmt(D.topArtists[0].minutes)} минут · ${fmt(D.topArtists[0].plays)} прослушиваний</p>
          <ol class="a-list a-in" ${r(4)} start="2">${D.topArtists.slice(1, 4).map((a, i) => `<li><span class="a-n">${i + 2}</span><span class="a-l">${esc(a.name)}</span><span class="a-v">${fmt(a.minutes)}${NB}мин</span></li>`).join("")}</ol>
        </div>`,
      toy: `
        <p class="a-kicker a-in" ${r(0)}>Трек года</p>
        <div class="a-cover a-in" ${r(1)}><img src="${t.cover}" alt=""></div>
        <div class="a-foot">
          <h2 class="a-title a-title--track a-in" ${r(2)}>${typo(t.title)}</h2>
          <p class="a-meta a-in" ${r(3)}>${esc(t.artist)}</p>
          <dl class="a-stats a-in" ${r(4)}>
            <div><dt>раз за год</dt><dd>${fmt(t.plays)}</dd></div>
            <div><dt>впервые</dt><dd>${D.toy.first}</dd></div>
            <div><dt>чаще всего</dt><dd>в${NB}${D.toy.peak}е</dd></div>
          </dl>
        </div>`,
      persona: `
        <p class="a-kicker a-in" ${r(0)}>Твой характер</p>
        <h2 class="a-ptitle a-in" ${r(1)}>Ночной<br>слушатель</h2>
        <p class="a-sub a-in" ${r(2)}>${D.persona.share}${NB}% музыки — после полуночи. Чаще всего ты слушал в${NB}01:00</p>
        <div class="a-hours a-in" ${r(3)} aria-hidden="true">
          <div class="a-hours__bars">${D.persona.hours.map((h, i) => `<i class="${h >= maxH * 0.8 ? "is-peak" : ""}" style="--h:${(h / maxH).toFixed(3)}"></i>`).join("")}</div>
          <div class="a-hours__axis"><span>00</span><span>06</span><span>12</span><span>18</span><span>24</span></div>
        </div>`,
      summary: `
        <div class="a-sum__head">
          <p class="a-kicker a-in" ${r(0)}>Итоги ${D.year}</p>
        </div>
        <div class="a-sum a-in" ${r(1)}>
          <div class="a-sum__cols">
            <div><h3>Артисты</h3><ol>${D.topArtists.map((a) => `<li><span>${esc(a.name)}</span></li>`).join("")}</ol></div>
            <div><h3>Треки</h3><ol>${D.topTracks.map((x) => `<li><span>${esc(x.title)}</span></li>`).join("")}</ol></div>
          </div>
          <dl class="a-sum__stats">
            <div><dt>Минут</dt><dd>${fmt(D.minutes)}</dd></div>
            <div><dt>Жанр</dt><dd>${esc(D.genre)}</dd></div>
            <div><dt>Характер</dt><dd>${esc(D.persona.title)}</dd></div>
          </dl>
        </div>
        <div class="a-actions a-in" ${r(3)}>
          <button type="button" class="a-btn a-btn--primary">${ICONS.share}Поделиться</button>
          <button type="button" class="a-btn">${ICONS.plus}В плейлист</button>
        </div>`,
    };
    const els = slides.map((s) => {
      const el = document.createElement("section");
      el.className = `slide s-${s.id}`;
      el.innerHTML = html[s.id];
      stage.append(el);
      return el;
    });
    // Палитры слайдов — из обложек; пока не посчитаны, спокойная
    const sources = {
      intro: t.cover,
      artist: D.topArtists[0].img,
      toy: t.cover,
      summary: D.topArtists[0].img,
    };
    story.palettes = { persona: MOODS.sad, minutes: MOODS.calm };
    for (const [id, url] of Object.entries(sources)) {
      window.RW.coverColors(url).then((c) => {
        story.palettes[id] = coverPalette(c);
        if (slides[story.i]?.id === id) wave.setPalette(story.palettes[id]);
      });
    }
    // Подпись пика и буквы месяцев стоят ровно на ленте
    new ResizeObserver(() => placeChart(story)).observe(stage);
    return els;
  }

  function placeChart(story) {
    const wave = story.wave;
    const stage = story.stage;
    const W = stage.clientWidth;
    const H = stage.clientHeight;
    if (!W) return;
    const left = W * 0.08;
    const right = W * 0.92;
    wave.chartX = [(left - W / 2) / H, (right - W / 2) / H];
    wave.chartY = [-0.34, -0.02];
    const D = window.RW.data;
    const max = Math.max(...D.months);
    const norm = D.months.map((v) => v / max);
    const peak = D.months.indexOf(max);
    const el = stage.querySelector(".a-peak");
    const x = left + (right - left) * (peak / 11);
    const y = H / 2 - (wave.chartY[0] + chartValue(norm, peak) * (wave.chartY[1] - wave.chartY[0])) * H;
    el.style.left = `${x}px`;
    el.style.top = `${y - 0.03 * H}px`;
  }

  function show(i, prev, dir, story) {
    const id = slides[i].id;
    const wave = story.wave;
    const first = prev < 0;
    wave.set(LAYOUT[id], { instant: first });
    if (story.palettes[id]) wave.setPalette(story.palettes[id], { instant: first });
    const box = id === "artist" ? story.slideEls[i].querySelector(".a-photo") : id === "toy" ? story.slideEls[i].querySelector(".a-cover") : null;
    wave.setBox(box);
    // Гребень проходит через центр фото или обложки
    if (box) {
      const b = box.getBoundingClientRect();
      const c = story.stage.getBoundingClientRect();
      wave.set({ offset: -((b.top + b.height / 2 - c.top) - c.height / 2) / c.height - 0.02 }, { instant: first });
    }
    wave.bpm = id === "toy" ? window.RW.data.toy.bpm : id === "persona" ? 70 : 112;
    if (id === "intro") wave.start();
    else if (!first) wave.skip();
    if (id === "minutes") {
      placeChart(story);
      countUp(story.slideEls[i].querySelector("[data-count]"), window.RW.data.minutes, 1800, 300);
    }
  }

  window.RW.variants.a = { id: "a", name: "Волна года", slides, build, show };
})();
