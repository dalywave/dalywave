// Список действий плагина — один на манифест, переводы и картинки по умолчанию (scripts/assets.ts).

export type Controller = 'Keypad' | 'Encoder' | 'Neo'

export interface ActionDef {
  id: string
  controllers: Controller[]
  ru: { name: string, tooltip: string, rotate?: string, push?: string, touch?: string, longTouch?: string }
  en: { name: string, tooltip: string, rotate?: string, push?: string, touch?: string, longTouch?: string }
  uk: { name: string, tooltip: string, rotate?: string, push?: string, touch?: string, longTouch?: string }
  /** Раскладка полосы по умолчанию */
  layout?: string
}

export const ACTIONS: ActionDef[] = [
  {
    id: 'nowplaying',
    controllers: ['Keypad', 'Neo'],
    ru: { name: 'Сейчас играет', tooltip: 'Обложка и прогресс трека. Нажатие — играть или пауза, долгое — «Нравится»' },
    en: { name: 'Now playing', tooltip: 'Cover and progress. Press to play or pause, hold to like' },
    uk: { name: 'Зараз грає', tooltip: 'Обкладинка й прогрес треку. Натискання — грати або пауза, довге — «Подобається»' },
    layout: 'layouts/neo.json',
  },
  {
    id: 'playback',
    controllers: ['Keypad'],
    ru: { name: 'Воспроизведение', tooltip: 'Играть, пауза, следующий или предыдущий трек' },
    en: { name: 'Playback', tooltip: 'Play, pause, next or previous track' },
    uk: { name: 'Відтворення', tooltip: 'Грати, пауза, наступний або попередній трек' },
  },
  {
    id: 'multimedia',
    controllers: ['Encoder'],
    ru: { name: 'Мультимедиа', tooltip: 'Крутилка: треки, перемотка или громкость', rotate: 'Треки', push: 'Играть / пауза', touch: 'Играть / пауза', longTouch: 'Сменить режим' },
    en: { name: 'Multimedia', tooltip: 'Dial: tracks, seek or volume', rotate: 'Tracks', push: 'Play / pause', touch: 'Play / pause', longTouch: 'Change mode' },
    uk: { name: 'Мультимедіа', tooltip: 'Регулятор: треки, перемотка або гучність', rotate: 'Треки', push: 'Грати / пауза', touch: 'Грати / пауза', longTouch: 'Змінити режим' },
    layout: 'layouts/track.json',
  },
  {
    id: 'volume',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Громкость', tooltip: 'Громкость DalyWave там, где играет музыка', rotate: 'Громкость', push: 'Играть / пауза', touch: 'Играть / пауза', longTouch: 'Без звука' },
    en: { name: 'Volume', tooltip: 'DalyWave volume on the device that is playing', rotate: 'Volume', push: 'Play / pause', touch: 'Play / pause', longTouch: 'Mute' },
    uk: { name: 'Гучність', tooltip: 'Гучність DalyWave там, де грає музика', rotate: 'Гучність', push: 'Грати / пауза', touch: 'Грати / пауза', longTouch: 'Без звуку' },
    layout: 'layouts/volume.json',
  },
  {
    id: 'shuffle',
    controllers: ['Keypad'],
    ru: { name: 'Перемешать', tooltip: 'Перемешивание очереди альбома или плейлиста' },
    en: { name: 'Shuffle', tooltip: 'Shuffle the album or playlist' },
    uk: { name: 'Перемішати', tooltip: 'Перемішування черги альбому або плейлиста' },
  },
  {
    id: 'repeat',
    controllers: ['Keypad'],
    ru: { name: 'Повтор', tooltip: 'Без повтора, всё по кругу, один трек' },
    en: { name: 'Repeat', tooltip: 'Off, repeat all, repeat one' },
    uk: { name: 'Повтор', tooltip: 'Без повтору, усе по колу, один трек' },
  },
  {
    id: 'like',
    controllers: ['Keypad'],
    ru: { name: 'Нравится', tooltip: 'Добавить трек в «Любимые» или убрать' },
    en: { name: 'Like', tooltip: 'Add the track to Liked or remove it' },
    uk: { name: 'Подобається', tooltip: 'Додати трек до «Вподобаного» або прибрати' },
  },
  {
    id: 'dislike',
    controllers: ['Keypad'],
    ru: { name: 'Не нравится', tooltip: 'Волна запомнит и перейдёт к следующему' },
    en: { name: 'Dislike', tooltip: 'The wave remembers and skips' },
    uk: { name: 'Не подобається', tooltip: 'Хвиля запам’ятає й перейде до наступного' },
  },
  {
    id: 'notnow',
    controllers: ['Keypad'],
    ru: { name: 'Не сейчас', tooltip: 'Пропустить без «не нравится»' },
    en: { name: 'Not now', tooltip: 'Skip without disliking' },
    uk: { name: 'Не зараз', tooltip: 'Пропустити без «не подобається»' },
  },
  {
    id: 'add',
    controllers: ['Keypad'],
    ru: { name: 'Добавить в…', tooltip: 'Текущий трек — в плейлист или в «Любимые»; повторно — убрать' },
    en: { name: 'Add to…', tooltip: 'Add the current track to a playlist or Liked; press again to remove' },
    uk: { name: 'Додати до…', tooltip: 'Поточний трек — до плейлиста або «Вподобаного»; повторно — прибрати' },
  },
  {
    id: 'collection',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Играть коллекцию', tooltip: 'Плейлист, альбом, артист, «Любимые», миксы, чарты, история', rotate: 'Листать', push: 'Играть', touch: 'Играть', longTouch: 'Обновить' },
    en: { name: 'Play collection', tooltip: 'Playlist, album, artist, Liked, mixes, charts, history', rotate: 'Browse', push: 'Play', touch: 'Play', longTouch: 'Refresh' },
    uk: { name: 'Грати колекцію', tooltip: 'Плейлист, альбом, виконавець, «Вподобане», мікси, чарти, історія', rotate: 'Гортати', push: 'Грати', touch: 'Грати', longTouch: 'Оновити' },
    layout: 'layouts/list.json',
  },
  {
    id: 'search',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Сохранённый поиск', tooltip: 'Найти и включить: треки, альбом или артиста', rotate: 'Листать', push: 'Играть', touch: 'Играть', longTouch: 'Обновить' },
    en: { name: 'Saved search', tooltip: 'Search and play tracks, an album or an artist', rotate: 'Browse', push: 'Play', touch: 'Play', longTouch: 'Refresh' },
    uk: { name: 'Збережений пошук', tooltip: 'Знайти й увімкнути: треки, альбом або виконавця', rotate: 'Гортати', push: 'Грати', touch: 'Грати', longTouch: 'Оновити' },
    layout: 'layouts/list.json',
  },
  {
    id: 'queue',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Очередь', tooltip: 'Что дальше; нажатие — к этому треку', rotate: 'Листать', push: 'Перейти', touch: 'Перейти', longTouch: 'К началу' },
    en: { name: 'Queue', tooltip: 'Up next; press to jump to it', rotate: 'Browse', push: 'Jump', touch: 'Jump', longTouch: 'Back to top' },
    uk: { name: 'Черга', tooltip: 'Що далі; натискання — до цього треку', rotate: 'Гортати', push: 'Перейти', touch: 'Перейти', longTouch: 'На початок' },
    layout: 'layouts/list.json',
  },
  {
    id: 'wave',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Моя волна', tooltip: 'Волна с настроением, занятием или сохранённой настройкой', rotate: 'Настроения', push: 'Включить', touch: 'Включить', longTouch: 'Обновить' },
    en: { name: 'My wave', tooltip: 'Wave with a mood, activity or saved setup', rotate: 'Moods', push: 'Start', touch: 'Start', longTouch: 'Refresh' },
    uk: { name: 'Моя хвиля', tooltip: 'Хвиля з настроєм, заняттям або збереженим налаштуванням', rotate: 'Настрої', push: 'Увімкнути', touch: 'Увімкнути', longTouch: 'Оновити' },
    layout: 'layouts/list.json',
  },
  {
    id: 'discover',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Открытия', tooltip: 'Только новое для вас', rotate: 'Листать', push: 'Играть', touch: 'Играть', longTouch: 'Ещё' },
    en: { name: 'Discover', tooltip: 'Only music new to you', rotate: 'Browse', push: 'Play', touch: 'Play', longTouch: 'More' },
    uk: { name: 'Відкриття', tooltip: 'Лише нове для вас', rotate: 'Гортати', push: 'Грати', touch: 'Грати', longTouch: 'Ще' },
    layout: 'layouts/list.json',
  },
  {
    id: 'transfer',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Передать на устройство', tooltip: 'Перенести музыку на компьютер, телефон или сайт', rotate: 'Устройства', push: 'Перенести', touch: 'Перенести' },
    en: { name: 'Play on device', tooltip: 'Move playback to a computer, phone or browser', rotate: 'Devices', push: 'Move', touch: 'Move' },
    uk: { name: 'Передати на пристрій', tooltip: 'Перенести музику на комп’ютер, телефон або сайт', rotate: 'Пристрої', push: 'Перенести', touch: 'Перенести' },
    layout: 'layouts/list.json',
  },
  {
    id: 'transition',
    controllers: ['Keypad'],
    ru: { name: 'Режим переходов', tooltip: 'Без пауз → Плавный → Умный → Диджей' },
    en: { name: 'Transitions', tooltip: 'Gapless → Smooth → Smart → DJ' },
    uk: { name: 'Режим переходів', tooltip: 'Без пауз → Плавний → Розумний → Діджей' },
  },
  {
    id: 'sleep',
    controllers: ['Keypad'],
    ru: { name: 'Таймер сна', tooltip: 'Пауза через заданное время; долгое нажатие — продлить' },
    en: { name: 'Sleep timer', tooltip: 'Pause after a while; hold to extend' },
    uk: { name: 'Таймер сну', tooltip: 'Пауза через заданий час; довге натискання — подовжити' },
  },
]
