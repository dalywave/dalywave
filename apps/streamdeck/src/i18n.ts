// Тексты на кнопках и полосе. Язык — как у Stream Deck: русский, украинский или английский (остальные —
// английский). Названия действий и подсказки — в rest.daly.wave.sdPlugin/{ru,en,uk}.json, тексты
// инспектора — ui/pi.js.

const RU = {
  connect: 'Подключите',
  offline: 'Нет связи',
  noDevice: 'Нет устройства',
  nothing: 'Тишина',
  volume: 'Громкость',
  muted: 'Без звука',
  sleepOff: 'Сон',
  wave: 'Моя волна',
  discover: 'Открытия',
  liked: 'Любимые треки',
  history: 'История',
  queueEmpty: 'Очередь пуста',
  next: 'Далее',
  of: 'из',
  empty: 'Пусто',
  loading: 'Загрузка',
  search: 'Поиск',
  thisDevice: 'Это устройство',
  playing: 'Играет',
  transition: { gapless: 'Без пауз', smooth: 'Плавный', smart: 'Умный', dj: 'Диджей' },
  repeat: { off: 'Повтор', all: 'Все', one: 'Один' },
  mood: { any: 'Любое', energetic: 'Бодрое', happy: 'Весёлое', calm: 'Спокойное', sad: 'Грустное' },
  activity: { any: '', wake: 'Утро', road: 'Дорога', work: 'Работа', sport: 'Тренировка', rest: 'Отдых', sleep: 'Сон', party: 'Вечеринка' },
  character: { fav: 'Любимое', mix: 'Знакомое и новое', new: 'Новое', pop: 'Популярное', rare: 'Редкое' },
  language: { any: '', ru: 'На русском', foreign: 'Иностранное', none: 'Без слов' },
  modeTracks: 'Треки',
  modeSeek: 'Перемотка',
  modeVolume: 'Громкость',
  added: 'Добавлено',
  min: 'мин',
}

type Strings = typeof RU

const EN: Strings = {
  connect: 'Connect',
  offline: 'Offline',
  noDevice: 'No device',
  nothing: 'Silence',
  volume: 'Volume',
  muted: 'Muted',
  sleepOff: 'Sleep',
  wave: 'My wave',
  discover: 'Discover',
  liked: 'Liked tracks',
  history: 'History',
  queueEmpty: 'Queue is empty',
  next: 'Next',
  of: 'of',
  empty: 'Empty',
  loading: 'Loading',
  search: 'Search',
  thisDevice: 'This device',
  playing: 'Playing',
  transition: { gapless: 'Gapless', smooth: 'Smooth', smart: 'Smart', dj: 'DJ' },
  repeat: { off: 'Repeat', all: 'All', one: 'One' },
  mood: { any: 'Any', energetic: 'Energetic', happy: 'Happy', calm: 'Calm', sad: 'Sad' },
  activity: { any: '', wake: 'Morning', road: 'Road', work: 'Work', sport: 'Workout', rest: 'Unwind', sleep: 'Sleep', party: 'Party' },
  character: { fav: 'Favourites', mix: 'Familiar and new', new: 'New', pop: 'Popular', rare: 'Rare' },
  language: { any: '', ru: 'In Russian', foreign: 'Foreign', none: 'Instrumental' },
  modeTracks: 'Tracks',
  modeSeek: 'Seek',
  modeVolume: 'Volume',
  added: 'Added',
  min: 'min',
}

const UK: Strings = {
  connect: 'Підключіть',
  offline: 'Немає зв’язку',
  noDevice: 'Немає пристрою',
  nothing: 'Тиша',
  volume: 'Гучність',
  muted: 'Без звуку',
  sleepOff: 'Сон',
  wave: 'Моя хвиля',
  discover: 'Відкриття',
  liked: 'Вподобане',
  history: 'Історія',
  queueEmpty: 'Черга порожня',
  next: 'Далі',
  of: 'з',
  empty: 'Порожньо',
  loading: 'Завантаження',
  search: 'Пошук',
  thisDevice: 'Цей пристрій',
  playing: 'Грає',
  transition: { gapless: 'Без пауз', smooth: 'Плавний', smart: 'Розумний', dj: 'Діджей' },
  repeat: { off: 'Повтор', all: 'Усі', one: 'Один' },
  mood: { any: 'Будь-яке', energetic: 'Бадьоре', happy: 'Веселе', calm: 'Спокійне', sad: 'Сумне' },
  activity: { any: '', wake: 'Ранок', road: 'Дорога', work: 'Робота', sport: 'Тренування', rest: 'Відпочинок', sleep: 'Сон', party: 'Вечірка' },
  character: { fav: 'Улюблене', mix: 'Знайоме й нове', new: 'Нове', pop: 'Популярне', rare: 'Рідкісне' },
  language: { any: '', ru: 'Російською', foreign: 'Іноземне', none: 'Без слів' },
  modeTracks: 'Треки',
  modeSeek: 'Перемотка',
  modeVolume: 'Гучність',
  added: 'Додано',
  min: 'хв',
}

export type Lang = 'ru' | 'en' | 'uk'

const STRINGS: Record<Lang, Strings> = { ru: RU, en: EN, uk: UK }

let currentLang: Lang = 'ru'

/** Язык Stream Deck: `ru`, `uk`, `en`, `de`… */
export function setLanguage(lang: string | undefined): void {
  const l = lang?.toLowerCase() ?? ''
  currentLang = l.startsWith('ru') ? 'ru' : l.startsWith('uk') ? 'uk' : 'en'
}

export function t(): Strings {
  return STRINGS[currentLang]
}

export function language(): Lang {
  return currentLang
}
