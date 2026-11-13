// Тексты на кнопках и полосе. Язык — как у Stream Deck: русский или английский (остальные — английский).
// Названия действий и подсказки — в rest.daly.wave.sdPlugin/{ru,en}.json, тексты инспектора — ui/i18n.js.

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

let current: Strings = RU

/** Язык Stream Deck: `ru`, `en`, `de`… */
export function setLanguage(lang: string | undefined): void {
  current = lang?.toLowerCase().startsWith('ru') ? RU : EN
}

export function t(): Strings {
  return current
}

export function isRussian(): boolean {
  return current === RU
}
