/* DalyWave — направление B «Импульс». Живой прототип интерфейса.
   Данные — window.DW_DATA из fixtures, маршруты — #home, #player, #wave, #components.
   Экраны и лист компонентов собираются одними и теми же функциями: так видно, что система одна. */
(() => {
  'use strict';

  const D = window.DW_DATA;
  const FIX = '../../fixtures/';
  const root = document.getElementById('app');
  const layer = document.getElementById('layer');

  // ——— Утилиты ———
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
  const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const fmt = (sec) => {
    const s = Math.max(0, Math.floor(sec));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  };
  const num = (n) => n.toLocaleString('ru-RU');
  const plural = (n, one, few, many) => {
    const a = n % 10, b = n % 100;
    if (a === 1 && b !== 11) return one;
    if (a >= 2 && a <= 4 && (b < 12 || b > 14)) return few;
    return many;
  };
  const MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];
  const fmtDate = (iso) => {
    const [, m, d] = iso.split('-').map(Number);
    return `${d} ${MONTHS[m - 1]}`;
  };
  const TYPE = { album: 'Альбом', ep: 'EP', single: 'Сингл', compile: 'Сборник' };
  const TODAY = new Date('2026-11-11T19:40:00');
  const daysAgo = (iso) => Math.round((TODAY - new Date(iso + 'T12:00:00')) / 86400000);

  function rng(seed) {
    let s = seed >>> 0;
    return () => {
      s = (s + 0x6d2b79f5) >>> 0;
      let t = s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  const icon = (name, o = {}) => `<i class="${o.fill ? 'ph-fill' : o.bold ? 'ph-bold' : 'ph'} ph-${name}${o.cls ? ' ' + o.cls : ''}" aria-hidden="true"></i>`;

  // Неразрывный пробел после коротких слов: предлог не висит в конце строки
  const SHORT = /(^|[\s(«—])(в|во|с|со|к|ко|о|об|у|и|а|но|на|не|по|за|из|от|до|для|без|при|про|под|над|что|как|—)\s/gi;
  const nb = (s) => String(s).replace(SHORT, '$1$2 ').replace(SHORT, '$1$2 ').replace(/ —/g, ' —');

  // Аватар — «сфера вкуса» из цветов граней (миксов дня): буква «З» в любом шрифте читается как цифра 3
  const avatar = (size = '') => `<button class="avatar${size ? ' avatar--' + size : ''}" aria-label="Профиль Захара и «Мой вкус»" title="Захар · Мой вкус"><span class="avatar__orb" aria-hidden="true"></span></button>`;

  // ——— Данные ———
  const artists = new Map(D.artists.map((a) => [a.id, a]));
  const albums = new Map(D.albums.map((a) => [a.id, a]));
  const tracks = new Map(D.tracks.map((t) => [t.id, t]));
  const artistByName = (n) => D.artists.find((a) => a.name === n);
  const trackBy = (title, artist) => D.tracks.find((t) => t.title === title && (!artist || t.artist === artist));
  const PREVIEW = new Map(D.previews.map((p) => [p.trackId, FIX + p.file]));
  const imgSrc = (p) => (p ? FIX + p : '');

  const T = {
    now: tracks.get(3580793301), // 3days — Mytch
    iwantout: tracks.get(3661025372),
    fall: trackBy('свободное падение', 'Mzlff'),
    noise: tracks.get(4276724202),
    gray: trackBy('Gray', 'Parker The Bandit'),
    toska: tracks.get(836932782),
    float: tracks.get(909494702),
    holy: tracks.get(2446965695),
    voice: tracks.get(2411993425),
  };

  // Почему трек в волне — строка под названием
  const REASON = new Map([
    [T.now.id, 'Похоже на Lowborn'],
    [T.iwantout.id, 'Ты добавил вручную'],
    [T.fall.id, 'Ты добавил вручную'],
    [T.noise.id, 'Новое для тебя'],
    [T.gray.id, 'Ты лайкнул 6 треков артиста'],
    [T.toska.id, 'Под спокойное настроение'],
    [T.float.id, 'Похоже на Mytch'],
    [T.holy.id, 'Похоже на Lowborn'],
    [T.voice.id, 'Новое для тебя'],
  ]);

  // В узкой очереди — короткая форма, чтобы причина не обрывалась на полуслове
  const REASON_SHORT = new Map([
    [T.gray.id, 'Любимый артист'],
    [T.toska.id, 'Под настроение'],
  ]);
  // Добавленное вручную объясняется честно: «Ты добавил вручную», а не причиной волны
  const MANUAL = new Set([T.iwantout.id, T.fall.id]);
  const reasonOf = (id) => (MANUAL.has(id) ? 'Ты добавил вручную' : REASON.get(id) || 'Из волны');
  const reasonShort = (id) => (MANUAL.has(id) ? 'Ты добавил вручную' : REASON_SHORT.get(id) || reasonOf(id));

  // Темп и тональность (круг Камелота). Волна подбирает соседей совместимыми — так и в данных
  const META = new Map([
    [T.now.id, { bpm: 92, key: '8A' }],
    [T.iwantout.id, { bpm: 96, key: '9A' }],
    [T.fall.id, { bpm: 88, key: '8A' }],
    [T.noise.id, { bpm: 94, key: '9A' }],
    [T.gray.id, { bpm: 90, key: '10A' }],
    [T.toska.id, { bpm: 86, key: '10B' }],
    [T.float.id, { bpm: 92, key: '11B' }],
    [T.holy.id, { bpm: 96, key: '11A' }],
    [T.voice.id, { bpm: 98, key: '12A' }],
  ]);
  const metaOf = (t) => META.get(t.id) || { bpm: 84 + (t.id % 56), key: `${(t.id % 12) + 1}${t.id % 2 ? 'A' : 'B'}` };
  // Совместимы: та же цифра (A↔B) или соседняя цифра с той же буквой
  const camelotOk = (a, b) => {
    const na = parseInt(a, 10), nb2 = parseInt(b, 10), la = a.slice(-1), lb = b.slice(-1);
    const d = Math.min(Math.abs(na - nb2), 12 - Math.abs(na - nb2));
    return (d === 0) || (d === 1 && la === lb);
  };

  const WAVE_OPTS = {
    activity: { label: 'Занятие', options: [['wake', 'Просыпаюсь'], ['road', 'В дороге'], ['work', 'Работаю'], ['train', 'Тренируюсь'], ['rest', 'Отдыхаю'], ['sleep', 'Засыпаю'], ['party', 'Вечеринка']] },
    character: { label: 'Характер', options: [['fav', 'Любимое'], ['mixed', 'Знакомое и новое'], ['new', 'Незнакомое'], ['popular', 'Популярное'], ['rare', 'Редкое']] },
    mood: { label: 'Настроение', options: [['energetic', 'Бодрое'], ['happy', 'Весёлое'], ['calm', 'Спокойное'], ['sad', 'Грустное']] },
    lang: { label: 'Язык', options: [['any', 'Любой'], ['ru', 'Русский'], ['foreign', 'Иностранный'], ['none', 'Без слов']] },
  };
  const MOOD_KEYS = ['energetic', 'happy', 'calm', 'sad'];
  const optLabel = (key, val) => WAVE_OPTS[key].options.find((o) => o[0] === val)?.[1] ?? '';

  const PRESETS = [
    { name: 'Работа', text: 'спокойное, без слов', wave: { activity: 'work', character: 'mixed', mood: 'calm', lang: 'none' } },
    { name: 'Спорт', text: 'бодрое, любимое', wave: { activity: 'train', character: 'fav', mood: 'energetic', lang: 'any' } },
    { name: 'Ночь', text: 'грустное, новое', wave: { activity: 'sleep', character: 'new', mood: 'sad', lang: 'foreign' } },
  ];

  const PLAYLISTS = [
    { id: 'drift', name: 'Ночной дрифт', count: 48, pinned: true, covers: [938784091, 1138050861, 1110576112, 932378661] },
    { id: 'work', name: 'Тихо, работаю', count: 112, pinned: true, covers: [1134273871, 573797331, 858273902, 827674281] },
    { id: 'hyper', name: 'Гиперпоп-терапия', count: 64, pinned: true, covers: [839409852, 957193101, 805631951, 727274781] },
    { id: 'road', name: 'Дорога домой', count: 37, pinned: false, covers: [678933591, 1031080042, 418402827, 908412752] },
  ];
  const coverOfAlbum = (id) => albums.get(id)?.cover || D.tracks.find((t) => t.albumId === id)?.cover;

  const FOLLOWED = ['Mytch', 'Rushex', 'Lowborn', 'Mzlff', '5opka', 'Parker The Bandit', 'Пошлая Молли'].map(artistByName);
  const HAS_NEW = new Set(['Mytch', 'Rushex']);

  const MIXES = [
    { n: 1, names: ['Mzlff', 'Пошлая Молли', 'SQWOZ BAB'], color: '#ff3d7f', deep: '#3b0a1f' },
    { n: 2, names: ['Rushex', 'Sayfalse', 'DJ FKU'], color: '#ff6a2a', deep: '#3a1405' },
    { n: 3, names: ['Lowborn', 'Bohnes', 'Adam Jensen'], color: '#4d7cff', deep: '#0b1846' },
    { n: 4, names: ['Mytch', 'Parker The Bandit', 'Poe the Passenger'], color: '#22c9b4', deep: '#06302b' },
    { n: 5, names: ['5opka', 'кис-кис', 'Электрофорез'], color: '#a46bff', deep: '#22104a' },
  ];

  const NEW_RELEASES = [1134273871, 1138050861, 1135471731, 1134970871, 1070416932, 1125801282, 1110576112, 1103896852]
    .map((id) => albums.get(id))
    .sort((a, b) => b.releaseDate.localeCompare(a.releaseDate));
  const DL_PROGRESS = new Map([[1138050861, 0.5]]);

  const DISCOVERIES = [
    [2124328597, 'Похоже на Mytch', 'server'],
    [2118942927, 'Похоже на Lowborn', 'downloaded'],
    [3321382751, 'Похоже на Parker The Bandit', 'server'],
    [1698366377, 'Похоже на Bohnes', 'missing'],
    [3652133052, 'Похоже на Rushex', 'server'],
    [2978170491, 'Похоже на Rushex', 'downloading'],
    [3510764021, 'Новое для тебя', 'server'],
    [4378564111, 'Похоже на 5opka', 'missing'],
  ].map(([id, reason, st]) => ({ t: tracks.get(id), reason, st }));
  DISCOVERIES.forEach((d) => { if (!REASON.has(d.t.id)) REASON.set(d.t.id, d.reason); });

  const RESUME = [
    { kind: 'album', title: 'Leverage', sub: 'Альбом · Mytch', meta: 'Трек 3 из 7', p: 0.38, cover: imgSrc(coverOfAlbum(1134273871)) },
    { kind: 'playlist', title: 'Ночной дрифт', sub: 'Плейлист · 48 треков', meta: 'Осталось 1 ч 6 мин', p: 0.55, mosaic: PLAYLISTS[0].covers },
    { kind: 'radio', title: 'Радио Lowborn', sub: 'Радио · похожее на Lowborn', meta: 'Сыграно 14 треков', p: null, artist: artistByName('Lowborn') },
    { kind: 'album', title: 'PHANTASMAGORIA', sub: 'Альбом · Mzlff', meta: 'Трек 5 из 8', p: 0.6, cover: imgSrc(coverOfAlbum(827674281)) },
  ];

  const DEVICES = [
    { id: 'mac', name: 'Этот Mac', sub: 'MacBook Pro', icon: 'laptop' },
    { id: 'iphone', name: 'iPhone Захара', sub: 'Пульт · в сети', icon: 'device-mobile' },
    { id: 'pc', name: 'ПК дома', sub: 'Windows · в сети', icon: 'desktop' },
    { id: 'web', name: 'Сайт в Chrome', sub: 'wave.daly.rest · 2 ч назад', icon: 'browser' },
  ];

  const TMODES = [
    { id: 'gapless', name: 'Без пауз', desc: 'Треки стыкуются вплотную' },
    { id: 'smooth', name: 'Плавный', desc: 'Наложение на 6 секунд' },
    { id: 'smart', name: 'Умный', desc: 'Сам выбирает место, голос не накладывается на голос' },
    { id: 'dj', name: 'Диджей', desc: 'Сведение по долям и фразам, как в сете' },
  ];

  // ——— Состояние ———
  const state = {
    route: 'home',
    playing: false,
    trackId: T.now.id,
    pos: 72,
    liked: new Set([T.iwantout.id, T.gray.id, 2118942927]),
    wave: { activity: 'rest', character: 'mixed', mood: 'calm', lang: 'any' },
    volume: 0.72,
    transition: 'dj',
    expr: 0.6,
    device: 'mac',
    queueOpen: true,
    manual: [T.iwantout.id, T.fall.id],
    fromWave: [T.noise.id, T.gray.id, T.toska.id, T.float.id, T.holy.id, T.voice.id],
    history: [],
    src: 'wave',
  };
  const cur = () => tracks.get(state.trackId);

  // ——— Небольшие части ———
  // Знак DalyWave (design/brand/glyph.svg): вставлен как есть, цвет — currentColor
  const GLYPH = '<svg class="glyph" viewBox="0 0 100 100" aria-hidden="true"><path d="M16 31C18.83 37.33 27.33 67.33 33 69C38.67 70.67 44.33 41.00 50 41C55.67 41.00 61.33 70.67 67 69C72.67 67.33 81.17 37.33 84 31" fill="none" stroke="currentColor" stroke-width="11.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  const BRAND = '../../brand/';
  const appIcon = (size, cls = '') => `<img class="app-icon ${cls}" src="${BRAND}icon-rounded.svg" width="${size}" height="${size}" alt="Значок DalyWave">`;
  const logo = () => `<a class="logo" href="#home" aria-label="DalyWave — на главную">${GLYPH}<span class="logo__word">DalyWave</span></a>`;

  const cover = (src, cls = '') => (src
    ? `<img class="${cls}" src="${esc(src)}" alt="" draggable="false">`
    : `<span class="cover-fallback ${cls}">${icon('music-notes-simple')}</span>`);

  const mosaic = (ids) => `<span class="mosaic">${ids.map((id) => cover(imgSrc(coverOfAlbum(id)))).join('')}</span>`;

  const eq = (paused) => `<span class="eq${paused ? ' is-paused' : ''}" aria-label="Играет"><i></i><i></i><i></i></span>`;

  const moodVars = (k) => `--a:var(--mood-${k}-a);--b:var(--mood-${k}-b)`;
  const moodDot = (k) => `<span class="mood-dot" style="${moodVars(k)}" aria-hidden="true"></span>`;

  function badge(kind, text) {
    switch (kind) {
      case 'quality': return `<span class="badge badge--quality" title="Формат и битрейт файла на сервере">${icon('waveform')}${text || 'Opus 160'}</span>`;
      case 'explicit': return `<span class="badge badge--explicit" title="Есть ненормативная лексика" aria-label="Explicit">E</span>`;
      case 'dl': return `<span class="badge badge--dl">${icon('arrow-circle-down', { fill: true })}${text || 'Скачано'}</span>`;
      case 'new': return `<span class="badge badge--new">${text || 'Новое'}</span>`;
      case 'server': return `<span class="badge badge--server">${icon('cloud-arrow-down')}${text || 'Скачается при запуске'}</span>`;
      default: return '';
    }
  }

  function ring(p, size = 20, label) {
    const r = (size - 3.5) / 2;
    const c = 2 * Math.PI * r;
    const done = p >= 1;
    return `<span class="dlring${done ? ' is-done' : ''}" style="--size:${size}px" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${Math.round(p * 100)}" aria-label="${esc(label || `Скачано ${Math.round(p * 100)} %`)}">
      <svg viewBox="0 0 ${size} ${size}" width="${size}" height="${size}"><circle class="dlring__track" cx="${size / 2}" cy="${size / 2}" r="${r}"/><circle class="dlring__bar" cx="${size / 2}" cy="${size / 2}" r="${r}" stroke-dasharray="${c.toFixed(2)}" stroke-dashoffset="${(c * (1 - clamp(p))).toFixed(2)}" transform="rotate(-90 ${size / 2} ${size / 2})"/></svg>
      ${icon(done ? 'check' : 'arrow-down', { bold: true, cls: 'dlring__icon' })}
    </span>`;
  }

  function stateIcon(st) {
    if (st === 'downloaded') return `<span class="tstate tstate--dl" title="Скачано на это устройство">${icon('arrow-circle-down', { fill: true })}</span>`;
    if (st === 'missing') return `<span class="tstate tstate--missing" title="Нет на сервере — скачается при запуске">${icon('cloud-arrow-down')}</span>`;
    if (st === 'downloading') return `<span class="tstate">${ring(0.64, 18)}</span>`;
    return '';
  }

  const kbd = (k) => `<kbd class="kbd">${k}</kbd>`;

  // ——— Форма волны трека ———
  function waveform(seed, n) {
    const r = rng(seed);
    const sec = [0.32, 0.58, 0.92, 0.6, 0.98, 0.46, 0.94, 0.28];
    const raw = [];
    for (let i = 0; i < n; i++) {
      const x = (i / n) * (sec.length - 1);
      const a = sec[Math.floor(x)], b = sec[Math.min(sec.length - 1, Math.floor(x) + 1)];
      const env = a + (b - a) * Math.pow(x - Math.floor(x), 3);
      raw.push(clamp(env * (0.5 + 0.5 * r()) * (0.9 + 0.1 * Math.sin(i * 1.9)), 0.1, 1));
    }
    return raw.map((v, i) => (v * 2 + (raw[i - 1] ?? v) + (raw[i + 1] ?? v)) / 4);
  }

  // Переход в режиме «Диджей» начинается на границе фразы — за 8 тактов до конца
  const transitionAt = (t) => {
    const dur = t.durationMs / 1000;
    const phrase = (8 * 4 * 60) / metaOf(t).bpm;
    return Math.max(dur * 0.6, dur - phrase - 4);
  };

  function scrubber(t, o = {}) {
    const dur = t.durationMs / 1000;
    const n = o.bars || 96;
    const bars = waveform(t.id, n);
    const tz = transitionAt(t) / dur;
    const pos = o.pos ?? 0;
    const p = pos / dur;
    return `<div class="scrub${o.size ? ' scrub--' + o.size : ''}" data-scrub data-dur="${dur}" style="--p:${(p * 100).toFixed(2)}%;--t:${(tz * 100).toFixed(2)}%">
      ${o.timesBelow ? '' : `<span class="scrub__time scrub__time--now tnum" data-pos>${fmt(pos)}</span>`}
      <div class="scrub__track" role="slider" tabindex="0" aria-label="Позиция в треке" aria-valuemin="0" aria-valuemax="${Math.round(dur)}" aria-valuenow="${Math.round(pos)}" aria-valuetext="${fmt(pos)} из ${fmt(dur)}">
        <div class="scrub__bars">${bars.map((h, i) => `<i class="${i / n < p ? 'on' : ''}${i / n >= tz ? ' tz' : ''}" style="--h:${h.toFixed(2)}"></i>`).join('')}</div>
        <span class="scrub__mark" aria-hidden="true"></span>
        ${o.label ? `<span class="scrub__label tnum" aria-hidden="true">${icon('vinyl-record')}Переход в ${fmt(transitionAt(t))}</span>` : ''}
        <span class="scrub__head" aria-hidden="true"></span>
      </div>
      ${o.timesBelow
        ? `<div class="scrub__times tnum"><span data-pos>${fmt(pos)}</span><span data-left>−${fmt(dur - pos)}</span></div>`
        : `<span class="scrub__time tnum">${fmt(dur)}</span>`}
    </div>`;
  }

  // ——— Компоненты волны ———
  function wavePlay(o = {}) {
    const playing = o.playing ?? state.playing;
    return `<button class="wave-play wave-play--${o.size || 'lg'}${playing ? ' is-playing' : ''}${o.loading ? ' is-loading' : ''}${o.cls ? ' ' + o.cls : ''}" ${o.static ? '' : 'data-action="wave-play" data-kick'} aria-label="${playing ? 'Поставить волну на паузу' : 'Слушать мою волну'}"${o.disabled ? ' disabled' : ''}>
      <span class="wave-play__ring" aria-hidden="true"></span>
      ${icon(playing ? 'pause' : 'play', { fill: true, cls: 'wave-play__icon' })}
    </button>`;
  }

  function tuner(o = {}) {
    return `<div class="tuner" role="group" aria-label="Настройки волны">${Object.entries(WAVE_OPTS).map(([key, g]) => `
      <button class="tuner__cell${o.open === key ? ' is-open' : ''}" ${o.static ? '' : 'data-action="tuner"'} data-key="${key}" aria-haspopup="listbox" aria-expanded="${o.open === key}">
        <span class="tuner__label">${g.label}</span>
        <span class="tuner__value">${key === 'mood' ? moodDot(state.wave.mood) : ''}<span class="ellipsis">${optLabel(key, state.wave[key])}</span>${icon('caret-down', { cls: 'tuner__caret' })}</span>
      </button>`).join('')}</div>`;
  }

  function listbox(key) {
    const g = WAVE_OPTS[key];
    return `<div class="listbox" role="listbox" aria-label="${g.label}">${g.options.map(([v, label]) => {
      const sel = state.wave[key] === v;
      return `<button class="listbox__opt${sel ? ' is-selected' : ''}" role="option" aria-selected="${sel}" data-action="set-wave" data-key="${key}" data-val="${v}">
        ${key === 'mood' ? moodDot(v) : ''}<span>${label}</span>${sel ? icon('check', { cls: 'listbox__check' }) : ''}
      </button>`;
    }).join('')}</div>`;
  }

  function moods(o = {}) {
    return `<div class="moods" role="radiogroup" aria-label="Настроение волны">${MOOD_KEYS.map((k) => {
      const sel = (o.value ?? state.wave.mood) === k;
      return `<button class="mood${sel ? ' is-selected' : ''}${o.hover === k ? ' is-hover' : ''}" role="radio" aria-checked="${sel}" ${o.static ? '' : 'data-action="mood"'} data-mood="${k}" style="${moodVars(k)}">
        <span class="mood__orb" aria-hidden="true"></span><span class="mood__label">${optLabel('mood', k)}</span>
      </button>`;
    }).join('')}</div>`;
  }

  function presets() {
    return `<div class="presets" role="list" aria-label="Сохранённые сочетания">${PRESETS.map((p, i) => `
      <button class="preset" role="listitem" data-action="preset" data-i="${i}" style="${moodVars(p.wave.mood)}" aria-label="${p.name}: ${p.text}">
        <span class="preset__glyph" aria-hidden="true"></span>
        <span class="preset__text"><span class="preset__name">${p.name}</span><span class="preset__desc">${p.text}</span></span>
      </button>`).join('')}
      <button class="preset-add" data-action="save-preset" aria-label="Сохранить текущее сочетание">${icon('plus')}</button>
    </div>`;
  }

  const MOOD_ADJ = { energetic: 'Бодрая', happy: 'Весёлая', calm: 'Спокойная', sad: 'Грустная' };
  const ACT = { wake: 'чтобы проснуться', road: 'в дорогу', work: 'для работы', train: 'для тренировки', rest: 'для отдыха', sleep: 'перед сном', party: 'для вечеринки' };
  const CHAR = { fav: 'только любимое', mixed: 'знакомое вперемешку с новым', new: 'только незнакомое', popular: 'популярное', rare: 'редкое и малоизвестное' };
  const LANG = { any: 'на любом языке', ru: 'на русском', foreign: 'на иностранных языках', none: 'без слов' };
  const BPM_RANGE = { energetic: '118–150', happy: '105–128', calm: '78–108', sad: '68–96' };
  const summary = () => {
    const w = state.wave;
    return nb(`${MOOD_ADJ[w.mood]} музыка ${ACT[w.activity]}: ${CHAR[w.character]}, ${LANG[w.lang]}.`);
  };

  // ——— Строка трека и карточки ———
  function trackRow(t, o = {}) {
    const playing = o.playing ?? (t.id === state.trackId && o.markPlaying !== false);
    const liked = state.liked.has(t.id);
    const st = o.state;
    const cls = ['trow', playing && 'is-playing', o.hover && 'is-hover', st === 'missing' && 'is-missing', o.compact && 'trow--compact', o.drag && 'trow--drag', o.idx != null && 'trow--idx'].filter(Boolean).join(' ');
    // Нет на сервере: артист остаётся, причина уступает место пояснению — трек скачается при запуске
    const tail = st === 'missing' ? 'скачается при запуске' : o.reason;
    const sub = `${t.explicit ? badge('explicit') : ''}<span class="ellipsis">${esc(t.artist)}${tail ? `<span class="trow__reason${st === 'missing' ? ' trow__reason--missing' : ''}"> · ${esc(tail)}</span>` : ''}</span>`;
    return `<div class="${cls}" data-track="${t.id}" tabindex="0" role="listitem" aria-label="${esc(t.title)} — ${esc(t.artist)}">
      ${o.idx != null ? `<span class="trow__idx">${playing ? eq(!state.playing && !o.forcePlay) : `<span class="trow__num tnum">${o.idx}</span>`}<span class="trow__play">${icon(playing && state.playing ? 'pause' : 'play', { fill: true })}</span></span>` : ''}
      <span class="trow__cover">${cover(imgSrc(t.cover))}${o.idx == null && playing ? `<span class="trow__cover-eq">${eq(!state.playing && !o.forcePlay)}</span>` : ''}</span>
      <span class="trow__main"><span class="trow__title ellipsis">${esc(t.title)}</span><span class="trow__sub">${sub}</span></span>
      <span class="trow__state">${stateIcon(st)}</span>
      ${o.noLike ? '' : `<button class="icon-btn icon-btn--sm trow__like${liked ? ' is-on' : ''}" data-action="like" data-id="${t.id}" aria-pressed="${liked}" aria-label="${liked ? 'Убрать из любимых' : 'Нравится'}">${icon('heart', { fill: liked })}</button>`}
      <span class="trow__dur tnum">${fmt(t.durationMs / 1000)}</span>
      ${o.drag ? `<span class="trow__drag" title="Перетащите, чтобы поменять порядок" aria-hidden="true">${icon('dots-six-vertical', { bold: true })}</span>` : ''}
      <button class="icon-btn icon-btn--sm trow__more" data-action="menu" data-id="${t.id}" aria-label="Действия с треком">${icon('dots-three', { bold: true })}</button>
    </div>`;
  }

  function cardPlay(label) {
    return `<button class="card-play" data-action="card-play" aria-label="${esc(label)}">${icon('play', { fill: true })}</button>`;
  }

  function albumCard(al, o = {}) {
    const isNew = o.isNew ?? daysAgo(al.releaseDate) <= 7;
    const p = o.progress ?? DL_PROGRESS.get(al.id);
    return `<div class="acard${o.hover ? ' is-hover' : ''}" data-album="${al.id}">
      <div class="acard__cover">${cover(imgSrc(al.cover))}
        ${isNew ? `<span class="acard__badges">${badge('new')}</span>` : ''}
        ${p != null ? `<span class="acard__dl" title="Сервер скачивает релиз">${ring(p, 22)}<span class="tnum">${Math.round(p * 4)} из 4</span></span>` : ''}
        ${cardPlay(`Слушать «${al.title}»`)}
      </div>
      <a class="acard__title ellipsis" href="#home">${esc(al.title)}</a>
      <span class="acard__sub ellipsis">${esc(al.artist)}</span>
      <span class="acard__meta">${TYPE[al.type] || 'Альбом'} · ${fmtDate(al.releaseDate)}</span>
    </div>`;
  }

  function artistCard(a, o = {}) {
    return `<div class="artist${o.hover ? ' is-hover' : ''}" data-artist="${a.id}">
      <div class="artist__pic">${cover(imgSrc(a.picture))}${cardPlay(`Радио ${a.name}`)}</div>
      <a class="artist__name ellipsis" href="#home">${esc(a.name)}</a>
      <span class="artist__sub">${o.sub || (a.seed ? 'Подписка' : 'Артист')}</span>
    </div>`;
  }

  function mixCover(mix, o = {}) {
    const pics = mix.names.map((n) => artistByName(n)).filter(Boolean);
    return `<div class="mixcover" style="--mix:${mix.color};--mix-deep:${mix.deep}">
      <div class="mixcover__photos">${pics.map((a, i) => `<span class="mixcover__ph mixcover__ph--${i + 1}">${cover(imgSrc(a.picture))}</span>`).join('')}</div>
      <span class="mixcover__shade" aria-hidden="true"></span>
      <span class="mixcover__label">Микс дня</span>
      <span class="mixcover__num">${mix.n}</span>
      ${o.noPlay ? '' : cardPlay(`Слушать «Микс дня ${mix.n}»`)}
    </div>`;
  }

  function mixCard(mix, o = {}) {
    const names = mix.names.slice(0, 2).join(', ');
    return `<div class="mix${o.hover ? ' is-hover' : ''}">
      ${mixCover(mix)}
      <a class="mix__title" href="#home">Микс дня ${mix.n}</a>
      <span class="mix__sub ellipsis">${esc(names)} и др.</span>
    </div>`;
  }

  function resumeTile(r, o = {}) {
    const art = r.mosaic
      ? mosaic(r.mosaic)
      : r.artist
        ? `<span class="resume__radio">${cover(imgSrc(r.artist.picture))}<span class="resume__radio-badge">${icon('broadcast', { bold: true })}</span></span>`
        : cover(r.cover);
    return `<div class="resume${o.hover ? ' is-hover' : ''}">
      <span class="resume__art">${art}</span>
      <span class="resume__text">
        <a class="resume__title ellipsis" href="#home">${esc(r.title)}</a>
        <span class="resume__sub ellipsis">${esc(r.sub)}</span>
        <span class="resume__meta">${r.p != null ? `<span class="resume__bar" style="--p:${r.p * 100}%"><i></i></span>` : ''}<span class="ellipsis">${esc(r.meta)}</span></span>
      </span>
      <button class="resume__play" data-action="card-play" aria-label="Продолжить «${esc(r.title)}»">${icon('play', { fill: true })}</button>
    </div>`;
  }

  function shelfHead(title, o = {}) {
    return `<header class="shelf__head">
      <div class="shelf__titles"><h2 class="shelf__title">${title}</h2>${o.sub ? `<p class="shelf__sub">${nb(o.sub)}</p>` : ''}</div>
      <div class="shelf__tools">
        ${o.all === false ? '' : `<a class="shelf__all" href="#home">Все</a>`}
        ${o.arrows ? `<button class="icon-btn icon-btn--sm shelf__arrow" aria-label="Назад" disabled>${icon('caret-left', { bold: true })}</button><button class="icon-btn icon-btn--sm shelf__arrow" aria-label="Дальше">${icon('caret-right', { bold: true })}</button>` : ''}
      </div>
    </header>`;
  }

  const shelf = (title, body, o = {}) => `<section class="shelf shelf--${o.kind || 'row'}" aria-label="${title}">${shelfHead(title, o)}${body}</section>`;

  // ——— Плеер: панель, мини-плеер, громкость, переходы ———
  function volume(v = state.volume) {
    const ic = v === 0 ? 'speaker-x' : v < 0.5 ? 'speaker-low' : 'speaker-high';
    return `<div class="vol"><button class="icon-btn icon-btn--sm vol__btn" data-action="mute" aria-label="Выключить звук">${icon(ic)}</button><input class="range" type="range" min="0" max="100" value="${Math.round(v * 100)}" style="--v:${v * 100}%" aria-label="Громкость" data-input="volume"></div>`;
  }

  function reactions(t, o = {}) {
    const liked = state.liked.has(t.id);
    return `<button class="icon-btn${o.size ? ' icon-btn--' + o.size : ''} react-like${liked ? ' is-on' : ''}" data-action="like" data-id="${t.id}" aria-pressed="${liked}" aria-label="${liked ? 'Убрать из любимых' : 'Нравится'}">${icon('heart', { fill: liked })}</button>
      <button class="icon-btn${o.size ? ' icon-btn--' + o.size : ''}" data-action="dislike" data-id="${t.id}" aria-label="Не нравится — больше не предлагать">${icon('prohibit')}</button>`;
  }

  function npTrack(t) {
    return `<span class="np-track__cover" data-action="open-player" role="button" aria-label="Открыть плеер">${cover(imgSrc(t.cover))}<span class="np-track__expand">${icon('arrows-out-simple', { bold: true })}</span></span>
      <span class="np-track__text">
        <a class="np-track__title ellipsis" href="#player">${esc(t.title)}</a>
        <span class="np-track__artist ellipsis">${esc(t.artist)}<span class="np-track__reason"> · ${esc(reasonOf(t.id))}</span></span>
      </span>
      <span class="np-track__actions">${reactions(t, { size: 'sm' })}</span>`;
  }

  function npBar(o = {}) {
    const t = o.track || cur();
    const playing = o.playing ?? state.playing;
    return `<footer class="npbar${o.cls ? ' ' + o.cls : ''}" aria-label="Сейчас играет">
      <div class="np-track" data-slot="np-track">${npTrack(t)}</div>
      <div class="np-center">
        <div class="np-transport">
          <button class="icon-btn icon-btn--sm" data-action="prev" aria-label="Предыдущий трек">${icon('skip-back', { fill: true })}</button>
          <button class="np-play" data-action="toggle" data-play-icon aria-label="${playing ? 'Пауза' : 'Играть'}">${icon(playing ? 'pause' : 'play', { fill: true })}</button>
          <button class="icon-btn icon-btn--sm" data-action="next" aria-label="Следующий — не сейчас">${icon('skip-forward', { fill: true })}</button>
        </div>
        <div data-slot="np-scrub">${scrubber(t, { pos: o.pos ?? state.pos, bars: 110 })}</div>
      </div>
      <div class="np-right">
        <button class="tmode-chip${o.chipOpen ? ' is-open' : ''}" data-action="pop" data-pop="transition" aria-haspopup="dialog" aria-label="Переходы: ${TMODES.find((m) => m.id === state.transition).name}">${tmodeIcon(state.transition)}<span>${TMODES.find((m) => m.id === state.transition).name}</span></button>
        <button class="icon-btn icon-btn--sm" data-action="lyrics" aria-label="Текст песни">${icon('microphone-stage')}</button>
        <button class="icon-btn icon-btn--sm${state.queueOpen && state.route === 'player' ? ' is-active' : ''}" data-action="queue" aria-label="Очередь" aria-pressed="${state.queueOpen && state.route === 'player'}">${icon('queue')}</button>
        <button class="icon-btn icon-btn--sm" data-action="pop" data-pop="devices" aria-haspopup="dialog" aria-label="Устройства">${icon('devices')}</button>
        ${volume()}
      </div>
    </footer>`;
  }

  function mini(o = {}) {
    const t = o.track || cur();
    const dur = t.durationMs / 1000;
    const playing = o.playing ?? state.playing;
    return `<div class="mini${o.cls ? ' ' + o.cls : ''}" data-slot="mini" style="--p:${((o.pos ?? state.pos) / dur) * 100}%;--t:${(transitionAt(t) / dur) * 100}%">
      <span class="mini__cover" data-action="open-player">${cover(imgSrc(t.cover))}</span>
      <span class="mini__text" data-action="open-player"><span class="mini__title ellipsis">${esc(t.title)}</span><span class="mini__artist ellipsis">${esc(t.artist)}</span></span>
      <button class="icon-btn mini__btn" data-action="toggle" data-play-icon aria-label="${playing ? 'Пауза' : 'Играть'}">${icon(playing ? 'pause' : 'play', { fill: true })}</button>
      <button class="icon-btn mini__btn" data-action="next" aria-label="Следующий">${icon('skip-forward', { fill: true })}</button>
      <span class="mini__progress" aria-hidden="true"><i></i><b></b></span>
    </div>`;
  }

  // Мини-схемы режимов перехода: уходящий трек — серый, входящий — цветом состояния
  function tmodeIcon(id) {
    return `<svg class="tm-ico" viewBox="0 0 20 20" width="18" height="18" aria-hidden="true">${{
      gapless: '<rect x="2" y="6" width="7.5" height="8" rx="1.5" fill="currentColor" opacity=".45"/><rect x="10.5" y="6" width="7.5" height="8" rx="1.5" fill="currentColor"/>',
      smooth: '<path d="M2 6h6l6 8H2z" fill="currentColor" opacity=".45"/><path d="M18 6h-6l-6 8h12z" fill="currentColor" opacity=".9"/>',
      smart: '<path d="M2 6h9l3 8H2z" fill="currentColor" opacity=".45"/><path d="M18 6h-4l-3 8h7z" fill="currentColor"/><circle cx="12.5" cy="3.2" r="1.4" fill="currentColor"/>',
      dj: '<circle cx="10" cy="10" r="7.2" fill="none" stroke="currentColor" stroke-width="1.6"/><circle cx="10" cy="10" r="2" fill="currentColor"/><path d="M10 2.8v3.4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>',
    }[id]}</svg>`;
  }

  function tmodeDiagram(id) {
    const out = 'fill="var(--tm-out)"';
    const inn = 'fill="var(--tm-in)"';
    const beats = (x0, x1) => Array.from({ length: 9 }, (_, i) => x0 + ((x1 - x0) * i) / 8).map((x) => `<rect x="${x.toFixed(1)}" y="25" width="1" height="3" fill="var(--tm-out)"/>`).join('');
    return `<svg viewBox="0 0 56 30" width="56" height="30" aria-hidden="true">${{
      gapless: `<rect x="2" y="7" width="25" height="14" rx="2" ${out}/><rect x="29" y="7" width="25" height="14" rx="2" ${inn}/>`,
      smooth: `<path d="M2 7h22l14 14H2z" ${out}/><path d="M54 7H32L18 21h36z" ${inn} opacity=".95"/>`,
      smart: `<path d="M2 7h28l6 14H2z" ${out}/><path d="M54 7H36l-6 14h24z" ${inn}/><rect x="9" y="3" width="13" height="2" rx="1" ${out}/><rect x="40" y="3" width="10" height="2" rx="1" ${inn}/>`,
      dj: `<path d="M2 7h30v14H2z" ${out}/><path d="M20 21V7h34v14z" ${inn} opacity=".85"/>${beats(20, 32)}<rect x="31" y="3" width="1.5" height="24" fill="var(--tm-in)"/>`,
    }[id]}</svg>`;
  }

  function tmodePicker(o = {}) {
    const val = o.value ?? state.transition;
    return `<div class="tmode" role="radiogroup" aria-label="Переходы между треками">
      <div class="pop__head"><span>Переходы между треками</span>${kbd('T')}</div>
      ${TMODES.map((m) => `
        <button class="tmode__opt${val === m.id ? ' is-selected' : ''}${o.hover === m.id ? ' is-hover' : ''}" role="radio" aria-checked="${val === m.id}" ${o.static ? '' : 'data-action="tmode"'} data-mode="${m.id}">
          <span class="tmode__diagram">${tmodeDiagram(m.id)}</span>
          <span class="tmode__text"><span class="tmode__name">${m.name}</span><span class="tmode__desc">${m.desc}</span></span>
          <span class="tmode__check">${icon('check', { bold: true })}</span>
        </button>`).join('')}
      <div class="tmode__expr">
        <div class="tmode__expr-head"><span>Выразительность сведения</span><span class="tnum" data-expr-val>${Math.round(state.expr * 100)} %</span></div>
        <input class="range range--pulse" type="range" min="0" max="100" value="${Math.round(state.expr * 100)}" style="--v:${state.expr * 100}%" aria-label="Выразительность сведения" data-input="expr">
        <div class="tmode__scale"><span>Незаметно</span><span>Эффектно</span></div>
      </div>
    </div>`;
  }

  function devicePicker(o = {}) {
    const curDev = o.value ?? state.device;
    return `<div class="devices" role="dialog" aria-label="Устройства">
      <div class="pop__head"><span>Где играет музыка</span></div>
      ${DEVICES.map((d) => {
        const on = d.id === curDev;
        return `<button class="device${on ? ' is-current' : ''}${o.hover === d.id ? ' is-hover' : ''}" ${o.static ? '' : 'data-action="device"'} data-device="${d.id}">
          <span class="device__icon">${icon(d.icon)}</span>
          <span class="device__text"><span class="device__name ellipsis">${d.name}</span><span class="device__sub ellipsis">${on ? 'Играет сейчас' : d.sub}</span></span>
          ${on ? eq(false) : `<span class="device__take">Включить здесь</span>`}
        </button>`;
      }).join('')}
      <p class="pop__foot">${icon('info')}<span>Остальные устройства — пульты: видят очередь и могут забрать музыку себе.</span></p>
    </div>`;
  }

  function contextMenu(t, o = {}) {
    const liked = state.liked.has(t.id);
    const items = [
      ['next-up', 'Играть следующим', 'arrow-line-up', '⇧N'],
      ['enqueue', 'В очередь', 'list-plus', ''],
      ['to-playlist', 'В плейлист', 'playlist', '›'],
      ['radio', 'Радио по треку', 'broadcast', ''],
      ['download', 'Скачать сюда', 'arrow-circle-down', ''],
      '-',
      ['like', liked ? 'Убрать из любимых' : 'Нравится', 'heart', 'L'],
      ['ban', 'Не рекомендовать', 'prohibit', ''],
      '-',
      ['fix', 'Неверная запись — исправить', 'flag', ''],
    ];
    return `<div class="menu" role="menu" aria-label="Действия: ${esc(t.title)}">
      <div class="menu__track">${cover(imgSrc(t.cover))}<span class="menu__track-text"><span class="ellipsis">${esc(t.title)}</span><span class="ellipsis">${esc(t.artist)}</span></span></div>
      ${items.map((it) => (it === '-' ? '<span class="menu__sep" role="separator"></span>' : `
        <button class="menu__item${o.hover === it[0] ? ' is-hover' : ''}${it[0] === 'fix' ? ' menu__item--quiet' : ''}" role="menuitem" ${o.static ? '' : `data-action="menu-${it[0]}" data-id="${t.id}"`}>
          ${icon(it[2], { fill: it[0] === 'like' && liked })}<span>${it[1]}</span>${it[3] ? (it[3] === '›' ? icon('caret-right', { cls: 'menu__sub' }) : `<span class="menu__kbd">${it[3]}</span>`) : ''}
        </button>`)).join('')}
    </div>`;
  }

  function toast(text, o = {}) {
    return `<div class="toast${o.static ? ' toast--static' : ''}" role="status">
      ${icon(o.icon || 'check-circle', { fill: true, cls: 'toast__icon' })}
      <span class="toast__text">${text}</span>
      ${o.undo ? `<button class="toast__undo" ${o.static ? '' : 'data-action="undo"'}>Отменить</button>` : ''}
      <span class="toast__timer" style="--d:${o.dur || 5}s" aria-hidden="true"></span>
    </div>`;
  }

  // ——— Оболочка приложения ———
  const SVG_SIGNAL = '<svg width="18" height="12" viewBox="0 0 18 12" aria-hidden="true"><rect x="0" y="8" width="3" height="4" rx="1" fill="currentColor"/><rect x="5" y="5.5" width="3" height="6.5" rx="1" fill="currentColor"/><rect x="10" y="3" width="3" height="9" rx="1" fill="currentColor"/><rect x="15" y="0" width="3" height="12" rx="1" fill="currentColor"/></svg>';
  const SVG_WIFI = '<svg width="16" height="12" viewBox="0 0 16 12" aria-hidden="true"><path d="M8 2.2c2.3 0 4.4.9 6 2.4l1.1-1.1A10 10 0 0 0 8 .6 10 10 0 0 0 .9 3.5L2 4.6a8.4 8.4 0 0 1 6-2.4Z" fill="currentColor"/><path d="M8 5.6c1.4 0 2.6.5 3.6 1.4l1.1-1.1A6.6 6.6 0 0 0 8 4a6.6 6.6 0 0 0-4.7 1.9L4.4 7c1-.9 2.2-1.4 3.6-1.4Z" fill="currentColor"/><path d="M8 9c.5 0 1 .2 1.3.5L8 11.8 6.7 9.5C7 9.2 7.5 9 8 9Z" fill="currentColor"/></svg>';
  const SVG_BATTERY = '<svg width="27" height="13" viewBox="0 0 27 13" aria-hidden="true"><rect x=".5" y=".5" width="23" height="12" rx="3.5" fill="none" stroke="currentColor" opacity=".4"/><rect x="2" y="2" width="17" height="9" rx="2" fill="currentColor"/><path d="M25 4.5v4c.8-.3 1.3-1.1 1.3-2s-.5-1.7-1.3-2Z" fill="currentColor" opacity=".45"/></svg>';

  const iosStatus = () => `<div class="ios-status" aria-hidden="true"><span class="ios-status__time">9:41</span><span class="ios-status__icons">${SVG_SIGNAL}${SVG_WIFI}${SVG_BATTERY}</span></div>`;
  const homeIndicator = () => '<div class="home-ind" aria-hidden="true"></div>';

  function tabbar() {
    const tab = (id, label, ic, active) => `<a class="tab${active ? ' is-active' : ''}" href="#${id === 'home' ? 'home' : 'home'}" ${active ? 'aria-current="page"' : ''}>${icon(ic, { fill: active })}<span>${label}</span></a>`;
    return `<nav class="tabbar" aria-label="Разделы">${tab('home', 'Главная', 'house', true)}${tab('search', 'Поиск', 'magnifying-glass')}${tab('library', 'Медиатека', 'books')}</nav>`;
  }

  function navItem(label, ic, active, hint) {
    return `<a class="nav__item${active ? ' is-active' : ''}" href="#home" ${active ? 'aria-current="page"' : ''}>${icon(ic, { fill: active })}<span>${label}</span>${hint ? `<span class="nav__hint">${hint}</span>` : ''}</a>`;
  }

  function sidebar() {
    return `<aside class="side" aria-label="Навигация">
      <div class="side__drag" data-tauri-drag-region></div>
      <div class="traffic" aria-hidden="true"><i class="tl tl--close"></i><i class="tl tl--min"></i><i class="tl tl--max"></i></div>
      ${logo()}
      <nav class="nav">
        ${navItem('Главная', 'house', state.route === 'home')}
        ${navItem('Поиск', 'magnifying-glass', false, `${kbd('⌘')}${kbd('K')}`)}
        ${navItem('Медиатека', 'books', false)}
      </nav>
      <div class="side__scroll">
        <div class="side__group">
          <div class="side__label"><span>Медиатека</span><button class="icon-btn icon-btn--xs" aria-label="Новый плейлист">${icon('plus', { bold: true })}</button></div>
          <a class="lib" href="#home"><span class="lib__art lib__art--liked">${icon('heart', { fill: true })}</span><span class="lib__text"><span class="lib__name ellipsis">Любимые треки</span><span class="lib__sub tnum">${num(1284)} ${plural(1284, 'трек', 'трека', 'треков')}</span></span></a>
          ${PLAYLISTS.map((pl) => `<a class="lib" href="#home"><span class="lib__art">${mosaic(pl.covers)}</span><span class="lib__text"><span class="lib__name ellipsis">${esc(pl.name)}</span><span class="lib__sub">${pl.pinned ? icon('push-pin', { fill: true, cls: 'lib__pin' }) : ''}Плейлист · ${pl.count}</span></span></a>`).join('')}
        </div>
        <div class="side__group">
          <div class="side__label"><span>Подписки</span><span class="side__count tnum">24</span></div>
          ${FOLLOWED.map((a) => `<a class="lib lib--artist" href="#home"><span class="lib__art lib__art--round">${cover(imgSrc(a.picture))}</span><span class="lib__text"><span class="lib__name ellipsis">${esc(a.name)}</span></span>${HAS_NEW.has(a.name) ? '<span class="lib__new" title="Новый релиз"></span>' : ''}</a>`).join('')}
        </div>
      </div>
    </aside>`;
  }

  function toolbar(center = '') {
    return `<div class="toolbar" data-tauri-drag-region>
      <div class="toolbar__nav">
        <button class="icon-btn icon-btn--sm" aria-label="Назад" data-action="back">${icon('caret-left', { bold: true })}</button>
        <button class="icon-btn icon-btn--sm" aria-label="Вперёд" disabled>${icon('caret-right', { bold: true })}</button>
      </div>
      <div class="toolbar__center">${center}</div>
      <div class="toolbar__right">
        <button class="dl-ind" data-action="downloads" aria-label="Сервер скачивает 2 трека">${ring(0.64, 20, 'Скачивается 2 трека')}<span class="tnum">2</span></button>
        ${avatar()}
      </div>
    </div>`;
  }

  // ——— Главная ———
  function hero() {
    return `<section class="hero" aria-label="Моя волна">
      <canvas class="hero__wave" data-wave="sphere" role="img" aria-label="Живая анимация волны"></canvas>
      <div class="hero__body">
        <p class="hero__hello">Добрый вечер, Захар</p>
        <h1 class="hero__title">Моя волна</h1>
        <p class="hero__summary" data-slot="summary">${summary()}</p>
        <p class="hero__facts" data-slot="facts">${heroFacts()}</p>
        <div class="hero__tuner" data-slot="tuner">${tuner()}</div>
        <div class="hero__moods" data-slot="moods">${moods()}</div>
        <div class="hero__presets">${presets()}</div>
      </div>
      <div class="hero__control" data-slot="hero-play">${wavePlay()}</div>
    </section>`;
  }
  const heroFacts = () => `<span class="tnum">${BPM_RANGE[state.wave.mood]} уд/мин</span><span class="dot-sep"></span><span>Подстраивается после каждого трека</span>`;

  function homeView() {
    return `
      <header class="ph-head">
        <div class="ph-head__text"><span class="ph-head__hello">Добрый вечер</span><span class="ph-head__name">Захар</span></div>
        <button class="dl-ind dl-ind--phone" aria-label="Сервер скачивает 2 трека">${ring(0.64, 22, 'Скачивается 2 трека')}</button>
        ${avatar('lg')}
      </header>
      ${hero()}
      <div class="shelves">
        ${shelf('Продолжить', `<div class="row row--resume">${RESUME.map((r) => resumeTile(r)).join('')}</div>`, { all: false, kind: 'resume' })}
        ${shelf('Миксы дня', `<div class="row row--mix">${MIXES.map((m) => mixCard(m)).join('')}</div>`, { sub: 'Собраны сегодня в 6:00 по граням твоего вкуса', kind: 'mix' })}
        ${shelf('Новинки подписок', `<div class="row row--cards">${NEW_RELEASES.slice(0, 6).map((a) => albumCard(a)).join('')}</div>`, { sub: 'Сервер скачивает их сам, как только выходят', kind: 'cards' })}
        ${shelf('Открытия недели', `<div class="row row--tracks" role="list">${DISCOVERIES.map((d, i) => trackRow(d.t, { idx: i + 1, reason: d.reason, state: d.st })).join('')}</div>`, { sub: 'Музыка, которой у тебя ещё не было', kind: 'tracks' })}
      </div>`;
  }

  // ——— Плеер ———
  function transitionCard(t) {
    const next = tracks.get(state.manual[0] ?? state.fromWave[0]);
    if (!next) return '';
    const a = metaOf(t), b = metaOf(next);
    const left = transitionAt(t) - state.pos;
    const barsA = waveform(t.id, 160).slice(104);
    const barsB = waveform(next.id, 160).slice(0, 56);
    return `<section class="xfade" aria-label="Следующий переход">
      <header class="xfade__head">
        <span class="xfade__kicker">${tmodeIcon('dj')}Следующий переход</span>
        <span class="xfade__when tnum" data-xfade-left>${left > 0 ? `через ${fmt(left)}` : 'идёт сейчас'}</span>
        <button class="btn btn--ghost btn--sm xfade__edit" data-action="pop" data-pop="transition">Настроить</button>
      </header>
      <div class="xfade__body">
        <div class="xfade__track"><span class="xfade__cover">${cover(imgSrc(t.cover))}</span><span class="xfade__text"><span class="ellipsis">${esc(t.title)}</span><span class="xfade__spec tnum">${a.bpm} уд/мин · ${a.key}</span></span></div>
        <div class="xfade__viz" aria-hidden="true">
          <div class="xfade__lane xfade__lane--out">${barsA.map((h, i) => `<i style="--h:${h.toFixed(2)};--o:${(1 - Math.max(0, i - 24) / 34).toFixed(2)}"></i>`).join('')}</div>
          <div class="xfade__lane xfade__lane--in">${barsB.map((h, i) => `<i style="--h:${h.toFixed(2)};--o:${Math.min(1, 0.1 + i / 26).toFixed(2)}"></i>`).join('')}</div>
          <span class="xfade__swap"><span>${camelotOk(a.key, b.key) ? 'смена баса' : 'фильтр'}</span></span>
        </div>
        <div class="xfade__track xfade__track--in"><span class="xfade__text"><span class="ellipsis">${esc(next.title)}</span><span class="xfade__spec tnum">${b.bpm} уд/мин · ${b.key}</span></span><span class="xfade__cover">${cover(imgSrc(next.cover))}</span></div>
      </div>
      <p class="xfade__note">${nb(camelotOk(a.key, b.key)
        ? `Сведение по долям: темп ${a.bpm} → ${b.bpm}, тональности ${a.key} и ${b.key} совместимы, вход на границе фразы через 8 тактов.`
        : `Тональности ${a.key} и ${b.key} не сочетаются — переход через фильтр с эхо-выходом на границе фразы, темп ${a.bpm} → ${b.bpm}.`)}</p>
    </section>`;
  }

  function playerView() {
    const t = cur();
    return `<section class="player" aria-label="Сейчас играет">
      <header class="player__head">
        <button class="icon-btn" data-action="close-player" aria-label="Свернуть плеер">${icon('caret-down', { bold: true })}</button>
        <div class="player__ctx" data-slot="ctx">${playerCtx()}</div>
        <button class="icon-btn" data-action="menu" data-id="${t.id}" aria-label="Ещё">${icon('dots-three', { bold: true })}</button>
      </header>
      <div class="player__stage">
        <canvas class="player__wave" data-wave="ring" role="img" aria-label="Волна вокруг обложки"></canvas>
        <figure class="player__cover" data-slot="cover" data-kick>${cover(imgSrc(t.cover))}</figure>
      </div>
      <div class="player__info" data-slot="player-info">${playerInfo(t)}</div>
      <div class="player__scrub" data-slot="player-scrub">${scrubber(t, { pos: state.pos, size: 'lg', timesBelow: true, label: true, bars: 72 })}</div>
      <div class="player__controls">
        <button class="icon-btn icon-btn--lg" data-action="dislike" aria-label="Не нравится">${icon('prohibit')}</button>
        <button class="icon-btn icon-btn--lg" data-action="prev" aria-label="Предыдущий">${icon('skip-back', { fill: true })}</button>
        <span data-slot="player-play">${wavePlay({ size: 'md' })}</span>
        <button class="icon-btn icon-btn--lg" data-action="next" aria-label="Следующий — не сейчас">${icon('skip-forward', { fill: true })}</button>
        <span data-slot="player-like">${likeBig(t)}</span>
      </div>
      <div class="player__secondary">
        <button class="icon-btn" data-action="pop" data-pop="devices" aria-label="Устройства">${icon('devices')}</button>
        <button class="tmode-chip" data-action="pop" data-pop="transition">${tmodeIcon(state.transition)}<span>${TMODES.find((m) => m.id === state.transition).name}</span></button>
        ${badge('quality')}
        <button class="icon-btn" data-action="lyrics" aria-label="Текст песни">${icon('microphone-stage')}</button>
        <button class="icon-btn" data-action="queue" aria-label="Очередь">${icon('queue')}</button>
      </div>
      <div class="player__xfade" data-slot="xfade">${transitionCard(t)}</div>
    </section>`;
  }

  const likeBig = (t) => {
    const liked = state.liked.has(t.id);
    return `<button class="icon-btn icon-btn--lg react-like${liked ? ' is-on' : ''}" data-action="like" data-id="${t.id}" aria-pressed="${liked}" aria-label="${liked ? 'Убрать из любимых' : 'Нравится'}">${icon('heart', { fill: liked })}</button>`;
  };

  const playerCtx = () => `<span class="player__ctx-title">Моя волна</span><span class="player__ctx-sub">${moodDot(state.wave.mood)}${optLabel('mood', state.wave.mood)}</span>`;

  function playerInfo(t) {
    return `<div class="player__titles">
        <h1 class="player__title${t.title.length > 14 ? ' player__title--long' : ''}">${esc(t.title)}</h1>
        <a class="player__artist ellipsis" href="#home">${esc(t.artist)}</a>
        <p class="reason">${icon('sparkle', { fill: true })}<span>${esc(reasonOf(t.id))}</span></p>
      </div>
      <div class="player__actions">
        ${badge('quality')}
        <span class="player__react">${reactions(t)}</span>
        <button class="btn btn--secondary btn--sm" data-action="more-like">${icon('plus-circle')}Больше такого</button>
      </div>`;
  }

  function queuePanel() {
    const t = cur();
    return `<aside class="rpanel" aria-label="Боковая панель">
      <nav class="tabs" role="tablist">
        <button class="tab-btn is-active" role="tab" aria-selected="true">Очередь</button>
        <button class="tab-btn" role="tab" aria-selected="false">Текст</button>
        <button class="tab-btn" role="tab" aria-selected="false">Устройства</button>
        <button class="tab-btn" role="tab" aria-selected="false">О треке</button>
      </nav>
      <div class="rpanel__body" data-slot="queue">${queueBody(t)}</div>
      <footer class="rpanel__foot">
        <button class="btn btn--ghost btn--sm">${icon('playlist')}Сохранить как плейлист</button>
        <button class="btn btn--ghost btn--sm">${icon('clock-counter-clockwise')}История</button>
      </footer>
    </aside>`;
  }

  function queueBody(t) {
    const man = state.manual.map((id) => tracks.get(id));
    const wav = state.fromWave.map((id) => tracks.get(id));
    return `
      <section class="qsec"><h3 class="qsec__title">Сейчас</h3>${trackRow(t, { compact: true, reason: reasonShort(t.id), playing: true, noLike: true })}</section>
      ${man.length ? `<section class="qsec"><h3 class="qsec__title">Далее — добавлено вручную <span class="qsec__count tnum">${man.length}</span><button class="qsec__clear" data-action="clear-manual">Очистить</button></h3>${man.map((m) => trackRow(m, { compact: true, drag: true, noLike: true, markPlaying: false })).join('')}</section>` : ''}
      <section class="qsec"><h3 class="qsec__title">Далее — из волны ${moodDot(state.wave.mood)}<span class="qsec__hint">${nb('Пересчитывается после каждого лайка и пропуска')}</span></h3>${wav.map((m) => trackRow(m, { compact: true, reason: reasonShort(m.id), noLike: true, markPlaying: false })).join('')}</section>`;
  }

  // ——— Экран волны с отладкой ———
  function waveView() {
    const pv = D.previews.map((p) => ({ ...p, t: tracks.get(p.trackId) }));
    return `<div class="wavescreen">
      <canvas class="wavescreen__canvas" data-wave="sphere" role="img" aria-label="Волна во весь экран"></canvas>
      <a class="wavescreen__back" href="#home">${icon('caret-left', { bold: true })}<span>На главную</span></a>
      <div class="wavescreen__title"><span class="wavescreen__kicker">Волна · направление B</span><span class="wavescreen__name">Импульс</span></div>
      <aside class="dbg" aria-label="Панель отладки волны">
        <header class="dbg__head"><span>Отладка</span><span class="dbg__fps tnum" data-fps>— к/с</span></header>
        <div class="dbg__row"><span class="dbg__label">Настроение · палитра перетекает за 1,2 с</span><div data-slot="moods">${moods()}</div></div>
        <div class="dbg__sliders">
          <div class="dbg__row"><span class="dbg__label">Энергия <b class="tnum" data-val="energy">авто</b></span><input class="range range--pulse" type="range" min="0" max="100" value="55" style="--v:55%" data-input="energy" aria-label="Энергия"></div>
          <div class="dbg__row"><span class="dbg__label">Темп <b class="tnum" data-val="bpm">120 уд/мин</b></span><input class="range" type="range" min="60" max="180" value="120" style="--v:50%" data-input="bpm" aria-label="Темп"></div>
        </div>
        <div class="dbg__row"><span class="dbg__label">Реакции</span>
          <div class="dbg__btns">
            <button class="dbg__btn" data-action="w-start">${icon('play', { fill: true })}<span>Старт</span></button>
            <button class="dbg__btn" data-action="w-skip">${icon('skip-forward', { fill: true })}<span>Пропуск</span></button>
            <button class="dbg__btn" data-action="w-like">${icon('heart', { fill: true })}<span>Лайк</span></button>
            <button class="dbg__btn" data-action="w-pause" data-w-pause>${icon('pause', { fill: true })}<span>Пауза</span></button>
          </div>
        </div>
        <div class="dbg__row"><span class="dbg__label">Превью — WebAudio</span>
          <div class="dbg__list" role="list">${pv.map((p) => `
            <button class="pv" role="listitem" data-action="preview" data-url="${FIX + p.file}" data-id="${p.trackId}">
              <span class="pv__cover">${cover(imgSrc(p.t.cover))}<span class="pv__state">${icon('play', { fill: true })}</span></span>
              <span class="pv__text"><span class="ellipsis">${esc(p.t.title)}</span><span class="ellipsis">${esc(p.t.artist)}</span></span>
              <span class="pv__bpm tnum" data-bpm-for="${p.trackId}">0:30</span>
            </button>`).join('')}</div>
        </div>
        <footer class="dbg__foot" data-dbg-status>Такт — симуляция, 120 уд/мин. Включите превью: темп и сетка долей посчитаются по звуку.</footer>
      </aside>
    </div>`;
  }

  // ——— Лист компонентов ———
  function spec(title, body, o = {}) {
    return `<section class="spec${o.wide ? ' spec--wide' : ''}" id="c-${o.id || ''}">
      <header class="spec__head"><h2 class="spec__title">${title}</h2>${o.note ? `<p class="spec__note">${nb(o.note)}</p>` : ''}</header>
      <div class="spec__body">${body}</div>
    </section>`;
  }
  const cell = (label, body, cls = '') => `<div class="specimen ${cls}"><span class="specimen__label">${label}</span><div class="specimen__stage">${body}</div></div>`;

  function componentsView() {
    const t = T.now;
    const long = trackBy('Belaya Polosa (The Crystal Method x Future Funk Squad Dub Remix)') || D.tracks[0];
    const swatch = (name, v, note, cls = '') => `<div class="swatch ${cls}"><span class="swatch__chip" style="background:${v}"></span><span class="swatch__name">${name}</span><span class="swatch__val tnum">${note || v}</span></div>`;
    const btnRow = (variant, label, ic) => ['', 'is-hover', 'is-press', 'is-focus', 'disabled'].map((st) => `<button class="btn btn--${variant}${st && st !== 'disabled' ? ' ' + st : ''}"${st === 'disabled' ? ' disabled' : ''}>${ic ? icon(ic, { fill: variant === 'primary' }) : ''}${label}</button>`).join('');
    const states = ['Обычное', 'Наведение', 'Нажатие', 'Фокус', 'Неактивно'];

    return `<div class="sheet">
      <header class="sheet__hero">
        <div class="sheet__brand">${appIcon(64)}<div><p class="sheet__kicker">DalyWave · направление B</p><h1 class="sheet__title">Импульс</h1></div></div>
        <p class="sheet__lead">${nb('Спокойная тёмная основа и один электрический акцент — лайм «Импульс». Он появляется только там, где музыка живая: кнопка волны, доля в анимации, отметка перехода, то, что играет сейчас. Всё остальное — графит, воздух и точная типографика.')}</p>
        <nav class="sheet__toc" aria-label="Разделы">${[['tokens', 'Токены'], ['buttons', 'Кнопки'], ['wave', 'Волна'], ['select', 'Выбор'], ['rows', 'Строки'], ['cards', 'Карточки'], ['player', 'Плеер'], ['popovers', 'Всплывающие'], ['status', 'Состояния'], ['inputs', 'Ввод']].map(([id, l]) => `<a href="#components" data-jump="c-${id}">${l}</a>`).join('')}</nav>
      </header>

      ${spec('Токены', `
        <div class="brandrow">
          <div class="brandrow__item">${appIcon(72)}<span class="specimen__label">Значок приложения</span></div>
          <div class="brandrow__item brandrow__item--lockup"><span class="logo logo--lg">${GLYPH}<span class="logo__word">DalyWave</span></span><span class="specimen__label">Знак и название в Onest — боковая панель</span></div>
          <div class="brandrow__item"><span class="brandrow__glyph">${GLYPH}</span><span class="specimen__label">Знак отдельно · цвет текста</span></div>
        </div>
        <div class="tokens">
          <div class="tokens__group"><h3>Поверхности</h3>${swatch('bg-0 · окно', 'var(--bg-0)', '#09090B')}${swatch('bg-1 · панель', 'var(--bg-1)', '#0F1013')}${swatch('bg-2 · карточка', 'var(--bg-2)', '#16171C')}${swatch('bg-3 · наведение', 'var(--bg-3)', '#1E1F26')}${swatch('bg-4 · выбрано', 'var(--bg-4)', '#272932')}</div>
          <div class="tokens__group"><h3>Текст</h3>${swatch('text-1', 'var(--text-1)', '#F3F4F6 · 17,3:1')}${swatch('text-2', 'var(--text-2)', '#A4A7B2 · 7,9:1')}${swatch('text-3', 'var(--text-3)', '#8A8D99 · 5,8:1')}${swatch('text-off', 'var(--text-off)', '#5D606B · неактивное')}</div>
          <div class="tokens__group"><h3>Акцент и служебные</h3>${swatch('pulse · Импульс', 'var(--pulse)', '#CDFF3F', 'swatch--pulse')}${swatch('pulse-ink', 'var(--pulse-ink)', '#0B0C07 · 16,8:1')}${swatch('danger', 'var(--danger)', '#FF6B5E')}${swatch('warn', 'var(--warn)', '#FFB547')}</div>
          <div class="tokens__group"><h3>Настроения — только для волны</h3>${MOOD_KEYS.map((k) => `<div class="swatch"><span class="swatch__chip swatch__chip--mood" style="${moodVars(k)}"></span><span class="swatch__name">${optLabel('mood', k)}</span><span class="swatch__val">${Wave.MOODS[k].mid.toUpperCase()} → ${Wave.MOODS[k].hi.toUpperCase()}</span></div>`).join('')}</div>
        </div>
        <div class="typescale">
          <div class="ts"><span class="ts__meta">Unbounded 600 · 56/60</span><span class="ts__sample ts--d1">Моя волна</span></div>
          <div class="ts"><span class="ts__meta">Unbounded 500 · 32/36</span><span class="ts__sample ts--d2">Микс дня 1</span></div>
          <div class="ts"><span class="ts__meta">Unbounded 500 · 20/28</span><span class="ts__sample ts--d3">Новинки подписок</span></div>
          <div class="ts"><span class="ts__meta">Onest 600 · 24/30</span><span class="ts__sample ts--h1">3days — Mytch</span></div>
          <div class="ts"><span class="ts__meta">Onest 600 · 15/20</span><span class="ts__sample ts--h2">Далее — из волны</span></div>
          <div class="ts"><span class="ts__meta">Onest 400 · 14/20</span><span class="ts__sample ts--body">Спокойная музыка для отдыха: знакомое вперемешку с новым, на любом языке.</span></div>
          <div class="ts"><span class="ts__meta">Onest 500 · 12/16, таблич.</span><span class="ts__sample ts--cap tnum">1:12 · 2:51 · 1 284 трека · 92 уд/мин</span></div>
          <div class="ts"><span class="ts__meta">Onest 600 · 11/14, разрядка</span><span class="ts__sample ts--over">Настроение</span></div>
        </div>
        <div class="scales">
          <div class="scale"><h3>Отступы · шаг 4</h3><div class="scale__row">${[4, 8, 12, 16, 20, 24, 32, 40, 48, 64].map((s) => `<span class="space" style="--s:${s}px"><i></i><b class="tnum">${s}</b></span>`).join('')}</div></div>
          <div class="scale"><h3>Скругления</h3><div class="scale__row">${[['xs', 4], ['sm', 6], ['md', 10], ['lg', 14], ['xl', 20], ['full', 999]].map(([n, r]) => `<span class="radius" style="--r:${r}px"><i></i><b>${n} · ${r === 999 ? '∞' : r}</b></span>`).join('')}</div></div>
          <div class="scale"><h3>Тени и свет</h3><div class="scale__row">${[['hl', 'Внутренний блик'], ['pop', 'Всплывающее'], ['float', 'Плавающее'], ['glow', 'Свечение акцента']].map(([n, l]) => `<span class="shadow shadow--${n}"><i></i><b>${l}</b></span>`).join('')}</div></div>
          <div class="scale"><h3>Движение</h3><div class="motion">${[['d-1', '120 мс', 'наведение'], ['d-2', '200 мс', 'нажатие, переключатели'], ['d-3', '320 мс', 'панели, всплывающие'], ['d-4', '560 мс', 'смена экрана'], ['wave', '1200 мс', 'перетекание палитры волны']].map(([n, v, l]) => `<div class="motion__row"><span class="motion__bar motion__bar--${n}"><i></i></span><span class="tnum">${v}</span><span>${l}</span></div>`).join('')}<p class="motion__curves">Кривые: <code>out (.2,.8,.2,1)</code> — появление, <code>in-out (.65,0,.35,1)</code> — перемещение, <code>spring (.34,1.56,.64,1)</code> — кнопки и переключатели.</p></div></div>
        </div>`, { id: 'tokens', wide: true, note: 'Цвета, шрифты Unbounded + Onest (оба с кириллицей), шаг сетки 4, иконки Phosphor Regular 20 px; заливка — только для активного состояния.' })}

      ${spec('Кнопки', `
        <div class="matrix"><div class="matrix__head"><span></span>${states.map((s) => `<span>${s}</span>`).join('')}</div>
          <div class="matrix__row"><span class="matrix__label">Главная</span>${btnRow('primary', 'Слушать', 'play')}</div>
          <div class="matrix__row"><span class="matrix__label">Вторичная</span>${btnRow('secondary', 'В медиатеку', 'plus')}</div>
          <div class="matrix__row"><span class="matrix__label">Тихая</span>${btnRow('ghost', 'Все')}</div>
          <div class="matrix__row"><span class="matrix__label">Иконка</span>${['', 'is-hover', 'is-press', 'is-focus', 'disabled'].map((st) => `<button class="icon-btn${st && st !== 'disabled' ? ' ' + st : ''}"${st === 'disabled' ? ' disabled' : ''} aria-label="Нравится">${icon('heart')}</button>`).join('')}</div>
          <div class="matrix__row"><span class="matrix__label">Иконка вкл.</span>${['', 'is-hover', 'is-press', 'is-focus', 'disabled'].map((st) => `<button class="icon-btn is-on${st && st !== 'disabled' ? ' ' + st : ''}"${st === 'disabled' ? ' disabled' : ''} aria-label="Нравится">${icon('heart', { fill: true })}</button>`).join('')}</div>
        </div>
        <p class="spec__aside">Лаймовая кнопка — одна на экран. Размеры: 32, 40, 48; иконочные — 28, 36, 44 (на телефоне зона нажатия не меньше 44).</p>`, { id: 'buttons', wide: true })}

      ${spec('Волна', `
        <div class="grid grid--4">
          ${cell('Кнопка волны · ждёт', `<div class="center-stage">${wavePlay({ playing: false, static: true })}</div>`)}
          ${cell('Играет · кольцо бьётся в долю', `<div class="center-stage">${wavePlay({ playing: true, static: true, cls: 'is-beat' })}</div>`)}
          ${cell('Загрузка · трек качается', `<div class="center-stage">${wavePlay({ playing: false, loading: true, static: true })}</div>`)}
          ${cell('Нет сети и запаса', `<div class="center-stage">${wavePlay({ playing: false, static: true, disabled: true })}</div>`)}
        </div>
        <div class="grid grid--2">
          ${cell('Тюнер волны — четыре настройки в одной строке', `<div class="stage-col">${tuner({ static: true })}<div class="tuner-open">${tuner({ static: true, open: 'activity' })}<div class="listbox-wrap">${listbox('activity')}</div></div></div>`, 'specimen--tall')}
          ${cell('Настроение · сферы показывают палитру волны', `<div class="stage-col">${moods({ static: true })}${moods({ static: true, value: 'energetic', hover: 'sad' })}<span class="specimen__hint">Сверху — спокойное выбрано; снизу — бодрое выбрано, наведение на «Грустное».</span></div>`, 'specimen--tall')}
        </div>
        ${cell('Сохранённые сочетания', presets())}`, { id: 'wave', wide: true, note: 'Свои типы компонентов для главной функции продукта: не набор чипсов, а инструмент настройки потока.' })}

      ${spec('Выбор', `
        <div class="grid grid--3">
          ${cell('Сегменты', `<div class="stage-col"><div class="seg" role="radiogroup"><button class="seg__opt is-selected" role="radio" aria-checked="true">Медиатека</button><button class="seg__opt" role="radio" aria-checked="false">Весь каталог</button></div><div class="seg"><button class="seg__opt">Сетка</button><button class="seg__opt is-hover">Список</button><button class="seg__opt is-selected">Обложки</button></div></div>`)}
          ${cell('Чипсы-фильтры', `<div class="chips"><button class="chip is-selected">${icon('check', { bold: true })}Альбомы</button><button class="chip">Синглы и EP</button><button class="chip is-hover">Участие</button><button class="chip" disabled>Концертные</button></div>`)}
          ${cell('Теги', `<div class="chips"><span class="tag">фонк</span><span class="tag">гиперпоп</span><span class="tag">альт-рок</span><span class="tag tag--mood" style="${moodVars('calm')}">${moodDot('calm')}спокойное</span><span class="tag tnum">92 уд/мин</span><span class="tag tnum">8A</span></div>`)}
        </div>
        <div class="grid grid--3">
          ${cell('Вкладки', `<nav class="tabs tabs--inline" role="tablist"><button class="tab-btn is-active" role="tab" aria-selected="true">Очередь</button><button class="tab-btn is-hover" role="tab" aria-selected="false">Текст</button><button class="tab-btn" role="tab" aria-selected="false">Устройства</button><button class="tab-btn" role="tab" aria-selected="false">О треке</button></nav>`)}
          ${cell('Переключатель', `<div class="stage-col">${[['Скачивать по Wi-Fi', true, ''], ['Умный запас', false, ''], ['Диджей на сайте', false, 'disabled']].map(([l, on, dis]) => `<label class="switch-row${dis ? ' is-disabled' : ''}"><span>${l}${dis ? '<small>Только в приложении</small>' : ''}</span><button class="switch" role="switch" aria-checked="${on}" ${dis ? 'disabled' : ''} aria-label="${l}"></button></label>`).join('')}</div>`)}
          ${cell('Подсказка и клавиши', `<div class="stage-col stage-col--center"><span class="tip-demo"><button class="icon-btn is-hover" aria-label="Следующий">${icon('skip-forward', { fill: true })}</button><span class="tooltip" role="tooltip">Не сейчас — пропустить без штрафа ${kbd('→')}</span></span><span class="kbd-row">${kbd('⌘')}${kbd('K')}<span>палитра команд</span></span><span class="kbd-row">${kbd('Пробел')}<span>пауза</span></span></div>`)}
        </div>`, { id: 'select', wide: true })}

      ${spec('Строки трека', `
        <div class="rows-demo" role="list">
          <div class="rows-demo__item"><span class="specimen__label">Обычная</span>${trackRow(T.float, { idx: 1, reason: 'Похоже на Mytch', state: 'server', markPlaying: false })}</div>
          <div class="rows-demo__item"><span class="specimen__label">Наведение</span>${trackRow(T.noise, { idx: 2, reason: 'Новое для тебя', hover: true, markPlaying: false })}</div>
          <div class="rows-demo__item"><span class="specimen__label">Играет</span>${trackRow(t, { idx: 3, reason: 'Похоже на Lowborn', playing: true, forcePlay: true })}</div>
          <div class="rows-demo__item"><span class="specimen__label">Нет на сервере — скачается при запуске</span>${trackRow(tracks.get(1698366377), { idx: 4, state: 'missing', markPlaying: false })}</div>
          <div class="rows-demo__item"><span class="specimen__label">Скачано сюда</span>${trackRow(tracks.get(2118942927), { idx: 5, state: 'downloaded', reason: 'Похоже на Lowborn', markPlaying: false })}</div>
          <div class="rows-demo__item"><span class="specimen__label">Качается</span>${trackRow(tracks.get(2978170491), { idx: 6, state: 'downloading', markPlaying: false })}</div>
          <div class="rows-demo__item"><span class="specimen__label">Длинное название</span>${trackRow(long, { idx: 7, markPlaying: false })}</div>
          <div class="rows-demo__item"><span class="specimen__label">В очереди</span>${trackRow(T.iwantout, { compact: true, drag: true, noLike: true, markPlaying: false })}</div>
        </div>`, { id: 'rows', wide: true, note: 'Одна строка на все списки: альбом, плейлист, очередь, поиск. Слот состояния справа от названия всегда на месте — строки не прыгают.' })}

      ${spec('Карточки и полки', `
        ${shelfHead('Новинки подписок', { sub: 'Заголовок полки: название, подпись, «Все» и стрелки', arrows: true })}
        <div class="grid grid--cards">
          ${cell('Альбом', albumCard(NEW_RELEASES[0]))}
          ${cell('Альбом · наведение', albumCard(NEW_RELEASES[2], { hover: true }))}
          ${cell('Релиз качается', albumCard(NEW_RELEASES[1]))}
          ${cell('Артист', artistCard(artistByName('Lowborn')))}
          ${cell('Артист · наведение', artistCard(artistByName('Mzlff'), { hover: true }))}
          ${cell('Микс дня', mixCard(MIXES[0]))}
        </div>
        <div class="grid grid--2">
          ${cell('Продолжить', resumeTile(RESUME[0]))}
          ${cell('Продолжить · наведение', resumeTile(RESUME[1], { hover: true }))}
        </div>`, { id: 'cards', wide: true, note: 'Обложки миксов собираются из фото артистов: снимок окрашивается в цвет грани вкуса, номер — крупно.' })}

      ${spec('Плеер', `
        ${cell('Панель «Сейчас играет» — десктоп', npBar({ cls: 'npbar--demo', playing: true }), 'specimen--flush')}
        <div class="grid grid--2">
          ${cell('Мини-плеер — телефон', mini({ cls: 'mini--demo', playing: true }))}
          ${cell('Громкость', `<div class="stage-col">${volume(0.72)}<div class="vol is-hover-demo">${volume(0.35)}</div></div>`)}
        </div>
        ${cell('Шкала в виде формы волны · лаймовая отметка — здесь начнётся переход', `<div class="stage-col stage-col--stretch">${scrubber(t, { pos: 72, bars: 160 })}${scrubber(t, { pos: 72, size: 'lg', timesBelow: true, label: true, bars: 150 })}</div>`)}
        ${cell('Следующий переход — карточка в развёрнутом плеере', transitionCard(t), 'specimen--flush')}`, { id: 'player', wide: true })}

      ${spec('Всплывающие', `
        <div class="grid grid--3 grid--top">
          ${cell('Режим переходов', tmodePicker({ static: true, hover: 'smart' }))}
          ${cell('Устройства', devicePicker({ static: true, hover: 'iphone' }))}
          ${cell('Контекстное меню', contextMenu(T.float, { static: true, hover: 'enqueue' }))}
        </div>
        ${cell('Уведомление с отменой', `<div class="stage-col stage-col--center">${toast('Удалено из плейлиста «Ночной дрифт»', { undo: true, static: true, icon: 'trash' })}${toast('Волна: Работа — спокойное, без слов', { static: true, icon: 'waveform' })}</div>`)}`, { id: 'popovers', wide: true })}

      ${spec('Состояния', `
        <div class="grid grid--3">
          ${cell('Значки', `<div class="chips">${badge('quality')}${badge('quality', 'FLAC 16/44')}${badge('explicit')}${badge('dl')}${badge('new')}${badge('server')}</div>`)}
          ${cell('Прогресс загрузки', `<div class="rings">${[0, 0.35, 0.72, 1].map((p) => `<span class="rings__item">${ring(p, 36)}<b class="tnum">${p === 1 ? 'Готово' : Math.round(p * 100) + ' %'}</b></span>`).join('')}</div>`)}
          ${cell('Скелет загрузки', `<div class="skel-demo"><span class="skel skel--cover"></span><span class="skel-lines"><span class="skel skel--line"></span><span class="skel skel--line skel--short"></span></span></div><div class="skel-demo"><span class="skel skel--cover"></span><span class="skel-lines"><span class="skel skel--line"></span><span class="skel skel--line skel--short"></span></span></div>`)}
        </div>
        <div class="grid grid--2">
          ${cell('Пустой экран подсказывает следующий шаг', `<div class="empty"><span class="empty__icon empty__icon--dots">${icon('arrow-circle-down')}</span><h3 class="empty__title">Здесь будут скачанные треки</h3><p class="empty__text">${nb('Скачайте альбом или включите «Умный запас» — волна будет играть даже без сети.')}</p><button class="btn btn--primary btn--sm">${icon('arrow-circle-down', { fill: true })}Включить запас</button></div>`)}
          ${cell('Ошибка говорит, что делать', `<div class="empty empty--error"><span class="empty__icon">${icon('cloud-slash')}</span><h3 class="empty__title">Сервер не отвечает</h3><p class="empty__text">${nb('Играем из скачанного: 312 треков. Остальное вернётся, когда появится связь.')}</p><button class="btn btn--secondary btn--sm">${icon('arrows-clockwise')}Повторить</button></div>`)}
        </div>`, { id: 'status', wide: true })}

      ${spec('Ввод', `
        <div class="grid grid--3">
          ${cell('Поиск · пусто', `<label class="search">${icon('magnifying-glass')}<input type="search" placeholder="Артист, трек или ссылка" aria-label="Поиск">${kbd('⌘K')}</label>`)}
          ${cell('Поиск · фокус', `<label class="search is-focus">${icon('magnifying-glass')}<input type="search" value="спокойный инструментал для работы" aria-label="Поиск"><button class="icon-btn icon-btn--xs" aria-label="Очистить">${icon('x', { bold: true })}</button></label>`)}
          ${cell('Поиск · ссылка', `<label class="search">${icon('link')}<input type="search" value="youtube.com/watch?v=Q7z…" aria-label="Поиск"><span class="search__hint">Ссылка YouTube</span></label>`)}
        </div>`, { id: 'inputs', wide: true })}

      <footer class="sheet__foot">${logo()}<span>Прототип направления B · данные Deezer · ${D.tracks.length} треков, ${D.artists.length} артистов</span></footer>
    </div>`;
  }

  // ——— Сборка экранов ———
  function shell(content, o = {}) {
    return `<div class="app route-${state.route}${o.panel ? ' has-panel' : ''}">
      ${iosStatus()}
      ${sidebar()}
      <main class="main" id="main">
        ${toolbar(o.center || '')}
        <div class="main__scroll" data-scroll>${content}</div>
      </main>
      ${o.panel ? queuePanel() : ''}
      ${npBar()}
      ${o.mini === false ? '' : mini()}
      ${o.tabbar === false ? '' : tabbar()}
      ${homeIndicator()}
    </div>`;
  }

  function render() {
    closePop();
    Wave.unmountAll();
    const r = state.route;
    document.documentElement.dataset.route = r;
    if (r === 'wave') root.innerHTML = waveView();
    else if (r === 'components') root.innerHTML = componentsView();
    else if (r === 'player') root.innerHTML = shell(playerView(), { panel: state.queueOpen, mini: false, tabbar: false, center: `<div class="player__ctx" data-slot="ctx">${playerCtx()}</div>` });
    else root.innerHTML = shell(homeView());
    afterRender();
  }

  let kickEls = [];
  function afterRender() {
    mountWaves();
    kickEls = $$('[data-kick]');
    const sc = $('[data-scroll]');
    const onScroll = () => {
      const y = sc && sc.scrollHeight > sc.clientHeight + 2 ? sc.scrollTop : window.scrollY;
      document.documentElement.classList.toggle('is-scrolled', y > 8);
    };
    sc?.addEventListener('scroll', onScroll, { passive: true });
    window.onscroll = onScroll;
    onScroll();
    if (state.route === 'wave') {
      // Отладка начинается с чистой палитры настроения; обложка подмешается, когда заиграет превью
      Wave.setPlaying(true);
      Wave.setBpm(120);
      Wave.setCover(null);
    } else if (state.route !== 'components') {
      Wave.setPlaying(state.playing);
      Wave.setBpm(metaOf(cur()).bpm);
      Wave.setCover(imgSrc(cur().cover));
    }
  }

  // Волна создаётся только на видимых холстах: скрытые варианты раскладки контекст не тратят
  function mountWaves() {
    $$('canvas[data-wave]').forEach((c) => {
      if (c.offsetParent !== null && c.clientWidth > 0) Wave.mount(c, { layout: c.dataset.wave });
    });
  }

  // ——— Точечные обновления (без перерисовки холстов) ———
  function refreshWaveSettings() {
    $$('[data-slot="tuner"]').forEach((el) => (el.innerHTML = tuner()));
    $$('[data-slot="moods"]').forEach((el) => (el.innerHTML = moods()));
    $$('[data-slot="summary"]').forEach((el) => (el.textContent = summary()));
    $$('[data-slot="facts"]').forEach((el) => (el.innerHTML = heroFacts()));
    $$('[data-slot="ctx"]').forEach((el) => (el.innerHTML = playerCtx()));
    $$('.seg__opt[data-mood]').forEach((el) => {
      const on = el.dataset.mood === state.wave.mood;
      el.classList.toggle('is-selected', on);
      el.setAttribute('aria-checked', on);
    });
    if (state.route === 'player') $('[data-slot="queue"]') && ($('[data-slot="queue"]').innerHTML = queueBody(cur()));
  }

  function refreshPlay() {
    $$('[data-play-icon]').forEach((b) => {
      b.innerHTML = icon(state.playing ? 'pause' : 'play', { fill: true });
      b.setAttribute('aria-label', state.playing ? 'Пауза' : 'Играть');
    });
    $$('[data-slot="hero-play"], [data-slot="player-play"]').forEach((el) => (el.innerHTML = wavePlay(el.dataset.slot === 'player-play' ? { size: 'md' } : {})));
    $$('.eq').forEach((e) => e.classList.toggle('is-paused', !state.playing));
    kickEls = $$('[data-kick]');
  }

  function refreshTrack() {
    const t = cur();
    $$('[data-slot="np-track"]').forEach((el) => (el.innerHTML = npTrack(t)));
    $$('[data-slot="np-scrub"]').forEach((el) => (el.innerHTML = scrubber(t, { pos: state.pos, bars: 110 })));
    $$('[data-slot="mini"]').forEach((el) => (el.outerHTML = mini()));
    $$('[data-slot="cover"]').forEach((el) => (el.innerHTML = cover(imgSrc(t.cover))));
    $$('[data-slot="player-info"]').forEach((el) => (el.innerHTML = playerInfo(t)));
    $$('[data-slot="player-scrub"]').forEach((el) => (el.innerHTML = scrubber(t, { pos: state.pos, size: 'lg', timesBelow: true, label: true, bars: 72 })));
    $$('[data-slot="player-like"]').forEach((el) => (el.innerHTML = likeBig(t)));
    $$('[data-slot="xfade"]').forEach((el) => (el.innerHTML = transitionCard(t)));
    $$('[data-slot="queue"]').forEach((el) => (el.innerHTML = queueBody(t)));
    $$('.trow').forEach((r) => {
      const on = Number(r.dataset.track) === t.id && !r.closest('.rows-demo');
      if (!r.closest('.qsec')) r.classList.toggle('is-playing', on);
    });
    Wave.setCover(imgSrc(t.cover));
    Wave.setBpm(metaOf(t).bpm);
    kickEls = $$('[data-kick]');
  }

  function refreshLikes() {
    $$('[data-action="like"][data-id]').forEach((b) => {
      const on = state.liked.has(Number(b.dataset.id));
      b.classList.toggle('is-on', on);
      b.setAttribute('aria-pressed', on);
      b.setAttribute('aria-label', on ? 'Убрать из любимых' : 'Нравится');
      b.innerHTML = icon('heart', { fill: on });
    });
  }

  function refreshProgress() {
    const t = cur();
    const dur = t.durationMs / 1000;
    const p = state.pos / dur;
    $$('[data-scrub]').forEach((s) => {
      if (s.closest('.sheet')) return;
      s.style.setProperty('--p', `${(p * 100).toFixed(2)}%`);
      const bars = s.querySelectorAll('.scrub__bars i');
      const k = Math.floor(p * bars.length);
      bars.forEach((b, i) => b.classList.toggle('on', i < k));
      s.querySelectorAll('[data-pos]').forEach((e) => (e.textContent = fmt(state.pos)));
      s.querySelectorAll('[data-left]').forEach((e) => (e.textContent = '−' + fmt(dur - state.pos)));
      const tr = s.querySelector('.scrub__track');
      tr?.setAttribute('aria-valuenow', Math.round(state.pos));
      tr?.setAttribute('aria-valuetext', `${fmt(state.pos)} из ${fmt(dur)}`);
    });
    $$('.mini').forEach((m) => !m.classList.contains('mini--demo') && m.style.setProperty('--p', `${p * 100}%`));
    const left = transitionAt(t) - state.pos;
    $$('[data-xfade-left]').forEach((e) => (e.textContent = left > 0 ? `через ${fmt(left)}` : 'идёт сейчас'));
  }

  // ——— Действия ———
  function setPlaying(on) {
    state.playing = on;
    Wave.setPlaying(on);
    if (on) {
      const url = PREVIEW.get(state.trackId);
      if (url) Wave.playPreview(url).catch(() => {});
    } else {
      Wave.stopPreview();
    }
    refreshPlay();
  }

  function next(manual = true) {
    // Сначала то, что добавлено руками, потом поток волны; источник запоминаем для «Назад»
    const fromManual = state.manual.length > 0;
    const nid = fromManual ? state.manual.shift() : state.fromWave.shift();
    if (!nid) return;
    state.history.push({ id: state.trackId, src: state.src });
    state.trackId = nid;
    state.src = fromManual ? 'manual' : 'wave';
    state.pos = 0;
    // Волна дозаполняет себя: в конец — что-то из открытий, чтобы очередь не кончалась
    const pool = DISCOVERIES.map((d) => d.t.id).filter((id) => id !== nid && !state.fromWave.includes(id));
    if (state.fromWave.length < 6 && pool.length) state.fromWave.push(pool[state.history.length % pool.length]);
    if (manual) Wave.skip();
    refreshTrack();
    refreshProgress();
    if (state.playing) {
      Wave.stopPreview();
      const url = PREVIEW.get(nid);
      if (url) Wave.playPreview(url).catch(() => {});
    }
  }

  function prev() {
    if (state.pos > 4 || !state.history.length) { state.pos = 0; refreshProgress(); return; }
    (state.src === 'manual' ? state.manual : state.fromWave).unshift(state.trackId);
    const h = state.history.pop();
    state.trackId = h.id;
    state.src = h.src;
    state.pos = 0;
    refreshTrack();
    refreshProgress();
    if (state.playing) {
      Wave.stopPreview();
      const url = PREVIEW.get(state.trackId);
      if (url) Wave.playPreview(url).catch(() => {});
    }
  }

  let toastTimer = 0;
  let undoFn = null;
  function showToast(text, o = {}) {
    $$('.toast-host').forEach((e) => e.remove());
    const host = document.createElement('div');
    host.className = 'toast-host';
    host.innerHTML = toast(text, o);
    document.body.appendChild(host);
    undoFn = o.undo || null;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => host.remove(), (o.dur || 5) * 1000);
  }

  function setWave(patch, o = {}) {
    Object.assign(state.wave, patch);
    if (patch.mood) Wave.setMood(patch.mood);
    refreshWaveSettings();
    if (o.restart) Wave.start();
  }

  // ——— Всплывающие ———
  let popAnchor = null;
  function openPop(html, anchor, o = {}) {
    closePop();
    popAnchor = anchor;
    anchor?.setAttribute('aria-expanded', 'true');
    anchor?.classList.add('is-open');
    layer.innerHTML = `<div class="pop pop--${o.kind || 'default'}">${html}</div>`;
    const pop = layer.firstElementChild;
    const r = anchor ? anchor.getBoundingClientRect() : { left: o.x, right: o.x, top: o.y, bottom: o.y, width: 0 };
    const pw = pop.offsetWidth, ph = pop.offsetHeight;
    const vw = window.innerWidth, vh = window.innerHeight;
    let x = o.align === 'right' ? r.right - pw : r.left;
    let y = r.bottom + 8;
    if (y + ph > vh - 8) y = Math.max(8, r.top - ph - 8);
    x = clamp(x, 8, vw - pw - 8);
    pop.style.left = `${x}px`;
    pop.style.top = `${y}px`;
    requestAnimationFrame(() => pop.classList.add('is-in'));
  }
  function closePop() {
    if (!layer) return;
    layer.innerHTML = '';
    popAnchor?.setAttribute('aria-expanded', 'false');
    popAnchor?.classList.remove('is-open');
    popAnchor = null;
  }

  document.addEventListener('click', (e) => {
    // Простые элементы листа и панелей: переключатель, чипсы, сегменты, вкладки
    const sw = e.target.closest('.switch');
    if (sw && !sw.disabled) { sw.setAttribute('aria-checked', String(sw.getAttribute('aria-checked') !== 'true')); return; }
    const chip = e.target.closest('.chip');
    if (chip && !chip.disabled) { chip.classList.toggle('is-selected'); return; }
    const one = e.target.closest('.seg__opt:not([data-action]), .tab-btn');
    if (one) {
      [...one.parentElement.children].forEach((c) => { c.classList.toggle(c.classList.contains('tab-btn') ? 'is-active' : 'is-selected', c === one); c.setAttribute('aria-' + (c.getAttribute('role') === 'tab' ? 'selected' : 'checked'), String(c === one)); });
      return;
    }
    const el = e.target.closest('[data-action]');
    if (!el) {
      if (!e.target.closest('.pop')) closePop();
      const j = e.target.closest('[data-jump]');
      if (j) { e.preventDefault(); document.getElementById(j.dataset.jump)?.scrollIntoView({ behavior: 'smooth', block: 'start' }); }
      return;
    }
    const a = el.dataset.action;
    const id = Number(el.dataset.id);
    if (!el.closest('.pop') && !['tuner', 'pop', 'menu'].includes(a)) closePop();
    switch (a) {
      case 'wave-play': {
        const on = !state.playing;
        setPlaying(on);
        if (on) Wave.start();
        break;
      }
      case 'toggle': setPlaying(!state.playing); break;
      case 'next': next(true); break;
      case 'prev': prev(); break;
      case 'like': {
        const tid = id || state.trackId;
        if (state.liked.has(tid)) state.liked.delete(tid);
        else { state.liked.add(tid); if (tid === state.trackId) Wave.like(); }
        refreshLikes();
        break;
      }
      case 'dislike': {
        const t = cur();
        showToast(`«${esc(t.title)}» больше не прозвучит в волне`, { icon: 'prohibit', undo: () => showToast('Вернули в рекомендации', { icon: 'arrow-u-up-left' }) });
        next(true);
        break;
      }
      case 'more-like': showToast('Волна возьмёт больше похожего на этот трек', { icon: 'sparkle' }); break;
      case 'mood': setWave({ mood: el.dataset.mood }); break;
      case 'tuner': {
        if (popAnchor === el) { closePop(); break; }
        openPop(listbox(el.dataset.key), el, { kind: 'list' });
        break;
      }
      case 'set-wave': setWave({ [el.dataset.key]: el.dataset.val }); closePop(); break;
      case 'preset': {
        const p = PRESETS[Number(el.dataset.i)];
        setWave(p.wave, { restart: true });
        if (!state.playing) setPlaying(true);
        showToast(`Волна: ${p.name} — ${p.text}`, { icon: 'waveform' });
        break;
      }
      case 'save-preset': showToast('Сочетание сохранено — оно появится на главной', { icon: 'push-pin' }); break;
      case 'pop': {
        if (popAnchor === el) { closePop(); break; }
        const kind = el.dataset.pop;
        openPop(kind === 'devices' ? devicePicker() : tmodePicker(), el, { kind, align: el.closest('.npbar') ? 'right' : 'left' });
        break;
      }
      case 'tmode': {
        state.transition = el.dataset.mode;
        $$('.tmode__opt').forEach((o) => { const on = o.dataset.mode === state.transition; o.classList.toggle('is-selected', on); o.setAttribute('aria-checked', on); });
        const m = TMODES.find((x) => x.id === state.transition);
        $$('.tmode-chip').forEach((c) => (c.innerHTML = `${tmodeIcon(m.id)}<span>${m.name}</span>`));
        break;
      }
      case 'device': {
        state.device = el.dataset.device;
        const d = DEVICES.find((x) => x.id === state.device);
        closePop();
        showToast(`Музыка играет: ${d.name}`, { icon: d.icon });
        break;
      }
      case 'menu': {
        const t = tracks.get(id) || cur();
        if (popAnchor === el) { closePop(); break; }
        openPop(contextMenu(t), el, { kind: 'menu', align: 'right' });
        break;
      }
      case 'menu-like': state.liked.has(id) ? state.liked.delete(id) : state.liked.add(id); refreshLikes(); closePop(); break;
      case 'menu-next-up': state.manual.unshift(id); MANUAL.add(id); closePop(); showToast('Сыграет следующим', { icon: 'arrow-line-up' }); if (state.route === 'player') $('[data-slot="queue"]').innerHTML = queueBody(cur()); break;
      case 'menu-enqueue': state.manual.push(id); MANUAL.add(id); closePop(); showToast('Добавлено в очередь', { icon: 'list-plus' }); if (state.route === 'player') $('[data-slot="queue"]').innerHTML = queueBody(cur()); break;
      case 'menu-ban': closePop(); showToast('Трек больше не будет предлагаться', { icon: 'prohibit', undo: () => showToast('Вернули в рекомендации', { icon: 'arrow-u-up-left' }) }); break;
      case 'menu-download': closePop(); showToast('Скачиваем на это устройство', { icon: 'arrow-circle-down' }); break;
      case 'menu-radio': closePop(); showToast('Радио по треку — после текущего', { icon: 'broadcast' }); break;
      case 'menu-to-playlist': closePop(); showToast('Добавлено в плейлист «Тихо, работаю»', { icon: 'playlist', undo: () => showToast('Убрали из плейлиста', { icon: 'arrow-u-up-left' }) }); break;
      case 'menu-fix': closePop(); showToast('Запись отправлена в «Спорные» — админка подберёт другой источник', { icon: 'flag' }); break;
      case 'undo': {
        const fn = undoFn;
        $$('.toast-host').forEach((x) => x.remove());
        undoFn = null;
        fn?.();
        break;
      }
      case 'clear-manual': state.manual = []; $('[data-slot="queue"]').innerHTML = queueBody(cur()); showToast('Очередь очищена', { icon: 'trash', undo: () => { state.manual = [T.iwantout.id, T.fall.id]; $('[data-slot="queue"]').innerHTML = queueBody(cur()); } }); break;
      case 'open-player': location.hash = '#player'; break;
      case 'close-player': location.hash = '#home'; break;
      case 'back': history.length > 1 ? history.back() : (location.hash = '#home'); break;
      case 'queue': {
        if (state.route !== 'player') { state.queueOpen = true; location.hash = '#player'; break; }
        state.queueOpen = !state.queueOpen;
        render();
        break;
      }
      case 'lyrics': showToast('Текст песни — в следующей итерации макета', { icon: 'microphone-stage' }); break;
      case 'downloads': showToast('Сервер скачивает 2 трека: HONOR и MONTAGEM VANDELA', { icon: 'arrow-circle-down' }); break;
      case 'card-play': e.preventDefault(); setPlaying(true); Wave.start(); break;
      case 'mute': state.volume = state.volume > 0 ? 0 : 0.72; $$('.vol').forEach((v) => !v.closest('.sheet') && (v.outerHTML = volume())); break;
      // Отладка волны
      case 'w-start': Wave.setPlaying(true); Wave.start(); setPauseLabel(); break;
      case 'w-skip': Wave.skip(); break;
      case 'w-like': Wave.like(); break;
      case 'w-pause': {
        const on = !Wave.state.playing;
        if (!on) { Wave.stopPreview(); $$('.pv').forEach((p) => p.classList.remove('is-playing')); }
        Wave.setPlaying(on);
        setPauseLabel();
        break;
      }
      case 'preview': playPreviewBtn(el); break;
      default: break;
    }
  });

  document.addEventListener('contextmenu', (e) => {
    const row = e.target.closest('.trow[data-track]');
    if (!row || row.closest('.sheet')) return;
    e.preventDefault();
    const t = tracks.get(Number(row.dataset.track));
    if (t) openPop(contextMenu(t), null, { kind: 'menu', x: e.clientX, y: e.clientY });
  });

  document.addEventListener('input', (e) => {
    const el = e.target.closest('[data-input]');
    if (!el) return;
    const v = Number(el.value);
    const min = Number(el.min || 0), max = Number(el.max || 100);
    el.style.setProperty('--v', `${((v - min) / (max - min)) * 100}%`);
    switch (el.dataset.input) {
      case 'volume': state.volume = v / 100; break;
      case 'expr': state.expr = v / 100; $$('[data-expr-val]').forEach((x) => (x.textContent = `${v} %`)); break;
      case 'energy': Wave.setEnergy(v / 100); $$('[data-val="energy"]').forEach((x) => (x.textContent = (v / 100).toFixed(2).replace('.', ','))); break;
      case 'bpm': Wave.setBpm(v); $$('[data-val="bpm"]').forEach((x) => (x.textContent = `${v} уд/мин`)); break;
      default: break;
    }
  });

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closePop();
    if (e.target.closest('input, textarea')) return;
    if (e.code === 'Space' && state.route !== 'components' && !e.target.closest('button, a')) { e.preventDefault(); setPlaying(!state.playing); }
    if (e.key === 'Enter' && e.target.matches('.trow')) { state.trackId = Number(e.target.dataset.track); state.pos = 0; refreshTrack(); setPlaying(true); }
  });

  function setPauseLabel() {
    const b = $('[data-w-pause]');
    if (b) b.innerHTML = Wave.state.playing ? `${icon('pause', { fill: true })}<span>Пауза</span>` : `${icon('play', { fill: true })}<span>Играть</span>`;
  }

  async function playPreviewBtn(el) {
    const url = el.dataset.url;
    if (el.classList.contains('is-playing')) {
      Wave.stopPreview();
      $$('.pv').forEach((p) => p.classList.remove('is-playing', 'is-loading'));
      setPauseLabel();
      return;
    }
    $$('.pv').forEach((p) => p.classList.remove('is-playing', 'is-loading'));
    el.classList.add('is-loading');
    const status = $('[data-dbg-status]');
    try {
      const tempo = await Wave.playPreview(url);
      el.classList.remove('is-loading');
      el.classList.add('is-playing');
      Wave.start();
      const t = tracks.get(Number(el.dataset.id));
      if (t) Wave.setCover(imgSrc(t.cover));
      const bpmSlider = $('[data-input="bpm"]');
      if (bpmSlider) { bpmSlider.value = Math.round(tempo.bpm); bpmSlider.style.setProperty('--v', `${((tempo.bpm - 60) / 120) * 100}%`); }
      $$('[data-val="bpm"]').forEach((x) => (x.textContent = `${String(tempo.bpm).replace('.', ',')} уд/мин`));
      if (status) status.textContent = `Сетка долей по звуку: ${String(tempo.bpm).replace('.', ',')} уд/мин, первая доля — ${tempo.offset.toFixed(2).replace('.', ',')} с. Энергия и атаки — из AnalyserNode.`;
      setPauseLabel();
    } catch (err) {
      el.classList.remove('is-loading');
      if (status) status.textContent = 'Превью не загрузилось — проверьте, что сервер макетов запущен.';
    }
  }

  // ——— Такты и время ———
  // Счётчик — по реально нарисованным кадрам: на телефоне волна сама держит 30 к/с
  let fpsDraws = 0, fpsT = performance.now();
  Wave.on('frame', (S) => {
    const k = S.kick.toFixed(3);
    for (const el of kickEls) el.style.setProperty('--kick', k);
    const now = performance.now();
    if (now - fpsT > 500) {
      const fps = Math.round(((S.draws - fpsDraws) * 1000) / (now - fpsT));
      fpsDraws = S.draws; fpsT = now;
      const f = $('[data-fps]');
      if (f) f.textContent = `${fps} к/с`;
      if (state.route === 'wave' && S.manualEnergy == null) $$('[data-val="energy"]').forEach((x) => (x.textContent = `авто · ${S.energy.toFixed(2).replace('.', ',')}`));
    }
  });

  Wave.on('previewend', () => {
    $$('.pv').forEach((p) => p.classList.remove('is-playing'));
    setPauseLabel();
    // На основных экранах превью — «трек целиком»: по его концу волна сводит в следующий
    if (state.route !== 'wave' && state.playing) next(false);
  });

  setInterval(() => {
    if (!state.playing || state.route === 'wave' || state.route === 'components') return;
    const dur = cur().durationMs / 1000;
    state.pos += 0.25;
    if (state.pos >= dur) next(false);
    else refreshProgress();
  }, 250);

  // ——— Маршруты ———
  function route() {
    const r = (location.hash || '#home').slice(1);
    const known = ['home', 'player', 'wave', 'components'];
    const nr = known.includes(r) ? r : 'home';
    const first = !route.done;
    route.done = true;
    // Открыли плеер — значит, музыка уже играет; на главной волна ждёт нажатия
    if (first) state.playing = nr === 'player';
    state.route = nr;
    render();
    window.scrollTo(0, 0);
  }

  // Платформа: телефон и планшет — iOS, иначе окно macOS
  function platform() {
    const coarse = window.matchMedia('(pointer: coarse)').matches;
    const w = window.innerWidth;
    document.documentElement.dataset.platform = w <= 640 ? 'phone' : coarse ? 'tablet' : 'mac';
  }
  platform();
  let lastW = window.innerWidth;
  window.addEventListener('resize', () => {
    platform();
    closePop();
    // Перешли через границу раскладки — монтируем волну на ставший видимым холст
    if ((lastW <= 640) !== (window.innerWidth <= 640)) mountWaves();
    lastW = window.innerWidth;
  });

  window.addEventListener('hashchange', route);
  route();
})();
