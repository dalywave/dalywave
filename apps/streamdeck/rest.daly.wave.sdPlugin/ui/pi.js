// Общее для инспекторов DalyWave: блок подключения аккаунта, переводы, связь с плагином.
// Поля настроек — sdpi-components (sdpi-components.js), они сами пишут настройки кнопки.
/* global SDPIComponents */
(function () {
  const STRINGS = {
    ru: {
      connectHint: 'Подключите DalyWave — кнопки будут управлять музыкой там, где она играет: на сайте, Mac, Windows или iPhone.',
      connect: 'Подключить',
      codeHint: 'Войдите в открывшемся окне браузера — после входа Stream Deck подключится сам.',
      openAgain: 'Открыть снова',
      cancel: 'Отмена',
      expired: 'Вход не завершён — подключите ещё раз.',
      error: 'Сервер не ответил. Проверьте сеть и попробуйте ещё раз.',
      connectedAs: 'Подключено',
      online: 'на связи',
      offline: 'нет связи с сервером',
      connecting: 'подключаемся…',
      disconnect: 'Отключить',
      server: 'Сервер',
      advanced: 'Дополнительно',
      // Поля
      mode: 'Действие',
      toggle: 'Играть / пауза',
      play: 'Играть',
      pause: 'Пауза',
      next: 'Следующий',
      previous: 'Предыдущий',
      shuffleToggle: 'Переключать',
      on: 'Включить',
      off: 'Выключить',
      volumeUp: 'Громче',
      volumeDown: 'Тише',
      mute: 'Без звука',
      step: 'Шаг, %',
      dialStep: 'Шаг крутилки, %',
      target: 'Куда',
      liked: 'Любимые треки',
      playlist: 'Плейлист',
      source: 'Что играть',
      history: 'История',
      album: 'Альбом',
      artist: 'Артист',
      chart: 'Чарт',
      playlists: 'Мои плейлисты',
      mixes: 'Собрано для вас',
      albums: 'Сохранённые альбомы',
      artists: 'Мои артисты',
      charts: 'Чарты',
      find: 'Найти',
      findPlaceholder: 'Название',
      found: 'Найдено',
      shuffle: 'Перемешать',
      query: 'Запрос',
      queryPlaceholder: 'Что найти',
      kind: 'Искать',
      tracks: 'Треки',
      where: 'Где',
      catalog: 'Везде',
      library: 'В медиатеке',
      preset: 'Настройка',
      ownSettings: 'Своя',
      mood: 'Настроение',
      activity: 'Занятие',
      character: 'Характер',
      language: 'Язык',
      any: 'Любое',
      device: 'Устройство',
      minutes: 'Минут',
      sleepMode: 'Нажатие',
      sleepToggle: 'Запустить / остановить',
      sleepRestart: 'Перезапустить',
      sleepExtend: 'Продлить',
      pulse: 'Пульс в такт',
      pulseLabel: 'Мягко мигать в такт треку',
      dialMode: 'Крутилка',
      modeTracks: 'Треки',
      modeSeek: 'Перемотка',
      modeVolume: 'Громкость',
      energetic: 'Бодрое', happy: 'Весёлое', calm: 'Спокойное', sad: 'Грустное',
      wake: 'Просыпаюсь', road: 'В дороге', work: 'Работаю', sport: 'Тренируюсь', rest: 'Отдыхаю', sleep: 'Засыпаю', party: 'Вечеринка',
      fav: 'Любимое', mix: 'Знакомое и новое', new: 'Незнакомое', pop: 'Популярное', rare: 'Редкое',
      langRu: 'Русский', langForeign: 'Иностранный', langNone: 'Без слов',
      helpQueue: 'Кнопка показывает следующий трек и переходит к нему. Крутилка листает очередь.',
      helpDiscover: 'Включает волну только из нового для вас. Крутилка листает новые треки.',
      helpNow: 'Нажатие — играть или пауза, долгое — «Нравится».',
      helpTransition: 'Без пауз → Плавный → Умный → Диджей. «Диджей» — там, где он есть.',
      helpLike: 'Добавляет текущий трек в «Любимые» или убирает.',
      helpDislike: 'Волна запомнит и перейдёт к следующему треку.',
      helpNotNow: 'Пропустить трек, не отмечая «Не нравится».',
      helpRepeat: 'По кругу: без повтора, всё, один трек.',
      helpMultimedia: 'Долгое касание полосы меняет режим. Поворот с нажатием — перемотка.',
      helpVolume: 'Громкость DalyWave на устройстве, где играет музыка.',
    },
    en: {
      connectHint: 'Connect DalyWave and your keys will control music wherever it plays: web, Mac, Windows or iPhone.',
      connect: 'Connect',
      codeHint: 'Sign in in the browser window that opened — Stream Deck connects on its own.',
      openAgain: 'Open again',
      cancel: 'Cancel',
      expired: 'Sign-in didn’t finish — connect again.',
      error: 'The server didn’t respond. Check your connection and try again.',
      connectedAs: 'Connected',
      online: 'online',
      offline: 'server unreachable',
      connecting: 'connecting…',
      disconnect: 'Disconnect',
      server: 'Server',
      advanced: 'Advanced',
      mode: 'Action',
      toggle: 'Play / pause',
      play: 'Play',
      pause: 'Pause',
      next: 'Next',
      previous: 'Previous',
      shuffleToggle: 'Toggle',
      on: 'Turn on',
      off: 'Turn off',
      volumeUp: 'Up',
      volumeDown: 'Down',
      mute: 'Mute',
      step: 'Step, %',
      dialStep: 'Dial step, %',
      target: 'Add to',
      liked: 'Liked tracks',
      playlist: 'Playlist',
      source: 'Play',
      history: 'History',
      album: 'Album',
      artist: 'Artist',
      chart: 'Chart',
      playlists: 'My playlists',
      mixes: 'Made for you',
      albums: 'Saved albums',
      artists: 'My artists',
      charts: 'Charts',
      find: 'Find',
      findPlaceholder: 'Title',
      found: 'Found',
      shuffle: 'Shuffle',
      query: 'Query',
      queryPlaceholder: 'What to find',
      kind: 'Search for',
      tracks: 'Tracks',
      where: 'Where',
      catalog: 'Everywhere',
      library: 'In library',
      preset: 'Setup',
      ownSettings: 'Custom',
      mood: 'Mood',
      activity: 'Activity',
      character: 'Character',
      language: 'Language',
      any: 'Any',
      device: 'Device',
      minutes: 'Minutes',
      sleepMode: 'Press',
      sleepToggle: 'Start / stop',
      sleepRestart: 'Restart',
      sleepExtend: 'Extend',
      pulse: 'Beat pulse',
      pulseLabel: 'Softly pulse to the beat',
      dialMode: 'Dial',
      modeTracks: 'Tracks',
      modeSeek: 'Seek',
      modeVolume: 'Volume',
      energetic: 'Energetic', happy: 'Happy', calm: 'Calm', sad: 'Sad',
      wake: 'Waking up', road: 'On the road', work: 'Working', sport: 'Working out', rest: 'Unwinding', sleep: 'Falling asleep', party: 'Party',
      fav: 'Favourites', mix: 'Familiar and new', new: 'New to me', pop: 'Popular', rare: 'Rare',
      langRu: 'Russian', langForeign: 'Foreign', langNone: 'Instrumental',
      helpQueue: 'The key shows the next track and jumps to it. The dial browses the queue.',
      helpDiscover: 'Starts a wave of music new to you. The dial browses new tracks.',
      helpNow: 'Press to play or pause, hold to like.',
      helpTransition: 'Gapless → Smooth → Smart → DJ. DJ only where available.',
      helpLike: 'Adds the current track to Liked or removes it.',
      helpDislike: 'The wave remembers and skips to the next track.',
      helpNotNow: 'Skip the track without disliking it.',
      helpRepeat: 'Cycles: off, repeat all, repeat one.',
      helpMultimedia: 'Long-touch the strip to change mode. Rotate while pressed to seek.',
      helpVolume: 'DalyWave volume on the device that is playing.',
    },
  }

  let lang = (navigator.language || 'en').toLowerCase().startsWith('ru') ? 'ru' : 'en'
  const s = key => (STRINGS[lang] && STRINGS[lang][key]) || STRINGS.en[key] || key

  function translate(root = document) {
    root.querySelectorAll('[data-t]').forEach((el) => { el.textContent = s(el.dataset.t) })
    root.querySelectorAll('[data-t-label]').forEach((el) => { el.setAttribute('label', s(el.dataset.tLabel)) })
    root.querySelectorAll('[data-t-placeholder]').forEach((el) => { el.setAttribute('placeholder', s(el.dataset.tPlaceholder)) })
  }

  const client = SDPIComponents.streamDeckClient
  const toPlugin = payload => client.send('sendToPlugin', payload)
  const listeners = []

  client.sendToPropertyInspector.subscribe((ev) => {
    const p = ev.payload || {}
    if (p.event === 'account') renderAccount(p)
    for (const fn of listeners) fn(p)
  })

  // ---------- блок аккаунта ----------

  const box = document.createElement('div')
  box.className = 'dw-account'

  function el(tag, cls, text) {
    const e = document.createElement(tag)
    if (cls) e.className = cls
    if (text !== undefined) e.textContent = text
    return e
  }

  function button(text, onClick, primary) {
    const b = el('button', primary ? 'dw-btn dw-btn--primary' : 'dw-btn', text)
    b.type = 'button'
    b.addEventListener('click', onClick)
    return b
  }

  let last = null

  function renderAccount(a) {
    last = a
    box.replaceChildren()
    document.body.classList.toggle('dw-unpaired', !a.connected)
    if (a.connected) {
      const row = el('div', 'dw-row')
      const who = el('div', 'dw-who')
      who.append(el('span', 'dw-dot dw-dot--' + a.status), el('span', '', `${s('connectedAs')}${a.userName ? ` · ${a.userName}` : ''}`))
      row.append(who, button(s('disconnect'), () => toPlugin({ event: 'disconnect' })))
      box.append(row)
      if (a.status === 'offline') box.append(el('p', 'dw-muted', s('offline')))
    }
    else if (a.pairing && a.pairing.phase === 'waiting') {
      box.append(el('p', 'dw-muted', s('codeHint')))
      const row = el('div', 'dw-row')
      row.append(button(s('openAgain'), () => toPlugin({ event: 'pair' }), true), button(s('cancel'), () => toPlugin({ event: 'cancelPair' })))
      box.append(row)
    }
    else {
      box.append(el('p', 'dw-muted', a.pairing && a.pairing.phase === 'expired' ? s('expired') : a.pairing && a.pairing.phase === 'error' ? s('error') : s('connectHint')))
      box.append(button(s('connect'), () => toPlugin({ event: 'pair' }), true))
    }
    // Адрес сервера — для проверки на своём сервере; по умолчанию wave.daly.rest
    const details = el('details', 'dw-advanced')
    details.append(el('summary', '', s('advanced')))
    const row = el('div', 'dw-row')
    const input = el('input', 'dw-input')
    input.value = (a.server || '').replace(/^https:\/\//, '')
    input.setAttribute('aria-label', s('server'))
    input.addEventListener('change', () => toPlugin({ event: 'setServer', server: input.value }))
    row.append(el('span', 'dw-label', s('server')), input)
    details.append(row)
    box.append(details)
  }

  /** Список для sdpi-select с datasource: перезапросить, когда подключились */
  function refreshLists() {
    document.querySelectorAll('sdpi-select[datasource]').forEach((e) => {
      if (typeof e.refresh === 'function') e.refresh()
    })
  }

  listeners.push((p) => {
    if (p.event === 'account' && p.connected && last && last.connected !== true) refreshLists()
  })

  document.addEventListener('DOMContentLoaded', () => {
    document.body.prepend(box)
    translate()
    renderAccount({ connected: false, status: 'unpaired', pairing: { phase: 'idle' }, server: '' })
    client.getConnectionInfo().then((info) => {
      const l = info && info.info && info.info.application && info.info.application.language
      if (l) {
        lang = String(l).toLowerCase().startsWith('ru') ? 'ru' : 'en'
        translate()
      }
      toPlugin({ event: 'account' })
    })
  })

  window.DW = { s, toPlugin, onMessage: fn => listeners.push(fn), translate }
})()
