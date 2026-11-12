// «Итоги года» — общий каркас макетов: данные, цвета обложек, проигрыватель историй.
// Направления (a.js, b.js, c.js) строят слайды и свои переходы; здесь — полосы, касания,
// удержание, автопереход и заморозка для скриншотов (?slide=3&hold).
(function () {
  "use strict";

  const IMG = "../fixtures/img/";
  const NB = " ";

  const data = {
    year: 2026,
    name: "Захар",
    minutes: 12480,
    plays: 3912,
    tracks: 1284,
    artists: 412,
    newArtists: 96,
    days: 341,
    genre: "Инди-поп",
    months: [620, 540, 890, 1020, 1180, 1340, 1610, 1420, 1050, 980, 1130, 700],
    topArtists: [
      { name: "Электрофорез", img: IMG + "artist-4558257.jpg", minutes: 1846, plays: 512 },
      { name: "Пошлая Молли", img: IMG + "artist-12080830.jpg", minutes: 1302, plays: 418 },
      { name: "дора", img: IMG + "artist-57812002.jpg", minutes: 1117, plays: 389 },
      { name: "Валентин Стрыкало", img: IMG + "artist-4315716.jpg", minutes: 964, plays: 301 },
      { name: "МУККА", img: IMG + "artist-59154662.jpg", minutes: 811, plays: 277 },
    ],
    topTracks: [
      { title: "Даже моя бэйби не знает", artist: "Пошлая Молли", cover: IMG + "album-957193101.jpg", plays: 214 },
      { title: "Всё было так", artist: "Электрофорез", cover: IMG + "album-186551052.jpg", plays: 176 },
      { title: "Моя одежда ещё пахнет тобой", artist: "дора", cover: IMG + "album-805630221.jpg", plays: 151 },
      { title: "Кладбище самолётов", artist: "Валентин Стрыкало", cover: IMG + "album-14769693.jpg", plays: 133 },
      { title: "Дотянуться до солнца", artist: "МУККА", cover: IMG + "album-922607561.jpg", plays: 121 },
    ],
    toy: { first: "14 февраля", peak: "март", minutes: 742, bpm: 132 },
    persona: {
      id: "night_owl",
      title: "Ночной слушатель",
      share: 34,
      peakHour: 1,
      hours: [10, 12, 8, 5, 2, 1, 0.5, 1, 2, 3, 3, 4, 4, 4, 4, 5, 5, 6, 6, 7, 8, 9, 10, 11],
    },
  };
  data.toy.track = data.topTracks[0];

  const MONTHS = ["январь", "февраль", "март", "апрель", "май", "июнь", "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь"];
  const MONTH_LETTERS = ["Я", "Ф", "М", "А", "М", "И", "И", "А", "С", "О", "Н", "Д"];

  // ---------- Числа и текст ----------

  const fmt = (n) => Math.round(n).toLocaleString("ru-RU").replace(/\s/g, NB);
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  // Неразрывный пробел после коротких слов: «в марте», «до дыр»
  const typo = (s) => esc(s).replace(/(^|\s)([а-яёА-ЯЁ]{1,2})\s/g, `$1$2${NB}`);

  // ---------- Цвета ----------

  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  function hexToRgb(hex) {
    const n = parseInt(hex.replace("#", ""), 16);
    return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
  }
  function rgbToHex(r, g, b) {
    return "#" + [r, g, b].map((v) => Math.round(clamp(v) * 255).toString(16).padStart(2, "0")).join("");
  }
  function rgbToHsl([r, g, b]) {
    const mx = Math.max(r, g, b);
    const mn = Math.min(r, g, b);
    const l = (mx + mn) / 2;
    if (mx === mn) return [0, 0, l];
    const d = mx - mn;
    const s = l > 0.5 ? d / (2 - mx - mn) : d / (mx + mn);
    let h = mx === r ? (g - b) / d + (g < b ? 6 : 0) : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return [h / 6, s, l];
  }
  function hsl(h, s, l) {
    h = ((h % 1) + 1) % 1;
    const f = (n) => {
      const k = (n + h * 12) % 12;
      const a = s * Math.min(l, 1 - l);
      return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    };
    return rgbToHex(f(0), f(8), f(4));
  }
  const toHsl = (hex) => rgbToHsl(hexToRgb(hex));
  function luminance(hex) {
    const lin = hexToRgb(hex).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
    return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
  }
  function contrast(a, b) {
    const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
    return (x + 0.05) / (y + 0.05);
  }

  // Цвета обложки — тот же счёт, что в apps/ui/app/utils/cover-colors.ts
  const colorCache = new Map();
  function coverColors(url) {
    if (colorCache.has(url)) return colorCache.get(url);
    const job = new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        const size = 28;
        const cv = document.createElement("canvas");
        cv.width = cv.height = size;
        const ctx = cv.getContext("2d", { willReadFrequently: true });
        ctx.drawImage(img, 0, 0, size, size);
        const d = ctx.getImageData(0, 0, size, size).data;
        const buckets = Array.from({ length: 12 }, () => ({ w: 0, r: 0, g: 0, b: 0 }));
        let dr = 0, dg = 0, db = 0, dn = 0;
        for (let i = 0; i < d.length; i += 4) {
          const r = d[i] / 255, g = d[i + 1] / 255, b = d[i + 2] / 255;
          const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
          const s = mx ? (mx - mn) / mx : 0;
          let h = 0;
          if (mx !== mn) {
            if (mx === r) h = ((g - b) / (mx - mn) + 6) % 6;
            else if (mx === g) h = (b - r) / (mx - mn) + 2;
            else h = (r - g) / (mx - mn) + 4;
          }
          const w = s * s * Math.sqrt(mx) + 0.02;
          const k = buckets[Math.floor((h / 6) * 12) % 12];
          k.w += w; k.r += r * w; k.g += g * w; k.b += b * w;
          if (mx < 0.55) { dr += r; dg += g; db += b; dn++; }
        }
        const sorted = buckets.filter((b) => b.w > 0).sort((a, b) => b.w - a.w);
        const pick = (b) => rgbToHex(b.r / b.w, b.g / b.w, b.b / b.w);
        const vivid = sorted[0] ? pick(sorted[0]) : "#3a5070";
        resolve({ vivid, second: sorted[1] ? pick(sorted[1]) : vivid, deep: dn ? rgbToHex(dr / dn, dg / dn, db / dn) : "#101820" });
      };
      img.onerror = () => resolve({ vivid: "#3a5070", second: "#3a5070", deep: "#101820" });
      img.src = url;
    });
    colorCache.set(url, job);
    return job;
  }

  // Плоское поле слайда из обложки — тот же счёт, что в apps/ui/app/utils/rewind-colors.ts
  const DARK_INK = "#0b0b10";
  function slideColors(palette, shift = 0) {
    const list = palette.filter(Boolean).map(toHsl);
    const ranked = list.map((c, i) => ({ c, score: c[1] * (1 - Math.abs(c[2] - 0.5)) + (i === 0 ? 0.05 : 0) })).sort((a, b) => b.score - a.score);
    const pick = ranked[Math.min(shift, ranked.length - 1)].c;
    const grey = pick[1] < 0.14;
    const bg = grey ? hsl(pick[0], pick[1], clamp(pick[2], 0.16, 0.24)) : hsl(pick[0], clamp(pick[1], 0.45, 0.88), clamp(pick[2], 0.3, 0.56));
    const ink = contrast(bg, "#ffffff") >= contrast(bg, DARK_INK) ? "#ffffff" : DARK_INK;
    const light = ink === DARK_INK;
    const b = toHsl(bg);
    const far = list.filter((c) => c !== pick)
      .map((c) => ({ c, d: Math.min(Math.abs(c[0] - b[0]), 1 - Math.abs(c[0] - b[0])) * (c[1] + 0.2) + Math.abs(c[2] - b[2]) * 0.4 }))
      .sort((x, y) => y.d - x.d)[0]?.c ?? b;
    let acc = [far[0], Math.max(far[1], grey ? far[1] : 0.5), light ? 0.22 : 0.74];
    if (contrast(hsl(...acc), bg) < 2.2) acc = [b[0], Math.min(1, b[1] + 0.1), light ? 0.14 : 0.86];
    return { bg, accent: hsl(...acc), ink, soft: light ? "rgba(11, 11, 16, 0.7)" : "rgba(255, 255, 255, 0.74)" };
  }

  // ---------- Иконки ----------

  const ICONS = {
    sound: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9.5h3.2L12 5.5v13l-4.8-4H4z" fill="currentColor"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
    mute: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9.5h3.2L12 5.5v13l-4.8-4H4z" fill="currentColor"/><path d="M16 9.5l5 5M21 9.5l-5 5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
    close: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/></svg>',
    share: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 15V4M7.5 8.5L12 4l4.5 4.5M5 13v6h14v-6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
    plus: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>',
  };

  // ---------- Проигрыватель историй ----------

  const reduced = matchMedia("(prefers-reduced-motion: reduce)");
  const easeOut = (k) => 1 - Math.pow(1 - k, 3);
  const easeOutExpo = (k) => (k >= 1 ? 1 : 1 - Math.pow(2, -10 * k));

  // Счётчик, который досчитывает до числа; при уменьшенном движении — сразу итог
  function countUp(el, to, ms = 1600, delay = 0) {
    if (!el) return;
    const start = performance.now() + delay;
    if (reduced.matches) {
      el.textContent = fmt(to);
      return;
    }
    el.textContent = fmt(0);
    const step = (now) => {
      const k = clamp((now - start) / ms);
      el.textContent = fmt(to * easeOutExpo(k));
      if (k < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  class Story {
    constructor(root, variant, opts = {}) {
      this.root = root;
      this.v = variant;
      this.hold = !!opts.hold;
      this.i = -1;
      this.t = 0;
      this.holding = false;
      this.muted = false;
      root.classList.add("story", `v-${variant.id}`);
      root.setAttribute("role", "dialog");
      root.setAttribute("aria-label", `Итоги ${data.year}`);
      const n = variant.slides.length;
      root.innerHTML = `
        <div class="st-stage"></div>
        <div class="st-top">
          <div class="st-bars" role="progressbar" aria-valuemin="1" aria-valuemax="${n}">${'<span class="st-bar"><i></i></span>'.repeat(n)}</div>
          <div class="st-tools">
            <button type="button" class="st-tool" data-act="mute" aria-label="Выключить звук">${ICONS.sound}</button>
            <button type="button" class="st-tool" data-act="close" aria-label="Закрыть">${ICONS.close}</button>
          </div>
        </div>`;
      this.stage = root.querySelector(".st-stage");
      this.bars = [...root.querySelectorAll(".st-bar i")];
      this.slideEls = variant.build(this.stage, data, this);
      root.querySelector('[data-act="mute"]').addEventListener("click", (e) => {
        this.muted = !this.muted;
        e.currentTarget.innerHTML = this.muted ? ICONS.mute : ICONS.sound;
        e.currentTarget.setAttribute("aria-label", this.muted ? "Включить звук" : "Выключить звук");
      });
      root.querySelector('[data-act="close"]').addEventListener("click", () => this.go(0, -1));
      this.bindTouch();
      this.last = performance.now();
      this.loop = this.loop.bind(this);
      this.go(clamp(opts.slide ?? 0, 0, n - 1), 1, true);
      requestAnimationFrame(this.loop);
    }

    bindTouch() {
      let timer = 0;
      let wasHold = false;
      const down = (e) => {
        if (e.button && e.button !== 0) return;
        wasHold = false;
        clearTimeout(timer);
        timer = setTimeout(() => {
          wasHold = true;
          this.holding = true;
          this.root.classList.add("is-held");
        }, 220);
      };
      // Нажатие слева (треть) — назад, справа — дальше; удержание — пауза. Кнопки — свои
      const interactive = (e) => e.target.closest("button, a");
      const release = () => {
        clearTimeout(timer);
        this.holding = false;
        this.root.classList.remove("is-held");
      };
      this.root.addEventListener("pointerdown", (e) => !interactive(e) && down(e));
      this.root.addEventListener("pointerup", (e) => {
        if (interactive(e)) return;
        release();
        if (wasHold) return;
        const rect = this.root.getBoundingClientRect();
        const dir = e.clientX - rect.left < rect.width * 0.32 ? -1 : 1;
        this.go(this.i + dir, dir);
      });
      this.root.addEventListener("pointercancel", release);
      this.root.tabIndex = 0;
      this.root.addEventListener("keydown", (e) => {
        if (e.key === "ArrowRight") this.go(this.i + 1, 1);
        else if (e.key === "ArrowLeft") this.go(this.i - 1, -1);
      });
    }

    go(i, dir = 1, first = false) {
      const n = this.v.slides.length;
      if (i >= n) i = 0;
      if (i < 0) i = 0;
      if (i === this.i && !first) {
        this.t = 0;
        return;
      }
      const prev = this.i;
      this.i = i;
      this.t = 0;
      this.root.dataset.slide = this.v.slides[i].id;
      this.stage.dataset.dir = dir > 0 ? "fwd" : "back";
      this.slideEls.forEach((el, k) => {
        el.classList.remove("is-on", "is-off");
        el.setAttribute("aria-hidden", k === i ? "false" : "true");
      });
      const cur = this.slideEls[i];
      void cur.offsetWidth; // анимации входа — заново
      cur.classList.add("is-on");
      if (prev >= 0 && prev !== i) this.slideEls[prev].classList.add("is-off");
      this.v.show?.(i, prev, dir, this);
      this.root.querySelector(".st-bars").setAttribute("aria-valuenow", String(i + 1));
    }

    loop(now) {
      const dt = Math.min(100, now - this.last);
      this.last = now;
      const slide = this.v.slides[this.i];
      if (!this.hold && !this.holding && !document.hidden) {
        this.t += dt;
        if (this.t >= slide.ms) this.go(this.i + 1, 1);
      }
      const k = this.hold ? 0.5 : clamp(this.t / slide.ms);
      this.bars.forEach((b, j) => (b.style.transform = `scaleX(${j < this.i ? 1 : j === this.i ? k : 0})`));
      this.v.frame?.(dt, this);
      requestAnimationFrame(this.loop);
    }
  }

  // ---------- Запуск: ?v=a&slide=3&hold ----------

  function boot(root) {
    const q = new URLSearchParams(location.search);
    const id = q.get("v") || "a";
    const variant = window.RW.variants[id] || window.RW.variants.a;
    const slide = q.has("slide") ? Number(q.get("slide")) : 0;
    return new Story(root, variant, { slide, hold: q.has("hold") });
  }

  window.RW = {
    data, MONTHS, MONTH_LETTERS, ICONS, NB,
    fmt, esc, typo, clamp, easeOut, easeOutExpo, countUp, reduced,
    hexToRgb, rgbToHex, toHsl, hsl, contrast, coverColors, slideColors,
    Story, boot, variants: {},
  };
})();
