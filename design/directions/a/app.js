// DalyWave — направление A «Полночь». Живой прототип: данные, компоненты, экраны.
// Компоненты — функции, возвращающие разметку; одни и те же функции собирают экраны
// и галерею компонентов, поэтому вид везде один.
(() => {
  "use strict";

  const D = window.DW_DATA;
  const ASSET = "../../fixtures/";
  const BRAND = "../../brand/";

  // BPM, фаза первой доли и огибающая громкости превью — посчитаны по MP3 заранее
  const ANALYSIS = {"3107728361":{"bpm":90.0,"phase":0.0,"peaks":[0.58,0.72,0.77,0.88,0.7,0.92,0.68,0.65,0.6,0.56,0.58,0.85,0.73,0.77,0.75,0.73,1.0,0.69,0.69,0.81,0.75,0.55,0.46,0.49,0.43,0.91,0.56,0.88,0.74,0.69,0.71,0.94,0.66,0.67,0.97,0.66,0.62,0.54,0.49,0.9,0.5,0.91,0.72,0.68,0.88,0.87,0.67,0.7,0.94,0.34,0.64,0.58,0.52,0.86,0.58,0.9,0.62,0.76,0.74,0.76,0.87,0.72,0.96,0.64,0.67,0.56,0.55,0.68,0.75,0.81,0.73,0.73,0.75,0.95,0.71,0.68,0.86,0.69,0.55,0.47,0.57,0.48,0.91,0.64,0.84,0.76,0.69,0.71,0.94,0.68,0.73,0.89,0.68,0.59,0.49,0.44,0.34,0.26,0.44,0.41,0.43,0.36,0.55,0.4,0.37,0.48,0.38,0.66,0.6,0.49,0.78,0.66,0.92,0.6,0.65,0.64,0.73,0.7,0.37,0.89,0.6,0.56,0.59,0.55,0.71,0.69,0.84,0.57,0.61,0.43,0.93,0.6,0.68,0.81,0.66,0.62,0.54,0.61,0.62,0.82,0.77,0.75,0.65,0.58,0.62,0.9,0.44,0.61,0.77,0.63,0.48,0.58,0.5,0.84,0.59,0.9,0.64,0.49,0.84,0.65]},"3580793301":{"bpm":161.5,"phase":0.052,"peaks":[0.73,0.84,0.78,0.88,0.89,0.83,0.85,0.79,0.69,0.84,0.81,0.76,0.93,0.95,0.89,0.81,0.82,0.68,0.75,0.92,0.81,0.81,0.79,0.84,0.83,0.82,0.82,0.92,0.94,0.98,0.77,0.81,0.85,0.75,0.75,0.81,0.81,0.86,0.82,0.79,0.86,0.71,0.74,0.81,0.86,0.91,0.81,0.86,0.81,0.8,0.87,0.86,0.7,0.84,0.79,0.63,0.84,0.76,0.8,0.81,0.9,0.82,0.82,0.81,0.88,0.78,0.89,0.94,0.79,0.85,0.79,0.62,0.93,0.92,0.85,0.92,0.94,0.82,0.66,0.73,0.76,0.88,0.74,0.97,0.74,0.84,0.93,0.88,0.86,0.79,0.89,0.95,0.94,0.85,0.82,0.63,0.87,0.81,0.65,0.7,0.84,0.8,0.69,0.72,0.93,0.94,0.5,0.66,0.84,0.8,0.75,0.87,0.92,0.81,0.93,0.78,0.89,0.94,0.77,0.86,0.85,0.88,0.92,0.77,0.79,0.89,0.91,0.81,0.81,0.91,0.85,0.71,1.0,0.96,0.81,0.97,0.96,0.81,0.67,0.84,0.75,0.84,0.8,0.84,0.78,0.81,0.81,0.77,0.84,0.86,0.8,0.85,0.84,0.88,0.82,0.89,0.87,0.78,0.74,0.74]},"3661025372":{"bpm":110.0,"phase":0.068,"peaks":[0.42,0.41,0.4,0.35,0.11,0.27,0.57,0.6,0.49,0.61,0.49,0.52,0.29,0.1,0.04,0.12,0.2,0.27,0.52,0.51,0.5,0.49,0.56,0.48,0.16,0.1,0.12,0.25,0.38,0.47,0.62,0.59,0.59,0.6,0.61,0.49,0.25,0.23,0.28,0.32,0.24,0.44,0.48,0.49,0.57,0.47,0.65,0.32,0.11,0.05,0.44,0.52,0.6,0.64,0.55,0.55,0.5,0.43,0.58,0.12,0.08,0.08,0.13,0.27,0.41,0.57,0.48,0.64,0.49,0.56,0.3,0.12,0.04,0.2,0.26,0.33,0.51,0.66,0.61,0.57,0.51,0.52,0.31,0.22,0.28,0.3,0.28,0.21,0.61,0.54,0.52,0.54,0.63,0.53,0.39,0.52,0.43,0.61,0.58,0.81,0.93,0.75,0.65,0.67,0.6,0.57,0.45,0.31,0.12,0.24,0.39,0.97,0.85,0.57,0.44,0.53,0.5,0.2,0.03,0.06,0.06,0.37,0.65,1.0,0.73,0.57,0.58,0.61,0.5,0.12,0.38,0.4,0.42,0.56,0.82,0.93,0.66,0.61,0.59,0.64,0.43,0.45,0.3,0.53,0.68,0.69,1.0,0.88,0.74,0.65,0.63,0.6,0.43,0.3,0.16,0.53,0.38,0.7,0.95,0.75]},"3898084851":{"bpm":141.5,"phase":0.013,"peaks":[0.23,0.21,0.17,0.25,0.37,0.4,0.31,0.33,0.36,0.38,0.34,0.41,0.5,0.38,0.39,0.34,0.26,0.25,0.4,0.44,0.34,0.4,0.39,0.31,0.28,0.28,0.27,0.34,0.44,0.3,0.36,0.39,0.29,0.34,0.29,0.36,0.35,0.37,0.26,0.32,0.25,0.37,0.34,0.32,0.27,0.34,0.42,0.38,0.46,0.38,0.38,0.32,0.28,0.2,0.21,0.21,0.37,0.36,0.33,0.37,0.3,0.42,0.42,0.4,0.41,0.39,0.42,0.37,0.35,0.33,0.41,0.37,0.36,0.45,0.42,0.45,0.34,0.25,0.32,0.32,0.29,0.36,0.33,0.33,0.5,0.44,0.4,0.31,0.31,0.25,0.27,0.21,0.38,0.42,0.34,0.43,0.37,0.42,0.42,0.43,0.4,0.44,0.45,0.44,0.33,0.28,0.33,0.35,0.32,0.32,0.37,0.53,0.66,0.86,0.92,0.77,0.72,0.65,0.99,0.59,0.68,0.62,0.86,0.77,0.61,0.6,0.67,1.0,0.73,0.76,0.63,0.79,0.93,0.86,0.74,0.68,0.99,0.75,0.74,0.69,0.83,0.83,0.69,0.7,0.68,0.97,0.75,0.93,0.72,0.68,0.88,0.66,0.67,0.69,0.95,0.77,0.74,0.67,0.77,0.9]},"4059700721":{"bpm":104.5,"phase":0.314,"peaks":[0.78,0.85,0.72,0.81,0.85,0.62,0.79,0.88,0.68,0.74,0.91,0.72,0.68,0.92,0.64,0.67,0.92,0.6,0.62,0.97,0.59,0.64,0.97,0.58,0.48,0.95,0.61,0.67,0.97,0.65,0.67,1.0,0.47,0.65,0.96,0.68,0.69,0.97,0.68,0.69,0.94,0.65,0.63,0.93,0.71,0.68,0.93,0.69,0.7,0.88,0.8,0.7,0.88,0.77,0.63,0.85,0.79,0.69,0.8,0.85,0.7,0.72,0.85,0.65,0.75,0.86,0.61,0.67,0.92,0.65,0.68,0.9,0.6,0.5,0.9,0.56,0.59,0.9,0.57,0.59,0.9,0.47,0.58,0.53,0.5,0.47,0.51,0.51,0.5,0.94,0.63,0.66,1.0,0.69,0.62,0.94,0.68,0.64,0.9,0.71,0.69,0.87,0.76,0.71,1.0,0.65,0.73,0.79,0.8,0.76,0.88,0.81,0.73,0.77,0.8,0.68,0.79,0.91,0.64,0.67,0.87,0.66,0.73,0.93,0.68,0.66,0.93,0.72,0.7,1.0,0.78,0.6,0.98,0.72,0.64,0.97,0.64,0.73,0.98,0.6,0.67,1.0,0.73,0.58,0.96,0.65,0.61,0.94,0.67,0.69,0.92,0.67,0.69,0.98,0.56,0.7,0.86,0.76,0.77,0.9]}};

  // ---------- Утилиты ----------

  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const esc = (v) => String(v ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const src = (p) => (p ? ASSET + p : "");
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const NB = " ";
  const isPhone = () => matchMedia("(max-width: 699px)").matches;
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

  function plural(n, forms) {
    const a = Math.abs(n) % 100;
    const b = a % 10;
    if (a > 10 && a < 20) return forms[2];
    if (b > 1 && b < 5) return forms[1];
    if (b === 1) return forms[0];
    return forms[2];
  }
  const num = (n) => n.toLocaleString("ru-RU");
  const dur = (ms) => {
    const s = Math.max(0, Math.round(ms / 1000));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  const MONTHS = ["янв.", "февр.", "марта", "апр.", "мая", "июня", "июля", "авг.", "сент.", "окт.", "нояб.", "дек."];
  const dateShort = (iso) => {
    const [, m, d] = iso.split("-").map(Number);
    return `${d}${NB}${MONTHS[m - 1]}`;
  };
  function mulberry32(a) {
    return () => {
      a = (a + 0x6d2b79f5) | 0;
      let t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // ---------- Данные ----------

  const artists = new Map(D.artists.map((a) => [a.id, a]));
  const albums = new Map(D.albums.map((a) => [a.id, a]));
  const tracks = new Map(D.tracks.map((t) => [t.id, t]));
  const artistNamed = (name) => D.artists.find((a) => a.name === name);
  const T = (id) => tracks.get(id);
  const artistPic = (name) => src(artistNamed(name)?.picture);

  const KEYS = ["Ля минор", "Ми минор", "Ре минор", "Фа минор", "До минор", "Соль минор", "Си минор", "Фа-диез минор", "До мажор", "Соль мажор", "Ре мажор", "Ля мажор"];
  const extra = new Map();
  // Метаданные анализа: темп, тональность, энергия, место перехода
  function meta(t) {
    if (extra.has(t.id)) return extra.get(t.id);
    const an = ANALYSIS[t.id];
    const r = mulberry32(t.id % 2147483647);
    const bpm = an ? an.bpm : Math.round(78 + r() * 64);
    const m = {
      bpm,
      bpmLabel: `${Math.round(bpm)}${NB}BPM`,
      key: KEYS[Math.floor(r() * KEYS.length)],
      energy: 0.35 + r() * 0.45,
    };
    if (t.id === 909494702) Object.assign(m, { bpm: 88, bpmLabel: `88${NB}BPM`, key: "Фа минор", energy: 0.46 });
    extra.set(t.id, m);
    return m;
  }

  function resample(arr, n) {
    const out = [];
    for (let i = 0; i < n; i++) {
      const a = Math.floor((i / n) * arr.length);
      const b = Math.max(a + 1, Math.floor(((i + 1) / n) * arr.length));
      let mx = 0;
      for (let j = a; j < b; j++) mx = Math.max(mx, arr[j]);
      out.push(mx);
    }
    return out;
  }
  // Огибающая трека: у превью — настоящая, у остальных — по форме песни
  function peaks(t, n) {
    const an = ANALYSIS[t.id];
    if (an) return resample(an.peaks, n).map((v) => clamp(v, 0.08, 1));
    const r = mulberry32((t.id * 7) % 2147483647);
    const sections = [[0, 0.3], [0.08, 0.56], [0.24, 0.88], [0.37, 0.6], [0.5, 0.92], [0.64, 0.46], [0.74, 0.96], [0.9, 0.34], [0.97, 0.16]];
    const raw = [];
    for (let i = 0; i < n; i++) {
      const x = i / (n - 1);
      let lvl = sections[0][1];
      for (const [s, l] of sections) if (x >= s) lvl = l;
      raw.push(clamp(lvl * (0.7 + 0.3 * r()) + (r() - 0.5) * 0.1, 0.08, 1));
    }
    return raw.map((v, i) => (v * 2 + (raw[i - 1] ?? v) + (raw[i + 1] ?? v)) / 4);
  }

  const MODES = [
    { id: "gapless", label: "Без пауз", icon: "arrows-in-line-horizontal", desc: "Треки стыкуются вплотную" },
    { id: "smooth", label: "Плавный", icon: "wave-sine", desc: "Наложение на 6 секунд" },
    { id: "smart", label: "Умный", icon: "sparkle", desc: "Сам находит место: после вокала, на вступлении" },
    { id: "dj", label: "Диджей", icon: "vinyl-record", desc: "Сведение в долю, как в сете" },
  ];
  // Где начнётся переход: у «Диджея» — за 8 тактов до конца
  function transitionAt(t, mode = state.mode) {
    const d = t.durationMs;
    if (mode === "gapless") return d;
    if (mode === "smooth") return d - 6000;
    const bars = (8 * 4 * 60000) / meta(t).bpm;
    return Math.round(d - bars - (mode === "smart" ? 3000 : 1500));
  }

  const ACTIVITIES = [
    { id: "wake", label: "Просыпаюсь", phrase: "для пробуждения", icon: "sun-horizon" },
    { id: "road", label: "В дороге", phrase: "в дорогу", icon: "car" },
    { id: "work", label: "Работаю", phrase: "для работы", icon: "laptop" },
    { id: "sport", label: "Тренируюсь", phrase: "для тренировки", icon: "barbell", hint: "Темп бега 165 шагов в минуту" },
    { id: "rest", label: "Отдыхаю", phrase: "для отдыха", icon: "coffee" },
    { id: "sleep", label: "Засыпаю", phrase: "перед сном", icon: "moon-stars", hint: "Энергия плавно снижается" },
    { id: "party", label: "Вечеринка", phrase: "для вечеринки", icon: "confetti" },
  ];
  const CHARACTERS = [
    { id: "fav", label: "Любимое", phrase: "только любимое", icon: "heart" },
    { id: "mix", label: "Знакомое и новое", phrase: "знакомое и новое", icon: "shuffle-angular" },
    { id: "new", label: "Незнакомое", phrase: "только новое", icon: "compass" },
    { id: "pop", label: "Популярное", phrase: "популярное", icon: "trend-up" },
    { id: "rare", label: "Редкое", phrase: "редкое", icon: "diamond" },
  ];
  const MOODS = [
    { id: "energetic", label: "Бодрое", fem: "Бодрая" },
    { id: "happy", label: "Весёлое", fem: "Весёлая" },
    { id: "calm", label: "Спокойное", fem: "Спокойная" },
    { id: "sad", label: "Грустное", fem: "Грустная" },
  ];
  const LANGS = [
    { id: "any", label: "Любой", phrase: "", icon: "globe-simple" },
    { id: "ru", label: "Русский", phrase: "на русском", icon: "translate" },
    { id: "foreign", label: "Иностранный", phrase: "на иностранных языках", icon: "airplane-tilt" },
    { id: "none", label: "Без слов", phrase: "без слов", icon: "microphone-slash" },
  ];
  const GROUPS = { activity: ACTIVITIES, character: CHARACTERS, mood: MOODS, lang: LANGS };
  const GROUP_LABEL = { activity: "Занятие", character: "Характер", mood: "Настроение", lang: "Язык" };
  const PRESETS = [
    { id: "work", title: "Работа", meta: "спокойное, без слов", icon: "laptop", set: { activity: "work", mood: "calm", character: "mix", lang: "none" } },
    { id: "road", title: "Дорога", meta: "бодрое, русское", icon: "car", set: { activity: "road", mood: "energetic", character: "fav", lang: "ru" } },
    { id: "night", title: "Ночь", meta: "грустное, редкое", icon: "moon-stars", set: { activity: "rest", mood: "sad", character: "rare", lang: "any" } },
  ];
  const MOOD_COLORS = {
    energetic: ["#7a2208", "#ff6a1a", "#ffd27a"],
    happy: ["#5c0f5e", "#ff4fa8", "#ffe9a8"],
    calm: ["#0a2a52", "#1d8cbc", "#aef0ff"],
    sad: ["#1c1946", "#5b50c4", "#d3ccff"],
  };

  const REASONS = {
    909494702: "Похоже на Lowborn",
    3661025372: "Вы слушали 34 раза",
    3573472271: "Добавлено вами",
    2979784991: "Похоже на Mytch",
    836932782: "Новое для тебя",
    2411993425: "Похоже на Parker The Bandit",
    3580793551: "Из любимого",
    1857926377: "Похоже на Mytch",
    1698366377: "Открытие недели",
    3657203312: "Давно не звучало",
    2124328597: "Похоже на Mytch",
    2662530582: "Похоже на Bohnes",
    4378564111: "Похоже на 5opka",
    3786658542: "Похоже на Rushex",
    1750553117: "Похоже на Mzlff",
    3580793301: "Из любимого",
    2446965695: "Похоже на Lowborn",
  };
  // Короткий предлог не остаётся в конце строки: «Похоже на Lowborn»
  const reasonOf = (t) => (REASONS[t.id] || "Новое для тебя").replace(/ (на|в|с|к|о|у) /g, ` $1${NB}`);

  // Треки, которых ещё нет на сервере, и уже скачанные на это устройство
  const REMOTE = new Set([836932782, 4378564111, 1698366377]);
  const DOWNLOADED = new Set([909494702, 3661025372, 3580793551, 2124328597, 3657203312]);

  const WAVE_POOL = [2979784991, 836932782, 2411993425, 3580793551, 1857926377, 1698366377, 3657203312, 2124328597, 2662530582, 3580793301, 2446965695, 3657134452, 3683220072, 3657365522, 2439865625];

  const RESUME = [
    { kind: "album", album: 827674281, meta: "Mzlff · осталось 3 трека", p: 0.62 },
    { kind: "playlist", title: "Ночной фонк", covers: [3898084851, 3805738582, 3652133052, 4249360291], meta: "Плейлист · 42 трека", p: 0.35 },
    { kind: "album", album: 1134273871, meta: "Mytch · новый альбом", p: 0.14 },
    { kind: "radio", artist: "Lowborn", title: "Радио Lowborn", meta: "Волна по артисту", p: 0 },
    { kind: "album", album: 1135471731, meta: "5opka · трек 2 из 3", p: 0.4 },
    { kind: "album", album: 678933591, meta: "Parker The Bandit · осталось 2 трека", p: 0.72 },
  ];

  const MIXES = [
    { n: 1, lead: "Mzlff", with: ["Пошлая Молли", "кис-кис"], c: ["#ff5d8f", "#2b0a1f"] },
    { n: 2, lead: "Lowborn", with: ["Bohnes", "Poe the Passenger"], c: ["#6aa8ff", "#0a1631"] },
    { n: 3, lead: "Rushex", with: ["Sayfalse", "DJ FKU"], c: ["#ff9a3c", "#2c0e03"] },
    { n: 4, lead: "Mytch", with: ["Parker The Bandit", "Demi the Daredevil"], c: ["#4fd8c2", "#04241f"] },
    { n: 5, lead: "5opka", with: ["SQWOZ BAB", "МУККА"], c: ["#b48cff", "#160a35"] },
  ];

  const NEW_RELEASES = [1138050861, 1134273871, 1135471731, 1134970871, 1052752402, 1070416932, 1125801282, 1110576112];
  const DISCOVER = [2124328597, 2411993425, 836932782, 1698366377, 2662530582, 4378564111, 3786658542, 1750553117];

  const PLAYLISTS = [
    { id: "phonk", title: "Ночной фонк", count: 42, covers: [3898084851, 3805738582, 3652133052, 4249360291] },
    { id: "work", title: "Тихо, работаю", count: 67, covers: [3661025372, 3580793301, 836932782, 2979784991] },
    { id: "hyper", title: "Хайперпоп на репите", count: 58, covers: [4383414051, 3573472271, 2970546131, 3510764021] },
    { id: "home", title: "Дорога домой", count: 31, covers: [3657134452, 2439865625, 1272436802, 4033695001] },
  ];
  const FOLLOWED = ["5opka", "Mzlff", "Rushex", "Lowborn", "Mytch", "Parker The Bandit", "Пошлая Молли", "Bohnes", "Sayfalse"];
  const FRESH = new Set(["Rushex", "Mytch"]);

  const DEVICES = [
    { id: "mac", name: "MacBook Pro", meta: "Это устройство", icon: "laptop" },
    { id: "iphone", name: "iPhone Захара", meta: "В сети", icon: "device-mobile" },
    { id: "pc", name: "ПК в комнате", meta: "Windows · был 2 ч назад", icon: "desktop" },
    { id: "ipad", name: "iPad", meta: "Не в сети", icon: "device-tablet", offline: true },
  ];

  // ---------- Состояние ----------

  const state = {
    route: "home",
    mood: "calm",
    activity: "work",
    character: "mix",
    lang: "any",
    preset: null,
    playing: true,
    current: 909494702,
    position: 72000,
    history: [3580793301, 2446965695],
    manual: [3661025372, 3573472271],
    wave: [2979784991, 836932782, 2411993425, 3580793551, 1857926377, 1698366377],
    liked: new Set([3661025372, 3580793551, 2124328597, 2979784991]),
    volume: 0.72,
    muted: false,
    mode: "dj",
    expressive: 0.6,
    device: "mac",
    panel: false,
    panelTab: "queue",
    scope: "library",
  };
  const cur = () => T(state.current);

  // ---------- Иконки и мелочи ----------

  const icon = (name, o = {}) => `<i class="${o.fill ? "ph-fill" : "ph"} ph-${name}${o.cls ? " " + o.cls : ""}" aria-hidden="true"></i>`;
  const GLYPH_PATH = "M16 31C18.83 37.33 27.33 67.33 33 69C38.67 70.67 44.33 41.00 50 41C55.67 41.00 61.33 70.67 67 69C72.67 67.33 81.17 37.33 84 31";
  const glyph = (cls = "brand__mark") => `<svg class="${cls}" viewBox="0 0 100 100" aria-hidden="true"><path d="${GLYPH_PATH}" fill="none" stroke="currentColor" stroke-width="11.5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  const Brand = () => `<a class="brand" href="#home" aria-label="DalyWave — на главную">${glyph()}<span class="brand__word">DalyWave</span></a>`;
  const Kbd = (...keys) => `<span class="kbd">${keys.map((k) => `<kbd>${esc(k)}</kbd>`).join("")}</span>`;
  const Eq = (paused) => `<span class="eq${paused ? " is-paused" : ""}" aria-hidden="true"><i></i><i></i><i></i></span>`;
  const Explicit = () => `<span class="badge badge--explicit" title="Ненормативная лексика" aria-label="Ненормативная лексика">E</span>`;
  const Quality = (q = "Opus 160") => `<span class="badge badge--quality" title="Качество потока">${icon("waveform", { cls: "i-14" })}${esc(q)}</span>`;

  // Галерея компонентов грузит обложки сразу: это справочная страница, её смотрят целиком
  let eagerImages = false;
  const loading = (eager) => (eager || eagerImages ? "eager" : "lazy");
  function Art(path, o = {}) {
    const cls = ["art", o.round ? "art--round" : "", o.cls || ""].filter(Boolean).join(" ");
    if (!path) return `<div class="${cls} art--fallback" aria-hidden="true">${icon("music-notes-simple")}</div>`;
    return `<div class="${cls}"${o.style ? ` style="${o.style}"` : ""}><img src="${esc(path)}" alt="${esc(o.alt || "")}" loading="${loading(o.eager)}" decoding="${eagerImages ? "sync" : "async"}"></div>`;
  }
  const Mosaic = (ids) => `<div class="mosaic">${ids.map((id) => `<img src="${src(T(id)?.cover)}" alt="" loading="${loading()}">`).join("")}</div>`;

  function IconBtn(name, label, o = {}) {
    const cls = ["icon-btn", o.size ? `icon-btn--${o.size}` : "", o.cls || "", o.on ? "is-on" : ""].filter(Boolean).join(" ");
    const attrs = o.attrs || "";
    return `<button class="${cls}" type="button" aria-label="${esc(label)}" data-tip="${esc(o.tip || label)}"${attrs}>${icon(name, { fill: o.fill })}</button>`;
  }

  // ---------- Кнопка волны ----------

  function WavePlay(o = {}) {
    const playing = o.playing ?? state.playing;
    const st = o.loading ? "is-loading" : playing ? "is-playing" : "is-idle";
    const label = playing ? "Пауза" : "Слушать волну";
    return `<button class="waveplay ${st}${o.cls ? " " + o.cls : ""}" type="button" style="--size:${o.size || 72}px" data-action="${o.action || "wave-toggle"}" data-bind="waveplay" aria-label="${label}">
      <span class="waveplay__halo"></span><span class="waveplay__ring"></span>
      <span class="waveplay__core">${icon(playing && !o.loading ? "pause" : "play", { fill: true })}</span>
    </button>`;
  }

  // ---------- Настройки волны ----------

  function Moods(variant = "compact", o = {}) {
    const sel = o.selected || state.mood;
    return `<div class="moods moods--${variant}" role="radiogroup" aria-label="Настроение">${MOODS.map(
      (m) => `<button class="mood${m.id === sel ? " is-selected" : ""}${o.hover === m.id ? " is-hover" : ""}" type="button" role="radio" aria-checked="${m.id === sel}" data-action="mood" data-value="${m.id}" data-mood="${m.id}" data-tip="${variant === "compact" ? m.label : ""}">
        <span class="mood__orb"></span><span class="mood__label">${m.label}</span></button>`
    ).join("")}</div>`;
  }
  const optLabel = (group, id) => GROUPS[group].find((x) => x.id === id)?.label || "";

  function TunerCell(group) {
    return `<button class="dock__cell" type="button" data-action="tuner" data-group="${group}" aria-haspopup="listbox" aria-expanded="false">
      <span class="dock__label">${GROUP_LABEL[group]}</span>
      <span class="tuner__value"><span data-bind="opt-${group}">${optLabel(group, state[group])}</span>${icon("caret-down")}</span>
    </button>`;
  }
  const Dock = () => `<div class="dock" role="group" aria-label="Настройки волны">
      <div class="dock__cell"><span class="dock__label">Настроение</span>${Moods("compact")}</div>
      ${TunerCell("activity")}${TunerCell("character")}${TunerCell("lang")}
    </div>`;
  // На телефоне у токена нет подписи группы, поэтому «Любой» уточняем
  const tokenLabel = (group, id) => (group === "lang" && id === "any" ? "Любой язык" : optLabel(group, id));
  const Token = (group) => `<button class="token" type="button" data-action="tuner" data-group="${group}" aria-haspopup="listbox" aria-expanded="false" aria-label="${GROUP_LABEL[group]}"><span data-bind="tok-${group}">${tokenLabel(group, state[group])}</span>${icon("caret-down")}</button>`;

  function Preset(p, o = {}) {
    const c = MOOD_COLORS[p.set.mood];
    return `<button class="preset${state.preset === p.id ? " is-active" : ""}${o.hover ? " is-hover" : ""}" type="button" data-action="preset" data-value="${p.id}" data-bind="preset">
      <span class="preset__swatch" style="--p1:${c[0]};--p2:${c[1]};--p3:${c[2]}">${icon(p.icon)}</span>
      <span class="preset__text"><span class="preset__title">${p.title}</span><span class="preset__meta">${p.meta}</span></span>
      ${icon("play", { fill: true, cls: "preset__go i-16" })}
    </button>`;
  }

  function summary() {
    const m = MOODS.find((x) => x.id === state.mood);
    const a = ACTIVITIES.find((x) => x.id === state.activity);
    const c = CHARACTERS.find((x) => x.id === state.character);
    const l = LANGS.find((x) => x.id === state.lang);
    return `${m.fem} волна ${a.phrase}${NB}— ${c.phrase}${l.phrase ? ", " + l.phrase : ""}`;
  }

  // ---------- Строка трека ----------

  function TrackRow(t, o = {}) {
    const m = meta(t);
    const playing = o.playing ?? (t.id === state.current);
    const liked = state.liked.has(t.id);
    const remote = o.remote ?? REMOTE.has(t.id);
    const down = o.downloaded ?? DOWNLOADED.has(t.id);
    const cls = ["trow", o.variant ? `trow--${o.variant}` : "", playing ? "is-playing" : "", remote ? "is-remote" : "", o.hover ? "is-hover" : ""].filter(Boolean).join(" ");
    let sub = esc(t.artist);
    if (remote) sub += `<span class="trow__note"><span class="sep">·</span>нет на сервере${NB}— скачается при запуске</span>`;
    else if (o.reason) sub += `<span class="sep">·</span><span class="trow__reason">${esc(o.reason === true ? reasonOf(t) : o.reason)}</span>`;
    const state_ = remote
      ? `<span class="state-icon state-icon--cloud" data-tip="Нет на сервере — скачается при запуске" aria-label="Нет на сервере — скачается при запуске" role="img">${icon("cloud-arrow-down")}</span>`
      : down
        ? `<span class="state-icon state-icon--down" data-tip="Скачано на это устройство">${icon("arrow-circle-down", { fill: true })}</span>`
        : "";
    return `<div class="${cls}" role="row" tabindex="-1" data-track="${t.id}" data-action="row-play">
      <div class="trow__lead"><span class="trow__idx tnum">${o.index ?? ""}</span>${Eq(!state.playing)}<span class="trow__play">${icon("play", { fill: true })}</span></div>
      <div class="trow__art">${Art(src(t.cover), { cls: "trow__cover" })}<span class="trow__art-eq">${Eq(!state.playing)}</span></div>
      <div class="trow__main">
        <div class="trow__title"><span>${esc(t.title)}</span>${t.explicit ? Explicit() : ""}</div>
        <div class="trow__sub">${sub}</div>
      </div>
      ${o.variant === "wide" ? `<div class="trow__album">${esc(t.album || "")}</div>` : ""}
      <div class="trow__end">
        ${state_}
        <button class="icon-btn icon-btn--sm trow__like${liked ? " is-liked" : ""}" type="button" data-action="like" data-track="${t.id}" aria-label="${liked ? "Убрать из любимых" : "Нравится"}" aria-pressed="${liked}">${icon("heart", { fill: liked })}</button>
        <span class="trow__dur tnum">${dur(t.durationMs)}</span>
        <button class="icon-btn icon-btn--sm trow__more" type="button" data-action="menu" data-track="${t.id}" aria-label="Ещё">${icon("dots-three")}</button>
      </div>
    </div>`;
  }

  // ---------- Карточки ----------

  function AlbumCard(al, o = {}) {
    const upcoming = al.releaseDate > "2026-11-11";
    const art = upcoming
      ? `<div class="art"><div class="soon-art"><img src="${artistPic(al.artist)}" alt=""><div class="soon-art__date"><span class="soon-art__day">${Number(al.releaseDate.slice(8))}</span><span class="soon-art__month">${MONTHS[Number(al.releaseDate.slice(5, 7)) - 1].replace(".", "")}</span></div></div></div>`
      : Art(src(al.cover), { alt: al.title });
    const type = { album: "Альбом", ep: "EP", single: "Сингл" }[al.type] || "Альбом";
    const sub = upcoming ? `Выйдет ${dateShort(al.releaseDate)} · ${type}` : o.sub || `${esc(al.artist)} · ${type}`;
    const badge = upcoming ? `<span class="badge badge--soon card__badge">Скоро</span>` : o.isNew ? `<span class="badge badge--new card__badge">Новое</span>` : "";
    return `<a class="card${o.hover ? " is-hover" : ""}" href="#home" data-album="${al.id}" draggable="false">
      <div class="card__art">${art}${badge}${upcoming ? "" : `<button class="play-btn card__play" type="button" data-action="play-album" aria-label="Слушать «${esc(al.title)}»">${icon("play", { fill: true })}</button>`}</div>
      <div class="card__body"><div class="card__title">${esc(al.title)}</div><div class="card__sub">${upcoming ? `${esc(al.artist)} · выйдет ${dateShort(al.releaseDate)}` : sub}</div></div>
    </a>`;
  }

  function ArtistCard(a, o = {}) {
    return `<a class="card card--artist${o.hover ? " is-hover" : ""}" href="#home" draggable="false">
      <div class="card__art">${Art(src(a.picture), { round: true, alt: a.name })}<button class="play-btn card__play" type="button" aria-label="Радио ${esc(a.name)}">${icon("play", { fill: true })}</button></div>
      <div class="card__body"><div class="card__title">${esc(a.name)}</div><div class="card__sub">${o.sub || `${num(a.fans)} ${plural(a.fans, ["слушатель", "слушателя", "слушателей"])}`}</div></div>
    </a>`;
  }

  function MixCover(mx) {
    const others = mx.with.map((n) => `<img src="${artistPic(n)}" alt="">`).join("");
    return `<div class="art"><div class="mix" style="--mx1:${mx.c[0]};--mx2:${mx.c[1]}">
      <img class="mix__photo" src="${artistPic(mx.lead)}" alt="">
      <span class="mix__label">${glyph("")}Микс дня</span>
      <span class="mix__num">${mx.n}</span>
      <span class="mix__faces">${others}</span>
    </div></div>`;
  }
  function MixCard(mx, o = {}) {
    return `<a class="card${o.hover ? " is-hover" : ""}" href="#home" draggable="false">
      <div class="card__art">${MixCover(mx)}<button class="play-btn card__play" type="button" data-action="play-mix" aria-label="Слушать «Микс дня ${mx.n}»">${icon("play", { fill: true })}</button></div>
      <div class="card__body"><div class="card__title">Микс дня ${mx.n}</div><div class="card__sub">${esc(mx.lead)}, ${esc(mx.with[0])} и${NB}др.</div></div>
    </a>`;
  }

  function Resume(it, o = {}) {
    let art = "";
    let title = it.title;
    if (it.kind === "album") {
      const al = albums.get(it.album);
      art = Art(src(al.cover), { alt: al.title });
      title = al.title;
    } else if (it.kind === "playlist") art = `<div class="art">${Mosaic(it.covers)}</div>`;
    else if (it.kind === "radio") art = Art(artistPic(it.artist), { round: true });
    const bar = it.p ? `<div class="resume__bar" style="--p:${it.p}"><i></i></div>` : "";
    return `<a class="resume${o.hover ? " is-hover" : ""}" href="#home" draggable="false">
      ${art}
      <div class="resume__text"><div class="resume__title">${esc(title)}</div><div class="resume__meta">${esc(it.meta)}</div>${bar}</div>
      <button class="play-btn" type="button" aria-label="Продолжить «${esc(title)}»">${icon("play", { fill: true })}</button>
    </a>`;
  }

  // Обложка «Открытий недели»: волна бренда в цветах настроения
  const GenCover = (title, kicker) => `<div class="art"><div class="gen" style="--g1:#05070c;--g2:var(--m2);--g3:var(--m3)">
      <span class="gen__kicker">${kicker}</span>
      <svg class="gen__wave" viewBox="0 0 200 60" preserveAspectRatio="none" aria-hidden="true"><path d="M0 40 C 30 40, 40 12, 70 14 S 110 52, 140 46 S 180 18, 200 22" fill="none" stroke="rgba(255,255,255,.9)" stroke-width="2.2"/><path d="M0 46 C 30 46, 44 22, 72 24 S 112 58, 142 52 S 182 28, 200 30" fill="none" stroke="rgba(255,255,255,.35)" stroke-width="1.4"/></svg>
      <span class="gen__title">${title}</span>
    </div></div>`;

  function ShelfHead(title, o = {}) {
    return `<header class="shelf__head">
      <div class="shelf__titles"><h2 class="shelf__title">${title}</h2>${o.hint ? `<p class="shelf__hint">${o.hint}</p>` : ""}</div>
      <div class="shelf__actions">
        ${o.all === false ? "" : `<a class="btn btn--ghost btn--sm" href="#home">Все</a>`}
        ${o.nav ? `<div class="shelf__nav">${IconBtn("caret-left", "Назад", { size: "sm", attrs: ` data-action="shelf-prev" disabled` })}${IconBtn("caret-right", "Вперёд", { size: "sm", attrs: ` data-action="shelf-next"` })}</div>` : ""}
      </div>
    </header>`;
  }

  // ---------- Форма волны трека ----------

  let wfSeq = 0;
  function Waveform(t, o = {}) {
    const bars = o.bars || 96;
    const pk = peaks(t, bars);
    const H = 40;
    const W = bars * 4;
    let d = "";
    pk.forEach((v, i) => {
      const h = Math.max(2, v * (H - 6));
      const x = i * 4 + 2;
      d += `M${x} ${((H - h) / 2).toFixed(1)}V${((H + h) / 2).toFixed(1)}`;
    });
    const path = `<path d="${d}" fill="none" stroke="currentColor" stroke-width="${o.stroke || 2.2}" stroke-linecap="round" vector-effect="non-scaling-stroke"/>`;
    const svg = (c) => `<svg class="${c}" viewBox="0 0 ${W} ${H}" preserveAspectRatio="none" aria-hidden="true">${path}</svg>`;
    const x = transitionAt(t) / t.durationMs;
    const p = o.p ?? (t.id === state.current ? state.position / t.durationMs : 0);
    wfSeq++;
    return `<div class="wf${o.cls ? " " + o.cls : ""}" role="slider" tabindex="0" aria-label="Позиция в треке" aria-valuemin="0" aria-valuemax="${Math.round(t.durationMs / 1000)}" aria-valuenow="${Math.round((p * t.durationMs) / 1000)}" aria-valuetext="${dur(p * t.durationMs)} из ${dur(t.durationMs)}" data-wf="${t.id}" style="--p:${p.toFixed(4)};--x:${state.mode === "gapless" ? 1 : x.toFixed(4)}">
      ${svg("wf__base")}${svg("wf__zone")}${svg("wf__played")}${svg("wf__played-zone")}
      ${state.mode === "gapless" ? "" : `<span class="wf__marker"></span>`}<span class="wf__head"></span>
      ${o.tip ? `<span class="wf__tip${x > 0.7 ? " wf__tip--end" : ""}"><span class="tooltip">${o.tip}</span></span>` : ""}
    </div>`;
  }

  const modeOf = () => MODES.find((m) => m.id === state.mode);
  const ModeChip = (o = {}) => `<button class="mode-chip${o.hover ? " is-hover" : ""}" type="button" data-action="modes" aria-haspopup="dialog" aria-expanded="false" data-tip="Переходы между треками">${icon(modeOf().icon)}<span>${modeOf().label}</span></button>`;

  function VolumeSlider(o = {}) {
    const v = o.v ?? (state.muted ? 0 : state.volume);
    const ic = v === 0 ? "speaker-x" : v < 0.45 ? "speaker-low" : "speaker-high";
    return `<div class="vol">${IconBtn(ic, v === 0 ? "Включить звук" : "Выключить звук", { size: "sm", attrs: ` data-action="mute" data-bind="vol-icon"` })}<input class="range${o.hover ? " is-hover" : ""}" type="range" min="0" max="1" step="0.01" value="${v}" style="--v:${v}" aria-label="Громкость" data-action="volume"${o.disabled ? " disabled" : ""}></div>`;
  }

  // ---------- Панель «Сейчас играет» ----------

  function NowBar(o = {}) {
    const t = o.track || cur();
    const liked = state.liked.has(t.id);
    const pos = o.position ?? state.position;
    return `<div class="nowbar__track">
        <a href="#player" aria-label="Открыть плеер">${Art(src(t.cover), { cls: "nowbar__cover", eager: true })}</a>
        <div class="nowbar__meta">
          <div class="nowbar__title"><a href="#player">${esc(t.title)}</a></div>
          <div class="nowbar__artist"><a href="#home">${esc(t.artist)}</a></div>
          <div class="nowbar__reason">${icon("sparkle", { fill: true })}${esc(reasonOf(t))}</div>
        </div>
      </div>
      <div class="nowbar__center">
        <div class="transport">
          ${IconBtn("thumbs-down", "Не нравится", { size: "sm", attrs: ` data-action="dislike"`, tip: "Не нравится — больше не ставить" })}
          ${IconBtn("skip-back", "Предыдущий", { size: "sm", fill: true, attrs: ` data-action="prev"`, tip: "Предыдущий · ⇧←" })}
          <button class="play-btn" type="button" data-action="toggle" data-bind="play" aria-label="${state.playing ? "Пауза" : "Слушать"}" data-tip="${state.playing ? "Пауза" : "Слушать"} · пробел">${icon(state.playing ? "pause" : "play", { fill: true })}</button>
          ${IconBtn("skip-forward", "Следующий", { size: "sm", fill: true, attrs: ` data-action="next"`, tip: "Следующий · ⇧→" })}
          <button class="icon-btn icon-btn--sm${liked ? " is-liked" : ""}" type="button" data-action="like" data-track="${t.id}" aria-label="${liked ? "Убрать из любимых" : "Нравится"}" aria-pressed="${liked}" data-tip="${liked ? "В любимых" : "Нравится · L"}">${icon("heart", { fill: liked })}</button>
        </div>
        <div class="scrub">
          <span class="scrub__time" data-bind="pos">${dur(pos)}</span>
          ${Waveform(t, { bars: 110, p: pos / t.durationMs })}
          <span class="scrub__time" data-bind="len">${dur(t.durationMs)}</span>
        </div>
      </div>
      <div class="nowbar__right">
        ${ModeChip()}
        ${IconBtn("microphone", "Текст", { size: "sm", attrs: ` data-action="panel" data-value="lyrics"`, on: state.panel && state.panelTab === "lyrics" })}
        ${IconBtn("queue", "Очередь", { size: "sm", attrs: ` data-action="panel" data-value="queue" data-bind="queue-btn"`, on: state.panel && state.panelTab === "queue" })}
        ${IconBtn("devices", "Устройства", { size: "sm", attrs: ` data-action="devices"` })}
        ${VolumeSlider()}
      </div>`;
  }

  function MiniInner(o = {}) {
    const t = o.track || cur();
    const p = (o.position ?? state.position) / t.durationMs;
    return `<a href="#player" aria-label="Открыть плеер">${Art(src(t.cover), { eager: true })}</a>
      <a class="mini__meta" href="#player"><span class="mini__title">${esc(t.title)}</span><span class="mini__sub">${icon("waveform")}${esc(t.artist)}</span></a>
      <div class="mini__actions">
        <button class="icon-btn" type="button" data-action="toggle" data-bind="play" aria-label="${state.playing ? "Пауза" : "Слушать"}">${icon(state.playing ? "pause" : "play", { fill: true })}</button>
        <button class="icon-btn" type="button" data-action="next" aria-label="Следующий">${icon("skip-forward", { fill: true })}</button>
      </div>
      <div class="mini__progress" data-bind="mini-p" style="--p:${p.toFixed(4)}"><i></i></div>`;
  }

  // ---------- Всплывающие панели ----------

  function ModePicker(o = {}) {
    return `<div class="pop__head"><span class="pop__title">Переходы между треками</span><span class="pop__hint">для волны</span></div>
      ${MODES.map((m) => `<button class="option-row${m.id === state.mode ? " is-selected" : ""}${o.hover === m.id ? " is-hover" : ""}" type="button" data-action="mode" data-value="${m.id}" role="menuitemradio" aria-checked="${m.id === state.mode}">
          <span class="option-row__icon">${icon(m.icon)}</span>
          <span class="option-row__text"><span class="option-row__title">${m.label}${m.id === "dj" ? `<span class="badge badge--meta">в приложении</span>` : ""}</span><span class="option-row__desc">${m.desc}</span></span>
          <span class="option-row__radio"></span>
        </button>${m.id === "dj" && state.mode === "dj" ? `<div class="option-extra"><div class="option-extra__label"><span>Выразительность</span><span>${state.expressive < 0.34 ? "Незаметно" : state.expressive < 0.67 ? "Как в сете" : "Эффектно"}</span></div><input class="range" type="range" min="0" max="1" step="0.01" value="${state.expressive}" style="--v:${state.expressive}" aria-label="Выразительность переходов" data-action="expressive"></div>` : ""}`).join("")}
      <div class="pop__foot">Следующий переход: ${dur(transitionAt(cur()))}, сведение по долям</div>`;
  }

  function DevicePicker(o = {}) {
    return `<div class="pop__head"><span class="pop__title">Где играть</span><span class="pop__hint">${DEVICES.filter((d) => !d.offline).length} в сети</span></div>
      ${DEVICES.map((d) => {
        const current = d.id === state.device;
        return `<button class="device${current ? " is-current" : ""}${d.offline ? " is-offline" : ""}${o.hover === d.id ? " is-hover" : ""}" type="button" data-action="device" data-value="${d.id}"${d.offline ? " disabled" : ""}>
          <span class="device__icon">${icon(d.icon)}</span>
          <span class="device__text"><span class="device__name">${d.name}</span><span class="device__meta">${current ? `${Eq(!state.playing)}Играет здесь` : d.meta}</span></span>
          ${current ? icon("check", { cls: "i-18" }) : ""}
        </button>`;
      }).join("")}
      <div class="pop__sep"></div>
      <div class="pop__foot">Управление идёт через сервер: можно переключать с телефона.</div>`;
  }

  function ContextMenu(t, o = {}) {
    const liked = state.liked.has(t.id);
    const item = (ic, label, end = "", attrs = "", cls = "") => `<button class="menu__item${cls}" type="button" role="menuitem"${attrs}>${icon(ic)}<span class="menu__label">${label}</span>${end ? `<span class="menu__end">${end}</span>` : ""}</button>`;
    return `<div class="pop__head"><span class="pop__title ellipsis">${esc(t.title)}</span><span class="pop__hint ellipsis">${esc(t.artist)}</span></div>
      ${item("arrow-bend-down-right", "Играть следующим", Kbd("⌘", "↵"), ` data-action="play-next" data-track="${t.id}"`, o.hover ? " is-hover" : "")}
      ${item("list-plus", "В очередь", "", ` data-action="enqueue" data-track="${t.id}"`)}
      ${item("playlist", "В плейлист", icon("caret-right", { cls: "i-16" }))}
      ${item("waveform", "Волна по треку", "", ` data-action="track-wave" data-track="${t.id}"`)}
      ${item(liked ? "heart-break" : "heart", liked ? "Убрать из любимых" : "В любимые", Kbd("L"), ` data-action="like" data-track="${t.id}"`)}
      <div class="pop__sep"></div>
      ${item("download-simple", "Скачать на это устройство", "", ` data-action="download" data-track="${t.id}"`)}
      ${item("user", "Перейти к артисту")}
      ${item("disc", "Перейти к альбому")}
      <div class="pop__sep"></div>
      ${item("prohibit", "Не рекомендовать", "", ` data-action="dislike" data-track="${t.id}"`)}
      ${item("warning-circle", "Неверная запись — исправить")}`;
  }

  function TunerMenu(group) {
    return `<div class="pop__head"><span class="pop__title">${GROUP_LABEL[group]}</span>${group === "activity" ? `<span class="pop__hint">задаёт ход сессии</span>` : ""}</div>
      ${GROUPS[group].map((x) => `<button class="menu__item${state[group] === x.id ? " is-selected" : ""}" type="button" role="menuitemradio" aria-checked="${state[group] === x.id}" data-action="set-opt" data-group="${group}" data-value="${x.id}">
          ${group === "mood" ? `<span class="mood" data-mood="${x.id}"><span class="mood__orb" style="width:18px;height:18px"></span></span>` : icon(x.icon)}
          <span class="menu__label">${x.label}${x.hint ? `<span class="menu__desc">${x.hint}</span>` : ""}</span>
          ${icon("check", { cls: "menu__check i-18" })}
        </button>`).join("")}`;
  }

  const Toast = (text, o = {}) => `<div class="toast${o.cls ? " " + o.cls : ""}" role="${o.cls === "toast--error" ? "alert" : "status"}">${icon(o.icon || "check-circle", { fill: true })}<span class="toast__text">${text}</span>${o.undo ? `<button class="toast__undo${o.hover ? " is-hover" : ""}" type="button" data-action="undo">Отменить</button>` : ""}${o.retry ? `<button class="toast__undo" type="button">${o.retry}</button>` : ""}</div>`;

  function Ring(p, o = {}) {
    const size = o.size || 24;
    const st = o.state || "progress";
    const C = 2 * Math.PI * 10;
    const inner = st === "done" ? icon("check", { cls: "" }) : st === "queued" ? icon("arrow-down") : `<span class="ring__stop"></span>`;
    const bar = st === "progress" ? `<circle class="ring__bar" cx="12" cy="12" r="10" fill="none" stroke-width="2.2" stroke-dasharray="${C.toFixed(2)}" stroke-dashoffset="${(C * (1 - p)).toFixed(2)}"/>` : st === "done" ? `<circle class="ring__bar" cx="12" cy="12" r="10" fill="none" stroke-width="2.2"/>` : "";
    const label = st === "done" ? "Скачано" : st === "queued" ? "В очереди на загрузку" : `Загрузка ${Math.round(p * 100)}${NB}%`;
    return `<span class="ring ring--${st}" style="--size:${size}px" role="img" aria-label="${label}" data-tip="${label}"><svg viewBox="0 0 24 24"><circle class="ring__track" cx="12" cy="12" r="10" fill="none" stroke-width="2.2"/>${bar}</svg>${inner}</span>`;
  }

  const Segmented = (items, sel, o = {}) => `<div class="segmented" role="tablist"${o.label ? ` aria-label="${o.label}"` : ""}>${items.map((it) => `<button class="segmented__item${it.id === sel ? " is-selected" : ""}" type="button" role="tab" aria-selected="${it.id === sel}" data-action="${o.action || "seg"}" data-value="${it.id}">${it.dot ? `<span class="segmented__dot" style="--dot:${it.dot}"></span>` : ""}${it.icon ? icon(it.icon) : ""}${it.label}</button>`).join("")}</div>`;
  const Tabs = (items, sel, o = {}) => `<div class="tabs" role="tablist">${items.map((it) => `<button class="tabs__item${it.id === sel ? " is-selected" : ""}${o.hover === it.id ? " is-hover" : ""}" type="button" role="tab" aria-selected="${it.id === sel}" data-action="${o.action || "tab"}" data-value="${it.id}">${it.label}${it.count != null ? `<span class="tabs__count tnum">${it.count}</span>` : ""}</button>`).join("")}</div>`;
  const Chip = (label, o = {}) => o.remove
    ? `<span class="chip chip--remove">${label}<button class="chip__x" type="button" aria-label="Убрать «${label}»">${icon("x", { cls: "i-14" })}</button></span>`
    : `<button class="chip${o.selected ? " is-selected" : ""}${o.hover ? " is-hover" : ""}" type="button" aria-pressed="${!!o.selected}"${o.disabled ? " disabled" : ""}>${o.selected ? icon("check") : o.icon ? icon(o.icon) : ""}${label}</button>`;
  const Search = (o = {}) => `<label class="search${o.value ? " is-filled" : ""}${o.focus ? " is-focus" : ""}${o.hover ? " is-hover" : ""}">${icon("magnifying-glass")}<input type="search" placeholder="${o.placeholder || "Треки, артисты, альбомы"}" value="${esc(o.value || "")}" aria-label="Поиск" data-search>${Kbd("⌘", "K")}<button class="icon-btn icon-btn--xs search__clear" type="button" aria-label="Очистить">${icon("x-circle", { fill: true })}</button></label>`;
  const Switch = (label, o = {}) => `<label class="switch${o.disabled ? " is-disabled" : ""}"><input type="checkbox"${o.on ? " checked" : ""}${o.disabled ? " disabled" : ""}><span class="switch__track"></span><span class="switch__text">${label}${o.hint ? `<span class="switch__hint">${o.hint}</span>` : ""}</span></label>`;
  const Empty = (o) => `<div class="empty"><div class="empty__icon">${icon(o.icon)}</div><div class="empty__title">${o.title}</div><p class="empty__text">${o.text}</p>${o.action ? `<button class="btn btn--secondary btn--sm" type="button">${o.action}</button>` : ""}</div>`;

  // ---------- Хром платформ ----------

  const IOS = `<div class="ios-status" aria-hidden="true"><span>9:41<span class="ios-status__date">Ср 11 нояб.</span></span><span class="ios-status__icons">
      <svg width="18" height="12" viewBox="0 0 18 12" fill="#fff"><rect x="0" y="7.5" width="3" height="4.5" rx="1"/><rect x="5" y="5" width="3" height="7" rx="1"/><rect x="10" y="2.5" width="3" height="9.5" rx="1"/><rect x="15" y="0" width="3" height="12" rx="1"/></svg>
      <svg width="16" height="12" viewBox="0 0 16 12" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round"><path d="M1.6 4.4a9.4 9.4 0 0 1 12.8 0"/><path d="M4.2 7.1a5.6 5.6 0 0 1 7.6 0"/><circle cx="8" cy="10" r="1.4" fill="#fff" stroke="none"/></svg>
      <svg width="27" height="13" viewBox="0 0 27 13"><rect x=".5" y=".5" width="23" height="12" rx="3.6" fill="none" stroke="#fff" stroke-opacity=".4"/><rect x="2" y="2" width="20" height="9" rx="2.2" fill="#fff"/><path d="M25 4.4v4.2c.8-.3 1.4-1.1 1.4-2.1s-.6-1.8-1.4-2.1z" fill="#fff" fill-opacity=".45"/></svg>
    </span></div>`;
  const TRAFFIC = `<div class="traffic" aria-hidden="true">
      <span><svg viewBox="0 0 6 6"><path d="M1 1l4 4M5 1L1 5" stroke="#4d0000" stroke-width="1.1"/></svg></span>
      <span><svg viewBox="0 0 6 6"><path d="M.8 3h4.4" stroke="#5a3d00" stroke-width="1.1"/></svg></span>
      <span><svg viewBox="0 0 6 6"><path d="M1.2 4.8V1.6l3.2 3.2H1.2zM4.8 1.2v3.2L1.6 1.2h3.2z" fill="#0a4a00"/></svg></span>
    </div>`;

  // ---------- Оболочка ----------

  function Sidebar() {
    const t = cur();
    return `<aside class="sidebar" aria-label="Навигация">
      <div class="sidebar__chrome" data-tauri-drag-region>${TRAFFIC}</div>
      <div class="sidebar__brand">${Brand()}</div>
      <nav class="nav">
        <a class="nav__item${state.route === "home" ? " is-active" : ""}" href="#home">${icon("house", { fill: state.route === "home" })}<span>Главная</span></a>
        <a class="nav__item" href="#home" data-action="focus-search">${icon("magnifying-glass")}<span>Поиск</span>${Kbd("⌘", "K")}</a>
        <a class="nav__item" href="#home">${icon("books")}<span>Медиатека</span></a>
      </nav>
      <div class="sidebar__scroll">
        <section class="side-sec" aria-label="Закреплённое">
          <header class="side-sec__head"><span>Закреплено</span>${IconBtn("plus", "Новый плейлист", { size: "xs" })}</header>
          <a class="side-item" href="#home"><span class="side-item__art side-item__art--liked">${icon("heart", { fill: true })}</span><span class="side-item__text"><span class="side-item__title">Любимые треки</span><span class="side-item__meta">${icon("push-pin", { fill: true })}<span data-bind="liked-count">${num(408 + state.liked.size)}</span> треков</span></span></a>
          ${PLAYLISTS.map((p, i) => `<a class="side-item" href="#home"><span class="side-item__art">${Mosaic(p.covers)}</span><span class="side-item__text"><span class="side-item__title">${p.title}</span><span class="side-item__meta">${i < 2 ? icon("push-pin", { fill: true }) : ""}Плейлист · ${p.count}</span></span></a>`).join("")}
        </section>
        <section class="side-sec" aria-label="Артисты">
          <header class="side-sec__head"><span>Артисты</span></header>
          ${FOLLOWED.map((n) => `<a class="side-item side-item--artist" href="#home"><span class="side-item__art">${`<img src="${artistPic(n)}" alt="" loading="lazy" style="width:100%;height:100%;object-fit:cover">`}</span><span class="side-item__text"><span class="side-item__title">${esc(n)}</span></span>${FRESH.has(n) ? `<span class="side-item__new" data-tip="Новый релиз"></span>` : ""}</a>`).join("")}
        </section>
      </div>
      <div class="sidebar__foot">
        <button class="sync" type="button" data-tip="Сервер скачивает музыку по подпискам">${Ring(48 / 132, { size: 28 })}<span class="sync__text"><span class="sync__title">Загрузка подписок</span><span class="sync__meta tnum">48 из 132 · Rushex, Mytch</span></span></button>
      </div>
    </aside>`;
  }

  const Topbar = () => `<header class="topbar" data-tauri-drag-region>
      <div class="topbar__nav">${IconBtn("caret-left", "Назад", { size: "sm" })}${IconBtn("caret-right", "Вперёд", { size: "sm", attrs: " disabled" })}</div>
      ${Search()}
      <div class="topbar__spacer" data-tauri-drag-region></div>
      <button class="avatar" type="button" aria-label="Профиль: Захар" data-tip="Захар · daly.rest">З</button>
    </header>`;

  function QueueBody() {
    const t = cur();
    const manual = state.manual.map((id, i) => TrackRow(T(id), { variant: "queue", index: i + 1, reason: false })).join("");
    const wave = state.wave.map((id, i) => TrackRow(T(id), { variant: "queue", index: state.manual.length + i + 1, reason: true })).join("");
    return `<section class="qsec">
        <header class="qsec__head"><h3 class="qsec__title">Сейчас</h3><span class="qsec__hint wave-tag"><span class="live-dot"></span>Моя волна · ${MOODS.find((m) => m.id === state.mood).label}</span></header>
        ${TrackRow(t, { variant: "queue", reason: true, playing: true })}
      </section>
      ${state.manual.length ? `<section class="qsec">
        <header class="qsec__head"><h3 class="qsec__title">Далее${NB}— добавлено вручную</h3><span class="qsec__hint"><button type="button" data-action="clear-manual">Очистить</button></span></header>
        ${manual}
      </section>` : ""}
      <section class="qsec">
        <header class="qsec__head"><h3 class="qsec__title">Далее${NB}— из волны</h3><span class="qsec__hint">подбирается на ходу</span></header>
        ${wave}
      </section>`;
  }

  function PanelBody() {
    if (state.panelTab === "queue") return QueueBody();
    if (state.panelTab === "lyrics") return Empty({ icon: "microphone", title: "Текста пока нет", text: `Сервер ищет текст для «${esc(cur().title)}». Обычно это занимает пару минут после первого прослушивания.`, action: "Найти вручную" });
    if (state.panelTab === "devices") return `<div style="padding:8px 0">${DevicePicker()}</div>`;
    const t = cur();
    const m = meta(t);
    const rows = [["Альбом", esc(t.album || t.title)], ["Длительность", dur(t.durationMs)], ["Темп", m.bpmLabel], ["Тональность", m.key], ["Качество", "Opus, 160 кбит/с"], ["На устройстве", DOWNLOADED.has(t.id) ? "Скачано, 3,4 МБ" : "Потоком с сервера"], ["Почему в волне", esc(reasonOf(t))]];
    return `<dl style="margin:12px 8px;display:grid;grid-template-columns:auto 1fr;gap:12px 20px;font-size:13px">${rows.map(([k, v]) => `<dt style="color:var(--text-3)">${k}</dt><dd style="margin:0">${v}</dd>`).join("")}</dl>`;
  }

  function Panel() {
    const tabs = [{ id: "queue", label: "Очередь" }, { id: "lyrics", label: "Текст" }, { id: "devices", label: "Устройства" }, { id: "about", label: "О треке" }];
    return `<aside class="panel" aria-label="Правая панель"><div class="panel__inner">
      <div class="panel__head">${Tabs(tabs, state.panelTab, { action: "panel-tab" })}${IconBtn("x", "Закрыть панель", { size: "sm", attrs: ` data-action="panel-close"` })}</div>
      <div class="panel__body" id="panel-body">${PanelBody()}</div>
      ${state.panelTab === "queue" ? `<div class="panel__foot"><span class="panel__foot-text">Волна не кончается: после этих треков подберёт ещё.</span><button class="btn btn--secondary btn--sm" type="button" data-action="save-queue">${icon("plus")}В плейлист</button></div>` : ""}
    </div></aside>`;
  }

  const TabBar = () => `<nav class="tabbar" aria-label="Разделы"><div class="tabbar__row">
      <a class="tabbar__item is-active" href="#home">${icon("house", { fill: true })}Главная</a>
      <a class="tabbar__item" href="#home">${icon("magnifying-glass")}Поиск</a>
      <a class="tabbar__item" href="#home">${icon("books")}Медиатека</a>
    </div></nav>`;

  function Shell(page) {
    return `${IOS}
      <div class="app${state.panel ? " is-panel" : ""}" data-route="${state.route}" id="app">
        ${Sidebar()}
        <main class="main" id="main">${Topbar()}<div class="page" id="page">${page}</div></main>
        ${Panel()}
        <footer class="nowbar" id="nowbar" aria-label="Сейчас играет">${NowBar()}</footer>
      </div>
      <div class="mini" id="mini" aria-label="Мини-плеер">${MiniInner()}</div>
      ${TabBar()}
      <div class="home-ind" aria-hidden="true"></div>`;
  }

  // ---------- Главная ----------

  function greeting() {
    return "Доброе утро";
  }
  function todayLabel() {
    const s = new Intl.DateTimeFormat("ru-RU", { weekday: "long", day: "numeric", month: "long" }).format(new Date(2026, 10, 11));
    return s.charAt(0).toUpperCase() + s.slice(1);
  }

  function HeroNow() {
    const t = cur();
    return `<div class="hero__now-label">${state.playing ? "Сейчас играет" : "На паузе"}</div>
      <div class="hero__now-title">${esc(t.title)}</div>
      <div class="hero__now-sub"><span class="ellipsis">${esc(t.artist)}</span><span class="reason">${icon("sparkle", { fill: true })}${esc(reasonOf(t))}</span></div>`;
  }

  function Hero() {
    return `<section class="hero" aria-label="Моя волна">
      <div class="hero__stage" id="hero-stage">
        <div class="hero__wave" id="hero-wave"></div>
        <div class="hero__intro">
          <p class="hero__eyebrow">${greeting()}, ${esc(D.me.name)}</p>
          <h1 class="hero__title">Моя волна</h1>
          <p class="hero__summary" data-bind="summary">${summary()}</p>
        </div>
        <div class="hero__play">${WavePlay({ size: 76 })}<div class="hero__now" id="hero-now">${HeroNow()}</div></div>
        <div class="hero__dock">${Dock()}</div>
      </div>
      <div class="hero__below">
        ${Moods("full")}
        <div class="tokens scroll-x">${Token("activity")}${Token("character")}${Token("lang")}</div>
        <div class="presets-row scroll-x" role="group" aria-label="Сохранённые сочетания">${PRESETS.map((p) => Preset(p)).join("")}
          <button class="preset preset--add" type="button" data-action="save-preset"><span class="preset__swatch">${icon("plus")}</span><span class="preset__text"><span class="preset__title">Сохранить</span><span class="preset__meta">текущее сочетание</span></span></button>
        </div>
      </div>
    </section>`;
  }

  function HomePage() {
    const releases = NEW_RELEASES.map((id) => albums.get(id)).filter(Boolean);
    return `<div class="home">
      <header class="phone-head">
        <div><div class="phone-head__date">${todayLabel()}</div><div class="phone-head__hello">${greeting()}, ${esc(D.me.name)}</div></div>
        <button class="avatar avatar--lg" type="button" aria-label="Профиль: Захар">З</button>
      </header>
      ${Hero()}
      <section class="shelf" aria-label="Продолжить">
        ${ShelfHead("Продолжить", { all: false })}
        <div class="row row--resume scroll-x">${RESUME.map((r) => Resume(r)).join("")}</div>
      </section>
      <section class="shelf" aria-label="Миксы дня">
        ${ShelfHead("Миксы дня", { hint: "Обновлены сегодня утром", nav: true })}
        <div class="row row--mix scroll-x" data-row>${MIXES.map((m) => MixCard(m)).join("")}</div>
      </section>
      <section class="shelf" aria-label="Новинки подписок">
        ${ShelfHead("Новинки подписок", { hint: "Сервер уже скачал всё, кроме анонсов", nav: true })}
        <div class="row scroll-x" data-row>${releases.map((al) => AlbumCard(al, { isNew: al.releaseDate >= "2026-11-04", sub: `${esc(al.artist)} · ${dateShort(al.releaseDate)}` })).join("")}</div>
      </section>
      <section class="shelf" aria-label="Открытия недели">
        ${ShelfHead("Открытия недели", { hint: "30 треков, которых вы ещё не слышали", all: true })}
        <div class="discover">
          <div class="discover__feature">
            ${GenCover("Открытия недели", "9–15 ноября")}
            <div class="discover__text">
              <div class="discover__meta"><div class="discover__title">Открытия недели</div><div class="discover__sub">Обновлено в понедельник · 1${NB}ч 38${NB}мин</div></div>
              <div class="discover__actions" style="margin-top:12px"><button class="btn btn--primary btn--sm" type="button" data-action="play-discover">${icon("play", { fill: true })}Слушать</button>${IconBtn("heart", "Сохранить подборку", { size: "sm" })}</div>
            </div>
          </div>
          <div class="discover__list" role="table" aria-label="Треки подборки">${DISCOVER.map((id, i) => TrackRow(T(id), { index: i + 1, reason: true })).join("")}</div>
        </div>
      </section>
    </div>`;
  }

  // ---------- Плеер ----------

  function PlayerPage() {
    const t = cur();
    const m = meta(t);
    const moodLabel = MOODS.find((x) => x.id === state.mood).label;
    const liked = state.liked.has(t.id);
    return `<div class="player" id="player">
      <div class="player__wave" id="player-wave"></div>
      <div class="player__collapse"><a class="icon-btn" href="#home" aria-label="Свернуть плеер" data-tip="Свернуть">${icon("caret-down")}</a></div>
      <div class="player__top"><span class="player__context"><span class="live-dot"></span>Моя волна · <span data-bind="mood-label">${moodLabel}</span></span></div>
      <div class="player__mood"><span class="player__mood-label">Настроение</span>${Moods("compact")}</div>
      <div class="player__stage">
        <header class="player__phone-head">
          <a class="icon-btn" href="#home" aria-label="Свернуть плеер">${icon("caret-down", { cls: "i-24" })}</a>
          <div class="player__phone-ctx"><div class="player__phone-kicker">Играет волна</div><div class="player__phone-title">Моя волна · <span data-bind="mood-label">${moodLabel}</span></div></div>
          <button class="icon-btn" type="button" data-action="menu" data-track="${t.id}" aria-label="Ещё">${icon("dots-three", { cls: "i-24" })}</button>
        </header>
        <div class="player__art" id="player-art">${Art(src(t.cover), { eager: true, alt: `Обложка «${t.title}»` })}</div>
        <div class="player__info">
          <div class="player__titlerow">
            <div class="player__titles">
              <h1 class="player__title">${esc(t.title)}</h1>
              <div class="player__artist"><a href="#home">${esc(t.artist)}</a></div>
            </div>
            ${Quality()}
          </div>
          <div class="player__reason reason">${icon("sparkle", { fill: true })}${esc(reasonOf(t))}</div>
          <div class="player__chips">${Quality()}<span class="badge badge--meta tnum">${m.bpmLabel}</span><span class="badge badge--meta">${m.key}</span>${DOWNLOADED.has(t.id) ? `<span class="badge badge--meta">${icon("arrow-circle-down", { fill: true })}На устройстве</span>` : ""}</div>
        </div>
        <div class="player__reactions">
          <button class="btn btn--secondary btn--sm" type="button" data-action="dislike">${icon("thumbs-down")}Не нравится</button>
          <button class="btn btn--secondary btn--sm" type="button" data-action="not-now">${icon("arrow-bend-up-right")}Не сейчас</button>
          <button class="btn btn--secondary btn--sm" type="button" data-action="more-like">${icon("sparkle")}Больше такого</button>
        </div>
        <div class="player__scrub">
          ${Waveform(t, { bars: 72, cls: "wf--lg", stroke: 2.6 })}
          <div class="player__times"><span data-bind="pos">${dur(state.position)}</span><span class="mid">${icon(modeOf().icon, { cls: "i-14" })}Переход в ${dur(transitionAt(t))}</span><span data-bind="remain">−${dur(t.durationMs - state.position)}</span></div>
        </div>
        <div class="player__controls">
          ${IconBtn("thumbs-down", "Не нравится", { attrs: ` data-action="dislike"` })}
          ${IconBtn("skip-back", "Предыдущий", { fill: true, attrs: ` data-action="prev"` })}
          ${WavePlay({ size: 80, action: "toggle" })}
          ${IconBtn("skip-forward", "Следующий", { fill: true, attrs: ` data-action="next"` })}
          <button class="icon-btn${liked ? " is-liked" : ""}" type="button" data-action="like" data-track="${t.id}" aria-label="${liked ? "Убрать из любимых" : "Нравится"}" aria-pressed="${liked}">${icon("heart", { fill: liked })}</button>
        </div>
        <div class="player__secondary">
          ${IconBtn("laptop", "Устройства", { attrs: ` data-action="devices"` })}
          ${ModeChip()}
          ${IconBtn("microphone", "Текст")}
          ${IconBtn("queue", "Очередь", { attrs: ` data-action="panel" data-value="queue"` })}
        </div>
      </div>
    </div>`;
  }

  // ---------- Экран волны ----------

  function WavePage() {
    const ws = window.Wave.state;
    const pvs = D.previews.map((p) => {
      const t = T(p.trackId);
      const an = ANALYSIS[p.trackId];
      return `<button class="pv" type="button" data-action="pv" data-value="${p.trackId}">
        ${Art(src(t.cover))}
        <span class="pv__text"><span class="pv__title">${esc(t.title)}</span><span class="pv__sub">${esc(t.artist)} · <span class="tnum">${Math.round(an.bpm)}${NB}BPM</span></span></span>
        <span class="pv__btn">${icon("play", { fill: true, cls: "i-16" })}</span>
        <span class="pv__bar"><i></i></span>
      </button>`;
    }).join("");
    return `<div class="wavepage">
      <div class="wavepage__canvas" id="full-wave"></div>
      <div class="wavepage__brand"><a class="icon-btn" href="#home" aria-label="На главную">${icon("arrow-left")}</a>${Brand()}</div>
      <aside class="debug" aria-label="Отладка волны">
        <header class="debug__head"><div class="debug__title"><span class="beat-dot" data-bind="beat-dot"></span>Волна</div><span class="debug__fps" data-bind="fps">— кадров/с</span></header>
        <div class="debug__body">
          <div><div class="debug__label">Настроение <span>перетекание 1,2${NB}с</span></div>${Moods("full")}</div>
          <div><div class="debug__label">Энергия <b data-bind="energy">${ws.energy.toFixed(2).replace(".", ",")}</b></div><input class="range" type="range" min="0" max="1" step="0.01" value="${ws.energy.toFixed(2)}" style="--v:${ws.energy}" aria-label="Энергия" data-action="w-energy"></div>
          <div><div class="debug__label">Темп <b data-bind="bpm">${Math.round(ws.bpm)}${NB}BPM</b></div><input class="range" type="range" min="60" max="180" step="1" value="${Math.round(ws.bpm)}" style="--v:${(ws.bpm - 60) / 120}" aria-label="Темп" data-action="w-bpm"></div>
          <div class="debug__actions">
            <button class="btn btn--secondary btn--sm" type="button" data-action="w-start">${icon("play")}Старт</button>
            <button class="btn btn--secondary btn--sm" type="button" data-action="w-skip">${icon("skip-forward")}Пропуск</button>
            <button class="btn btn--secondary btn--sm" type="button" data-action="w-like">${icon("heart")}Лайк</button>
          </div>
          <div class="meters">
            <span>Бас</span><span class="meter" data-bind="m0"><i></i></span>
            <span>Середина</span><span class="meter" data-bind="m1"><i></i></span>
            <span>Верх</span><span class="meter" data-bind="m2"><i></i></span>
            <span>Источник</span><span data-bind="source" style="color:var(--text-2)">часы, 120 BPM</span>
          </div>
          <div><div class="debug__label">Превью, живой звук <span>30${NB}с</span></div><div class="previews">${pvs}</div></div>
        </div>
      </aside>
    </div>`;
  }

  // ---------- Галерея компонентов ----------

  function ComponentsPage() {
    const t = cur();
    const tPlay = T(909494702);
    const sec = (n, title, desc, body) => `<section class="sec" id="c-${n}"><header class="sec__head"><span class="sec__num tnum">${String(n).padStart(2, "0")}</span><div><h2 class="sec__title">${title}</h2>${desc ? `<p class="sec__desc">${desc}</p>` : ""}</div></header>${body}</section>`;
    const spec = (label, body, o = {}) => `<div class="spec"><div class="spec__label">${label}</div><div class="spec__box${o.col ? " spec__box--col" : ""}${o.plain ? " spec__box--plain" : ""}${o.dark ? " spec__box--dark" : ""}"${o.style ? ` style="${o.style}"` : ""}>${body}</div></div>`;
    const states = (rows) => `<div class="states">${rows.map(([n, b]) => `<span class="states__name">${n}</span><div style="display:flex;gap:12px;align-items:center;flex-wrap:wrap;min-width:0">${b}</div>`).join("")}</div>`;
    const swatch = (name, val, bg) => `<div class="swatch"><div class="swatch__chip" style="background:${bg || val}"></div><div><div class="swatch__name">${name}</div><div class="swatch__val">${val}</div></div></div>`;
    const btnStates = (variant, label, ic) => ["", "is-hover", "is-pressed", "is-focus", "is-disabled"].map((s) => `<button class="btn btn--${variant} ${s}" type="button"${s === "is-disabled" ? " disabled" : ""}>${ic ? icon(ic, { fill: ic === "play" }) : ""}${label}</button>`).join("");
    const discoverRows = [
      ["По умолчанию", TrackRow(T(2662530582), { index: 4, reason: true, playing: false })],
      ["Наведение", TrackRow(T(2124328597), { index: 1, reason: true, playing: false, hover: true })],
      ["Играет", TrackRow(tPlay, { index: 2, reason: true, playing: true })],
      ["Нет на сервере", TrackRow(T(836932782), { index: 3, playing: false, remote: true })],
      ["Скачано", TrackRow(T(3657203312), { index: 5, reason: true, playing: false, downloaded: true })],
    ];
    return `<div class="sheet">
      <header class="sheet__head">
        <div><p class="sheet__kicker">DalyWave · направление A «Полночь»</p><h1 class="sheet__title">Компоненты</h1><p class="sheet__lead">Словарь интерфейса: свои типы элементов с${NB}состояниями. Одни и${NB}те${NB}же функции собирают и${NB}эту страницу, и${NB}экраны.</p></div>
        <nav class="sheet__nav"><a class="btn btn--secondary btn--sm" href="#home">${icon("house")}Главная</a><a class="btn btn--secondary btn--sm" href="#player">${icon("play-circle")}Плеер</a><a class="btn btn--secondary btn--sm" href="#wave">${icon("waveform")}Волна</a></nav>
      </header>

      ${sec(1, "Логотип и значок", "Знак — одна линия волны в форме «W». Не перерисовывается.", `<div class="grid grid--4">
        ${spec("Знак в цвете текста и акцента", `<div style="display:flex;align-items:center;gap:20px"><span style="color:var(--text-1)">${glyph("glyph-xl")}</span><span style="color:var(--accent)">${glyph("glyph-xl")}</span></div>`, { style: "justify-content:center;height:140px" })}
        ${spec("Знак и слово в боковой панели", Brand(), { style: "justify-content:center;height:140px" })}
        ${spec("Значок приложения", `<img src="${BRAND}icon-rounded.svg" alt="Значок DalyWave" width="96" height="96">`, { style: "justify-content:center;height:140px" })}
        ${spec("Значок macOS", `<img src="${BRAND}icon-macos.svg" alt="Значок DalyWave для macOS" width="112" height="112">`, { style: "justify-content:center;height:140px" })}
      </div>`)}

      ${sec(2, "Цвет", "Почти чёрная основа. Акцент меняется вместе с настроением волны и встречается только у того, что играет.", `
        <div class="swatches">${[["ink-0 окно", "#040507"], ["ink-1 фон", "#08090c"], ["ink-2", "#0e1015"], ["ink-3 поля", "#14171d"], ["ink-4 поповеры", "#1b1f27"], ["text-1", "#f3f5f9"], ["text-2 · 68 %", "rgba(232,236,244,.68)"], ["text-3 · 56 %", "rgba(232,236,244,.56)"], ["line · 9 %", "rgba(255,255,255,.09)"], ["fill-2 · 7,5 %", "rgba(255,255,255,.075)"], ["Успех", "#7fdcae"], ["Ошибка", "#ff8080"]].map(([n, v]) => swatch(n, v)).join("")}</div>
        <div class="mood-swatches" style="margin-top:24px">${MOODS.map((m) => { const c = MOOD_COLORS[m.id]; return `<div class="mood-swatch" style="background:radial-gradient(120% 90% at 20% 110%, ${c[1]}, transparent 60%), radial-gradient(90% 70% at 100% 0%, ${c[2]}99, transparent 55%), ${c[0]}"><span>${m.label}</span></div>`; }).join("")}</div>`)}

      ${sec(3, "Типографика", "Onest: одна гарнитура, кириллица, табличные цифры в длительностях.", `<div class="typescale">
        ${[["Дисплей · 56/56 · 750", `<span style="font-size:56px;font-weight:750;letter-spacing:-.035em;line-height:1">Моя волна</span>`], ["Заголовок 1 · 40 · 750", `<span style="font-size:40px;font-weight:750;letter-spacing:-.035em;line-height:1.05">Float</span>`], ["Заголовок 2 · 28 · 750", `<span style="font-size:28px;font-weight:750;letter-spacing:-.02em">Доброе утро, Захар</span>`], ["Заголовок 3 · 22 · 700", `<span style="font-size:22px;font-weight:700;letter-spacing:-.02em">Открытия недели</span>`], ["Подзаголовок · 17", `<span style="font-size:17px;color:var(--text-2)">Спокойная волна для работы${NB}— знакомое и${NB}новое</span>`], ["Текст · 14", `<span>Сервер уже скачал всё, кроме анонсов. «Ёлочки», тире${NB}— и${NB}буква ё.</span>`], ["Подпись · 13 · 12", `<span style="font-size:13px;color:var(--text-2)">Call Me Karizma · Похоже на Lowborn</span>`], ["Метка · 11 · капс", `<span style="font-size:11px;font-weight:650;letter-spacing:.09em;text-transform:uppercase;color:var(--text-3)">Закреплено</span>`], ["Цифры · табличные", `<span class="tnum" style="font-size:17px">1:12 · 2:49 · 12${NB}480 · 0:08</span>`]].map(([m, s]) => `<div class="typescale__row"><span class="typescale__meta">${m}</span>${s}</div>`).join("")}
      </div>`)}

      ${sec(4, "Отступы, скругления, тени, движение", "Сетка 4/8. Длительности и кривые общие для всех элементов.", `<div class="grid grid--2">
        ${spec("Отступы", `<div class="spacing-demo">${[4, 8, 12, 16, 24, 32, 48, 64].map((s) => `<div style="width:${s}px;height:${s}px" title="${s}"></div>`).join("")}</div>`)}
        ${spec("Скругления", `<div class="radius-demo">${[["6", 6], ["8", 8], ["12", 12], ["16", 16], ["24", 24], ["32", 32], ["круг", 999]].map(([n, r]) => `<div style="border-radius:${r}px">${n}</div>`).join("")}</div>`)}
        ${spec("Тени", `<div style="display:flex;gap:20px;flex-wrap:wrap">${[["Карточка", "var(--shadow-1)"], ["Обложка", "var(--shadow-cover)"], ["Поповер", "var(--shadow-pop)"]].map(([n, s]) => `<div style="width:120px;height:72px;border-radius:14px;background:var(--ink-3);box-shadow:${s};display:grid;place-items:center;font-size:12px;color:var(--text-3)">${n}</div>`).join("")}</div>`)}
        ${spec("Движение", `<div class="motion-demo" style="width:100%">${[["120 мс", "наведение, нажатие"], ["200 мс", "поповеры, переключатели"], ["320 мс", "панели, карточки"], ["560 мс", "смена экрана"], ["1200 мс", "перетекание настроения"], ["ease-out", "cubic-bezier(.16,1,.3,1)"]].map(([a, b]) => `<div><b>${a}</b>${b}</div>`).join("")}</div>`, { col: true })}
      </div>`)}

      ${sec(5, "Кнопки", "Основная, второстепенная, текстовая, круглая с иконкой. Состояния: обычное, наведение, нажатие, фокус, неактивна.", `<div class="spec__box spec__box--col">${states([
        ["Основная", btnStates("primary", "Слушать", "play")],
        ["Второстепенная", btnStates("secondary", "Подписаться")],
        ["Текстовая", btnStates("ghost", "Все")],
        ["С иконкой", ["", "is-hover", "is-pressed", "is-focus", "is-disabled"].map((s) => `<button class="icon-btn ${s}" type="button" aria-label="Нравится"${s === "is-disabled" ? " disabled" : ""}>${icon("heart")}</button>`).join("") + `<button class="icon-btn is-liked" type="button" aria-label="В любимых">${icon("heart", { fill: true })}</button><button class="icon-btn is-on" type="button" aria-label="Очередь открыта">${icon("queue")}</button>`],
        ["Размеры", `<button class="btn btn--primary btn--sm" type="button">Маленькая</button><button class="btn btn--primary" type="button">Обычная</button><button class="btn btn--primary btn--lg" type="button">Крупная</button><button class="play-btn" type="button" aria-label="Слушать">${icon("play", { fill: true })}</button>`],
      ])}</div>`)}

      ${sec(6, "Кнопка волны", "Главный элемент главной. Кольцо в цветах настроения вращается и пульсирует в долю.", `<div class="grid grid--4">
        ${spec("Ждёт", WavePlay({ playing: false, size: 76, action: "noop" }), { dark: true, style: "justify-content:center;height:160px" })}
        ${spec("Играет", WavePlay({ playing: true, size: 76, action: "noop" }), { dark: true, style: "justify-content:center;height:160px" })}
        ${spec("Загружается", WavePlay({ loading: true, size: 76, action: "noop" }), { dark: true, style: "justify-content:center;height:160px" })}
        ${spec("Наведение и фокус", WavePlay({ playing: false, size: 76, action: "noop", cls: "is-hover is-focus" }), { dark: true, style: "justify-content:center;height:160px" })}
      </div>`)}

      ${sec(7, "Настройки волны", "Настроение задаёт цвет волны. Остальное — в компактных ячейках с выпадающим списком, а не стеной чипов.", `<div class="grid grid--2">
        ${spec("Настроение · компактно (наведение на «Бодрое»)", Moods("compact", { hover: "energetic" }))}
        ${spec("Настроение · телефон", Moods("full"), { col: true })}
        ${spec("Док настроек", `<div class="scroll-x" style="max-width:100%">${Dock()}</div>`, { dark: true })}
        ${spec("Токены на телефоне", `<div style="display:flex;gap:8px;flex-wrap:wrap">${Token("activity")}${Token("character")}${Token("lang")}</div>`, { dark: true })}
        ${spec("Сохранённые сочетания · обычное, наведение, текущее", `<div style="display:flex;flex-direction:column;gap:2px;width:100%">${Preset(PRESETS[0], { hover: true })}${Preset(PRESETS[1])}${Preset(PRESETS[2])}</div>`, { col: true })}
        ${spec("Список вариантов", `<div class="pop pop--static">${TunerMenu("activity")}</div>`, { plain: true })}
      </div>`)}

      ${sec(8, "Сегменты, вкладки, чипы", "", `<div class="grid grid--3">
        ${spec("Сегменты", `<div style="display:flex;flex-direction:column;gap:12px">${Segmented([{ id: "library", label: "Медиатека" }, { id: "all", label: "Весь каталог" }], "library")}${Segmented([{ id: "grid", label: "Сетка", icon: "squares-four" }, { id: "list", label: "Список", icon: "rows" }], "list")}</div>`)}
        ${spec("Вкладки", Tabs([{ id: "q", label: "Очередь" }, { id: "l", label: "Текст" }, { id: "d", label: "Устройства" }, { id: "a", label: "О треке" }], "q", { hover: "l" }), { col: true })}
        ${spec("Чипы", `${Chip("Альт-рэп", { selected: true })}${Chip("Фонк")}${Chip("Дарк-поп", { hover: true })}${Chip("Хайперпоп", { icon: "lightning" })}${Chip("Lowborn", { remove: true })}${Chip("Шугейз", { disabled: true })}`)}
      </div>`)}

      ${sec(9, "Строка трека", "Одна строка для очереди, подборок и альбомов. Причина появления — серым после артиста.", `<div class="spec__box spec__box--col" style="gap:4px">${discoverRows.map(([n, r]) => `<div class="states"><span class="states__name">${n}</span>${r}</div>`).join("")}</div>`)}

      ${sec(10, "Карточки", "Альбом, артист, микс дня с обложкой из фото артиста и цвета, «Продолжить», анонс без обложки.", `<div class="grid grid--5">
        ${spec("Альбом", AlbumCard(albums.get(1134273871), { isNew: true, sub: "Mytch · 6 нояб." }), { plain: true })}
        ${spec("Альбом · наведение", AlbumCard(albums.get(1138050861), { hover: true, sub: "Rushex · EP" }), { plain: true })}
        ${spec("Анонс без обложки", AlbumCard(albums.get(1052752402)), { plain: true })}
        ${spec("Артист", ArtistCard(artistNamed("Lowborn"), { sub: "Подписка · 54 релиза" }), { plain: true })}
        ${spec("Микс дня", MixCard(MIXES[0]), { plain: true })}
      </div>
      <div class="grid grid--3" style="margin-top:24px">
        ${spec("Продолжить", Resume(RESUME[0]), { plain: true })}
        ${spec("Продолжить · наведение", Resume(RESUME[1], { hover: true }), { plain: true })}
        ${spec("Радио артиста", Resume(RESUME[3]), { plain: true })}
      </div>`)}

      ${sec(11, "Заголовок полки", "", `<div class="spec__box spec__box--col">${ShelfHead("Миксы дня", { hint: "Обновлены сегодня утром", nav: true })}${ShelfHead("Продолжить", { all: false })}</div>`)}

      ${sec(12, "Плеер", "Панель внизу на компьютере и мини-плеер на телефоне. Прогресс — форма волны трека с отметкой будущего перехода.", `
        <div class="spec"><div class="spec__label">Панель «Сейчас играет»</div><div class="spec__box spec__box--plain scroll-x"><footer class="nowbar nowbar--static">${NowBar()}</footer></div></div>
        <div class="grid grid--3" style="margin-top:24px">
          ${spec("Мини-плеер", `<div class="mini mini--static">${MiniInner()}</div>`, { plain: true })}
          ${spec("Форма волны · наведение у отметки перехода", `<div style="width:100%;padding-top:34px">${Waveform(tPlay, { bars: 80, p: 0.43, cls: "is-hover", tip: `Переход в ${dur(transitionAt(tPlay))}${NB}· <span class="tooltip__muted">сведение по долям</span>` })}</div>`, { col: true })}
          ${spec("Громкость · обычная, наведение, без звука", `<div style="display:flex;flex-direction:column;gap:8px">${VolumeSlider({ v: 0.72 })}${VolumeSlider({ v: 0.4, hover: true })}${VolumeSlider({ v: 0 })}</div>`, { col: true })}
        </div>`)}

      ${sec(13, "Всплывающие", "Переходы, устройства, контекстное меню, подсказка, уведомление с отменой.", `<div class="grid grid--3">
        ${spec("Режим переходов", `<div class="pop pop--static" style="max-width:none">${ModePicker()}</div>`, { plain: true })}
        ${spec("Устройства", `<div class="pop pop--static" style="max-width:none">${DevicePicker({ hover: "iphone" })}</div>`, { plain: true })}
        ${spec("Контекстное меню", `<div class="pop pop--static" style="max-width:none">${ContextMenu(T(3661025372), { hover: true })}</div>`, { plain: true })}
      </div>
      <div class="grid grid--2" style="margin-top:24px">
        ${spec("Подсказки", `<span class="tooltip">Нравится ${Kbd("L")}</span><span class="tooltip">Пауза <span class="tooltip__muted">· пробел</span></span><span class="tooltip">Скачано на это устройство</span>`, { style: "padding-top:28px" })}
        ${spec("Уведомление с отменой", `<div style="display:flex;flex-direction:column;gap:10px;align-items:flex-start">${Toast("Убрано из «Ночной фонк»", { undo: true, icon: "trash" })}${Toast("Больше не будем ставить «Тоска»", { undo: true, hover: true, icon: "prohibit" })}${Toast("«Тоска» не скачалась: источник не ответил", { cls: "toast--error", icon: "warning-circle", retry: "Повторить" })}</div>`, { col: true })}
      </div>`)}

      ${sec(14, "Значки и индикаторы", "", `<div class="grid grid--3">
        ${spec("Значки", `${Quality()}${Quality("FLAC")}${Explicit()}<span class="badge badge--new">Новое</span><span class="badge badge--soon">Скоро</span><span class="badge badge--meta">88${NB}BPM</span><span class="state-icon state-icon--down">${icon("arrow-circle-down", { fill: true })}</span><span class="state-icon state-icon--cloud">${icon("cloud-arrow-down")}</span>`)}
        ${spec("Загрузка на устройство", `${Ring(0, { state: "queued", size: 28 })}${Ring(0.36, { size: 28 })}${Ring(0.82, { size: 28 })}${Ring(1, { state: "done", size: 28 })}<span style="font-size:12px;color:var(--text-3)">в очереди · 36${NB}% · 82${NB}% · готово</span>`)}
        ${spec("Играет · клавиши", `${Eq(false)}<span class="live-dot"></span>${Kbd("⌘", "K")}${Kbd("Пробел")}${Kbd("⇧", "→")}${Kbd("L")}`)}
      </div>`)}

      ${sec(15, "Поля и переключатели", "", `<div class="grid grid--3">
        ${spec("Поиск · обычный, фокус, заполнен", `<div style="display:flex;flex-direction:column;gap:12px;width:100%">${Search()}${Search({ focus: true })}${Search({ value: "спокойный инструментал для работы" })}</div>`, { col: true })}
        ${spec("Переключатель", `<div style="display:flex;flex-direction:column;gap:16px">${Switch("Выравнивать громкость", { on: true, hint: "Треки звучат одинаково громко" })}${Switch("Скачивать по сотовой сети")}${Switch("Режим «Диджей» в браузере", { disabled: true, hint: "Доступен в приложении" })}</div>`, { col: true })}
        ${spec("Сегменты в поиске", Segmented([{ id: "library", label: "Медиатека" }, { id: "all", label: "Весь каталог" }], "all"), { col: true })}
      </div>`)}

      ${sec(16, "Загрузка и пустые экраны", "", `<div class="grid grid--2">
        ${spec("Скелетон", `<div style="display:flex;flex-direction:column;gap:14px;width:100%">${[0, 1, 2].map(() => `<div style="display:flex;align-items:center;gap:12px"><div class="skel" style="width:40px;height:40px"></div><div style="flex:1;display:flex;flex-direction:column;gap:8px"><div class="skel skel--line" style="width:46%"></div><div class="skel skel--line" style="width:28%"></div></div><div class="skel skel--line" style="width:32px"></div></div>`).join("")}<div style="display:flex;gap:16px">${[0, 1, 2].map(() => `<div style="flex:1;display:flex;flex-direction:column;gap:10px"><div class="skel" style="aspect-ratio:1;border-radius:12px"></div><div class="skel skel--line" style="width:70%"></div></div>`).join("")}</div></div>`, { col: true })}
        ${spec("Пустой экран", Empty({ icon: "download-simple", title: "Здесь будет скачанное", text: "Скачайте плейлист или альбом, и он будет играть без сети — в самолёте и в метро.", action: "Выбрать, что скачать" }), { col: true })}
      </div>`)}
    </div>`;
  }

  // ---------- Рендер и маршруты ----------

  const root = $("#root");
  let lastLayout = null;

  function routeFromHash() {
    const h = location.hash.replace("#", "");
    return ["home", "player", "wave", "components"].includes(h) ? h : "home";
  }

  // Правая панель: в плеере на компьютере открыта по умолчанию, на главной — закрыта
  const panelByRoute = { home: false, player: true };

  function render() {
    closePop();
    state.route = routeFromHash();
    const doc = state.route === "components";
    document.documentElement.classList.toggle("is-doc", doc);
    document.body.classList.toggle("is-doc", doc);
    document.documentElement.dataset.route = state.route;
    if (state.route in panelByRoute) state.panel = panelByRoute[state.route] && !isPhone();
    if (state.route === "components") {
      eagerImages = true;
      root.innerHTML = ComponentsPage();
      eagerImages = false;
    }
    else if (state.route === "wave") root.innerHTML = WavePage();
    else root.innerHTML = Shell(state.route === "player" ? PlayerPage() : HomePage());
    if (state.route === "components") window.scrollTo(0, 0);
    mountWave();
    bindDynamic();
    sync();
  }

  function mountWave() {
    const phone = isPhone();
    lastLayout = phone;
    if (state.route === "home") window.Wave.mount($("#hero-wave"), { layout: phone ? "hero-phone" : "hero" });
    else if (state.route === "player") window.Wave.mount($("#player-wave"), { layout: phone ? "stage-phone" : "stage", box: $("#player-art .art") });
    else if (state.route === "wave") window.Wave.mount($("#full-wave"), { layout: phone ? "full-phone" : "full" });
  }

  let beatEls = [];
  function bindDynamic() {
    beatEls = $$(".waveplay, .player__art, [data-bind='beat-dot']");
    const main = $("#main");
    if (main) {
      const top = $(".topbar");
      const status = $(".ios-status");
      main.addEventListener("scroll", () => {
        top && top.classList.toggle("is-scrolled", main.scrollTop > 8);
        status && status.classList.toggle("is-scrolled", main.scrollTop > 4);
      }, { passive: true });
    }
    $$("[data-row]").forEach(updateShelfNav);
    $$("[data-row]").forEach((row) => row.addEventListener("scroll", () => updateShelfNav(row), { passive: true }));
  }

  function updateShelfNav(row) {
    const shelf = row.closest(".shelf");
    if (!shelf) return;
    const prev = $("[data-action='shelf-prev']", shelf);
    const next = $("[data-action='shelf-next']", shelf);
    if (prev) prev.disabled = row.scrollLeft < 4;
    if (next) next.disabled = row.scrollLeft + row.clientWidth >= row.scrollWidth - 4;
  }

  // Состояние → разметка там, где не нужен полный перерисовщик
  function sync() {
    document.documentElement.dataset.mood = state.mood;
    $$("[data-bind='summary']").forEach((el) => (el.textContent = summary()));
    $$(".mood[data-action='mood']").forEach((el) => {
      const on = el.dataset.value === state.mood;
      el.classList.toggle("is-selected", on);
      el.setAttribute("aria-checked", on);
    });
    $$(".segmented__item[data-action='mood']").forEach((el) => {
      const on = el.dataset.value === state.mood;
      el.classList.toggle("is-selected", on);
      el.setAttribute("aria-selected", on);
    });
    const moodLabel = MOODS.find((m) => m.id === state.mood).label;
    $$("[data-bind='mood-label']").forEach((el) => (el.textContent = moodLabel));
    for (const g of ["activity", "character", "lang"]) {
      $$(`[data-bind='opt-${g}']`).forEach((el) => (el.textContent = optLabel(g, state[g])));
      $$(`[data-bind='tok-${g}']`).forEach((el) => (el.textContent = tokenLabel(g, state[g])));
    }
    $$("[data-bind='preset']").forEach((el) => el.classList.toggle("is-active", el.dataset.value === state.preset));
    syncPlay();
    syncProgress();
  }

  function syncPlay() {
    $$("[data-bind='waveplay']").forEach((el) => {
      if (el.closest(".sheet")) return;
      el.classList.toggle("is-playing", state.playing);
      el.classList.toggle("is-idle", !state.playing);
      el.setAttribute("aria-label", state.playing ? "Пауза" : "Слушать волну");
      el.querySelector(".waveplay__core").innerHTML = icon(state.playing ? "pause" : "play", { fill: true });
    });
    $$("[data-bind='play']").forEach((el) => {
      if (el.closest(".sheet")) return;
      el.setAttribute("aria-label", state.playing ? "Пауза" : "Слушать");
      el.innerHTML = icon(state.playing ? "pause" : "play", { fill: true });
    });
    $$(".eq").forEach((el) => !el.closest(".sheet") && el.classList.toggle("is-paused", !state.playing));
    const label = $(".hero__now-label");
    if (label) label.textContent = state.playing ? "Сейчас играет" : "На паузе";
  }

  function syncProgress() {
    const t = cur();
    const p = clamp(state.position / t.durationMs, 0, 1);
    $$(`[data-wf='${t.id}']`).forEach((el) => {
      if (el.closest(".sheet")) return;
      el.style.setProperty("--p", p.toFixed(4));
      el.setAttribute("aria-valuenow", Math.round(state.position / 1000));
      el.setAttribute("aria-valuetext", `${dur(state.position)} из ${dur(t.durationMs)}`);
    });
    $$("[data-bind='pos']").forEach((el) => !el.closest(".sheet") && (el.textContent = dur(state.position)));
    $$("[data-bind='remain']").forEach((el) => (el.textContent = `−${dur(t.durationMs - state.position)}`));
    $$("[data-bind='mini-p']").forEach((el) => !el.closest(".sheet") && el.style.setProperty("--p", p.toFixed(4)));
  }

  // Смена трека: перерисовываем только то, что от него зависит
  function refreshTrack() {
    const nb = $("#nowbar");
    if (nb) nb.innerHTML = NowBar();
    const mini = $("#mini");
    if (mini) mini.innerHTML = MiniInner();
    const hn = $("#hero-now");
    if (hn) hn.innerHTML = HeroNow();
    const pb = $("#panel-body");
    if (pb) pb.innerHTML = PanelBody();
    if (state.route === "player") {
      $("#page").innerHTML = PlayerPage();
      mountWave();
    }
    $$(".trow[data-track]").forEach((row) => {
      const on = Number(row.dataset.track) === state.current && !row.closest(".sheet");
      row.classList.toggle("is-playing", on);
    });
    beatEls = $$(".waveplay, .player__art, [data-bind='beat-dot']");
    applyTrack();
    sync();
  }

  // Цвет обложки → фон и палитра волны
  const colorCache = new Map();
  function coverColors(url) {
    if (colorCache.has(url)) return colorCache.get(url);
    const p = new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        try {
          const c = document.createElement("canvas");
          c.width = c.height = 28;
          const x = c.getContext("2d", { willReadFrequently: true });
          x.drawImage(img, 0, 0, 28, 28);
          const d = x.getImageData(0, 0, 28, 28).data;
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
            const k = Math.floor((h / 6) * 12) % 12;
            Object.assign(buckets[k], { w: buckets[k].w + w, r: buckets[k].r + r * w, g: buckets[k].g + g * w, b: buckets[k].b + b * w });
            if (mx < 0.55) { dr += r; dg += g; db += b; dn++; }
          }
          const sorted = buckets.filter((b) => b.w > 0).sort((a, b) => b.w - a.w);
          const toHex = (r, g, b) => "#" + [r, g, b].map((v) => Math.round(clamp(v, 0, 1) * 255).toString(16).padStart(2, "0")).join("");
          const pick = (b) => toHex(b.r / b.w, b.g / b.w, b.b / b.w);
          const vivid = sorted[0] ? pick(sorted[0]) : "#3a5070";
          const second = sorted[1] ? pick(sorted[1]) : vivid;
          const deep = dn ? toHex(dr / dn, dg / dn, db / dn) : "#101820";
          resolve({ vivid, second, deep });
        } catch (e) {
          resolve(null);
        }
      };
      img.onerror = () => resolve(null);
      img.src = url;
    });
    colorCache.set(url, p);
    return p;
  }

  let applyToken = 0;
  async function applyTrack(o = {}) {
    const t = cur();
    const m = meta(t);
    const bpm = m.bpm > 140 ? m.bpm / 2 : m.bpm;
    window.Wave.setBpm(bpm);
    window.Wave.setTrackEnergy(clamp(m.energy * (state.mood === "energetic" ? 1.35 : state.mood === "happy" ? 1.2 : state.mood === "sad" ? 0.8 : 1), 0.15, 0.95));
    const token = ++applyToken;
    const c = await coverColors(src(t.cover));
    if (!c || token !== applyToken) return;
    const rs = document.documentElement.style;
    rs.setProperty("--amb-1", c.vivid);
    rs.setProperty("--amb-2", c.second);
    rs.setProperty("--amb-3", c.deep);
    window.Wave.setCover([c.vivid, c.deep], { instant: o.instant });
  }

  // ---------- Воспроизведение (имитация: звук — на экране волны) ----------

  let lastTick = performance.now();
  setInterval(() => {
    const now = performance.now();
    const dt = now - lastTick;
    lastTick = now;
    if (!state.playing || state.route === "wave" || state.route === "components") return;
    state.position += dt;
    if (state.position >= cur().durationMs) next({ auto: true });
    else syncProgress();
  }, 250);

  function refill() {
    while (state.wave.length < 6) {
      const pool = WAVE_POOL.filter((id) => id !== state.current && !state.wave.includes(id) && !state.manual.includes(id));
      state.wave.push(pool[Math.floor(Math.random() * pool.length)] || WAVE_POOL[0]);
    }
  }

  function next(o = {}) {
    state.history.push(state.current);
    state.current = state.manual.shift() || state.wave.shift();
    state.position = 0;
    refill();
    if (!o.auto) window.Wave.skip();
    refreshTrack();
  }
  function prev() {
    if (state.position > 4000 || !state.history.length) {
      state.position = 0;
      syncProgress();
      return;
    }
    state.manual.unshift(state.current);
    state.current = state.history.pop();
    state.position = 0;
    window.Wave.skip();
    refreshTrack();
  }

  function setPlaying(on, o = {}) {
    if (on === state.playing) return;
    state.playing = on;
    if (on && o.start) window.Wave.start();
    else window.Wave.setPlaying(on);
    syncPlay();
  }

  // ---------- Всплывающие слои ----------

  let pop = null;
  function closePop() {
    if (!pop) return;
    pop.anchor?.setAttribute("aria-expanded", "false");
    pop.el.remove();
    pop = null;
  }
  function openPop(anchor, html, o = {}) {
    const same = pop && pop.anchor === anchor && !o.at;
    closePop();
    if (same) return;
    const el = document.createElement("div");
    el.className = "pop";
    el.setAttribute("role", o.role || "menu");
    el.innerHTML = html;
    $("#layer").append(el);
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    let x, y;
    if (o.at) {
      x = o.at.x;
      y = o.at.y;
    } else {
      const r = anchor.getBoundingClientRect();
      x = o.align === "end" ? r.right - w : r.left;
      y = r.bottom + 8;
      if (y + h > innerHeight - 12) y = r.top - h - 8;
    }
    x = clamp(x, 12, innerWidth - w - 12);
    y = clamp(y, 12, innerHeight - h - 12);
    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
    el.style.setProperty("--origin", y < (anchor?.getBoundingClientRect?.().top ?? 0) ? "bottom left" : "top left");
    anchor?.setAttribute?.("aria-expanded", "true");
    pop = { el, anchor, kind: o.kind };
    // Фокус — на текущем значении, чтобы клавиатура начинала с выбранного
    (el.querySelector(".is-selected, .is-current, [aria-checked='true']") || el.querySelector("button:not([disabled]), input"))?.focus({ preventScroll: true });
  }

  let toastTimer = 0;
  let undoFn = null;
  function toast(text, o = {}) {
    const box = $("#toasts");
    box.innerHTML = Toast(text, o);
    undoFn = o.undo || null;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      const el = box.firstElementChild;
      if (!el) return;
      el.classList.add("is-leaving");
      setTimeout(() => el.remove(), 220);
      undoFn = null;
    }, o.undo ? 5000 : 2600);
  }

  // Подсказки по наведению
  let tipTimer = 0;
  let tipEl = null;
  function hideTip() {
    clearTimeout(tipTimer);
    tipEl?.remove();
    tipEl = null;
  }
  function showTip(target, html, at) {
    hideTip();
    tipEl = document.createElement("div");
    tipEl.className = "tip-layer";
    tipEl.innerHTML = `<span class="tooltip">${html}</span>`;
    document.body.append(tipEl);
    const r = target.getBoundingClientRect();
    const w = tipEl.offsetWidth;
    const h = tipEl.offsetHeight;
    const cx = at ? at.x : r.left + r.width / 2;
    let y = r.top - h - 10;
    if (y < 8) {
      y = r.bottom + 10;
      tipEl.firstElementChild.classList.add("tooltip--below");
    }
    tipEl.style.left = `${clamp(cx - w / 2, 8, innerWidth - w - 8)}px`;
    tipEl.style.top = `${y}px`;
  }

  document.addEventListener("pointerover", (e) => {
    if (e.pointerType === "touch") return;
    const el = e.target.closest("[data-tip]");
    if (!el || !el.dataset.tip || el.closest(".sheet") || el.closest(".pop")) return;
    clearTimeout(tipTimer);
    tipTimer = setTimeout(() => showTip(el, esc(el.dataset.tip)), 450);
  });
  document.addEventListener("pointerout", (e) => {
    const el = e.target.closest("[data-tip]");
    if (el && !el.contains(e.relatedTarget)) hideTip();
  });

  // Форма волны: время под курсором и отметка перехода
  document.addEventListener("pointermove", (e) => {
    const wf = e.target.closest("[data-wf]");
    if (!wf || wf.closest(".sheet") || e.pointerType === "touch") return;
    const t = T(Number(wf.dataset.wf));
    const r = wf.getBoundingClientRect();
    const f = clamp((e.clientX - r.left) / r.width, 0, 1);
    const x = transitionAt(t) / t.durationMs;
    const nearMarker = state.mode !== "gapless" && Math.abs(f - x) * r.width < 10;
    const html = nearMarker ? `Переход в ${dur(transitionAt(t))}${NB}· <span class="tooltip__muted">${modeOf().label.toLowerCase()}, 8 тактов</span>` : `<span class="tnum">${dur(f * t.durationMs)}</span>`;
    showTip(wf, html, { x: nearMarker ? r.left + x * r.width : e.clientX });
  });
  document.addEventListener("pointerdown", (e) => {
    const wf = e.target.closest("[data-wf]");
    if (wf && !wf.closest(".sheet") && Number(wf.dataset.wf) === state.current) {
      const r = wf.getBoundingClientRect();
      state.position = clamp((e.clientX - r.left) / r.width, 0, 1) * cur().durationMs;
      syncProgress();
    }
  });

  // ---------- Действия ----------

  const actions = {
    "wave-toggle": () => setPlaying(!state.playing, { start: true }),
    toggle: () => setPlaying(!state.playing),
    next: () => next(),
    prev: () => prev(),
    "not-now": () => {
      next();
      toast("Пропустили без штрафа — трек ещё вернётся", { icon: "arrow-bend-up-right" });
    },
    dislike: (el) => {
      const id = Number(el.dataset.track) || state.current;
      const t = T(id);
      closePop();
      if (id === state.current) next();
      toast(`Больше не будем ставить «${esc(t.title)}»`, { icon: "prohibit", undo: () => toast("Вернули в рекомендации", { icon: "arrow-counter-clockwise" }) });
    },
    "more-like": () => {
      window.Wave.like();
      toast(`Больше похожего на «${esc(cur().title)}»`, { icon: "sparkle" });
    },
    like: (el) => {
      const id = Number(el.dataset.track) || state.current;
      const was = state.liked.has(id);
      if (was) state.liked.delete(id);
      else state.liked.add(id);
      if (!was && id === state.current) window.Wave.like();
      closePop();
      $$(`[data-action='like'][data-track='${id}']`).forEach((b) => {
        if (b.closest(".sheet")) return;
        b.classList.toggle("is-liked", !was);
        b.setAttribute("aria-pressed", !was);
        b.setAttribute("aria-label", !was ? "Убрать из любимых" : "Нравится");
        b.innerHTML = icon("heart", { fill: !was });
      });
      $$("[data-bind='liked-count']").forEach((n) => (n.textContent = num(408 + state.liked.size)));
      const t = T(id);
      if (was) toast(`«${esc(t.title)}» убран из любимых`, { icon: "heart-break", undo: () => actions.like(el) });
      else toast(`«${esc(t.title)}» — в любимых`, { icon: "heart" });
    },
    mood: (el) => {
      if (state.mood === el.dataset.value) return;
      state.mood = el.dataset.value;
      state.preset = null;
      window.Wave.setMood(state.mood);
      applyTrack();
      sync();
      const wb = $("[data-bind='mood-label']");
      if (wb) wb.textContent = MOODS.find((m) => m.id === state.mood).label;
    },
    tuner: (el) => openPop(el, TunerMenu(el.dataset.group), { role: "listbox" }),
    "set-opt": (el) => {
      state[el.dataset.group] = el.dataset.value;
      state.preset = null;
      if (el.dataset.group === "mood") window.Wave.setMood(state.mood);
      closePop();
      sync();
      toast("Волна подстроится со следующего трека", { icon: "waveform" });
    },
    preset: (el) => {
      const p = PRESETS.find((x) => x.id === el.dataset.value);
      Object.assign(state, p.set);
      state.preset = p.id;
      window.Wave.setMood(state.mood);
      applyTrack();
      if (!state.playing) setPlaying(true, { start: true });
      else window.Wave.skip();
      sync();
      toast(`Волна «${p.title}»: ${p.meta}`, { icon: "waveform" });
    },
    "save-preset": () => toast("Сочетание сохранено", { icon: "bookmark-simple" }),
    modes: (el) => openPop(el, ModePicker(), { role: "dialog", align: "end" }),
    mode: (el) => {
      state.mode = el.dataset.value;
      closePop();
      refreshTrack();
      toast(`Переходы: «${modeOf().label}»`, { icon: modeOf().icon });
    },
    devices: (el) => openPop(el, DevicePicker(), { role: "dialog", align: "end" }),
    device: (el) => {
      state.device = el.dataset.value;
      closePop();
      toast(`Играет на «${DEVICES.find((d) => d.id === state.device).name}»`, { icon: "devices" });
    },
    menu: (el) => openPop(el, ContextMenu(T(Number(el.dataset.track))), { align: "end" }),
    "play-next": (el) => {
      const id = Number(el.dataset.track);
      state.manual = [id, ...state.manual.filter((x) => x !== id)];
      closePop();
      const pb = $("#panel-body");
      if (pb) pb.innerHTML = PanelBody();
      toast(`«${esc(T(id).title)}» сыграет следующим`, { icon: "arrow-bend-down-right", undo: () => { state.manual = state.manual.filter((x) => x !== id); if ($("#panel-body")) $("#panel-body").innerHTML = PanelBody(); } });
    },
    enqueue: (el) => {
      const id = Number(el.dataset.track);
      if (!state.manual.includes(id)) state.manual.push(id);
      closePop();
      const pb = $("#panel-body");
      if (pb) pb.innerHTML = PanelBody();
      toast(`«${esc(T(id).title)}» — в очереди`, { icon: "list-plus" });
    },
    "track-wave": (el) => {
      closePop();
      toast(`Волна по треку «${esc(T(Number(el.dataset.track)).title)}»`, { icon: "waveform" });
    },
    download: (el) => {
      closePop();
      toast(`Скачиваем «${esc(T(Number(el.dataset.track)).title)}» на это устройство`, { icon: "download-simple" });
    },
    "row-play": (el, e) => {
      if (e.target.closest("button")) return;
      const id = Number(el.dataset.track);
      if (id === state.current) return setPlaying(!state.playing);
      state.history.push(state.current);
      state.current = id;
      state.manual = state.manual.filter((x) => x !== id);
      state.wave = state.wave.filter((x) => x !== id);
      refill();
      state.position = 0;
      window.Wave.skip();
      if (!state.playing) setPlaying(true);
      refreshTrack();
    },
    panel: (el) => {
      const tab = el.dataset.value;
      if (isPhone()) return toast(tab === "queue" ? "Очередь откроется шторкой снизу" : "Текст откроется шторкой снизу", { icon: "queue" });
      if (state.panel && state.panelTab === tab) state.panel = false;
      else Object.assign(state, { panel: true, panelTab: tab });
      panelByRoute[state.route] = state.panel;
      $("#app").classList.toggle("is-panel", state.panel);
      $(".panel").outerHTML = Panel();
      $("#nowbar").innerHTML = NowBar();
      syncProgress();
    },
    "panel-tab": (el) => {
      state.panelTab = el.dataset.value;
      $(".panel").outerHTML = Panel();
      $("#nowbar").innerHTML = NowBar();
      syncProgress();
    },
    "panel-close": () => {
      state.panel = false;
      panelByRoute[state.route] = false;
      $("#app").classList.remove("is-panel");
      $("#nowbar").innerHTML = NowBar();
      syncProgress();
    },
    "clear-manual": () => {
      const was = state.manual;
      state.manual = [];
      $("#panel-body").innerHTML = PanelBody();
      toast("Очередь вручную очищена", { icon: "trash", undo: () => { state.manual = was; if ($("#panel-body")) $("#panel-body").innerHTML = PanelBody(); } });
    },
    "save-queue": () => toast("Очередь сохранена как плейлист «Волна, 11 ноября»", { icon: "playlist" }),
    undo: () => {
      const fn = undoFn;
      undoFn = null;
      $("#toasts").innerHTML = "";
      fn && fn();
    },
    "shelf-prev": (el) => scrollShelf(el, -1),
    "shelf-next": (el) => scrollShelf(el, 1),
    "focus-search": (el, e) => {
      e.preventDefault();
      $(".topbar [data-search]")?.focus();
    },
    "play-album": (el, e) => { e.preventDefault(); toast("Альбом в очереди после текущего трека", { icon: "list-plus" }); },
    "play-mix": (el, e) => { e.preventDefault(); toast("Микс дня начнётся после текущего трека", { icon: "list-plus" }); },
    "play-discover": () => toast("«Открытия недели» — следующими в очереди", { icon: "list-plus" }),
    mute: () => {
      state.muted = !state.muted;
      if (!state.muted && state.volume === 0) state.volume = 0.5;
      $$(".nowbar .vol").forEach((v) => (v.outerHTML = VolumeSlider()));
    },
    // Экран волны
    "w-start": () => window.Wave.start(),
    "w-skip": () => window.Wave.skip(),
    "w-like": () => window.Wave.like(),
    pv: async (el) => {
      const id = Number(el.dataset.value);
      const p = D.previews.find((x) => x.trackId === id);
      const a = window.Wave.audio;
      if (a.playing && a.url === src(p.file)) return window.Wave.pauseAudio();
      const an = ANALYSIS[id];
      await window.Wave.playAudio(src(p.file), { bpm: an.bpm > 140 ? an.bpm / 2 : an.bpm, phase: an.phase });
      const c = await coverColors(src(T(id).cover));
      if (c) window.Wave.setCover([c.vivid, c.deep]);
      window.Wave.skip();
    },
    noop: () => {},
  };

  function scrollShelf(el, dir) {
    const row = $("[data-row]", el.closest(".shelf"));
    row.scrollBy({ left: dir * row.clientWidth * 0.8, behavior: reduced() ? "auto" : "smooth" });
  }

  document.addEventListener("click", (e) => {
    const el = e.target.closest("[data-action]");
    if (pop && !e.target.closest(".pop") && (!el || el !== pop.anchor)) closePop();
    if (!el) return;
    if (el.closest(".sheet") && !["mood"].includes(el.dataset.action)) {
      if (el.tagName === "A") e.preventDefault();
      return;
    }
    const fn = actions[el.dataset.action];
    if (fn && el.tagName !== "INPUT") fn(el, e);
  });

  document.addEventListener("input", (e) => {
    const el = e.target;
    if (el.classList.contains("range")) el.style.setProperty("--v", (el.value - el.min) / (el.max - el.min));
    const a = el.dataset.action;
    if (a === "volume") {
      state.volume = Number(el.value);
      state.muted = state.volume === 0;
      const ic = $(".icon-btn", el.closest(".vol"));
      if (ic) ic.innerHTML = icon(state.volume === 0 ? "speaker-x" : state.volume < 0.45 ? "speaker-low" : "speaker-high");
      if (window.Wave.audio.playing) window.Wave.state.volume = state.volume;
    } else if (a === "expressive") state.expressive = Number(el.value);
    else if (a === "w-energy") window.Wave.setEnergy(Number(el.value));
    else if (a === "w-bpm") {
      window.Wave.setBpm(Number(el.value));
      const b = $("[data-bind='bpm']");
      if (b) b.textContent = `${el.value}${NB}BPM`;
    }
    if (el.matches("[data-search]")) el.closest(".search").classList.toggle("is-filled", !!el.value);
  });

  document.addEventListener("contextmenu", (e) => {
    const row = e.target.closest(".trow[data-track]");
    if (!row || row.closest(".sheet")) return;
    e.preventDefault();
    openPop(null, ContextMenu(T(Number(row.dataset.track))), { at: { x: e.clientX, y: e.clientY } });
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      closePop();
      hideTip();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      $(".topbar [data-search]")?.focus();
      return;
    }
    if (e.target.closest("input, textarea, [contenteditable]")) return;
    if (state.route !== "home" && state.route !== "player") return;
    if (e.code === "Space") {
      e.preventDefault();
      setPlaying(!state.playing);
    } else if (e.key === "ArrowRight" && e.shiftKey) next();
    else if (e.key === "ArrowLeft" && e.shiftKey) prev();
    else if (e.key.toLowerCase() === "l" && !e.metaKey && !e.ctrlKey) actions.like({ dataset: {} });
  });

  window.addEventListener("hashchange", render);
  window.addEventListener("resize", () => {
    closePop();
    hideTip();
    if (isPhone() !== lastLayout) mountWave();
    else window.Wave.remeasure();
  });

  // Ритм волны → кнопки и обложка; на экране волны — показания отладки
  window.Wave.onFrame((s) => {
    const b = s.beat.toFixed(3);
    for (const el of beatEls) el.style.setProperty("--beat", b);
    if (state.route !== "wave") return;
    const fps = $("[data-bind='fps']");
    if (fps) fps.textContent = `${Math.round(s.fps)} кадров/с`;
    const en = $("[data-bind='energy']");
    if (en) en.textContent = s.energy.toFixed(2).replace(".", ",");
    const er = $("[data-action='w-energy']");
    if (er && document.activeElement !== er && s.source === "audio") {
      er.value = s.energy.toFixed(2);
      er.style.setProperty("--v", s.energy);
    }
    s.bands.forEach((v, i) => $(`[data-bind='m${i}']`)?.style.setProperty("--v", clamp(v * 1.6, 0, 1)));
    const so = $("[data-bind='source']");
    const a = window.Wave.audio;
    if (so) so.textContent = s.source === "audio" ? `звук превью, ${Math.round(s.bpm)}${NB}BPM` : s.source === "clock" ? `часы, ${Math.round(s.bpm)}${NB}BPM` : "без музыки — дыхание";
    const bp = $("[data-bind='bpm']");
    if (bp && s.source === "audio") bp.textContent = `${Math.round(s.bpm)}${NB}BPM`;
    $$(".pv").forEach((el) => {
      const p = D.previews.find((x) => x.trackId === Number(el.dataset.value));
      const on = a.playing && a.url === src(p.file);
      el.classList.toggle("is-playing", on);
      el.querySelector(".pv__btn").innerHTML = icon(on ? "pause" : "play", { fill: true, cls: "i-16" });
      if (on) el.style.setProperty("--p", (a.time / a.duration).toFixed(3));
    });
  });

  window.addEventListener("wave:audio", () => {
    if (state.route !== "wave") return;
    // Когда превью закончилось, волна возвращается к часам
    if (!window.Wave.audio.playing) window.Wave.setPlaying(true);
  });

  // ---------- Старт ----------

  document.documentElement.dataset.mood = state.mood;
  window.Wave.setMood(state.mood, { instant: true });
  window.Wave.setPlaying(state.playing);
  applyTrack({ instant: true });
  render();
})();
