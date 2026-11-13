// Список действий плагина — один на манифест, переводы и картинки по умолчанию (scripts/assets.ts).

export type Controller = 'Keypad' | 'Encoder' | 'Neo'

export interface ActionDef {
  id: string
  controllers: Controller[]
  ru: { name: string, tooltip: string, rotate?: string, push?: string, touch?: string, longTouch?: string }
  en: { name: string, tooltip: string, rotate?: string, push?: string, touch?: string, longTouch?: string }
  /** Раскладка полосы по умолчанию */
  layout?: string
}

export const ACTIONS: ActionDef[] = [
  {
    id: 'nowplaying',
    controllers: ['Keypad', 'Neo'],
    ru: { name: 'Сейчас играет', tooltip: 'Обложка и прогресс трека. Нажатие — играть или пауза, долгое — «Нравится»' },
    en: { name: 'Now playing', tooltip: 'Cover and progress. Press to play or pause, hold to like' },
    layout: 'layouts/neo.json',
  },
  {
    id: 'playback',
    controllers: ['Keypad'],
    ru: { name: 'Воспроизведение', tooltip: 'Играть, пауза, следующий или предыдущий трек' },
    en: { name: 'Playback', tooltip: 'Play, pause, next or previous track' },
  },
  {
    id: 'multimedia',
    controllers: ['Encoder'],
    ru: { name: 'Мультимедиа', tooltip: 'Крутилка: треки, перемотка или громкость', rotate: 'Треки', push: 'Играть / пауза', touch: 'Играть / пауза', longTouch: 'Сменить режим' },
    en: { name: 'Multimedia', tooltip: 'Dial: tracks, seek or volume', rotate: 'Tracks', push: 'Play / pause', touch: 'Play / pause', longTouch: 'Change mode' },
    layout: 'layouts/track.json',
  },
  {
    id: 'volume',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Громкость', tooltip: 'Громкость DalyWave там, где играет музыка', rotate: 'Громкость', push: 'Играть / пауза', touch: 'Играть / пауза', longTouch: 'Без звука' },
    en: { name: 'Volume', tooltip: 'DalyWave volume on the device that is playing', rotate: 'Volume', push: 'Play / pause', touch: 'Play / pause', longTouch: 'Mute' },
    layout: 'layouts/volume.json',
  },
  {
    id: 'shuffle',
    controllers: ['Keypad'],
    ru: { name: 'Перемешать', tooltip: 'Перемешивание очереди альбома или плейлиста' },
    en: { name: 'Shuffle', tooltip: 'Shuffle the album or playlist' },
  },
  {
    id: 'repeat',
    controllers: ['Keypad'],
    ru: { name: 'Повтор', tooltip: 'Без повтора, всё по кругу, один трек' },
    en: { name: 'Repeat', tooltip: 'Off, repeat all, repeat one' },
  },
  {
    id: 'like',
    controllers: ['Keypad'],
    ru: { name: 'Нравится', tooltip: 'Добавить трек в «Любимые» или убрать' },
    en: { name: 'Like', tooltip: 'Add the track to Liked or remove it' },
  },
  {
    id: 'dislike',
    controllers: ['Keypad'],
    ru: { name: 'Не нравится', tooltip: 'Волна запомнит и перейдёт к следующему' },
    en: { name: 'Dislike', tooltip: 'The wave remembers and skips' },
  },
  {
    id: 'notnow',
    controllers: ['Keypad'],
    ru: { name: 'Не сейчас', tooltip: 'Пропустить без «не нравится»' },
    en: { name: 'Not now', tooltip: 'Skip without disliking' },
  },
  {
    id: 'add',
    controllers: ['Keypad'],
    ru: { name: 'Добавить в…', tooltip: 'Текущий трек — в плейлист или в «Любимые»; повторно — убрать' },
    en: { name: 'Add to…', tooltip: 'Add the current track to a playlist or Liked; press again to remove' },
  },
  {
    id: 'collection',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Играть коллекцию', tooltip: 'Плейлист, альбом, артист, «Любимые», миксы, чарты, история', rotate: 'Листать', push: 'Играть', touch: 'Играть', longTouch: 'Обновить' },
    en: { name: 'Play collection', tooltip: 'Playlist, album, artist, Liked, mixes, charts, history', rotate: 'Browse', push: 'Play', touch: 'Play', longTouch: 'Refresh' },
    layout: 'layouts/list.json',
  },
  {
    id: 'search',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Сохранённый поиск', tooltip: 'Найти и включить: треки, альбом или артиста', rotate: 'Листать', push: 'Играть', touch: 'Играть', longTouch: 'Обновить' },
    en: { name: 'Saved search', tooltip: 'Search and play tracks, an album or an artist', rotate: 'Browse', push: 'Play', touch: 'Play', longTouch: 'Refresh' },
    layout: 'layouts/list.json',
  },
  {
    id: 'queue',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Очередь', tooltip: 'Что дальше; нажатие — к этому треку', rotate: 'Листать', push: 'Перейти', touch: 'Перейти', longTouch: 'К началу' },
    en: { name: 'Queue', tooltip: 'Up next; press to jump to it', rotate: 'Browse', push: 'Jump', touch: 'Jump', longTouch: 'Back to top' },
    layout: 'layouts/list.json',
  },
  {
    id: 'wave',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Моя волна', tooltip: 'Волна с настроением, занятием или сохранённой настройкой', rotate: 'Настроения', push: 'Включить', touch: 'Включить', longTouch: 'Обновить' },
    en: { name: 'My wave', tooltip: 'Wave with a mood, activity or saved setup', rotate: 'Moods', push: 'Start', touch: 'Start', longTouch: 'Refresh' },
    layout: 'layouts/list.json',
  },
  {
    id: 'discover',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Открытия', tooltip: 'Только новое для вас', rotate: 'Листать', push: 'Играть', touch: 'Играть', longTouch: 'Ещё' },
    en: { name: 'Discover', tooltip: 'Only music new to you', rotate: 'Browse', push: 'Play', touch: 'Play', longTouch: 'More' },
    layout: 'layouts/list.json',
  },
  {
    id: 'transfer',
    controllers: ['Keypad', 'Encoder'],
    ru: { name: 'Передать на устройство', tooltip: 'Перенести музыку на компьютер, телефон или сайт', rotate: 'Устройства', push: 'Перенести', touch: 'Перенести' },
    en: { name: 'Play on device', tooltip: 'Move playback to a computer, phone or browser', rotate: 'Devices', push: 'Move', touch: 'Move' },
    layout: 'layouts/list.json',
  },
  {
    id: 'transition',
    controllers: ['Keypad'],
    ru: { name: 'Режим переходов', tooltip: 'Без пауз → Плавный → Умный → Диджей' },
    en: { name: 'Transitions', tooltip: 'Gapless → Smooth → Smart → DJ' },
  },
  {
    id: 'sleep',
    controllers: ['Keypad'],
    ru: { name: 'Таймер сна', tooltip: 'Пауза через заданное время; долгое нажатие — продлить' },
    en: { name: 'Sleep timer', tooltip: 'Pause after a while; hold to extend' },
  },
]
