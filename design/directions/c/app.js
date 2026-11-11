// DalyWave · направление C «Сцена» — живой макет.
// Данные из design/fixtures, состояние плеера, маршруты по #hash и словарь компонентов.
// Экраны собираются из одних и тех же функций-компонентов — лист #components это доказывает.
(() => {
  'use strict';

  const D = window.DW_DATA;
  const W = window.Wave;
  const app = document.getElementById('app');
  const layer = document.getElementById('layer');
  const root = document.documentElement;

  // ---------- Мелочи ----------
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
  const img = (p) => (p ? '../../fixtures/' + p : '');
  const NB = ' ';
  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const fmtTime = (ms) => {
    const s = Math.max(0, Math.floor(ms / 1000));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  };
  const fmtNum = (n) => new Intl.NumberFormat('ru-RU').format(n);
  const plural = (n, [one, few, many]) => {
    const a = n % 10, b = n % 100;
    return a === 1 && b !== 11 ? one : a >= 2 && a <= 4 && (b < 12 || b > 14) ? few : many;
  };
  const dateShort = (iso) => new Intl.DateTimeFormat('ru-RU', { day: 'numeric', month: 'short' }).format(new Date(iso + 'T12:00:00'));
  const ic = (n, cls = '') => `<i class="ph ph-${n}${cls ? ' ' + cls : ''}" aria-hidden="true"></i>`;
  const icf = (n, cls = '') => `<i class="ph-fill ph-${n}${cls ? ' ' + cls : ''}" aria-hidden="true"></i>`;
  const kbd = (keys) => `<span class="kbd">${keys.map((k) => `<kbd>${k}</kbd>`).join('')}</span>`;
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  const TODAY = new Date('2026-11-11T12:00:00');

  // ---------- Данные ----------
  const TR = new Map(D.tracks.map((t) => [t.id, t]));
  const AL = new Map(D.albums.map((a) => [a.id, a]));
  const AR = new Map(D.artists.map((a) => [a.id, a]));
  const arName = new Map(D.artists.map((a) => [a.name, a]));
  const PREV = new Map(D.previews.map((p) => [p.trackId, '../../fixtures/' + p.file]));
  const tr = (artist, title) => D.tracks.find((t) => t.artist === artist && t.title === title);
  const pic = (name) => img(arName.get(name)?.picture);
  const cover = (id) => img(AL.get(id)?.cover);
  const TYPE = { album: 'Альбом', ep: 'EP', single: 'Сингл', compile: 'Сборник' };

  const q = (artist, title, reason) => ({ t: tr(artist, title), reason });
  const CURRENT = q('Mytch', '3days', 'Похоже на Lowborn');
  const MANUAL = [q('Lowborn', 'I Want Out'), q('Rushex', 'ZAKHELO (Slowed)')];
  const WAVE_NEXT = [
    q('Sayfalse', 'ELA DANCA (Slowed)', 'Похоже на Rushex'),
    q('Booker', 'GDE PAPA LONGMIX', 'Похоже на Mzlff'),
    q('Bohnes', 'Vicious', 'Похоже на Lowborn'),
    q('Poe the Passenger', 'HOLOGRAM', 'Новое для тебя'),
    q('Пошлая Молли', 'Адская колыбельная', 'Знакомое'),
    q('Call Me Karizma', 'Float', 'Похоже на Parker The Bandit'),
    q('VOILÀ', 'Figure You Out', 'Похоже на Mytch'),
    q('Mzlff', 'свободное падение', 'Из твоих лайков'),
  ].filter((x) => x.t);

  const PLAYLISTS = [
    { id: 'tram', name: 'Ночной трамвай', count: 42, mosaic: [1134273871, 418402827, 993460401, 908412752] },
    { id: 'gym', name: 'Фонк в зал', count: 64, mosaic: [1110576112, 932378661, 1047983372, 873069602] },
    { id: 'work', name: 'Тихо, работаю', count: 118, mosaic: [573797331, 1031080042, 678933591, 1070460172] },
    { id: 'hyper', name: 'Хайпер 2026', count: 37, mosaic: [1135471731, 827674281, 875495732, 805631951] },
  ];
  const FOLLOWED = ['5opka', 'Mzlff', 'Пошлая Молли', 'Rushex', 'Sayfalse', 'Lowborn', 'Mytch', 'Parker The Bandit', 'Bohnes'];

  // Миксы: дуотон из фото артиста, цвета подобраны под жанр микса
  const MIXES = [
    { n: 1, lineup: ['Mzlff', 'Пошлая Молли', '5opka'], photo: 'Mzlff', duo: ['#1c0838', '#ff4fd8'], focus: '50% 50%' },
    { n: 2, lineup: ['Rushex', 'Sayfalse', 'DJ FKU'], photo: 'Rushex', duo: ['#250504', '#ff5a1f'], focus: '50% 20%' },
    { n: 3, lineup: ['Lowborn', 'Mytch', 'Bohnes'], photo: 'Bohnes', duo: ['#03131d', '#48d4ff'], focus: '50% 35%' },
    { n: 4, lineup: ['Parker The Bandit', 'Demi the Daredevil', 'VOILÀ'], photo: 'Demi the Daredevil', duo: ['#0f1906', '#c8f25a'], focus: '50% 35%' },
    { n: 5, lineup: ['Molchat Doma', 'МУККА', 'Электрофорез'], photo: 'Molchat Doma', duo: ['#141414', '#ece6dc'], focus: '50% 30%' },
  ];
  const NEW_RELEASES = [1138050861, 1134273871, 1135471731, 1134970871, 1070416932, 1125801282, 1110576112, 1103896852].map((id) => AL.get(id)).filter(Boolean);
  const DISCOVER = [
    { ...q('Bohnes', 'Vicious', 'Похоже на Lowborn'), state: 'down' },
    q('Poe the Passenger', 'HOLOGRAM', 'Похоже на Mytch'),
    q('Demi the Daredevil', 'Voice Notes', 'Похоже на Parker The Bandit'),
    q('DJ FKU', 'MONTAGEM ECLIPSE (Slowed)', 'Похоже на Rushex'),
    q('Электрофорез', 'По разбитым зеркалам', 'Похоже на Mzlff'),
    { ...q('Pepel Nahudi', 'осколки', 'Похоже на 5opka'), state: 'off' },
  ].filter((x) => x.t);
  const RESUME = [
    { kind: 'Альбом', title: 'PHANTASMAGORIA', sub: 'Mzlff', art: cover(827674281), p: 50, meta: '4 из 8' },
    { kind: 'Плейлист', title: 'Ночной трамвай', sub: 'Захар', mosaic: PLAYLISTS[0].mosaic, p: 35, meta: `ещё 1${NB}ч 12${NB}мин` },
    { kind: 'Радио артиста', title: 'Lowborn', sub: 'Волна по артисту', art: pic('Lowborn'), p: null, meta: 'Волна по артисту' },
    { kind: 'EP', title: 'HEAVY METAL 3', sub: '5opka', art: cover(1135471731), p: 66, meta: '2 из 3' },
  ];

  // Настройки волны — четыре группы из плана
  const OPTS = {
    activity: {
      label: 'Занятие',
      items: [
        { id: 'any', name: 'Любое', icon: 'circles-three' },
        { id: 'wake', name: 'Просыпаюсь', icon: 'sun-horizon', desc: 'Энергия постепенно растёт' },
        { id: 'road', name: 'В дороге', icon: 'train' },
        { id: 'work', name: 'Работаю', icon: 'laptop' },
        { id: 'train', name: 'Тренируюсь', icon: 'person-simple-run', desc: `Темп бега 170${NB}шагов в минуту` },
        { id: 'rest', name: 'Отдыхаю', icon: 'couch' },
        { id: 'sleep', name: 'Засыпаю', icon: 'moon-stars', desc: 'Энергия плавно снижается' },
        { id: 'party', name: 'Вечеринка', icon: 'confetti' },
      ],
    },
    character: {
      label: 'Характер',
      items: [
        { id: 'fav', name: 'Любимое', desc: 'То, что ты лайкал и слушаешь часто' },
        { id: 'mixed', name: 'Знакомое и новое', desc: 'Две трети знакомого, треть открытий' },
        { id: 'new', name: 'Незнакомое', desc: 'Ничего из прослушанного' },
        { id: 'popular', name: 'Популярное', desc: 'Главное у похожих артистов' },
        { id: 'rare', name: 'Редкое', desc: 'Глубокие треки и малоизвестные имена' },
      ],
    },
    mood: {
      label: 'Настроение',
      items: W.MOOD_ORDER.map((id) => ({ id, name: W.MOODS[id].name })),
    },
    lang: {
      label: 'Язык',
      items: [
        { id: 'any', name: 'Любой' },
        { id: 'ru', name: 'Русский' },
        { id: 'foreign', name: 'Иностранный' },
        { id: 'none', name: 'Без слов', desc: 'Инструментал и вокал без текста' },
      ],
    },
  };
  const PRESETS = [
    { id: 'work', name: 'Работа', sub: 'спокойное, без слов', set: { activity: 'work', character: 'mixed', mood: 'calm', lang: 'none' } },
    { id: 'run', name: 'Пробежка', sub: 'бодрое, темп 170', set: { activity: 'train', character: 'fav', mood: 'energetic', lang: 'any' } },
    { id: 'night', name: 'Ночь', sub: 'грустное, редкое', set: { activity: 'rest', character: 'rare', mood: 'sad', lang: 'any' } },
  ];
  const MODES = [
    { id: 'gapless', name: 'Без пауз', icon: 'arrows-merge', desc: 'Треки стыкуются вплотную', before: 0 },
    { id: 'crossfade', name: 'Плавный', icon: 'wave-sine', desc: `Наложение на 8${NB}секунд`, before: 8000 },
    { id: 'smart', name: 'Умный', icon: 'sparkle', desc: 'После вокала, без наложения голоса на голос', before: 14000 },
    { id: 'dj', name: 'Диджей', icon: 'faders', desc: 'Сведение в темп, по долям и фразам', before: 32000 },
  ];
  const DEVICES = {
    mac: { name: 'Этот Mac', sub: 'MacBook Pro · динамики', icon: 'laptop' },
    iphone: { name: 'iPhone Захара', sub: 'В сети', icon: 'device-mobile' },
    ipad: { name: 'iPad', sub: 'Не в сети · был вчера', icon: 'device-tablet', off: true },
    web: { name: 'wave.daly.rest', sub: 'Chrome · Windows', icon: 'globe' },
  };

  // ---------- Состояние ----------
  const S = {
    route: 'home',
    layout: 'desktop',
    platform: 'macos',
    wave: { activity: 'work', character: 'mixed', mood: 'calm', lang: 'any' },
    playing: false,
    current: CURRENT,
    pos: 72000,
    manual: [...MANUAL],
    next: [...WAVE_NEXT],
    history: [],
    liked: new Set([MANUAL[0].t.id, WAVE_NEXT[WAVE_NEXT.length - 1].t.id]),
    volume: 0.72,
    transition: 'dj',
    expressive: 0.6,
    device: 'mac',
    panel: false,
    panelTab: 'queue',
    repeat: false,
  };
  W.setVolume(S.volume);

  const label = (group, id) => OPTS[group].items.find((x) => x.id === id)?.name ?? '';
  // Сводка — фраза о том, какой будет волна, а не повтор значений пульта
  function summary(w = S.wave) {
    const mood = { energetic: 'Бодрая', cheerful: 'Весёлая', calm: 'Спокойная', sad: 'Грустная' }[w.mood];
    const act = { any: '', wake: ' для бодрого утра', road: ' в дорогу', work: ' для работы', train: ' для тренировки', rest: ' для отдыха', sleep: ' перед сном', party: ' для вечеринки' }[w.activity];
    const chr = { fav: 'только любимое', mixed: 'знакомое вперемешку с новым', new: 'только то, что ты ещё не слышал', popular: 'главное у похожих артистов', rare: 'редкие треки и малоизвестные имена' }[w.character];
    const lang = { any: 'на любом языке', ru: 'на русском', foreign: 'на иностранных языках', none: 'без слов' }[w.lang];
    return `${mood} волна${act}: ${chr}, ${lang}.`;
  }
  const settingsText = () => [label('activity', S.wave.activity), label('character', S.wave.character).toLowerCase(), { any: '', ru: 'на русском', foreign: 'на иностранном', none: 'без слов' }[S.wave.lang]].filter(Boolean).join(' · ');
  const presetActive = (p) => Object.entries(p.set).every(([k, v]) => S.wave[k] === v);
  const modeOf = (id = S.transition) => MODES.find((m) => m.id === id);
  const markAt = (t = S.current.t) => clamp((t.durationMs - modeOf().before) / t.durationMs, 0, 1);
  const isNew = (iso) => (TODAY - new Date(iso + 'T12:00:00')) / 864e5 <= 7;

  // ---------- Компоненты ----------
  // Каждая функция возвращает разметку; состояния для листа компонентов — через opts.state.
  const st = (s) => (s ? ' is-' + s : '');

  function btn({ label: text, icon, iconFill, variant = 'secondary', size = '', act = '', state = '', attrs = '' }) {
    return `<button class="btn btn--${variant}${size ? ' btn--' + size : ''}${st(state)}"${act ? ` data-act="${act}"` : ''}${state === 'disabled' ? ' disabled' : ''} ${attrs}>${icon ? ic(icon) : ''}${iconFill ? icf(iconFill) : ''}${text ? `<span>${text}</span>` : ''}</button>`;
  }
  function iconBtn({ icon, fill = false, label: aria, act = '', size = 's', state = '', on = false, tip = '', keys = '', attrs = '' }) {
    return `<button class="icon-btn${size ? ' icon-btn--' + size : ''}${st(state)}${on ? ' is-on' : ''}" aria-label="${aria}"${act ? ` data-act="${act}"` : ''}${tip ? ` data-tip="${tip}"` : ''}${keys ? ` data-kbd="${keys}"` : ''}${state === 'disabled' ? ' disabled' : ''} ${attrs}>${fill ? icf(icon) : ic(icon)}</button>`;
  }
  function playBtn({ playing = S.playing, size = '', state = '', act = 'play', tip = true } = {}) {
    return `<button class="play-btn${size ? ' play-btn--' + size : ''}${st(state)}" data-act="${act}" aria-label="${playing ? 'Пауза' : 'Слушать'}"${tip ? ` data-tip="${playing ? 'Пауза' : 'Слушать'}" data-kbd="Space"` : ''}>${icf(playing ? 'pause' : 'play')}</button>`;
  }
  function wavePlay({ playing = S.playing, size = '', state = '', act = 'wave-play' } = {}) {
    const cls = `wave-play${size ? ' wave-play--' + size : ''}${playing ? ' is-playing' : ''}${st(state)}`;
    return `<div class="${cls}" data-pulse><span class="wave-play__ring"></span><button class="wave-play__btn" data-act="${act}" aria-label="${playing ? 'Пауза' : 'Слушать «Мою волну»'}">${icf(playing ? 'pause' : 'play', playing ? '' : 'ph-play')}</button></div>`;
  }
  const moodDot = (id, size = '') => `<span class="mood-dot${size ? ' mood-dot--' + size : ''}" data-mood="${id}" aria-hidden="true"></span>`;

  function consoleSel(state = {}) {
    const cell = (g) => {
      const v = S.wave[g];
      return `<button class="console__cell${state[g] ? ' is-' + state[g] : ''}" data-act="pick" data-group="${g}" aria-haspopup="menu" aria-expanded="${state[g] === 'open'}">
        <span class="console__label">${OPTS[g].label}</span>
        <span class="console__value">${g === 'mood' ? moodDot(v) : ''}${label(g, v)}</span>${ic('caret-down')}</button>`;
    };
    return `<div class="console" role="group" aria-label="Настройки волны">${['activity', 'character', 'mood', 'lang'].map(cell).join('')}</div>`;
  }
  function moodSel({ value = S.wave.mood, row = false, state = {} } = {}) {
    return `<div class="mood-sel${row ? ' mood-sel--row' : ''}" role="radiogroup" aria-label="Настроение">${W.MOOD_ORDER.map((id) =>
      `<button class="mood-opt${state[id] ? ' is-' + state[id] : ''}" role="radio" aria-checked="${id === value}" data-act="set-wave" data-group="mood" data-value="${id}">${moodDot(id, 'l')}<span>${W.MOODS[id].name}</span></button>`).join('')}</div>`;
  }
  function presetBtn(p, state = '') {
    return `<button class="preset${st(state)}" data-act="preset" data-id="${p.id}" aria-pressed="${state === 'selected' || (!state && presetActive(p))}">${moodDot(p.set.mood)}<span class="preset__name">${p.name}</span><span class="preset__sub">${p.sub}</span></button>`;
  }
  function seg(options, value, { act = 'seg', name = '' } = {}) {
    return `<div class="seg" role="radiogroup"${name ? ` aria-label="${name}"` : ''}>${options.map((o) =>
      `<button class="seg__opt${st(o.state)}" role="radio" aria-checked="${o.id === value}" data-act="${act}" data-value="${o.id}"${o.disabled ? ' disabled' : ''}>${o.icon ? o.icon : ''}${o.name}</button>`).join('')}</div>`;
  }
  function tabs(items, value, act = 'tab') {
    return `<div class="tabs" role="tablist">${items.map((t) =>
      `<button class="tab${st(t.state)}" role="tab" aria-selected="${t.id === value}" data-act="${act}" data-value="${t.id}">${t.name}${t.count != null ? `<span class="tab__count">${t.count}</span>` : ''}</button>`).join('')}</div>`;
  }
  const chip = (text, { pressed = false, icon = '', state = '' } = {}) =>
    `<button class="chip${st(state)}" aria-pressed="${pressed}"${state === 'disabled' ? ' disabled' : ''}>${icon ? ic(icon) : ''}${text}</button>`;
  const switchEl = (text, on, { disabled = false, act = 'switch' } = {}) =>
    `<button class="switch" role="switch" aria-checked="${on}" data-act="${act}"${disabled ? ' disabled' : ''}><span class="switch__track"></span>${text}</button>`;

  const badge = {
    quality: (t = 'Opus 160') => `<span class="badge badge--quality">${t}</span>`,
    explicit: () => `<span class="badge badge--explicit" title="Ненормативная лексика" aria-label="Ненормативная лексика">E</span>`,
    down: () => `<span class="badge badge--down" data-tip="Скачано на это устройство">${icf('check-circle')}<span class="sr">Скачано</span></span>`,
    cloud: (text = 'Скачается при запуске') => `<span class="badge badge--cloud" data-tip="Нет на сервере">${ic('cloud-arrow-down')}${text}</span>`,
    fresh: () => `<span class="badge badge--new">Новое</span>`,
  };
  const ring = (p, { size = '', done = false, icon = '' } = {}) =>
    `<span class="ring-wrap"><span class="ring${size ? ' ring--' + size : ''}${done ? ' is-done' : ''}" style="--p:${p}" role="progressbar" aria-valuenow="${p}" aria-valuemin="0" aria-valuemax="100"></span>${icon}</span>`;
  const reasonEl = (text) => (text ? `<span class="reason" title="${esc(text)}">${icf('sparkle')}<span>${esc(text).replace(' на ', ' на' + NB)}</span></span>` : '');

  function art(src, alt = '') {
    return src ? `<img src="${src}" alt="${esc(alt)}" loading="lazy" decoding="async">` : '';
  }
  function mosaic(ids) {
    return `<div class="mosaic">${ids.map((id) => `<img src="${cover(id)}" alt="" loading="lazy">`).join('')}</div>`;
  }

  function trackRow(item, { i = 1, state = '', compact = false, col = 'reason' } = {}) {
    const t = item.t;
    const playing = state === 'playing' || (!state && S.current.t.id === t.id);
    const s = state || item.state || '';
    const cls = `track${compact ? ' track--compact' : ''}${playing ? ' is-playing' : ''}${s && s !== 'playing' ? ' is-' + s : ''}`;
    // Состояние файла — рядом с артистом: скачано (зелёная стрелка) или «скачается при запуске»
    const dl = s === 'down' ? `<span class="track__dl" data-tip="Скачано на это устройство">${icf('arrow-circle-down')}<span class="sr">Скачано</span></span>` : '';
    const phone = S.layout === 'phone';
    const off = s === 'off' ? `<span class="track__flag" data-tip="Нет на сервере — скачается при запуске">${ic('cloud-arrow-down')}<span>${phone ? 'нет на сервере' : 'скачается при запуске'}</span></span>` : '';
    const why = phone && !off && item.reason ? `<span class="track__dot">·</span>${reasonEl(item.reason)}` : '';
    const eq = `<span class="eq${S.playing || state === 'playing' ? '' : ' is-paused'}" aria-hidden="true"><i></i><i></i><i></i></span>`;
    if (compact) {
      return `<div class="${cls}" tabindex="0" data-track="${t.id}" data-ctx="track:${t.id}" role="listitem">
        <div class="track__art">${art(img(t.cover))}</div>
        <div class="track__main"><div class="track__title"><span>${esc(t.title)}</span>${t.explicit ? badge.explicit() : ''}</div>
          <div class="track__sub">${playing ? eq : ''}${dl}<span class="track__artist">${esc(t.artist)}</span>${item.reason ? `<span class="track__dot">·</span>${reasonEl(item.reason)}` : ''}</div></div>
        <div class="track__dur">${fmtTime(t.durationMs)}</div></div>`;
    }
    return `<div class="${cls}" tabindex="0" data-track="${t.id}" data-ctx="track:${t.id}" role="listitem">
      <div class="track__idx"><span class="track__n">${i}</span>${icf('play')}${eq}</div>
      <div class="track__art">${art(img(t.cover))}</div>
      <div class="track__main"><div class="track__title"><span>${esc(t.title)}</span>${t.explicit ? badge.explicit() : ''}</div><div class="track__sub">${dl}<span class="track__artist">${esc(t.artist)}</span>${off}${why}</div></div>
      <div class="track__col">${col === 'reason' && item.reason ? reasonEl(item.reason) : esc(t.album ?? '')}</div>
      <div class="track__dur">${fmtTime(t.durationMs)}</div>
      ${iconBtn({ icon: 'dots-three', label: 'Ещё', act: 'ctx', attrs: `data-ctx-for="track:${t.id}"` }).replace('class="icon-btn', 'class="icon-btn track__more')}
    </div>`;
  }

  function albumCard(a, { state = '' } = {}) {
    const fresh = isNew(a.releaseDate);
    return `<div class="card${st(state)}" data-ctx="album:${a.id}" style="--gel:var(--c-line-3)" data-gel="${img(a.cover)}">
      <div class="card__art">${art(img(a.cover), a.title)}${fresh ? `<span class="card__badge">${badge.fresh()}</span>` : ''}
        ${playBtnCard(`Слушать «${esc(a.title)}»`)}</div>
      <a class="card__open" href="#home" data-act="soon" data-what="Страница альбома" aria-label="${esc(a.title)} — ${esc(a.artist)}"></a>
      <div class="card__title"><span>${esc(a.title)}</span>${a.explicit ? badge.explicit() : ''}</div>
      <div class="card__meta">${esc(a.artist)}${S.layout === 'phone' ? '' : ' · ' + (TYPE[a.type] ?? 'Альбом')} · ${dateShort(a.releaseDate)}</div>
    </div>`;
  }
  const playBtnCard = (aria) => `<button class="play-btn card__play" data-act="play-ctx" aria-label="${aria}">${icf('play')}</button>`;
  function artistCard(name, { state = '' } = {}) {
    const a = arName.get(name);
    return `<div class="card card--artist${st(state)}" data-ctx="artist:${a.id}" data-gel="${img(a.picture)}" style="--gel:var(--c-line-3)">
      <div class="card__art">${art(img(a.picture), name)}${playBtnCard(`Радио ${esc(name)}`)}</div>
      <a class="card__open" href="#home" data-act="soon" data-what="Страница артиста" aria-label="${esc(name)}"></a>
      <div class="card__title"><span>${esc(name)}</span></div>
      <div class="card__meta">${fmtNum(a.fans)} ${plural(a.fans, ['слушатель', 'слушателя', 'слушателей'])} в Deezer</div>
    </div>`;
  }
  function mixCard(m, { state = '' } = {}) {
    // Хедлайнер набирается так крупно, как позволяет длина имени
    const L = m.lineup[0].length;
    const hl = L <= 6 ? 17 : L <= 9 ? 14 : L <= 13 ? 11 : 8.2;
    const more = m.lineup[0].length + m.lineup[1].length > 26 ? m.lineup[0] : m.lineup.slice(0, 2).join(', ');
    return `<div class="card card--mix${st(state)}" data-ctx="mix:${m.n}" style="--gel:${m.duo[1]}">
      <div class="card__art">
        <div class="mix-cover" style="--duo-a:${m.duo[0]};--duo-b:${m.duo[1]};--hl:${hl}cqw;--focus:${m.focus}">
          <img src="${pic(m.photo)}" alt="" loading="lazy" decoding="async">
          <div class="mix-cover__tone"></div><div class="mix-cover__scrim"></div>
          <div class="mix-cover__head"><span>Микс дня</span><span class="mix-cover__n">${String(m.n).padStart(2, '0')}</span></div>
          <div class="mix-cover__lineup">${m.lineup.map((n) => `<span>${esc(n)}</span>`).join('')}</div>
        </div>
        ${playBtnCard(`Слушать «Микс дня ${m.n}»`)}
      </div>
      <a class="card__open" href="#home" data-act="soon" data-what="Микс дня" aria-label="Микс дня ${m.n}"></a>
      <div class="card__title"><span>Микс дня ${m.n}</span></div>
      <div class="card__meta">${esc(more)} и${NB}др.</div>
    </div>`;
  }
  function resumeCard(r, { state = '' } = {}) {
    const kind = r.kind === 'Радио артиста' ? r.kind : `${r.kind} · ${r.sub}`;
    return `<button class="resume${st(state)}" data-act="soon" data-what="Продолжение">
      <span class="resume__art">${r.mosaic ? mosaic(r.mosaic) : art(r.art)}${icf('play')}</span>
      <span class="resume__text">
        <span class="resume__kind">${esc(kind)}</span>
        <span class="resume__title">${esc(r.title)}</span>
        <span class="resume__meta">${r.p != null ? `<span class="resume__bar" style="--p:${r.p}"><i></i></span>` : ic('waveform')}<span>${r.meta}</span></span>
      </span></button>`;
  }
  function shelfHead(title, { sub = '', all = true, arrows = false, id = '' } = {}) {
    return `<header class="shelf__head"><div class="shelf__titles"><h2 class="shelf__title"${id ? ` id="${id}"` : ''}>${title}</h2>${sub ? `<p class="shelf__sub">${sub}</p>` : ''}</div>
      <div class="shelf__tools">${all ? `<a class="link" href="#home" data-act="soon" data-what="${title}">Все</a>` : ''}${arrows ? iconBtn({ icon: 'caret-left', label: 'Назад', act: 'rail', attrs: 'data-dir="-1"' }) + iconBtn({ icon: 'caret-right', label: 'Вперёд', act: 'rail', attrs: 'data-dir="1"' }) : ''}</div></header>`;
  }

  // Форма волны трека: одинаковая для одного трека, громче в припевах
  const wfCache = new Map();
  function waveform(id, n) {
    const key = id + ':' + n;
    if (wfCache.has(key)) return wfCache.get(key);
    let s = (id % 2147483646) + 1;
    const rnd = () => (s = (s * 16807) % 2147483647) / 2147483647;
    const out = [];
    for (let i = 0; i < n; i++) {
      const x = i / n;
      const song = 0.5 + 0.5 * Math.sin(x * Math.PI * 4.3 + 1.1) ** 2;
      const env = Math.min(1, x * 8 + 0.15) * Math.min(1, (1 - x) * 6 + 0.1);
      out.push(Math.max(0.1, (0.3 + 0.7 * song) * env * (0.55 + 0.45 * rnd())));
    }
    wfCache.set(key, out);
    return out;
  }
  function scrubber({ t = S.current.t, pos = S.pos, bars = 110, state = '', mark = true, id = '' } = {}) {
    const wf = waveform(t.id, bars);
    const d = wf.map((v, i) => `M${i * 4 + 2} ${16 - v * 14}V${16 + v * 14}`).join('');
    const svg = (cls) => `<svg class="${cls}" viewBox="0 0 ${bars * 4} 32" preserveAspectRatio="none" aria-hidden="true"><path d="${d}" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" vector-effect="non-scaling-stroke"/></svg>`;
    const p = clamp(pos / t.durationMs);
    return `<div class="scrub${st(state)}" style="--p:${p};--mark:${markAt(t)};--h:${state === 'hover' ? 0.72 : 0}" data-scrub${id ? ` id="${id}"` : ''} role="slider" tabindex="0" aria-label="Позиция в треке" aria-valuemin="0" aria-valuemax="${Math.round(t.durationMs / 1000)}" aria-valuenow="${Math.round(pos / 1000)}" aria-valuetext="${fmtTime(pos)} из ${fmtTime(t.durationMs)}">
      <span class="scrub__hover"></span>${mark ? '<span class="scrub__tail"></span>' : ''}${svg('scrub__base')}${svg('scrub__played')}${mark ? `<span class="scrub__mark" data-tip="${modeOf().name}: переход в ${fmtTime(t.durationMs - modeOf().before)}"></span>` : ''}<span class="scrub__head"></span></div>`;
  }
  function range(v, { act = 'volume', label: aria = 'Громкость', state = '' } = {}) {
    return `<div class="range${st(state)}" style="--v:${Math.round(v * 100)}" role="slider" tabindex="0" aria-label="${aria}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${Math.round(v * 100)}" data-range="${act}"><div class="range__track"><div class="range__fill"></div><div class="range__knob"></div></div></div>`;
  }
  function volumeEl(v = S.volume, state = '') {
    const icon = v === 0 ? 'speaker-simple-x' : v < 0.4 ? 'speaker-simple-low' : 'speaker-simple-high';
    return `<div class="volume">${iconBtn({ icon, label: v === 0 ? 'Включить звук' : 'Выключить звук', act: 'mute' })}${range(v, { state })}</div>`;
  }
  function modePill(state = '') {
    const m = modeOf();
    return `<button class="mode-pill${st(state)}" data-act="transition" aria-haspopup="menu" aria-expanded="false" data-tip="Переходы между треками">${ic(m.icon)}${m.name}</button>`;
  }

  function menu(items, { title = '' } = {}) {
    return `${title ? `<div class="pop__title t-overline">${title}</div>` : ''}${items.map((it) => {
      if (it === '-') return '<div class="pop__sep" role="separator"></div>';
      const role = it.checked != null ? 'menuitemradio' : 'menuitem';
      return `<button class="menu-item${it.danger ? ' menu-item--danger' : ''}${it.desc ? ' menu-item--tall' : ''}${st(it.state)}" role="${role}"${it.checked != null ? ` aria-checked="${it.checked}"` : ''} data-act="${it.act || 'menu'}"${it.value != null ? ` data-value="${it.value}"` : ''}${it.group ? ` data-group="${it.group}"` : ''}${it.disabled ? ' disabled' : ''}>
        ${it.dot ? moodDot(it.dot, 'l') : it.icon ? (it.fill ? icf(it.icon) : ic(it.icon)) : '<span></span>'}<span>${it.name}</span>
        ${it.desc ? `<span class="menu-item__desc">${it.desc}</span>` : ''}
        <span class="menu-item__end">${it.end || ''}${it.checked ? ic('check') : ''}${it.sub ? ic('caret-right') : ''}</span></button>`;
    }).join('')}`;
  }
  function transitionMenu() {
    const items = MODES.map((m) => ({ icon: m.icon, name: m.name, desc: m.desc, checked: S.transition === m.id, act: 'set-mode', value: m.id }));
    return `${menu(items, { title: 'Переходы между треками' })}
      <div class="pop__sep"></div>
      <div style="padding:8px 10px 10px;display:grid;gap:10px">
        <div style="display:flex;justify-content:space-between"><span class="t-small">Выразительность</span><span class="t-caption">${S.transition === 'dj' ? 'заметно' : 'для «Диджея»'}</span></div>
        ${range(S.expressive, { act: 'expressive', label: 'Выразительность переходов' })}
        <div style="display:flex;justify-content:space-between" class="t-caption"><span>незаметно</span><span>эффектно</span></div>
      </div>`;
  }
  function devicesMenu() {
    const items = Object.entries(DEVICES).map(([id, d]) => ({
      icon: d.icon, name: d.name, desc: id === S.device ? 'Играет здесь' : d.sub, checked: id === S.device, act: 'set-device', value: id, disabled: d.off,
    }));
    return menu(items, { title: 'Где играть' }) + `<div class="pop__sep"></div><div class="t-caption" style="padding:6px 10px 8px;display:flex;gap:8px;align-items:center">${ic('broadcast')}Управление идёт через сервер</div>`;
  }
  function ctxMenu(kind) {
    const base = [
      { icon: 'arrow-bend-down-right', name: 'Играть следующим', act: 'ctx-next' },
      { icon: 'list-plus', name: 'В очередь', act: 'ctx-queue' },
      { icon: 'playlist', name: 'В плейлист', sub: true, act: 'soon' },
      { icon: 'radio', name: kind === 'artist' ? 'Радио артиста' : 'Волна по треку', act: 'ctx-radio' },
      '-',
      { icon: 'download-simple', name: 'Скачать сюда', end: kbd(['⌘', 'S']), act: 'ctx-download' },
      { icon: 'share-network', name: 'Поделиться', act: 'soon' },
      '-',
      { icon: 'warning-circle', name: 'Неверная запись — исправить', act: 'soon' },
      { icon: 'prohibit', name: 'Не рекомендовать', danger: true, act: 'ctx-ban' },
    ];
    return menu(base);
  }
  function toastHtml(text, { icon = 'check-circle', undo = true, stat = false } = {}) {
    return `<div class="toast${stat ? ' toast--static' : ''}" role="status">${icf(icon)}<span class="toast__text">${text}</span>${undo ? btn({ label: 'Отменить', size: 's', act: 'undo' }) : ''}${iconBtn({ icon: 'x', label: 'Закрыть', act: 'toast-close' })}<i class="toast__time"></i></div>`;
  }
  function emptyState({ title, text, action }) {
    return `<div class="empty"><svg class="empty__art" viewBox="0 0 100 100" aria-hidden="true" style="color:var(--c-text-3)"><use href="#i-glyph"/></svg><div class="t-title-s">${title}</div><p class="t-small">${text}</p>${action ? btn({ label: action, size: 's' }) : ''}</div>`;
  }
  function searchField({ value = '', state = '', placeholder = 'Трек, артист или «спокойный инструментал»' } = {}) {
    return `<label class="search${st(state)}">${ic('magnifying-glass')}<input type="search" value="${esc(value)}" placeholder="${placeholder}" aria-label="Поиск">${value ? iconBtn({ icon: 'x-circle', label: 'Очистить' }) : kbd(['⌘', 'K'])}</label>`;
  }
  const tipEl = (text, keys) => `<span class="tip tip--static" role="tooltip">${text}${keys ? kbd(keys) : ''}</span>`;
  const logo = () => `<a class="logo" href="#home" aria-label="DalyWave — на главную"><svg class="logo__glyph" viewBox="0 0 100 100" aria-hidden="true"><use href="#i-glyph"/></svg><span class="logo__word">DalyWave</span></a>`;

  // ---------- Каркас ----------
  function traffic() {
    return `<div class="traffic" aria-hidden="true"><i></i><i></i><i></i></div>`;
  }
  function sidebar() {
    const r = S.route;
    const libRow = (artHtml, title, meta, { round = false, act = 'soon', what = '', end = '' } = {}) =>
      `<button class="lib-row${round ? ' lib-row--artist' : ''}" data-act="${act}" data-what="${what}" data-tip="${esc(title)}" data-tip-when="rail"><span class="lib-row__art${round ? ' lib-row__art--round' : ''}">${artHtml}</span><span class="lib-row__text"><span class="lib-row__title">${esc(title)}</span>${meta ? `<span class="lib-row__meta">${meta}</span>` : ''}</span><span class="lib-row__end">${end}</span></button>`;
    return `<aside class="sidebar" aria-label="Навигация">
      <div class="sidebar__top drag" data-tauri-drag-region>${S.platform === 'macos' ? traffic() : ''}</div>
      ${logo()}
      <nav class="nav" aria-label="Разделы">
        <a class="nav__item" href="#home"${r === 'home' ? ' aria-current="page"' : ''}>${r === 'home' ? icf('house') : ic('house')}<span>Главная</span></a>
        <button class="nav__item" data-act="soon" data-what="Поиск">${ic('magnifying-glass')}<span>Поиск</span>${kbd(['⌘', 'K'])}</button>
        <button class="nav__item" data-act="soon" data-what="Медиатека">${ic('books')}<span>Медиатека</span></button>
      </nav>
      <div class="sidebar__scroll">
        <div class="sidebar__label"><span class="t-overline">Закреплено</span>${iconBtn({ icon: 'plus', label: 'Новый плейлист', act: 'soon', tip: 'Новый плейлист' })}</div>
        ${libRow(`<span class="liked-art" style="display:grid;width:100%;height:100%">${icf('heart')}</span>`, 'Любимые треки', `${fmtNum(1248)} треков`, { what: 'Любимые треки' })}
        ${PLAYLISTS.map((p) => libRow(mosaic(p.mosaic), p.name, `${p.count} ${plural(p.count, ['трек', 'трека', 'треков'])}`, { what: 'Плейлист' })).join('')}
        <hr>
        <div class="sidebar__label"><span class="t-overline">Артисты</span><a class="link" href="#home" data-act="soon" data-what="Артисты">Все 24</a></div>
        ${FOLLOWED.slice(0, 7).map((n) => libRow(art(pic(n)), n, '', { round: true, what: 'Страница артиста' })).join('')}
      </div>
      <div class="sidebar__foot" data-tip="Сервер скачивает: MONTAGEM VANDELA, Leverage, HEAVY METAL 3" data-tip-when="rail">${ring(62)}<span>Качаю на сервер: <b>3 трека</b></span></div>
    </aside>`;
  }
  function topbar() {
    const back = S.route === 'player';
    return `<div class="topbar drag" data-tauri-drag-region>
      ${iconBtn({ icon: 'caret-left', label: 'Назад', act: back ? 'go-home' : 'soon', state: back ? '' : 'disabled', attrs: back ? 'data-tip="Назад"' : '' })}
      ${iconBtn({ icon: 'caret-right', label: 'Вперёд', act: 'soon', state: 'disabled' })}
      <div class="topbar__spacer"></div>
      <button class="avatar" data-act="soon" data-what="Профиль" aria-label="Профиль: Захар">З</button>
    </div>`;
  }
  function npInner() {
    const c = S.current, t = c.t, liked = S.liked.has(t.id);
    return `<div class="np-track">
        <a class="np-track__art" href="#player" aria-label="Открыть плеер">${art(img(t.cover), t.album)}</a>
        <div class="np-track__text"><div class="np-track__title"><a href="#player">${esc(t.title)}</a>${t.explicit ? badge.explicit() : ''}</div>
          <div class="np-track__artist"><a href="#home" data-act="soon" data-what="Страница артиста">${esc(t.artist)}</a></div></div>
        ${iconBtn({ icon: 'heart', fill: liked, label: liked ? 'Убрать из любимых' : 'Нравится', act: 'like', state: liked ? 'liked' : '', tip: liked ? 'Убрать из любимых' : 'Нравится', keys: 'L', attrs: `aria-pressed="${liked}"` })}
      </div>
      <div class="np-center">
        <div class="transport">
          ${iconBtn({ icon: 'thumbs-down', label: 'Не нравится', act: 'dislike', tip: 'Не нравится — пропустить' })}
          ${iconBtn({ icon: 'skip-back', fill: true, label: 'Предыдущий', act: 'prev', tip: 'Предыдущий', keys: '⌘ ←' })}
          ${playBtn()}
          ${iconBtn({ icon: 'skip-forward', fill: true, label: 'Следующий', act: 'next', tip: 'Следующий', keys: '⌘ →' })}
          ${iconBtn({ icon: S.repeat ? 'repeat-once' : 'repeat', label: 'Повтор', act: 'repeat', tip: 'Повторять трек', on: S.repeat, attrs: `aria-pressed="${S.repeat}"` })}
        </div>
        <div class="scrub-row"><span class="scrub-row__t" data-t="pos">${fmtTime(S.pos)}</span>${scrubber()}<span class="scrub-row__t">${fmtTime(t.durationMs)}</span></div>
      </div>
      <div class="np-right">
        ${modePill()}
        ${iconBtn({ icon: 'microphone-stage', label: 'Текст', act: 'panel-tab', tip: 'Текст', attrs: 'data-value="lyrics"', on: S.panel && S.panelTab === 'lyrics' }).replace('class="icon-btn', 'class="icon-btn hide-tablet')}
        ${iconBtn({ icon: 'queue', label: 'Очередь', act: 'panel-tab', tip: 'Очередь', attrs: `data-value="queue" aria-pressed="${S.panel && S.panelTab === 'queue'}"`, on: S.panel && S.panelTab === 'queue' })}
        ${iconBtn({ icon: 'speaker-hifi', label: 'Устройства', act: 'devices', tip: 'Где играть', attrs: 'aria-haspopup="menu" aria-expanded="false"' })}
        ${volumeEl()}
      </div>`;
  }
  function miniInner() {
    const t = S.current.t;
    return `<a class="mini__open" href="#player" aria-label="Открыть плеер: ${esc(t.title)}"></a>
      <div class="mini__art">${art(img(t.cover))}</div>
      <div class="mini__text"><div class="mini__title">${esc(t.title)}</div><div class="mini__sub">${esc(t.artist)} · Моя волна</div></div>
      ${iconBtn({ icon: S.playing ? 'pause' : 'play', fill: true, size: '', label: S.playing ? 'Пауза' : 'Слушать', act: 'play', attrs: 'style="position:relative;z-index:1"' })}
      ${iconBtn({ icon: 'skip-forward', fill: true, size: '', label: 'Следующий', act: 'next', attrs: 'style="position:relative;z-index:1"' })}
      <div class="mini__bar"><i data-pbar></i></div>`;
  }
  function phoneChrome() {
    const r = S.route;
    const tab = (id, icon, name, act = '') =>
      `<${act ? 'button' : 'a href="#' + id + '"'} class="tabbar__item"${r === id ? ' aria-current="page"' : ''}${act ? ` data-act="${act}" data-what="${name}"` : ''}>${r === id ? icf(icon) : ic(icon)}<span>${name}</span></${act ? 'button' : 'a'}>`;
    return `<div class="ios-status" aria-hidden="true"><span class="ios-status__time">9:41</span><span class="ios-status__icons"><svg width="18" height="12"><use href="#i-signal"/></svg><svg width="16" height="12"><use href="#i-wifi"/></svg><svg width="27" height="13"><use href="#i-battery"/></svg></span></div>
      <div class="mini" id="mini" style="--p:${S.pos / S.current.t.durationMs}">${miniInner()}</div>
      <nav class="tabbar" aria-label="Разделы">${tab('home', 'house', 'Главная')}${tab('search', 'magnifying-glass', 'Поиск', 'soon')}${tab('library', 'books', 'Медиатека', 'soon')}</nav>
      <div class="home-ind" aria-hidden="true"></div>`;
  }
  function ipadStatus() {
    return `<div class="ipad-status" aria-hidden="true"><div>9:41 <span style="font-weight:500">Ср 11 нояб.</span></div><div><svg width="16" height="12"><use href="#i-wifi"/></svg><span style="font-weight:500">100 %</span><svg width="27" height="13"><use href="#i-battery"/></svg></div></div>`;
  }
  function panelHtml() {
    const tabsList = [
      { id: 'queue', name: 'Очередь' },
      { id: 'lyrics', name: 'Текст' },
      { id: 'devices', name: 'Устройства' },
      { id: 'about', name: 'О треке' },
    ];
    return `<div class="panel__head">${tabs(tabsList, S.panelTab, 'panel-tab')}${iconBtn({ icon: 'x', label: 'Закрыть панель', act: 'panel-close', tip: 'Закрыть' })}</div>
      <div class="panel__scroll" id="panelBody">${panelBody()}</div>`;
  }
  function queueHtml() {
    const c = S.current;
    return `<div class="q-section"><span class="t-overline">Сейчас</span></div>
      <div role="list">${trackRow(c, { compact: true })}</div>
      ${S.manual.length ? `<div class="q-section"><span class="t-overline">Далее — добавлено вручную</span><button class="link" data-act="clear-manual">Очистить</button></div>
      <div role="list">${S.manual.map((x) => trackRow(x, { compact: true })).join('')}</div>` : ''}
      <div class="q-section"><span class="t-overline">Далее — из волны</span><span class="t-caption">${label('mood', S.wave.mood)}</span></div>
      <div role="list">${S.next.slice(0, 7).map((x) => trackRow(x, { compact: true })).join('')}</div>
      <div class="q-foot">${ic('waveform')}<div><b>Волна подбирает дальше на ходу</b>По лайкам, пропускам и времени суток. Порядок можно менять перетаскиванием.</div></div>`;
  }
  function panelBody() {
    const t = S.current.t;
    if (S.panelTab === 'queue') return queueHtml();
    if (S.panelTab === 'lyrics') return `<div style="padding:24px 8px">${emptyState({ title: `Текста для «${esc(t.title)}» пока нет`, text: 'Сервер поищет его ночью, вместе с анализом трека. Можно найти и вручную.', action: 'Найти текст' })}</div>`;
    if (S.panelTab === 'devices') return `<div class="pop pop--static" style="max-width:none;background:none;box-shadow:none;padding:8px 0">${devicesMenu()}</div>`;
    const facts = [
      ['Темп', '118 уд/мин'], ['Тональность', '8A · ля минор'], ['Громкость', '−9,2 LUFS'], ['Качество', 'Opus 160 кбит/с'],
      ['Источник', 'YouTube Music · совпадение 97 %'], ['На сервере', 'с 3 ноября'], ['Альбом', esc(t.album)],
    ];
    return `<div style="padding:12px 8px;display:grid;gap:0">${facts.map(([k, v]) => `<div class="type-row" style="grid-template-columns:120px 1fr;padding:12px 0"><span class="t-caption">${k}</span><span class="t-small" style="color:var(--c-text)">${v}</span></div>`).join('')}</div>`;
  }

  // ---------- Экраны ----------
  function viewHome() {
    const phone = S.layout === 'phone';
    const t = S.current.t;
    const playText = S.playing ? '<b>Волна играет</b>' : '<b>Продолжить волну</b>';
    const heroBody = phone
      ? `<h1 class="hero__title t-hero">Моя волна</h1>
         <div class="hero__play">${wavePlay({ size: 'm' })}<div class="hero__play-text" data-bind="hero-now">${playText}<span class="ell">${esc(t.title)} — ${esc(t.artist)}</span></div></div>
         ${moodSel()}
         <button class="settings-link" data-act="wave-sheet" aria-label="Настроить волну">${ic('sliders-horizontal')}<span data-bind="settings">${settingsText()}</span>${ic('caret-right')}</button>`
      : `<div class="hero__over"><span class="live${S.playing ? ' is-on' : ''}" data-bind="live"></span><span class="t-overline" data-bind="over">${S.playing ? `В эфире · ${W.state.bpm}${NB}уд/мин` : 'Персональный поток'}</span></div>
         <h1 class="hero__title t-hero">Моя волна</h1>
         <p class="hero__summary" data-bind="summary">${summary()}</p>
         <div class="hero__play">${wavePlay()}<div class="hero__play-text" data-bind="hero-now">${playText}<span>${esc(t.title)} — ${esc(t.artist)}${S.playing ? '' : ' · с ' + fmtTime(S.pos)}</span></div></div>
         <div class="hero__controls"><div data-bind="console">${consoleSel()}</div>
           <div class="hero__presets"><span class="t-overline">Сохранённые</span><div class="presets" data-bind="presets">${PRESETS.map((p) => presetBtn(p)).join('')}</div></div></div>`;
    const date = 'Среда, 11 ноября';
    return `<div class="home">
      ${phone ? `<div class="phone-head"><div><div class="phone-head__date">${date}</div><div class="phone-head__hello">Доброе утро, Захар</div></div><button class="avatar avatar--l" data-act="soon" data-what="Профиль" aria-label="Профиль: Захар">З</button></div>` : ''}
      <section class="hero" aria-label="Моя волна">
        <div class="wave-host" data-wave="${phone ? 'heroPhone' : 'hero'}"><canvas aria-hidden="true"></canvas></div>
        <div class="hero__scrim"></div>
        <div class="hero__body">${heroBody}</div>
      </section>
      ${phone ? `<section class="shelf" style="margin-top:28px" aria-labelledby="sh-presets">${shelfHead('Сохранённые волны', { all: false, id: 'sh-presets' })}<div class="rail rail--presets presets presets--phone" data-bind="presets">${PRESETS.map((p) => presetBtn(p)).join('')}</div></section>` : ''}
      <section class="shelf" aria-labelledby="sh-cont">${shelfHead('Продолжить', { all: false, id: 'sh-cont' })}<div class="grid grid--4 as-rail">${RESUME.map((r) => resumeCard(r)).join('')}</div></section>
      <section class="shelf" aria-labelledby="sh-mix">${shelfHead('Миксы дня', { sub: 'Обновлены сегодня в 6:00', arrows: !phone, id: 'sh-mix' })}<div class="rail rail--4 rail--mix" data-rail>${MIXES.map((m) => mixCard(m)).join('')}</div></section>
      <section class="shelf" aria-labelledby="sh-new">${shelfHead('Новинки подписок', { sub: `${NEW_RELEASES.length} релизов за пять недель`, arrows: !phone, id: 'sh-new' })}<div class="rail" data-rail>${NEW_RELEASES.map((a) => albumCard(a)).join('')}</div></section>
      <section class="shelf" aria-labelledby="sh-disc">${shelfHead('Открытия недели', { sub: 'Новое для тебя, рядом с тем, что ты любишь', id: 'sh-disc' })}
        <div class="feature">
          <button class="feature__poster" data-act="play-ctx" aria-label="Слушать «Открытия недели»" style="--duo-a:#0a0d24;--duo-b:#9fc2ff">
            <span class="mix-cover" style="--focus:50% 30%"><img src="${pic('Bohnes')}" alt="" loading="lazy"><span class="mix-cover__tone"></span><span class="mix-cover__scrim"></span></span>
            <span class="feature__over">Каждый понедельник</span>
            <span class="feature__body"><span class="feature__title">Открытия<br>недели</span>
              <span class="feature__meta"><span>30 треков · 1${NB}ч 41${NB}мин</span><span class="play-btn" aria-hidden="true">${icf('play')}</span></span></span>
          </button>
          <div class="feature__list" role="list">${DISCOVER.map((x, i) => trackRow(x, { i: i + 1, state: x.state === 'off' ? 'off' : '' })).join('')}</div>
        </div>
      </section>
    </div>`;
  }

  function viewPlayer() {
    if (S.layout === 'phone') {
      return `<div class="pp" role="dialog" aria-label="Плеер">
        <div class="wave-host" data-wave="player" data-anchor=".pp__cover"><canvas aria-hidden="true"></canvas></div>
        <div class="pp__shade"></div>
        <div class="pp__head">
          <a class="icon-btn" href="#home" aria-label="Свернуть плеер">${ic('caret-down')}</a>
          <div class="pp__ctx"><b>Моя волна</b><span data-bind="pp-mood">${moodDot(S.wave.mood)}${label('mood', S.wave.mood)}</span></div>
          ${iconBtn({ icon: 'dots-three', size: '', label: 'Ещё', act: 'ctx', attrs: `data-ctx-for="track:${S.current.t.id}"` })}
        </div>
        <div class="pp__art"><div class="pp__cover" data-bind="pp-cover">${art(img(S.current.t.cover), S.current.t.album)}</div></div>
        <div class="pp__body" data-bind="pp-body">${ppBody()}</div>
      </div>`;
    }
    return `<div class="stage">
      <div class="wave-host" data-wave="stage" data-anchor=".stage__cover"><canvas aria-hidden="true"></canvas></div>
      <div class="stage__floor"></div>
      <div class="stage__inner" data-bind="stage">${stageBody()}</div>
    </div>`;
  }
  function stageBody() {
    const c = S.current, t = c.t, liked = S.liked.has(t.id);
    return `<div class="stage__cover">${art(img(t.cover), t.album)}</div>
      <div class="stage__over">${moodDot(S.wave.mood)}<span class="t-overline">Моя волна · ${label('mood', S.wave.mood)}</span></div>
      <h1 class="stage__artist t-display">${esc(t.artist)}</h1>
      <div class="stage__title"><span>${esc(t.title)}</span>${t.explicit ? badge.explicit() : ''}${badge.quality()}</div>
      ${c.reason ? `<div class="stage__reason">${reasonEl(c.reason)}</div>` : '<div class="stage__reason t-caption">Добавлено вручную</div>'}
      <div class="reactions">
        ${btn({ label: 'Не нравится', icon: 'thumbs-down', variant: 'glass', act: 'dislike' })}
        ${btn({ label: 'Не сейчас', icon: 'clock', variant: 'glass', act: 'not-now' })}
        ${btn({ label: 'Больше такого', icon: 'sparkle', variant: 'glass', act: 'more-like' })}
        ${btn({ label: liked ? 'В любимых' : 'Нравится', ...(liked ? { iconFill: 'heart' } : { icon: 'heart' }), variant: 'glass', act: 'like', attrs: `aria-pressed="${liked}"` })}
      </div>`;
  }
  function ppBody() {
    const c = S.current, t = c.t, liked = S.liked.has(t.id);
    return `<div class="pp__meta"><div style="min-width:0"><div class="pp__title"><span>${esc(t.title)}</span>${t.explicit ? badge.explicit() : ''}</div><div class="pp__artist ell">${esc(t.artist)}</div></div></div>
      <div class="pp__reason">${c.reason ? reasonEl(c.reason) : '<span class="t-caption">Добавлено вручную</span>'}</div>
      <div class="pp__scrub">${scrubber({ bars: 72 })}</div>
      <div class="pp__times"><span data-t="pos">${fmtTime(S.pos)}</span>${badge.quality()}<span>${fmtTime(t.durationMs)}</span></div>
      <div class="pp__controls">
        ${iconBtn({ icon: 'thumbs-down', size: '', label: 'Не нравится', act: 'dislike' })}
        ${iconBtn({ icon: 'skip-back', fill: true, size: '', label: 'Предыдущий', act: 'prev' })}
        ${playBtn({ tip: false })}
        ${iconBtn({ icon: 'skip-forward', fill: true, size: '', label: 'Следующий', act: 'next' })}
        ${iconBtn({ icon: 'heart', fill: liked, size: '', label: liked ? 'Убрать из любимых' : 'Нравится', act: 'like', state: liked ? 'liked' : '', attrs: `aria-pressed="${liked}"` })}
      </div>
      <div class="pp__secondary">
        ${iconBtn({ icon: 'speaker-hifi', size: '', label: 'Устройства', act: 'devices' })}
        ${modePill()}
        ${iconBtn({ icon: 'microphone-stage', size: '', label: 'Текст', act: 'sheet', attrs: 'data-value="lyrics"' })}
        ${iconBtn({ icon: 'queue', size: '', label: 'Очередь', act: 'sheet', attrs: 'data-value="queue"' })}
      </div>`;
  }

  function viewWave() {
    const prevs = D.previews.map((p) => ({ t: TR.get(p.trackId), url: '../../fixtures/' + p.file })).filter((x) => x.t);
    return `<div class="wv">
      <div class="wave-host" data-wave="full"${S.layout === 'phone' ? ' data-center="0.34"' : ''}><canvas aria-hidden="true"></canvas></div>
      ${S.layout === 'phone' ? '' : `<div class="wv__top">${logo()}<a class="btn btn--glass btn--s" href="#home">${ic('arrow-left')}На главную</a></div>`}
      <section class="dbg" aria-label="Отладка волны">
        <div class="dbg__head"><span class="t-overline">Волна · отладка</span><span class="dbg__meters"><span class="beat-dot" data-pulse-dot></span><span data-bpm>${W.state.bpm}${NB}уд/мин</span><span data-fps>— к/с</span></span></div>
        <div class="dbg__field"><span class="t-caption">Настроение</span>${seg(W.MOOD_ORDER.map((id) => ({ id, name: W.MOODS[id].name })), S.wave.mood, { act: 'dbg-mood', name: 'Настроение' })}</div>
        <div class="dbg__field"><span class="t-caption dbg__label">Энергия<span class="tnum" data-energy>0,30</span></span><div class="dbg__energy">${range(W.state.energyAuto ? 0.3 : W.state.energy, { act: 'energy', label: 'Энергия' })}<button class="chip chip--s" data-act="energy-auto" aria-pressed="${W.state.energyAuto}">Авто</button></div></div>
        <div class="dbg__field"><span class="t-caption">Реакции</span><div class="dbg__btns">${btn({ label: 'Старт', icon: 'play-circle', size: 's', act: 'dbg-start' })}${btn({ label: 'Пропуск', icon: 'skip-forward', size: 's', act: 'dbg-skip' })}${btn({ label: 'Лайк', icon: 'heart', size: 's', act: 'dbg-like' })}</div></div>
        <div class="dbg__field"><span class="t-caption dbg__label">Превью · 30${NB}секунд<span class="tnum" data-prev-time></span></span>
          <div class="prev-list">${prevs.map((p) => `<button class="prev" data-act="dbg-preview" data-url="${p.url}" data-cover="${img(p.t.cover)}" aria-label="Слушать превью: ${esc(p.t.title)} — ${esc(p.t.artist)}"><span class="prev__art">${art(img(p.t.cover))}${icf('play')}</span><span class="prev__text"><span class="prev__t">${esc(p.t.title)}</span><span class="prev__s">${esc(p.t.artist)}</span></span><span class="prev__len t-caption tnum">0:30</span></button>`).join('')}</div></div>
      </section>
    </div>`;
  }

  // Лист компонентов — те же функции, что и в экранах
  function viewComponents() {
    const sec = (n, title, body) => `<section class="sec" aria-labelledby="sec-${n}"><div class="sec__head"><span class="sec__n">${n}</span><h2 class="t-display-m" id="sec-${n}">${title}</h2></div><div class="sec__grid">${body}</div></section>`;
    const spec = (name, body, { span = 4, note = '' } = {}) => `<div class="spec spec--${span}"><div class="spec__name">${name}${note ? `<span>${note}</span>` : ''}</div>${body}</div>`;
    const state = (cap, html, wide = false) => `<div class="state${wide ? ' state--wide' : ''}">${html}<span class="t-caption">${cap}</span></div>`;
    const swatch = (name, val, cssVal = val) => `<div class="swatch"><i style="background:${cssVal}"></i><div>${name}<span>${val}</span></div></div>`;
    const t = S.current.t;
    const states5 = (fn) => ['', 'hover', 'active', 'focus', 'disabled'].map((s, i) => state(['обычная', 'наведение', 'нажатие', 'фокус', 'неактивна'][i], fn(s))).join('');

    return `<div class="sheet">
      <header class="sheet__head"><div><div class="t-overline">DalyWave · направление C</div><h1 class="t-display" style="margin-top:12px">Сцена: компоненты</h1>
        <p>Свет даёт только музыка: зал тёмный и тихий, цвет приходит из обложек и настроения волны. Крупный узкий шрифт — для больших моментов, рабочие элементы спокойные и точные.</p></div>
        <div style="display:flex;gap:8px;flex-wrap:wrap">${btn({ label: 'Главная', icon: 'house', variant: 'secondary', size: 's', attrs: 'onclick="location.hash=\'home\'"' })}${btn({ label: 'Волна', icon: 'waveform', variant: 'secondary', size: 's', attrs: 'onclick="location.hash=\'wave\'"' })}</div></header>

      ${sec('00', 'Основа', `
        ${spec('Цвет', `<div class="swatches">
          ${swatch('Сцена', '#060607')}${swatch('Фон', '#0B0B0D')}${swatch('Панель', '#101013')}${swatch('Поверхность', '#16161A')}${swatch('Поверхность 2', '#1D1D22')}
          ${swatch('Текст', '#F3F0EA')}${swatch('Текст 2', '#B4B1AB')}${swatch('Текст 3', '#8D8A85')}${swatch('Ошибка', '#FF7A6B')}${swatch('Скачано', '#74DCA2')}
          ${swatch('Гель обложки', 'из обложки', 'var(--gel)')}${W.MOOD_ORDER.map((id) => swatch('Свет · ' + W.MOODS[id].name, W.MOODS[id].c.slice(0, 2).join(' → '), `var(--mood-${id})`)).join('')}</div>`, { span: 12, note: 'контраст текста на фоне: 17,6 · 8,9 · 5,8' })}
        ${spec('Типографика', `<div>
          <div class="type-row"><span class="t-caption">Герой · Fira Sans Extra Condensed 900 · 132/0,86</span><span class="t-hero" style="font-size:96px">Моя волна</span></div>
          <div class="type-row"><span class="t-caption">Дисплей · 900 · 88</span><span class="t-display" style="font-size:64px">Parker The Bandit</span></div>
          <div class="type-row"><span class="t-caption">Дисплей M · 800 · 40</span><span class="t-display-m">Открытия недели</span></div>
          <div class="type-row"><span class="t-caption">Дисплей S · 800 · 22</span><span class="t-display-s">Микс дня 01 · Пошлая Молли</span></div>
          <div class="type-row"><span class="t-caption">Заголовок L · Golos Text 650 · 24/30</span><span class="t-title-l">I Want Out</span></div>
          <div class="type-row"><span class="t-caption">Заголовок M · 650 · 20/26</span><span class="t-title-m">Новинки подписок</span></div>
          <div class="type-row"><span class="t-caption">Текст · 400 · 14/20</span><span class="t-body">Волна подбирает дальше на ходу — по лайкам и пропускам.</span></div>
          <div class="type-row"><span class="t-caption">Мелкий · 13/18 и подпись · 12/16</span><span><span class="t-small">Mytch · Leverage · 2026</span> <span class="t-caption tnum">· 3:07 · 12${NB}480 треков</span></span></div>
          <div class="type-row"><span class="t-caption">Надстрочный · 600 · 11 · 0,12em</span><span class="t-overline">Далее — из волны</span></div></div>`, { span: 8, note: 'две гарнитуры, обе с кириллицей' })}
        ${spec('Знак и иконка', `<div class="brand-row"><img class="app-icon" src="../../brand/icon-rounded.svg" alt="Иконка DalyWave — сумерки"><img class="app-icon app-icon--mac" src="../../brand/icon-macos.svg" alt="Иконка для macOS"></div>
          <div class="brand-row brand-row--sizes">${[64, 40, 28, 20].map((s) => `<img src="../../brand/icon-rounded.svg" alt="" width="${s}" height="${s}">`).join('')}<span class="t-caption">64 · 40 · 28 · 20</span></div>
          <div class="brand-row"><span class="brand-tile">${logo()}</span><span class="brand-tile brand-tile--light">${logo()}</span><span class="brand-tile brand-tile--mood">${logo()}</span></div>
          <p class="t-caption">Знак и иконка не перерисовываются. Слово набрано интерфейсным шрифтом, знак красится текущим цветом: на свету, на тёмном, в цвете настроения.</p>`, { span: 4 })}
        ${spec('Отступы и скругления', `<div style="display:grid;gap:8px">${[4, 8, 12, 16, 24, 32, 48, 64].map((v) => `<div style="display:flex;align-items:center;gap:12px"><span class="t-caption tnum" style="width:28px">${v}</span><i style="display:block;height:8px;width:${v * 2}px;background:var(--c-surface-3);border-radius:2px"></i></div>`).join('')}</div>
          <div style="display:flex;gap:12px;flex-wrap:wrap">${[['4', 'значки'], ['6', 'обложки'], ['10', 'строки'], ['14', 'панели'], ['20', 'сцена'], ['999', 'кнопки']].map(([r, n]) => `<div style="display:grid;gap:6px;justify-items:center"><i style="display:block;width:52px;height:52px;background:var(--c-surface-2);box-shadow:var(--sh-edge);border-radius:${Math.min(+r, 26)}px"></i><span class="t-caption">${r} · ${n}</span></div>`).join('')}</div>`, { span: 6 })}
        ${spec('Движение и глубина', `<div style="display:grid;gap:0">${[['120 мс', 'наведение, нажатие', 'ease-out'], ['200 мс', 'смена состояния', 'ease-out'], ['320 мс', 'панели, экраны', 'cubic-bezier(.2,.8,.2,1)'], ['560 мс', 'раскрытие волны', 'пружина'], ['1200 мс', 'перетекание палитры настроения', 'ease-in-out']].map(([d, w, e]) => `<div class="type-row" style="grid-template-columns:72px 1fr auto;padding:10px 0"><span class="t-small tnum" style="color:var(--c-text)">${d}</span><span class="t-small">${w}</span><span class="t-caption">${e}</span></div>`).join('')}</div>
          <div style="display:flex;gap:16px;flex-wrap:wrap"><div class="pop pop--static" style="min-width:0;padding:14px 16px"><span class="t-caption">Всплывающее · край + глубина</span></div><div style="padding:14px 16px;border-radius:14px;background:var(--c-surface);box-shadow:var(--sh-gel);--gel:#48d4ff"><span class="t-caption">Свет обложки под карточкой</span></div></div>`, { span: 6 })}
      `)}

      ${sec('01', 'Кнопки', `
        ${spec('Основная · вторичная · призрачная · на сцене', `
          <div class="states">${states5((s) => btn({ label: 'Слушать', iconFill: 'play', variant: 'primary', state: s }))}</div>
          <div class="states">${states5((s) => btn({ label: 'В медиатеку', icon: 'plus', variant: 'secondary', state: s }))}</div>
          <div class="states">${states5((s) => btn({ label: 'Все', variant: 'ghost', state: s }))}</div>
          <div class="states" style="padding:16px;border-radius:14px;background:radial-gradient(120% 140% at 80% 0%, #1b2a52, #060607 70%)">${states5((s) => btn({ label: 'Больше такого', icon: 'sparkle', variant: 'glass', state: s }))}</div>`, { span: 8, note: 'высота 32 · 40 · 48' })}
        ${spec('Иконки и воспроизведение', `
          <div class="states">${['', 'hover', 'active', 'focus', 'disabled'].map((s, i) => state(['обычная', 'наведение', 'нажатие', 'фокус', 'неактивна'][i], iconBtn({ icon: 'shuffle', size: '', label: 'Перемешать', state: s }))).join('')}</div>
          <div class="states">${state('включена', iconBtn({ icon: 'queue', size: '', label: 'Очередь', on: true }))}${state('лайк', iconBtn({ icon: 'heart', fill: true, size: '', label: 'Нравится', state: 'liked' }))}${state('S · 32', iconBtn({ icon: 'dots-three', label: 'Ещё' }))}${state('L · 48', iconBtn({ icon: 'thumbs-down', size: 'l', label: 'Не нравится' }))}</div>
          <div class="states">${state('пауза', playBtn({ playing: false, tip: false }))}${state('играет', playBtn({ playing: true, tip: false }))}${state('наведение', playBtn({ playing: false, state: 'hover', tip: false }))}${state('64', playBtn({ size: 'l', tip: false }))}</div>`, { span: 4 })}
      `)}

      ${sec('02', 'Управление волной', `
        ${spec('Кнопка волны', `<div class="states" style="gap:28px;padding:12px 4px">${state('ожидание', wavePlay({ playing: false, act: 'noop' }))}${state('играет', wavePlay({ playing: true, act: 'noop' }))}${state('загрузка', wavePlay({ playing: false, state: 'loading', act: 'noop' }))}${state('наведение', wavePlay({ playing: false, state: 'hover', act: 'noop' }))}</div>
          <p class="t-caption">Кольцо дышит в долю трека, при игре по нему бежит луч.</p>`, { span: 4 })}
        ${spec('Пульт волны', `<div class="states">${state('обычный', consoleSel())}${state('канал открыт · наведение', consoleSel({ mood: 'open', activity: 'hover' }))}</div>
          <div class="pop pop--static" style="width:260px">${menu(OPTS.mood.items.map((m) => ({ dot: m.id, name: m.name, checked: m.id === S.wave.mood, act: 'noop' })), { title: 'Настроение' })}</div>`, { span: 8, note: 'четыре канала вместо стены чипов' })}
        ${spec('Выбор настроения', `${state('настроение меняет свет волны за 1,2 с', moodSel({ state: { cheerful: 'hover' } }), true)}${state('строка', moodSel({ row: true }), true)}`, { span: 6 })}
        ${spec('Сохранённые сочетания', `<div class="states">${state('обычное', presetBtn(PRESETS[1], 'none'))}${state('наведение', presetBtn(PRESETS[2], 'hover'))}${state('выбрано', presetBtn(PRESETS[0], 'selected'))}</div>
          <div class="mini-wave"><div class="wave-host" data-wave="card"><canvas aria-hidden="true"></canvas></div></div><span class="t-caption">Волна в карточке: 2 ленты, 11 линий, 30 к/с на телефоне</span>`, { span: 6 })}
      `)}

      ${sec('03', 'Переключатели и ввод', `
        ${spec('Сегменты и вкладки', `${state('сегменты', seg([{ id: 'lib', name: 'Медиатека' }, { id: 'all', name: 'Весь каталог' }, { id: 'off', name: 'Скачанное', state: 'hover' }], 'lib'), true)}
          ${state('вкладки', `<div style="height:40px;border-bottom:1px solid var(--c-line)">${tabs([{ id: 'q', name: 'Очередь' }, { id: 'l', name: 'Текст', state: 'hover' }, { id: 'd', name: 'Устройства' }, { id: 'a', name: 'О треке' }], 'q')}</div>`, true)}`, { span: 4 })}
        ${spec('Чипы и метки', `<div class="states">${state('обычный', chip('Фонк'))}${state('наведение', chip('Гиперпоп', { state: 'hover' }))}${state('выбран', chip('Альт-рэп', { pressed: true }))}${state('неактивен', chip('Джаз', { state: 'disabled' }))}</div>
          <div class="states">${state('метки', `<span style="display:flex;gap:6px"><span class="tag">Rap/Hip Hop</span><span class="tag">2026</span><span class="tag">8A</span></span>`)}</div>`, { span: 4 })}
        ${spec('Переключатель', `<div class="states" style="flex-direction:column;gap:14px">${state('включён', switchEl('Умный запас на устройстве', true))}${state('выключен', switchEl('Только по Wi‑Fi', false))}${state('неактивен', switchEl('Кэш без ограничений', false, { disabled: true }))}</div>`, { span: 4 })}
        ${spec('Поиск', `${state('пустой', searchField(), true)}${state('фокус', searchField({ state: 'focus' }), true)}${state('с запросом', searchField({ value: 'спокойный инструментал для работы' }), true)}`, { span: 6 })}
        ${spec('Подсказка и клавиши', `<div class="states">${state('подсказка', tipEl('Пауза', ['Space']))}${state('с сочетанием', tipEl('Следующий', ['⌘', '→']))}${state('клавиши', kbd(['⌘', 'K']))}</div>
          <div class="states">${state('громкость · обычная', `<div style="width:160px">${volumeEl(0.72)}</div>`)}${state('наведение', `<div style="width:160px">${volumeEl(0.45, 'hover')}</div>`)}${state('без звука', `<div style="width:160px">${volumeEl(0)}</div>`)}</div>`, { span: 6 })}
      `)}

      ${sec('04', 'Строка трека', spec('Состояния строки', `<div role="list" style="display:grid;gap:4px">
          ${[['', 'обычная'], ['hover', 'наведение'], ['playing', 'играет'], ['off', 'нет на сервере — скачается при запуске'], ['down', 'скачано на устройство']].map(([s, cap], i) => {
            const x = DISCOVER[i] ?? WAVE_NEXT[i];
            return `<div style="display:grid;grid-template-columns:200px minmax(0,1fr);align-items:center;gap:16px"><span class="t-caption">${cap}</span>${trackRow({ t: x.t, reason: x.reason }, { i: i + 1, state: s })}</div>`;
          }).join('')}
          <div style="display:grid;grid-template-columns:200px minmax(0,1fr);align-items:center;gap:16px"><span class="t-caption">компактная · очередь</span><div style="max-width:360px">${trackRow(CURRENT, { compact: true, state: 'playing' })}</div></div>
        </div>`, { span: 12, note: 'длинные названия обрезаются многоточием' }))}

      ${sec('05', 'Карточки и полки', `
        ${spec('Заголовок полки', shelfHead('Новинки подписок', { sub: '8 релизов за пять недель', arrows: true }), { span: 12 })}
        ${spec('Альбом', `<div class="grid" style="grid-template-columns:minmax(0,1fr) minmax(0,1fr);column-gap:16px">${state('обычная', albumCard(NEW_RELEASES[0]), true)}${state('наведение', albumCard(NEW_RELEASES[1], { state: 'hover' }), true)}</div>`, { span: 4 })}
        ${spec('Артист', `<div class="grid" style="grid-template-columns:minmax(0,1fr) minmax(0,1fr);column-gap:16px">${state('обычная', artistCard('Lowborn'), true)}${state('наведение', artistCard('Пошлая Молли', { state: 'hover' }), true)}</div>`, { span: 4 })}
        ${spec('Микс дня', `<div class="grid" style="grid-template-columns:minmax(0,1fr) minmax(0,1fr);column-gap:16px">${state('дуотон + лайнап', mixCard(MIXES[1]), true)}${state('наведение', mixCard(MIXES[3], { state: 'hover' }), true)}</div>`, { span: 4 })}
        ${spec('Продолжить', `<div class="states" style="flex-direction:column">${state('обычная', resumeCard(RESUME[0]), true)}${state('наведение', resumeCard(RESUME[1], { state: 'hover' }), true)}</div>`, { span: 6 })}
        ${spec('Пустой экран и загрузка', `<div style="display:grid;grid-template-columns:1fr 1fr;gap:16px">${emptyState({ title: 'В плейлисте пока пусто', text: 'Перетащи сюда треки или включи «Продолжать волной».', action: 'Найти музыку' })}
          <div style="display:grid;gap:12px;align-content:start">${[0, 1, 2].map(() => `<div style="display:grid;grid-template-columns:44px 1fr;gap:12px;align-items:center"><span class="skel" style="height:44px"></span><span style="display:grid;gap:8px"><span class="skel" style="height:10px;width:70%"></span><span class="skel" style="height:10px;width:40%"></span></span></div>`).join('')}<span class="skel" style="aspect-ratio:2/1"></span></div></div>`, { span: 6 })}
      `)}

      ${sec('06', 'Плеер', `
        ${spec('Нижняя панель плеера', `<div class="np-scroll"><div class="npbar npbar--static" style="border-radius:14px;height:84px">${npInner()}</div></div>`, { span: 12, note: 'отметка на форме волны — где начнётся переход' })}
        ${spec('Форма волны трека', `${state('обычная', `<div style="width:100%">${scrubber({ bars: 90 })}</div>`, true)}${state('наведение', `<div style="width:100%">${scrubber({ bars: 90, state: 'hover' })}</div>`, true)}${state('без отметки перехода · «Без пауз» в альбоме', `<div style="width:100%">${scrubber({ bars: 90, mark: false, pos: 20000 })}</div>`, true)}`, { span: 6 })}
        ${spec('Мини-плеер', `<div class="mini mini--static" style="--p:${S.pos / t.durationMs}">${miniInner()}</div><span class="t-caption">Поверх вкладок на телефоне, подкрашен цветом обложки</span>`, { span: 6 })}
        ${spec('Режим переходов', `<div class="pop pop--static" style="width:320px">${transitionMenu()}</div>`, { span: 4 })}
        ${spec('Устройства', `<div class="pop pop--static" style="width:300px">${devicesMenu()}</div>`, { span: 4 })}
        ${spec('Контекстное меню', `<div class="pop pop--static" style="width:280px">${ctxMenu('track')}</div>`, { span: 4 })}
      `)}

      ${sec('07', 'Уведомления и значки', `
        ${spec('Уведомление с отменой', `${toastHtml('Добавлено в «Любимые треки»', { stat: true })}${toastHtml('Не будем ставить «Vicious»', { icon: 'thumbs-down', stat: true })}`, { span: 6 })}
        ${spec('Значки', `<div class="states">${state('качество', badge.quality())}${state('lossless', badge.quality('FLAC'))}${state('explicit', badge.explicit())}${state('скачано', badge.down())}${state('новое', badge.fresh())}${state('нет на сервере', badge.cloud())}</div>`, { span: 6 })}
        ${spec('Загрузка', `<div class="states" style="gap:24px">${state('в очереди', ring(0, { size: 'l', icon: ic('download-simple') }))}${state('35 %', ring(35, { size: 'l', icon: icf('stop') }))}${state('70 %', ring(70, { size: 'l', icon: icf('stop') }))}${state('готово', ring(100, { size: 'l', done: true, icon: ic('check') }))}${state('в строке', ring(62))}</div>`, { span: 6 })}
        ${spec('Свет настроения', `<div class="states" style="gap:24px">${W.MOOD_ORDER.map((id) => state(W.MOODS[id].name, moodDot(id, 'l'))).join('')}</div><p class="t-caption">Точка — это прожектор: тот же цвет получает волна.</p>`, { span: 6 })}
      `)}
    </div>`;
  }

  // ---------- Сборка ----------
  let waveViews = [];
  const BARE = new Set(['wave', 'components']);

  function detect() {
    const w = innerWidth;
    const coarse = matchMedia('(pointer: coarse)').matches;
    const layout = w < 700 ? 'phone' : w < 1100 ? 'tablet' : 'desktop';
    const platform = layout === 'phone' ? 'ios' : coarse ? 'ipados' : 'macos';
    return { layout, platform };
  }

  function render() {
    closePop();
    closeSheet();
    const { layout, platform } = detect();
    S.layout = layout;
    S.platform = platform;
    root.dataset.layout = layout;
    root.dataset.platform = platform;
    root.dataset.route = S.route;
    const bare = BARE.has(S.route);
    const shell = !bare && layout !== 'phone';
    app.className = 'app' + (S.panel && shell ? ' has-panel' : '');
    app.innerHTML = bare
      ? `<main class="main" id="main"><div class="view" id="view"></div></main>${S.route === 'wave' && layout === 'phone' ? phoneChrome() : ''}`
      : `${shell ? sidebar() : ''}
         <main class="main${S.route === 'player' ? ' is-stage' : ''}" id="main">${shell ? topbar() : ''}<div class="view" id="view"></div></main>
         ${shell ? `<aside class="panel" aria-label="Боковая панель">${panelHtml()}</aside><footer class="npbar" id="npbar" aria-label="Сейчас играет">${npInner()}</footer>` : ''}
         ${layout === 'phone' ? phoneChrome() : ''}
         ${platform === 'ipados' ? ipadStatus() + '<div class="home-ind" aria-hidden="true"></div>' : ''}`;
    renderView();
  }

  function renderView() {
    const view = $('#view');
    waveViews.forEach((v) => v.destroy());
    waveViews = [];
    const html = { home: viewHome, player: viewPlayer, wave: viewWave, components: viewComponents }[S.route]();
    view.innerHTML = html;
    if (!reduced.matches) {
      view.classList.remove('is-entering');
      void view.offsetWidth;
      view.classList.add('is-entering');
    }
    mountWaves(view);
    sampleCards(view);
    bindScroll();
    window.scrollTo(0, 0);
  }

  function mountWaves(scope) {
    $$('[data-wave]', scope).forEach((host) => {
      const c = host.querySelector('canvas');
      if (!c) return;
      const v = W.mount(c, { preset: host.dataset.wave, ...(host.dataset.center ? { centerY: Number(host.dataset.center) } : {}) });
      waveViews.push(v);
      // Лента проходит через центр обложки — обложка «в объятиях» волны
      if (host.dataset.anchor) {
        const place = () => {
          const a = $(host.dataset.anchor, scope), hr = host.getBoundingClientRect();
          if (!a || !hr.height) return;
          const r = a.getBoundingClientRect();
          v.set?.({ centerY: (r.top + r.height / 2 - hr.top) / hr.height });
        };
        requestAnimationFrame(place);
        new ResizeObserver(place).observe(host);
      }
    });
  }

  // Цвет света под карточкой — из её обложки
  function sampleCards(scope) {
    const run = () => $$('[data-gel]', scope).forEach(async (el) => {
      const s = await W.colors.sample(el.dataset.gel);
      if (s && !s.mono && s.vivid[0]) el.style.setProperty('--gel', W.colors.rgbToHex(W.colors.gel(s.vivid[0], 0.72, 0.1, 0.18)));
    });
    (window.requestIdleCallback || setTimeout)(run, { timeout: 800 });
  }

  function bindScroll() {
    const main = $('#main');
    const tb = $('.topbar');
    const status = $('.ios-status');
    const onScroll = () => {
      const y = S.layout === 'phone' ? window.scrollY : main.scrollTop;
      tb?.classList.toggle('is-scrolled', y > 8);
      status?.classList.toggle('is-solid', y > 380 && S.route === 'home');
    };
    main?.addEventListener('scroll', onScroll, { passive: true });
    window.onscroll = onScroll;
  }

  // ---------- Цвет текущего трека ----------
  async function applyGel() {
    const t = S.current.t;
    const s = await W.colors.sample(img(t.cover));
    let gel, deep;
    if (s && !s.mono && s.vivid[0]) {
      gel = W.colors.rgbToHex(W.colors.gel(s.vivid[0], 0.8, 0.09, 0.16));
      deep = W.colors.rgbToHex(W.colors.gel(s.vivid[0], 0.26, 0.04, 0.08));
    } else {
      const m = W.colors.hexToRgb(W.MOODS[S.wave.mood].c[1]);
      gel = W.colors.rgbToHex(W.colors.gel(m, 0.8, 0.09, 0.15));
      deep = W.colors.rgbToHex(W.colors.gel(m, 0.24, 0.03, 0.06));
    }
    root.style.setProperty('--gel', gel);
    root.style.setProperty('--gel-deep', deep);
    W.setCover(img(t.cover));
  }
  function applyMood(instant) {
    const m = W.MOODS[S.wave.mood];
    root.style.setProperty('--mood-light', m.c[0]);
    W.setMood(S.wave.mood, instant);
  }

  // ---------- Воспроизведение ----------
  let ticker = 0, lastTick = 0;
  function tick() {
    const now = performance.now();
    const dt = now - lastTick;
    lastTick = now;
    if (!S.playing) return;
    S.pos += dt;
    if (S.pos >= S.current.t.durationMs) {
      if (S.repeat) S.pos = 0;
      else return next('auto');
    }
    paintProgress();
  }
  function paintProgress() {
    const t = S.current.t, p = clamp(S.pos / t.durationMs);
    $$('[data-scrub]').forEach((el) => {
      if (el.closest('.sheet')) return;
      el.style.setProperty('--p', p);
      el.setAttribute('aria-valuenow', Math.round(S.pos / 1000));
    });
    $$('[data-t="pos"]').forEach((el) => (el.textContent = fmtTime(S.pos)));
    $$('#mini').forEach((el) => el.style.setProperty('--p', p));
  }
  function startAudio() {
    const url = PREV.get(S.current.t.id);
    if (url) {
      // Превью 30 с: звук настоящий, анализатор ведёт волну
      W.play(url);
    } else {
      W.pause();
      W.setPlaying(true);
    }
  }
  function play() {
    S.playing = true;
    startAudio();
    lastTick = performance.now();
    clearInterval(ticker);
    ticker = setInterval(tick, 250);
    refreshPlayback();
  }
  function pause() {
    S.playing = false;
    clearInterval(ticker);
    W.pause();
    refreshPlayback();
  }
  const toggle = () => (S.playing ? pause() : play());

  function next(why = 'skip') {
    const nx = S.manual.shift() || S.next.shift();
    if (!nx) return;
    S.history.push(S.current);
    S.next.push(S.current.reason ? S.current : { ...S.current, reason: 'Знакомое' });
    S.current = nx;
    S.pos = 0;
    W.skip();
    applyGel();
    if (S.playing) startAudio();
    refreshTrack();
    void why;
  }
  function prev() {
    if (S.pos > 3000 || !S.history.length) {
      S.pos = 0;
      if (S.playing) { const a = W.audio; if (a) a.currentTime = 0; }
      return paintProgress();
    }
    S.next.unshift(S.current);
    S.current = S.history.pop();
    S.pos = 0;
    W.skip();
    applyGel();
    if (S.playing) startAudio();
    refreshTrack();
  }

  W.on('audio', (e) => {
    if (S.route === 'wave') return dbgAudio(e);
    if (e.type === 'ended' && S.playing) next('auto');
    if (e.type === 'blocked') { pause(); toast('Браузер не дал включить звук — нажми ещё раз', { icon: 'speaker-simple-x', undo: false }); }
  });

  // Обновления без перерисовки сцены: холст волны остаётся жить
  function refreshPlayback() {
    $$('[data-act="play"]').forEach((b) => {
      if (b.closest('.sheet')) return;
      b.innerHTML = icf(S.playing ? 'pause' : 'play');
      b.setAttribute('aria-label', S.playing ? 'Пауза' : 'Слушать');
      if (b.dataset.tip) b.dataset.tip = S.playing ? 'Пауза' : 'Слушать';
    });
    $$('.wave-play').forEach((w) => {
      if (w.closest('.sheet')) return;
      w.classList.toggle('is-playing', S.playing);
      const b = w.querySelector('button');
      b.innerHTML = icf(S.playing ? 'pause' : 'play', S.playing ? '' : 'ph-play');
      b.setAttribute('aria-label', S.playing ? 'Пауза' : 'Слушать «Мою волну»');
    });
    $$('.eq').forEach((e) => { if (!e.closest('.sheet')) e.classList.toggle('is-paused', !S.playing); });
    heroNow();
  }
  function heroNow() {
    const t = S.current.t;
    $$('[data-bind="hero-now"]').forEach((el) => {
      el.innerHTML = `<b>${S.playing ? 'Волна играет' : 'Продолжить волну'}</b><span class="ell">${esc(t.title)} — ${esc(t.artist)}${S.playing || S.layout === 'phone' ? '' : ' · с ' + fmtTime(S.pos)}</span>`;
    });
    $$('[data-bind="live"]').forEach((el) => el.classList.toggle('is-on', S.playing));
    $$('[data-bind="over"]').forEach((el) => (el.textContent = S.playing ? `В эфире · ${W.state.bpm}${NB}уд/мин` : 'Персональный поток'));
  }
  function refreshTrack() {
    const np = $('#npbar');
    if (np) np.innerHTML = npInner();
    const mini = $('#mini');
    if (mini) { mini.innerHTML = miniInner(); mini.style.setProperty('--p', 0); }
    const stage = $('[data-bind="stage"]');
    if (stage) stage.innerHTML = stageBody();
    const ppb = $('[data-bind="pp-body"]');
    if (ppb) ppb.innerHTML = ppBody();
    const ppc = $('[data-bind="pp-cover"]');
    if (ppc) ppc.innerHTML = art(img(S.current.t.cover), S.current.t.album);
    const pb = $('#panelBody');
    if (pb) pb.innerHTML = panelBody();
    const sh = $('.sheet-body[data-kind="queue"]');
    if (sh) sh.innerHTML = queueHtml();
    refreshPlayback();
  }
  function refreshWaveUI() {
    $$('[data-bind="summary"]').forEach((el) => (el.textContent = summary()));
    $$('[data-bind="console"]').forEach((el) => (el.innerHTML = consoleSel()));
    $$('[data-bind="presets"]').forEach((el) => (el.innerHTML = PRESETS.map((p) => presetBtn(p)).join('')));
    $$('[data-bind="settings"]').forEach((el) => (el.textContent = settingsText()));
    $$('.mood-sel:not(.sheet .mood-sel)').forEach((el) => $$('.mood-opt', el).forEach((o) => o.setAttribute('aria-checked', o.dataset.value === S.wave.mood)));
    $$('[data-bind="pp-mood"]').forEach((el) => (el.innerHTML = moodDot(S.wave.mood) + label('mood', S.wave.mood)));
    const stage = $('[data-bind="stage"] .stage__over');
    if (stage) stage.innerHTML = `${moodDot(S.wave.mood)}<span class="t-overline">Моя волна · ${label('mood', S.wave.mood)}</span>`;
    const pb = $('#panelBody');
    if (pb && S.panelTab === 'queue') pb.innerHTML = panelBody();
  }
  function setWave(group, value) {
    if (S.wave[group] === value) return;
    S.wave[group] = value;
    if (group === 'mood') applyMood();
    refreshWaveUI();
  }

  function like() {
    const id = S.current.t.id;
    const had = S.liked.has(id);
    had ? S.liked.delete(id) : S.liked.add(id);
    if (!had) W.like();
    refreshTrack();
    toast(had ? `Убрано из «Любимых треков»` : `Добавлено в «Любимые треки»`, {
      icon: had ? 'heart-break' : 'heart',
      undo: () => { had ? S.liked.add(id) : S.liked.delete(id); refreshTrack(); },
    });
  }
  function dislike() {
    const cur = S.current;
    next('dislike');
    toast(`Не будем ставить «${esc(cur.t.title)}»`, {
      icon: 'thumbs-down',
      undo: () => { S.next.unshift(S.current); S.current = cur; S.pos = 0; applyGel(); if (S.playing) startAudio(); refreshTrack(); },
    });
  }

  // ---------- Всплывающее ----------
  let pop = null;
  function openPop(anchor, html, { place = 'bottom', cls = '', x, y, width } = {}) {
    closePop();
    const el = document.createElement('div');
    el.className = 'pop ' + cls;
    el.setAttribute('role', 'menu');
    el.innerHTML = html;
    if (width) el.style.width = width + 'px';
    layer.append(el);
    const pr = el.getBoundingClientRect();
    let left, top;
    if (x != null) {
      left = x; top = y;
    } else {
      const r = anchor.getBoundingClientRect();
      left = place.includes('end') ? r.right - pr.width : place.includes('center') ? r.left + r.width / 2 - pr.width / 2 : r.left;
      top = place.startsWith('top') ? r.top - pr.height - 8 : r.bottom + 8;
      if (top + pr.height > innerHeight - 8 && !place.startsWith('top')) top = r.top - pr.height - 8;
      el.style.setProperty('--origin', place.startsWith('top') ? 'bottom center' : 'top left');
    }
    left = clamp(left, 8, innerWidth - pr.width - 8);
    top = clamp(top, 8, innerHeight - pr.height - 8);
    el.style.left = left + 'px';
    el.style.top = top + 'px';
    anchor?.setAttribute('aria-expanded', 'true');
    pop = { el, anchor };
    const first = el.querySelector('[aria-checked="true"]') || el.querySelector('.menu-item:not(:disabled), .mood-opt');
    first?.focus({ preventScroll: true });
  }
  function closePop(focusBack) {
    if (!pop) return;
    pop.anchor?.setAttribute('aria-expanded', 'false');
    if (focusBack) pop.anchor?.focus({ preventScroll: true });
    pop.el.remove();
    pop = null;
  }

  // Шторка снизу — на телефоне вместо всплывающих меню
  let sheet = null;
  function openSheet(title, html, kind = '') {
    closeSheet();
    const el = document.createElement('div');
    el.className = 'sheet-wrap';
    el.innerHTML = `<div class="sheet-scrim" data-act="sheet-close"></div><div class="bsheet" role="dialog" aria-label="${esc(title)}"><div class="bsheet__grab"></div><div class="bsheet__head"><span class="t-title-s">${title}</span>${iconBtn({ icon: 'x', size: '', label: 'Закрыть', act: 'sheet-close' })}</div><div class="sheet-body" data-kind="${kind}">${html}</div></div>`;
    layer.append(el);
    sheet = el;
  }
  function closeSheet() { sheet?.remove(); sheet = null; }

  let toastTimer = 0, toastUndo = null;
  function toast(text, { icon = 'check-circle', undo = null } = {}) {
    $$('.toast', layer).forEach((t) => t.remove());
    layer.insertAdjacentHTML('beforeend', toastHtml(text, { icon, undo: !!undo }));
    toastUndo = typeof undo === 'function' ? undo : null;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => $$('.toast', layer).forEach((t) => t.remove()), 5000);
  }

  // Подсказки с клавишами — только на устройствах с мышью
  let tipTimer = 0, tipEl2 = null;
  const finePointer = matchMedia('(hover: hover) and (pointer: fine)');
  document.addEventListener('pointerover', (e) => {
    if (!finePointer.matches) return;
    const el = e.target.closest('[data-tip]');
    if (!el || el.closest('.sheet') || el.closest('.pop')) return;
    if (el.dataset.tipWhen === 'rail' && S.layout !== 'tablet') return;
    clearTimeout(tipTimer);
    tipTimer = setTimeout(() => showTip(el), 450);
  });
  document.addEventListener('pointerout', (e) => {
    if (e.target.closest('[data-tip]')) { clearTimeout(tipTimer); hideTip(); }
  });
  document.addEventListener('pointerdown', () => { clearTimeout(tipTimer); hideTip(); }, true);
  function showTip(el) {
    hideTip();
    const keys = el.dataset.kbd ? el.dataset.kbd.split(' ') : null;
    layer.insertAdjacentHTML('beforeend', tipEl(el.dataset.tip, keys).replace('tip tip--static', 'tip'));
    tipEl2 = layer.lastElementChild;
    const r = el.getBoundingClientRect(), tr2 = tipEl2.getBoundingClientRect();
    const above = r.top > 60;
    tipEl2.style.left = clamp(r.left + r.width / 2 - tr2.width / 2, 8, innerWidth - tr2.width - 8) + 'px';
    tipEl2.style.top = (above ? r.top - tr2.height - 8 : r.bottom + 8) + 'px';
  }
  function hideTip() { tipEl2?.remove(); tipEl2 = null; }

  // ---------- Действия ----------
  const ACT = {
    play: toggle,
    'wave-play': toggle,
    next: () => next('skip'),
    prev,
    like,
    dislike,
    'not-now': () => { next('not-now'); toast('Пропустили без штрафа — трек ещё вернётся', { icon: 'clock', undo: false }); },
    'more-like': () => { W.like(); toast(`Волна добавит больше похожего на «${esc(S.current.t.title)}»`, { icon: 'sparkle', undo: false }); },
    repeat: () => { S.repeat = !S.repeat; refreshTrack(); },
    'go-home': () => (location.hash = 'home'),
    noop: () => {},
    soon: (el) => toast(`${el.dataset.what || 'Этот раздел'} — в следующих макетах`, { icon: 'info', undo: false }),
    'play-ctx': () => { if (!S.playing) play(); W.start(); toast('Включаем — волна подстроится под выбор', { icon: 'play-circle', undo: false }); },
    pick: (el) => {
      if (pop && pop.anchor === el) return closePop();
      const g = el.dataset.group;
      const items = OPTS[g].items.map((it) => ({
        ...(g === 'mood' ? { dot: it.id } : { icon: it.icon }), name: it.name, desc: it.desc, checked: S.wave[g] === it.id, act: 'set-wave', group: g, value: it.id,
      }));
      openPop(el, menu(items, { title: OPTS[g].label }), { place: 'top', width: g === 'character' ? 300 : 260 });
    },
    'set-wave': (el) => {
      setWave(el.dataset.group, el.dataset.value);
      if (pop) closePop(true);
    },
    preset: (el) => {
      const p = PRESETS.find((x) => x.id === el.dataset.id);
      const moodChanged = S.wave.mood !== p.set.mood;
      S.wave = { ...p.set };
      if (moodChanged) applyMood();
      refreshWaveUI();
      W.start();
      if (!S.playing) play();
      toast(`Волна «${p.name}»: ${p.sub}`, { icon: 'waveform', undo: false });
    },
    transition: (el) => {
      if (pop && pop.anchor === el) return closePop();
      if (S.layout === 'phone') return openSheet('Переходы между треками', `<div class="pop pop--static" style="max-width:none;background:none;box-shadow:none;padding:0">${transitionMenu()}</div>`);
      openPop(el, transitionMenu(), { place: 'top-end', width: 320 });
    },
    'set-mode': (el) => {
      S.transition = el.dataset.value;
      closePop(true);
      closeSheet();
      refreshTrack();
      toast(`Переходы: «${modeOf().name}»`, { icon: 'faders', undo: false });
    },
    devices: (el) => {
      if (pop && pop.anchor === el) return closePop();
      if (S.layout === 'phone') return openSheet('Где играть', `<div class="pop pop--static" style="max-width:none;background:none;box-shadow:none;padding:0">${devicesMenu()}</div>`);
      openPop(el, devicesMenu(), { place: 'top-end', width: 300 });
    },
    'set-device': (el) => {
      S.device = el.dataset.value;
      closePop(true);
      closeSheet();
      toast(`Играет на устройстве «${DEVICES[S.device].name}»`, { icon: 'speaker-hifi', undo: false });
    },
    'panel-tab': (el) => {
      const v = el.dataset.value;
      if (S.panel && S.panelTab === v && el.closest('.npbar')) S.panel = false;
      else { S.panel = true; S.panelTab = v; }
      $('#app').classList.toggle('has-panel', S.panel);
      const ph = $('.panel');
      if (ph) ph.innerHTML = panelHtml();
      const np = $('#npbar');
      if (np) np.innerHTML = npInner();
    },
    'panel-close': () => {
      S.panel = false;
      $('#app').classList.remove('has-panel');
      const np = $('#npbar');
      if (np) np.innerHTML = npInner();
    },
    'clear-manual': () => {
      const was = S.manual;
      S.manual = [];
      refreshTrack();
      toast('Очередь «добавлено вручную» очищена', { icon: 'trash', undo: () => { S.manual = was; refreshTrack(); } });
    },
    sheet: (el) => {
      const v = el.dataset.value;
      if (v === 'queue') openSheet('Очередь', queueHtml(), 'queue');
      else openSheet('Текст', emptyState({ title: `Текста для «${esc(S.current.t.title)}» пока нет`, text: 'Сервер поищет его ночью, вместе с анализом трека.', action: 'Найти текст' }));
    },
    'sheet-close': () => closeSheet(),
    'wave-sheet': () => {
      const group = (g) => `<div class="t-overline" style="padding:14px 4px 6px">${OPTS[g].label}</div><div style="display:flex;flex-wrap:wrap;gap:8px">${OPTS[g].items.map((it) => `<button class="chip" aria-pressed="${S.wave[g] === it.id}" data-act="set-wave-chip" data-group="${g}" data-value="${it.id}">${it.icon ? ic(it.icon) : ''}${it.name}</button>`).join('')}</div>`;
      openSheet('Настройки волны', `<div style="padding:0 4px 8px">${group('activity')}${group('character')}${group('lang')}</div>`, 'wave');
    },
    'set-wave-chip': (el) => {
      setWave(el.dataset.group, el.dataset.value);
      $$(`[data-act="set-wave-chip"][data-group="${el.dataset.group}"]`).forEach((c) => c.setAttribute('aria-pressed', c.dataset.value === el.dataset.value));
    },
    ctx: (el) => {
      const [kind] = (el.dataset.ctxFor || 'track').split(':');
      if (S.layout === 'phone') return openSheet(S.current.t.title, `<div class="pop pop--static" style="max-width:none;background:none;box-shadow:none;padding:0">${ctxMenu(kind)}</div>`);
      openPop(el, ctxMenu(kind), { place: 'bottom-end', width: 280 });
    },
    'ctx-next': () => { closePop(); closeSheet(); toast('Сыграет следующим', { icon: 'arrow-bend-down-right', undo: () => {} }); },
    'ctx-queue': () => { closePop(); closeSheet(); toast('Добавлено в очередь', { icon: 'list-plus', undo: () => {} }); },
    'ctx-radio': () => { closePop(); closeSheet(); W.start(); toast('Волна по треку — подбираем похожее', { icon: 'radio', undo: false }); },
    'ctx-download': () => { closePop(); closeSheet(); toast('Скачиваем на это устройство', { icon: 'download-simple', undo: () => {} }); },
    'ctx-ban': () => { closePop(); closeSheet(); toast('Больше не будем рекомендовать', { icon: 'prohibit', undo: () => {} }); },
    menu: () => closePop(),
    undo: () => { toastUndo?.(); toastUndo = null; $$('.toast', layer).forEach((t) => t.remove()); },
    'toast-close': () => $$('.toast', layer).forEach((t) => t.remove()),
    rail: (el) => {
      const rail = el.closest('.shelf')?.querySelector('[data-rail]');
      rail?.scrollBy({ left: Number(el.dataset.dir) * rail.clientWidth * 0.8, behavior: reduced.matches ? 'auto' : 'smooth' });
    },
    mute: () => {
      S.volume = S.volume > 0 ? 0 : 0.72;
      W.setVolume(S.volume);
      const np = $('#npbar');
      if (np) np.innerHTML = npInner();
    },
    switch: (el) => el.setAttribute('aria-checked', el.getAttribute('aria-checked') !== 'true'),
    seg: (el) => $$('.seg__opt', el.parentElement).forEach((o) => o.setAttribute('aria-checked', o === el)),
    tab: (el) => $$('.tab', el.parentElement).forEach((o) => o.setAttribute('aria-selected', o === el)),
    // Отладка волны
    'dbg-mood': (el) => {
      $$('.seg__opt', el.parentElement).forEach((o) => o.setAttribute('aria-checked', o === el));
      S.wave.mood = el.dataset.value;
      applyMood();
    },
    'dbg-start': () => W.start(),
    'dbg-skip': () => W.skip(),
    'dbg-like': () => W.like(),
    'dbg-preview': (el) => {
      const playingThis = W.state.audio && W.state.url === el.dataset.url;
      if (playingThis) { W.pause(); return dbgMark(); }
      W.play(el.dataset.url);
      W.setCover(el.dataset.cover);
      dbgMark(el.dataset.url);
    },
    'energy-auto': (el) => {
      const on = el.getAttribute('aria-pressed') !== 'true';
      el.setAttribute('aria-pressed', on);
      W.setEnergyAuto(on);
    },
  };
  function dbgMark(url = W.state.audio ? W.state.url : null) {
    $$('.prev').forEach((p) => {
      const on = p.dataset.url === url;
      p.classList.toggle('is-playing', on);
      const i = p.querySelector('.prev__art i');
      if (i) i.className = `ph-fill ph-${on ? 'pause' : 'play'}`;
    });
  }
  function dbgAudio(e) {
    if (e.type === 'ended' || e.type === 'pause') dbgMark(null);
    if (e.type === 'time') { const el = $('[data-prev-time]'); if (el) el.textContent = `${fmtTime(e.t * 1000)} / ${fmtTime((e.d || 30) * 1000)}`; }
  }

  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-act]');
    if (pop && !e.target.closest('.pop') && (!el || el !== pop.anchor)) closePop();
    if (!el) return;
    const fn = ACT[el.dataset.act];
    if (!fn) return;
    if (el.tagName === 'A' && el.dataset.act !== 'go-home') e.preventDefault();
    fn(el, e);
  });
  document.addEventListener('contextmenu', (e) => {
    const el = e.target.closest('[data-ctx]');
    if (!el || S.layout === 'phone') return;
    e.preventDefault();
    openPop(null, ctxMenu(el.dataset.ctx.split(':')[0]), { x: e.clientX, y: e.clientY, width: 280 });
  });
  document.addEventListener('dblclick', (e) => {
    const row = e.target.closest('.track[data-track]');
    if (row && !row.closest('.sheet')) toast('Играем с этого трека — волна продолжит дальше', { icon: 'play-circle', undo: false });
  });
  document.addEventListener('keydown', (e) => {
    const tag = e.target.tagName;
    if (e.key === 'Escape') { if (pop) closePop(true); else closeSheet(); return; }
    if (pop && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      const items = $$('.menu-item:not(:disabled)', pop.el);
      const i = items.indexOf(document.activeElement);
      items[(i + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
      e.preventDefault();
      return;
    }
    if (tag === 'INPUT' || tag === 'TEXTAREA') return;
    if (e.code === 'Space' && tag !== 'BUTTON' && !e.target.closest('[role="slider"]')) { e.preventDefault(); toggle(); }
    if ((e.metaKey || e.ctrlKey) && e.key === 'ArrowRight') { e.preventDefault(); next('skip'); }
    if ((e.metaKey || e.ctrlKey) && e.key === 'ArrowLeft') { e.preventDefault(); prev(); }
    if (e.key === 'Enter' && e.target.matches('.track[data-track]')) toast('Играем с этого трека — волна продолжит дальше', { icon: 'play-circle', undo: false });
  });

  // Ползунки и форма волны: мышь, палец и клавиатура
  document.addEventListener('pointerdown', (e) => {
    const r = e.target.closest('[data-range], [data-scrub]');
    if (!r || r.closest('.sheet')) return;
    e.preventDefault();
    r.setPointerCapture?.(e.pointerId);
    r.classList.add('is-drag');
    const move = (ev) => slide(r, (ev.clientX - r.getBoundingClientRect().left) / r.getBoundingClientRect().width);
    move(e);
    const up = () => { r.classList.remove('is-drag'); r.removeEventListener('pointermove', move); r.removeEventListener('pointerup', up); r.removeEventListener('pointercancel', up); };
    r.addEventListener('pointermove', move);
    r.addEventListener('pointerup', up);
    r.addEventListener('pointercancel', up);
  });
  document.addEventListener('pointermove', (e) => {
    const r = e.target.closest?.('[data-scrub]');
    if (r) r.style.setProperty('--h', clamp((e.clientX - r.getBoundingClientRect().left) / r.getBoundingClientRect().width));
  });
  document.addEventListener('keydown', (e) => {
    const r = e.target.closest?.('[data-range], [data-scrub]');
    if (!r || !['ArrowLeft', 'ArrowRight'].includes(e.key) || e.metaKey || e.ctrlKey) return;
    e.preventDefault();
    const d = e.key === 'ArrowRight' ? 1 : -1;
    if (r.dataset.scrub != null) slide(r, (S.pos + d * 5000) / S.current.t.durationMs);
    else slide(r, Number(r.style.getPropertyValue('--v')) / 100 + d * 0.05);
  });
  function slide(r, f) {
    f = clamp(f);
    if (r.dataset.scrub != null) {
      S.pos = f * S.current.t.durationMs;
      const a = W.audio;
      if (S.playing && a && isFinite(a.duration)) W.seek(f * a.duration);
      paintProgress();
      return;
    }
    r.style.setProperty('--v', Math.round(f * 100));
    r.setAttribute('aria-valuenow', Math.round(f * 100));
    const kind = r.dataset.range;
    if (kind === 'volume') { S.volume = f; W.setVolume(f); }
    if (kind === 'expressive') S.expressive = f;
    if (kind === 'energy') {
      W.setEnergy(f);
      const auto = $('[data-act="energy-auto"]');
      auto?.setAttribute('aria-pressed', 'false');
      const lbl = $('[data-energy]');
      if (lbl) lbl.textContent = f.toFixed(2).replace('.', ',');
    }
  }

  // Пульс кольца кнопки и точки доли — каждый кадр волны
  W.on('frame', ({ pulse, energy, bpm, fps }) => {
    for (const el of $$('[data-pulse]')) if (!el.closest('.sheet')) el.style.setProperty('--pulse', pulse.toFixed(3));
    const dot = $('[data-pulse-dot]');
    if (dot) {
      dot.style.setProperty('--pulse', pulse.toFixed(3));
      const b = $('[data-bpm]'), f = $('[data-fps]'), en = $('[data-energy]');
      if (b) b.textContent = `${Math.round(bpm)}${NB}уд/мин`;
      if (f) f.textContent = `${fps}${NB}к/с`;
      if (en && W.state.energyAuto) {
        en.textContent = energy.toFixed(2).replace('.', ',');
        // В режиме «Авто» ползунок показывает энергию, которую слышит анализатор
        $('[data-range="energy"]')?.style.setProperty('--v', Math.round(energy * 100));
      }
    }
  });

  // ---------- Маршруты ----------
  function go() {
    const r = (location.hash.slice(1) || 'home').split('?')[0];
    const nextRoute = ['home', 'player', 'wave', 'components'].includes(r) ? r : 'home';
    const prevRoute = S.route;
    S.route = nextRoute;
    if (nextRoute === 'player' && S.layout !== 'phone' && prevRoute !== 'player') { S.panel = true; S.panelTab = 'queue'; }
    // Обложка «перелетает» из панели в плеер
    const swap = () => render();
    if (document.startViewTransition && !reduced.matches && prevRoute !== nextRoute && !BARE.has(prevRoute) && !BARE.has(nextRoute) && booted) {
      document.startViewTransition(swap);
    } else swap();
  }
  let booted = false;
  window.addEventListener('hashchange', go);
  let lastLayout = detect().layout;
  addEventListener('resize', () => {
    const l = detect().layout;
    if (l !== lastLayout) { lastLayout = l; render(); }
  });

  applyMood(true);
  applyGel();
  go();
  booted = true;

  // Для отладки из консоли и для скриншотов состояний
  window.DW = { state: S, play, pause, next, like, setWave, toast };
})();
