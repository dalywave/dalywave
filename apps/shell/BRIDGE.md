# Мост интерфейса к оболочке приложения

Договор между `apps/ui` и оболочкой Tauri (`apps/shell`) для macOS, Windows, iPhone и iPad. Звук
играет аудиодвижок на Rust (`crates/client/audio`) — не WebView; «Сейчас играет», медиаклавиши,
экран блокировки и трей ведёт он же. Данные интерфейс берёт с сервера сам, с токеном устройства
из оболочки.

## Как подключить

Команды и события — через `@tauri-apps/api` (уже в зависимостях `apps/ui`). Типизированные
обёртки — `apps/shell/bridge/shell.ts`: файл без зависимостей, его можно скопировать в
`apps/ui/app/bridge/` или подключить псевдонимом.

```ts
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createShell, shellInfo, trackSnapArea } from './shell'

const info = shellInfo()            // синхронно; null — это сайт, а не приложение
const shell = createShell({ invoke, listen })
```

Признак приложения — `shellInfo() !== null` (то же, что `'__TAURI_INTERNALS__' in window`).
В `Bridge` из `app/bridge/types.ts`: `kind: 'tauri'`, `platform: info.os`; `isDesktopApp` —
`true` на macOS и Windows, `false` на iPhone и iPad (там нет окна с заголовком).

Каждая команда — промис. Ошибка приходит объектом `{ code, message }` (`isShellError(e)`):
по `code` интерфейс выбирает текст из i18n, `message` — для журнала.

## Сведения о платформе

`window.__DALYWAVE_SHELL__` задаётся до любого кода страницы — по нему первая отрисовка
сразу получает нужный заголовок окна. `shell.platformInfo()` возвращает то же, но с живым
замером «светофора» macOS.

```jsonc
// macOS
{
  "os": "macos", "arch": "aarch64", "appVersion": "0.1.0",
  "windowControls": "mac-inset",
  "titleBar": { "height": 48, "controls": { "x": 18, "y": 17, "width": 60, "height": 14 } },
  "backdrop": "none", "dev": false, "serverUrl": "https://wave.daly.rest", "locale": "ru"
}
// Windows 11
{
  "os": "windows", "arch": "x86_64", "appVersion": "0.1.0",
  "windowControls": "windows",
  "titleBar": { "height": 32, "controls": null },
  "backdrop": "mica", "dev": false, "serverUrl": "https://wave.daly.rest", "locale": "ru"
}
```

```jsonc
// iPhone (на iPad — "os": "ipados")
{
  "os": "ios", "arch": "aarch64", "appVersion": "0.1.0",
  "windowControls": "none",
  "titleBar": { "height": 0, "controls": null },
  "backdrop": "none", "dev": false, "serverUrl": "https://wave.daly.rest", "locale": "ru"
}
```

| Поле | Что значит |
|---|---|
| `os` | `macos`, `windows`, `linux`, `ios`, `ipados` |
| `windowControls` | `mac-inset` — кнопки окна рисует macOS поверх интерфейса; `windows` — интерфейс рисует свои; `none` — обычная рамка или окна нет (iPhone, iPad) |
| `titleBar.height` | рекомендуемая высота своей строки заголовка, CSS-пиксели |
| `titleBar.controls` | где лежит «светофор» (от левого верхнего угла окна); на Windows — `null` |
| `backdrop` | `mica` — под прозрачными местами интерфейса системный фон Windows 11; иначе `none` |
| `dev` | приложение запущено через `cargo tauri dev` с dev-сервера интерфейса |
| `serverUrl` | адрес сервера при запуске; дальше — `shell.server.get()` и событие `server:changed` |
| `locale` | язык меню в трее (`ru` / `uk` / `en`), меняется `shell.setLocale()` |

## Заголовок окна

Системной рамки у окна нет: интерфейс рисует свою строку заголовка (`--titlebar-h`).

**iPhone и iPad.** Окна нет: WebView во весь экран, под вырезом и полоской «домой». Отступы —
`env(safe-area-inset-*)` (в интерфейсе `--safe-top`, `--safe-bottom`; в `<meta name="viewport">`
нужен `viewport-fit=cover`). Строка состояния и системные окна — в теме приложения (см. «Тема»).
Из команд `window_*` там есть только `window_set_theme` и `window_set_back_swipe`; `update_*` нет.

**macOS (`mac-inset`).** Кнопки 14×14 с шагом 23: x 18–78, по центру строки высотой 48
(как `.sidebar__chrome` в макете «Полночь»). Левее x = 90 ничего интерактивного не ставить.
В полноэкранном режиме (`window:fullscreen` → `true`) кнопок в окне нет — отступ убрать.

**Windows (`windows`).** Справа три свои кнопки (Windows 11: 46 × 32, иконки Segoe Fluent):

| Кнопка | Действие | Состояние |
|---|---|---|
| Свернуть | `shell.window.minimize()` | |
| Развернуть / восстановить | `shell.window.toggleMaximize()` → новое состояние | иконка — по `window:maximized` |
| Закрыть | `shell.window.close()` — окно прячется в трей | красный фон при наведении |

Для меню привязки окон (Snap Layouts) при наведении на «развернуть» вызвать
`trackSnapArea(shell, кнопка)`. Над кнопкой лежит невидимое системное окно, поэтому CSS
`:hover` там не срабатывает — подсветку взять из атрибутов `[data-caption-hover]` и
`[data-caption-pressed]` (их ставит `trackSnapArea` по событию `window:caption`); щелчок по
ней разворачивает окно сам, свой обработчик клика на эту кнопку не нужен.
Пока окно не в фокусе (`window:focus` → `false`), заголовок и кнопки приглушить.

**Перетаскивание.** Строке заголовка (и пустой верхней части боковой панели) поставить
`data-tauri-drag-region="deep"`: окно тянется за любое место внутри, кроме интерактивных
элементов — кнопки, ссылки, поля, `[tabindex]`, `role="button"` и подобные не тянут сами.
Без значения атрибут действует только на сам элемент; `="false"` отключает поддерево.
Двойной щелчок по такому месту разворачивает окно (на macOS — как в системе, по отпусканию).
Своя логика — `shell.window.startDragging()` в `mousedown`.

**Фон.** macOS: фон окна — цвета темы (`#08090c`, `--ink-1`, или `#ffffff`), прозрачности и
системного размытия нет.
С материалом `sidebar` боковая панель становится `#1E1F24` — светлее содержимого
(`#121316`) и с оттенком обоев, а в «Полночи» она темнее (`#0D0E13`); снимок —
`design/shots/shell/vibrancy-sidebar-test.png`. Windows с
`backdrop: 'mica'`: у `html` и `body` фон прозрачный, панели — полупрозрачные
(`--glass`), тогда под ними видна Mica; при `none` — непрозрачный фон, как на сайте.

**Тема.** Каждое окно (главное и `/mini`) при старте и при смене выбора в настройках вызывает
`shell.window.setTheme('dark' | 'light' | null)` (`null` — «как в системе»). Оболочка применяет
тему сразу ко всем окнам и запоминает её (`settings.json`, поле `theme`; записи нет — тёмная):
следующий запуск открывает окна уже в ней, без вспышки фона другой темы.

| | Что делает тема |
|---|---|
| macOS | внешний вид всего приложения (NSAppearance): «светофор», меню, полосы прокрутки, `prefers-color-scheme`; фон окна и WebView — `#08090c` или `#ffffff` |
| Windows | светлая или тёмная рамка и оттенок Mica; без Mica (`backdrop: 'none'`) — фон окна цвета темы, с Mica фон прозрачный |
| iPhone, iPad | `overrideUserInterfaceStyle` окна: `prefers-color-scheme`, системные окна (меню, вход, AirPlay) и строка состояния (светлый текст в тёмной теме, тёмный — в светлой); фон под WebView цвета темы |

«Как в системе» следует за системой и после запуска: фон перекрашивается, когда система меняет
тему. Заставка iPhone и iPad (до загрузки интерфейса) всегда тёмная.

## Команды

| Вызов (`shell.…`) | Команда Tauri | Результат |
|---|---|---|
| `platformInfo()` | `platform_info` | `PlatformInfo` |
| `window.minimize()` | `window_minimize` | — |
| `window.toggleMaximize()` | `window_toggle_maximize` | `boolean` — развёрнуто ли теперь |
| `window.close()` | `window_close` | — окно в трей, музыка играет |
| `window.startDragging()` | `window_start_dragging` | — |
| `window.isMaximized()` | `window_is_maximized` | `boolean` |
| `window.setTheme(theme)` | `window_set_theme { theme }` | — `'dark'`, `'light'` или `null` (как в системе); все окна, запоминается. Есть и на iPhone, iPad |
| `window.setBackSwipe(enabled)` | `window_set_back_swipe { enabled }` | — iPhone, iPad: «назад» смахиванием от левого края (штатный жест WKWebView со снимком предыдущей страницы). Интерфейс включает его на вложенных страницах с историей позади; на корневых вкладках, под листом и полным плеером — выключает. «Вперёд» от правого края выключен всегда. На других системах — ничего |
| `window.setSnapArea(rect)` | `window_set_snap_area { rect }` | — (только Windows; `trackSnapArea` вызывает сам) |
| `auth.login({ demo? })` | `auth_login { demo }` | `Session` — ждёт возврата из браузера до 10 минут. `demo: true` — вход в общий аккаунт публичного демо (`/api/auth/app/start?demo=1`): сервер сразу отдаёт код, экран входа не открывается |
| `auth.cancel()` | `auth_cancel` | `boolean` — был ли незаконченный вход |
| `auth.logout()` | `auth_logout` | — |
| `auth.getToken()` | `auth_token` | `string \| null` — токен устройства `dwd_…` |
| `auth.session()` | `auth_session` | `Session \| null` — без сети |
| `openExternal(url)` | `open_external { url }` | — http, https, mailto |
| `server.get()` / `server.set(url)` | `server_url_get` / `server_url_set { url }` | адрес сервера |
| `setLocale(locale)` | `shell_set_locale { locale }` | — |
| `updates.check()` | `update_check` | `UpdateInfo \| null` — проверить сейчас, найденное качается в фоне |
| `updates.state()` | `update_state` | `UpdateState` |
| `updates.install()` | `update_install` | — «Перезапустить»: ставит скачанное и перезапускает |
| `player.load(items, { start, startMs, autoplay })` | `player_load { items, start, startMs, autoplay }` | — новая очередь движка |
| `player.setUpcoming(items)` | `player_set_upcoming { items }` | — всё после текущего трека |
| `player.play()` / `pause()` / `toggle()` | `player_play` / `player_pause` / `player_toggle` | — |
| `player.seek(positionMs)` | `player_seek { positionMs }` | — |
| `player.next()` / `previous()` / `stop()` | `player_next` / `player_previous` / `player_stop` | — по очереди движка |
| `player.setVolume(volume)` | `player_set_volume { volume }` | — 0…1 |
| `player.setTransition(t)` | `player_set_transition { transition }` | — `{ mode: 'gapless' }` \| `{ mode: 'crossfade', ms }` \| `{ mode: 'dj', expressiveness }` (0…1) |
| `player.setNormalize(enabled)` | `player_set_normalize { enabled }` | — выравнивание по LUFS |
| `player.setLevels(enabled)` | `player_set_levels { enabled }` | — события `player:level` |
| `player.state()` | `player_state` | `PlayerSnapshot` |
| `player.setEq(eq)` | `player_set_eq { eq }` | — эквалайзер этого устройства, запоминается; `null` — выключить (см. «Эквалайзер») |
| `player.eq()` | `player_eq` | `PlayerEq \| null` — сохранённый |
| `player.setSleep(timer)` | `player_set_sleep { timer }` | — `{ mode: 'minutes', ms }` \| `{ mode: 'track' }`; `null` — снять |
| `player.sleep()` | `player_sleep` | `PlayerSleep` — таймер сна сейчас |
| `player.setModes(shuffle, repeat)` | `player_set_modes { shuffle, repeat }` | — перемешивание и повтор (`'off' \| 'all' \| 'one'`), см. «Повтор» |
| `player.now()` | `player_now` | `PlayerNow \| null` — что играет (мини-плеер) |
| `player.remote(action)` | `player_remote { action }` | — `toggle`, `play`, `pause`, `next`, `previous` как с пульта |
| `mini.open()` / `close()` / `toggle()` | `mini_open` / `mini_close` / `mini_toggle` | — мини-плеер, macOS и Windows |
| `audio.outputs()` | `audio_outputs` | `AudioOutputs` — куда идёт звук (см. «Звук») |
| `system.idleMs()` | `system_idle_ms` | `number \| null` — сколько мс не трогали мышь и клавиатуру (macOS, Windows; `presence.rs`) |
| `audio.setOutput(id)` | `audio_set_output { id }` | `AudioOutputs` — macOS и Windows; `null` — как в системе |
| `audio.showPicker()` | `audio_show_picker` | — системное окно AirPlay и Bluetooth; iPhone и iPad |
| `downloader.state()` | `downloader_state` | `DownloaderSnapshot` — macOS и Windows |
| `downloader.setEnabled(enabled)` | `downloader_set_enabled { enabled }` | `DownloaderSnapshot` |
| `downloader.setPaused(paused)` | `downloader_set_paused { paused }` | `DownloaderSnapshot` |
| `downloader.recreateToken()` | `downloader_recreate_token` | `DownloaderSnapshot` — только в `token-rejected` |
| `downloader.openLog()` | `downloader_open_log` | — журнал в «Консоли» (Windows — в Блокноте) |
| `recognize.listen()` | `recognize_listen` | `RecognizeMatch` — слушает до совпадения, не дольше 15 с (см. «Распознать») |
| `recognize.cancel()` | `recognize_cancel` | `boolean` — шло ли распознавание |
| `recognize.takePending()` | `recognize_take_pending` | `boolean` — просили открыть экран до загрузки страницы |
| `siri.take()` | `siri_take` | `SiriCommand[]` — команды Siri, которые интерфейс ещё не выполнил (и забыть их), см. «Siri» |
| — | `focus_wave` | `FocusWave \| null` — сочетание волны из фильтра фокуса iOS (`{ id, name, mood?, activity?, character?, language? }`), см. «Фокусирование» |
| `links.take()` | `links_take` | `string \| null` — последняя необработанная ссылка `dalywave://…` (и забыть её), см. «Поделиться» |
| `discord.state()` | `discord_state` | `DiscordState` — `{ available, enabled, connected }`, macOS и Windows |
| `discord.setEnabled(enabled)` | `discord_set_enabled { enabled }` | `DiscordState` — переключатель «Статус в Discord», запоминается |
| — | `plugin:clipboard-manager\|write_text { text }` | — ссылка в буфер обмена (macOS и Windows, только запись) |

## События (`shell.on(name, handler)`)

| Событие | Данные | Когда |
|---|---|---|
| `window:maximized` | `{ maximized }` | окно развернули или восстановили (кнопкой, Win+↑, привязкой) |
| `window:fullscreen` | `{ fullscreen }` | полноэкранный режим macOS |
| `window:focus` | `{ focused }` | окно получило или потеряло фокус |
| `window:caption` | `{ button: 'maximize', state: 'enter' \| 'leave' \| 'down' \| 'up' }` | Windows: мышь над кнопкой «развернуть» |
| `auth:changed` | `{ session: Session \| null }` | вход или выход |
| `media:command` | `MediaCommand` | команда пульта, которую движок не выполнил сам (см. «Звук») |
| `player:state` | `{ state: 'idle' \| 'loading' \| 'playing' \| 'paused' \| 'ended', buffering }` | состояние движка |
| `player:position` | `{ id, positionMs, durationMs }` | раз в 250 мс во время игры, после перемотки и паузы |
| `player:track` | `{ id, index, reason: 'auto' \| 'user', durationMs }` | играет другой элемент очереди |
| `player:buffering` | `{ buffering }` | ждём данных из сети |
| `player:error` | `{ id \| null, message }` | трек не открылся или оборвался |
| `player:ended` | `null` | очередь движка доиграла |
| `player:level` | `{ rms, peak, bands: [low, mid, high] }` | ~20 раз в секунду, пока включено `setLevels(true)` |
| `player:sleep` | `PlayerSleep` | таймер сна поставили, сняли, началось затухание, сработал (`finished`) |
| `player:sound` | `{ available }` | системный вывод не отвечает (аудиосервер завис) — движок ждёт и повторяет с отступом; `true` — снова работает или пауза |
| `player:now` | `PlayerNow \| null` | только окну мини-плеера: другой трек, пауза, конец очереди |
| `audio:outputs` | `AudioOutputs` | появилось или пропало устройство вывода, сменились устройство по умолчанию или маршрут |
| `server:changed` | `{ serverUrl }` | сменили адрес сервера |
| `system:lock` | `{ locked }` | macOS: экран заблокирован или разблокирован (подхват на другом устройстве, `presence.rs`) |
| `focus:wave` | `FocusWave \| null` | iPhone и iPad: включился фокус с сочетанием DalyWave или выключился (`focus.rs`) |
| `update:available` | `UpdateInfo` | проверка нашла обновление, скачивание началось |
| `update:progress` | `{ downloadedBytes, totalBytes }` | ход скачивания, до 4 раз в секунду |
| `update:ready` | `UpdateInfo` | скачано и проверено подписью — можно «Перезапустить» |
| `update:state` | `UpdateState` | каждая перемена у обновления (и ход скачивания) |
| `deep-link:open` | `{ url }` | ссылка `dalywave://…`, кроме возврата входа; саму ссылку забирать `links.take()` |
| `downloader:state` | `DownloaderSnapshot` | загрузчик на компьютере: каждая перемена, ход задания — до 4 раз в секунду |
| `recognize:open` | `null` | открыть экран «Распознать»: пункт трея macOS, быстрое действие на значке iPhone |

## Вход и запросы к серверу

1. При запуске `auth.session()`: `null` — экран входа; иначе `session.user` уже можно
   показать без сети, а `/api/auth/me` уточнит роли (без ролей — экран «ждём доступа», как на сайте).
2. Кнопка «Войти через daly.rest» → `auth.login()`. Оболочка открывает системный браузер, вход
   идёт через daly.rest, сервер возвращает одноразовый код на `dalywave://auth`, оболочка
   сверяет `state`, меняет код на токен (PKCE S256) и кладёт его в связку ключей (macOS
   Keychain, Windows Credential Manager). Пока промис висит — экран «Завершите вход в браузере»
   с кнопкой «Отмена» → `auth.cancel()`.
3. Запросы — с токеном устройства; cookie сайта в приложении не используется. Токен держать
   в памяти и обновлять по `auth:changed`. Ответ 401 — `auth.logout()` и экран входа.
   ```ts
   const token = await shell.auth.getToken()
   const res = await fetch(`${serverUrl}/api/auth/me`, { headers: { Authorization: `Bearer ${token}` } })
   ```
4. «Выйти» → `auth.logout()`: сессия отзывается на сервере (если он доступен), токен стирается.
5. Аварийный вход паролем — только на сайте: он открывает cookie-сессию.
6. iPhone и iPad: страница входа открывается не в Safari, а в системном окне поверх приложения
   (ASWebAuthenticationSession; сначала iOS спрашивает «DalyWave хочет войти через
   wave.daly.rest»). Возврат `dalywave://auth` окно ловит само; если оно не открылось — Safari и
   та же ссылка (схема зарегистрирована в Info.plist). Токен — в защищённой связке ключей iOS.

Вход на каждом сервере свой: токен и профиль хранятся по адресу сервера.

| Код ошибки `auth.login()` | Что случилось |
|---|---|
| `cancelled` | отменили в приложении или на daly.rest |
| `timeout` | за 10 минут вход не завершили |
| `expired`, `state`, `provider`, `disabled`, `invalid_request` | ответ сервера на возврате: вход устарел, чужой браузер, daly.rest недоступен, аккаунт заблокирован, ошибка запроса |
| `invalid_grant`, `forbidden`, `too_many_requests` | ответ сервера при обмене кода |
| `network`, `server` | сервер недоступен или ответил неожиданно |
| `keychain` | связка ключей отказала |

**Адрес сервера.** В `cargo tauri dev` — `http://localhost:3400`: страница открыта с того же
адреса, Nuxt проксирует `/api` на :8400, относительные `/api/…` работают. В собранном
приложении страница живёт на `tauri://localhost` (macOS, iOS) или `https://tauri.localhost`
(Windows), поэтому адреса API — абсолютные: `https://wave.daly.rest/api/…`. Сервер разрешает
эти адреса в CORS (с заголовком `Authorization`). Байты аудио движок качает сам (не WebView) —
CORS им не нужен.

**CSP собранного приложения:** скрипты — только свои (`'self'`, WASM разрешён, inline-скрипты
сборки Nuxt оболочка хеширует сама); стили — свои и inline; картинки, звук и запросы — `https:`,
`http://localhost:*`, `data:`, `blob:`; фреймов нет. Шрифты — только из сборки.

## Звук

Играет движок на Rust: декодирование (Opus, AAC, FLAC, MP3, Vorbis), склейка без пауз до сэмпла,
кроссфейд по равномощной кривой, выравнивание громкости к −14 LUFS. Звук идёт и с закрытым окном,
и в фоне на iPhone, когда WebView усыплён.

**Очередь.** `player.load(items)` — текущий трек (обычно один элемент), `player.setUpcoming(items)` —
что дальше: вручную добавленное, остаток альбома или плейлиста, волна. Движок готовит следующий
трек за 30 с до конца и переходит сам — без пауз или наложением (`setTransition`), — сообщая
`player:track` с `reason: 'auto'`. Плеер интерфейса двигает по нему свою очередь и историю.
Очередь движка кончилась — `player:ended`: следующий трек решает интерфейс (докачка волны,
трек, который ещё скачивается на сервер).

```ts
interface PlayerItem {
  id: string            // uid элемента очереди интерфейса — вернётся в событиях
  trackId: string       // id трека на сервере
  url?: string | null   // подписанная ссылка; null — движок спросит сервер сам
  loudnessLufs?: number | null; truePeakDb?: number | null; durationMs?: number | null
  title: string; artist: string; album?: string | null; artworkUrl?: string | null
}
```

**«Диджей»** (`setTransition({ mode: 'dj', expressiveness })`, только приложения). Движок сам берёт
анализ текущего и следующего трека — `GET /api/tracks/{trackId}/analysis` с токеном устройства (сетка
долей, фразы, тональность, вокал, энергия; 404 — анализа нет) — и выбирает переход: граница фразы
уходящего после вокала, вход входящего с сильной доли, приём (сведение по долям со сменой баса,
фильтр, эхо-выход, резкая смена) и длина 8/16/32 такта по выразительности. Входящий растягивается до
темпа уходящего без смены высоты и за 4 такта возвращается к своему. Нет анализа, неуверенная сетка,
темпы расходятся больше чем на 8 % — «Умный» (наложение 4 с после вокала). `player:track` приходит в
момент вступления входящего; `player:position` у растянутого трека — позиция в файле. Подробно —
`crates/transitions`, решения — `docs/decisions.md`.

**Ссылки.** Если ссылки нет или она истекла посреди трека (подписанная ссылка R2 живёт 6 часов;
хранилище отвечает 400/401/403/404/410), движок сам запрашивает `GET /api/tracks/{trackId}/stream`
с токеном устройства и продолжает с той же позиции — уже скачанное не качается заново. Ответ 202
(трек ещё скачивается) — `player:error` и следующий трек. В запросе — сеть устройства (`?network=wifi|cellular`): качество сервер
выбирает по настройкам слушателя «По Wi-Fi» и «По сотовой» (на сотовой с «Экономией» — Opus 64 кбит/с).

**Пульт системы.** Медиаклавиши, наушники, Пункт управления и экран блокировки (macOS, iOS —
MediaPlayer; Windows — SMTC), трей: движок выполняет команду сам — в том числе «следующий» и
«предыдущий» по своей очереди. Интерфейсу уходит `media:command` только то, что движок не может:
«играть» без очереди (запустить волну), «следующий» в конце очереди движка, «предыдущий» в первые
3 с первого трека (история — в интерфейсе).

```ts
type MediaCommand =
  | { action: 'play' | 'pause' | 'toggle' | 'next' | 'previous' | 'stop', source: 'system' | 'tray' }
  | { action: 'seek', positionMs: number, source: 'system' }     // ползунок в Пункте управления
  | { action: 'seekBy', offsetMs: number, source: 'system' }     // плашка Windows
  | { action: 'setShuffle', shuffle: boolean, source: 'system' }  // Пункт управления, CarPlay (macOS, iOS)
  | { action: 'setRepeat', mode: 'off' | 'all' | 'one', source: 'system' }
```

**Повтор.** Перемешивание и повтор всей очереди ведёт плеер интерфейса (новые круги списка — через
`setUpcoming`), повтор одного трека — движок: после `setModes(…, 'one')` следом за текущим всегда стоит
его копия, она готовится заранее и склеивается без паузы (или переходом), в том числе в фоне на iPhone.
Наружу копия — тот же элемент: `player:track` с тем же `id` и `reason: 'auto'`. «Следующий» и
«предыдущий» с пульта повтор пропускают; «предыдущий» дальше 3 с — трек сначала. Оба режима видны в
Пункте управления (`changeShuffleModeCommand`, `changeRepeatModeCommand`); их смена там приходит
`media:command`, повтор одного движок включает сам сразу. Плашка Windows (souvlaki) таких кнопок не умеет.

«Сейчас играет» (название, исполнитель, альбом, обложка, длительность, позиция, пауза) движок
публикует сам по своим событиям; `navigator.mediaSession` в приложении не задавать.

**Волна.** Анализатора WebAudio в приложении нет: `player.setLevels(true)` включает
`player:level` ~20 раз в секунду — RMS, пик и три полосы (до 150 Гц, 150–2000 Гц, выше 2 кГц)
до общей громкости. Включать, только пока волна на экране.

**iPhone.** Аудиосессия `.playback` (звук и при беззвучном режиме), фон — `UIBackgroundModes:
audio`. Звонок, Siri, будильник — пауза и продолжение, если iOS разрешит; наушники отключили —
пауза. В фоне `player:position` и `player:level` в WebView не уходят; при возвращении приложения
приходят свежие `player:state` и `player:position`.

**AirPlay 2.** Когда звук идёт в AirPlay (iOS: маршрут аудиосессии `longFormAudio`; macOS: выбрана
или стала устройством по умолчанию колонка AirPlay), движок выводит через AVSampleBufferAudioRenderer
и отдаёт звук на секунды вперёд. Для интерфейса ничего не меняется: `player:position`, `player:track`
(`reason: 'auto'`), `player:level`, `player:ended` и «Сейчас играет» приходят по тому, что слышно, —
текст песни и строка прогресса идут вровень с колонкой. Пауза, перемотка, другой трек и громкость
действуют сразу. Смена маршрута открывает вывод заново со слышимого места (`player:*` как при смене
устройства). Проверка без колонок — `DALYWAVE_OUTPUT=buffered`.

**Смена устройства вывода.** macOS сама переводит поток на новое устройство по умолчанию;
устройство пропало — пауза; поток стал непригоден (другая частота, сброс медиасервисов iOS) —
вывод открывается заново, трек продолжается с той же позиции.

### Эквалайзер

Десять полос (31, 62, 125, 250, 500 Гц, 1, 2, 4, 8, 16 кГц), колоколы шириной в октаву, ±12 дБ; в
движке — на общем выходе (`crates/client/audio/src/eq.rs`). Предусиление опускает звук на самый
высокий подъём АЧХ, остальное ловит ограничитель — подъём полос не перегружает. Смена пресета и
движение полос — плавные (150 мс), выключенный эквалайзер звук не трогает вовсе.

```ts
type PlayerEqPreset = 'off' | 'bass-boost' | 'bass-cut' | 'vocal' | 'acoustic' | 'electronic' | 'loudness' | 'custom'
interface PlayerEq { preset: PlayerEqPreset, gains: number[] }   // gains — полосы «Своего», дБ
```

Пресеты — таблица движка (`EqPreset::gains`), та же — в `apps/ui/app/utils/eq.ts` (сайт играет те же
полосы через WebAudio). Настройка своя у каждого устройства: оболочка хранит её в `settings.json` и
включает с запуска движка — до интерфейса. `gains` помнятся и когда выбран другой пресет.

### Таймер сна

`player.setSleep({ mode: 'minutes', ms })` — пауза через `ms` по часам (пауза не останавливает
отсчёт); `{ mode: 'track' }` — в конце текущего трека. Последние 15 с звук затухает по равномощной
кривой, затем пауза; «в конце трека» затухание кончается ровно на стыке, следующий трек встаёт в
начало и ждёт. Перемотка и другой трек переносят затухание к новому концу. После паузы громкость
сама возвращается. Таймер живёт в движке — работает и в фоне на iPhone, когда WebView усыплён.

```ts
interface PlayerSleep {
  timer: 'minutes' | 'track' | null
  endsAtMs: number | null      // когда пауза, мс от эпохи Unix
  remainingMs: number | null
  fading: boolean
  finished: boolean            // сработал: музыка на паузе
}
```

### Мини-плеер

macOS и Windows: окно 360 × 136 поверх всех — обложка, название, кнопки, полоса прокрутки. Открывается
из трея («Мини-плеер»), из «⋯» панели плеера и по ⌘⇧M (Ctrl+Shift+M); главное окно на это время
прячется. Закрытие (своя кнопка, ⌘W, пункт трея) возвращает главное окно. Место окна запоминается.

Это не второй плеер: та же страница интерфейса (`/mini`, признак `window.__DALYWAVE_MINI__`),
сторы плеера и сокет там не запускаются. Окно получает те же события `player:*` и `player:now`,
кнопки — `player.remote(...)` (как трей: что движок не может, уходит главному окну `media:command`),
перемотка — `player.seek`.

### Куда идёт звук

Шторка «Устройства» плеера: `audio.outputs()` при открытии, дальше — событие `audio:outputs`.

```ts
interface AudioOutputs {
  mode: 'list' | 'picker'
  outputs: { id: string, name: string, kind: AudioOutputKind, current: boolean, spatial?: boolean }[]
  followsSystem: boolean
}
type AudioOutputKind = 'builtin' | 'headphones' | 'bluetooth' | 'airplay' | 'usb' | 'hdmi' | 'other'
```

**macOS и Windows — `mode: 'list'`.** Звук выводит движок, поэтому устройство выбирает
приложение. `outputs` — все устройства вывода системы в её порядке, с именами как в настройках
звука («Динамики MacBook Pro», «AirPods Захара»); на macOS здесь и колонки AirPlay, которые Mac
сейчас видит в сети. `id` — постоянный id устройства (`coreaudio:<UID>`, `wasapi:<id>`), `kind` на
macOS — по типу подключения из CoreAudio (встроенный выход с гнездом наушников — `headphones`), на
Windows — что сообщила система. Строка «Как в системе» — `audio.setOutput(null)`, выбрана при
`followsSystem: true`: поток следует за устройством по умолчанию (`current` — у него).
`audio.setOutput(id)` открывает вывод на устройстве заново: трек продолжается с той же позиции,
пауза остаётся паузой; промис отвечает уже новым списком (ждёт переключения до 3 с, AirPlay
подключается дольше обычного). Выбор запоминается и действует со следующего запуска; устройства
нет при запуске — звук идёт на устройство по умолчанию. Выбранное устройство пропало (колонка
AirPlay ушла из сети, ЦАП выдернули) — пауза, вывод переходит на устройство по умолчанию, приходит
`audio:outputs` с `followsSystem: true`; вернувшееся устройство само обратно не выбирается — чтобы
звук неожиданно не заиграл в другой комнате. Нет такого `id` — ошибка `invalid`.

**iPhone и iPad — `mode: 'picker'`.** Маршрут звука решает iOS: выбрать AirPlay, AirPods или
динамик можно только в её окне — `audio.showPicker()` (то же окно, что у кнопки AirPlay в Пункте
управления; на iPad — поповер у нижнего края экрана по центру). `outputs` — текущий маршрут, обычно
один элемент с `current: true`: `name` — как его называет система («Динамик», «Гостиная»), `id` —
тип порта (`Speaker`, `Headphones`, `BluetoothA2DPOutput`, `AirPlay`…). Выбор в окне приходит
событием `audio:outputs`; `followsSystem` всегда `true`. `spatial: true` — на этом выходе включено
пространственное аудио (AirPods, Beats): режим «Фиксированное» или «С отслеживанием головы» человек
выбирает в Пункте управления (удержать громкость), сводит звук сама iOS. Включили или выключили —
тоже приходит `audio:outputs`.

`audio.setOutput()` на iPhone и `audio.showPicker()` на компьютере отклоняются с кодом
`unsupported`.

**Когда приходит `audio:outputs`.** Только если список отличается от последнего, что интерфейс
получил. macOS сообщает о переменах сама; iOS — при каждой смене маршрута. На Windows слушателя
устройств нет: список перечитывается раз в 3 с в течение минуты после `audio.outputs()` — пока
шторка открыта, звать его при открытии (и, если шторка висит дольше, раз в минуту). Везде событие
приходит и после того, как движок открыл вывод заново.

**Проверки без звука.** `DW_VOLUME=0` — громкость движка умножается на 0: всё работает, звука
нет. `DALYWAVE_SMOKE=http://127.0.0.1:8420` — очередь из тестового сервера
(`crates/client/audio/tools/serve.py`) играет сразу после запуска, в журнал — треки, позиция,
уровень, «Сейчас играет» и нагрузка.

## Управление между устройствами

Приложение — устройство в списке «Где играть», как и каждая вкладка сайта (сервер —
`crates/server/src/connect.rs`, интерфейс — `apps/ui/app/stores/connect.ts`). Всё идёт по тому же
WebSocket `/api/ws`, что и живые события, сообщениями `connect.*`; отдельных команд оболочки нет.

- **Кто это.** Id установки — в `localStorage` WebView (`dw.connect.device`), имя сервер берёт из
  входа приложения («MacBook Pro Захара»), вид — `laptop` (macOS), `desktop` (Windows), `phone`,
  `tablet`. Возможности: громкость и «Диджей».
- **Активное устройство** владеет очередью и раз в 5 с (и при каждом изменении) публикует снимок:
  трек, позиция, очередь, громкость, режим переходов. Движок сам переходит к следующему треку
  (`player:track`) — плеер интерфейса двигает очередь, и снимок уходит пультам.
- **Команды пульта** (пауза, следующий, перемотка, громкость, «играть альбом здесь»…) приходят
  интерфейсу и выполняются тем же плеером, что и кнопки на экране: дальше — команды движка
  `player_*` из таблицы выше. Ответ `connect.ack` уходит сразу после выполнения.
- **Перенос.** Выбрали другое устройство — здесь звук останавливается (`player_stop`: очередь
  движка очищается, медиаклавиши больше не играют старое), плеер становится пультом. Перенесли
  сюда — движок получает очередь и позицию из снимка (`player_load` с `startMs`).
- **Пульт в приложении.** Медиаклавиши и Пункт управления при пустом движке присылают
  `media:command` — он уходит командой активному устройству.

**iPhone и iPad в фоне.** Музыку в фоне играет движок, а WebView iOS усыпляет: сокет молчит, снимки
и знаки жизни не уходят. Через 15 с сервер считает iPhone пропавшим — на других устройствах
появляется «Продолжить здесь», хотя музыка на телефоне играет; команды с пульта на телефон в фоне
отвечают `offline`. Вернулись в приложение — мост сам переподключает сокет (`visibilitychange`),
приложение снова представляется (`connect.hello`), шлёт знак жизни и свежий снимок. Если за это
время воспроизведение забрали на другое устройство, телефон получает `connect.active` и
останавливается. Управлять телефоном в фоне можно будет, когда сокет переедет в ядро на Rust
(после этапа 5).

## Умный офлайн

Все приложения (macOS, Windows, iPhone, iPad): локальная база медиатеки, загрузки на устройство,
волна без сети (`src/offline/`). В интерфейсе — `bridge.offline` (на сайте `null`), обёртка —
`app/bridge/tauri/offline.ts`, состояние — `useOfflineStore()`.

| Команда | Аргументы | Результат |
|---|---|---|
| `offline_status` | — | `OfflineStatus` (ниже) |
| `offline_set_settings` | `patch: { limitBytes?, wifiOnly?, reserveEnabled?, forceOffline? }` | `OfflineStatus` |
| `offline_sync` | — | — синхронизировать базу сейчас (ждёт прохода до 10 с) |
| `offline_read` | `query: { what, id?, ids?, cursor?, limit?, q? }` | данные в типах API или `null` — в базе нет, идти на сервер. `what`: `overview`, `liked`, `albums`, `artists`, `playlists`, `playlist`, `album`, `artist`, `search`, `tracks` |
| `offline_pin` / `offline_unpin` | `pin: { kind: 'track' \| 'album' \| 'playlist' \| 'artist' \| 'liked', refId }` | `OfflineStatus` |
| `offline_clear` | — | `OfflineStatus` — удалить все файлы и закреплённое, умный запас выключается |
| `offline_downloads` | — | `TrackDownload[]` — все треки на устройстве и ход их загрузки |
| `offline_reserve` | — | `{ trackIds, readyIds: string[], builtAt: string \| null }` — состав умного запаса в порядке загрузки и уже скачанные из него |
| `offline_source` | `trackId` | `{ durationMs, loudnessLufs, truePeakDb, waveform }` скачанного или `null` |
| `offline_wave_next` | `session, settings, count, exclude` | `{ track, reason }[]` — волна из скачанного |
| `offline_wave_react` | `session, trackId, kind, positionMs` | — реакция в волне без сети |
| `offline_played` | `play: { trackId, playedMs, durationMs, endReason, wave }` | — в историю устройства |
| `haptic` | `kind` | — тактильный отклик (ниже) |

```ts
interface OfflineStatus {
  online: boolean                       // «Без сети» выключено, сеть есть, сервер отвечает
  network: 'wifi' | 'cellular' | 'none' | 'unknown'
  synced: boolean; syncedAt: string | null; syncing: boolean
  settings: { limitBytes: number, wifiOnly: boolean, reserveEnabled: boolean, forceOffline: boolean }
  usage: { pinnedBytes, reserveBytes, cacheBytes, totalBytes, files: number }
  downloads: { active, queued, failed, overLimit: number, waitingForWifi: boolean }
  pins: { kind, refId: string | null, tracksTotal, tracksReady, bytes: number }[]
  reserve: { tracks, ready: number, builtAt: string | null }
}
interface TrackDownload { trackId: string, state: 'queued' | 'downloading' | 'ready' | 'failed', received: number, total: number }
```

События: `offline:state` (`OfflineStatus`, перемены хода — не чаще раза в секунду), `downloads:progress`
(`TrackDownload`, до 4 раз в секунду на трек), `offline:synced` (медиатека в базе изменилась — перечитать
списки).

**Как экраны получают данные.** Медиатека (`library.*`, `playlists.list/get`) — из базы, если она уже
синхронизирована; каталог (`catalog.album/artist/search/lookup/track`) — с сервера, а без сети или при сбое
сети — из базы. После изменения медиатеки мост ждёт синхронизации базы — экран сразу видит перемену.
`media.source` скачанного трека отдаёт громкость и огибающую из базы без ссылки, а плеер оболочки сам
подставляет файл (`player_load`, `player_set_upcoming`); без сети нескачанное — `unavailable` сразу.
Волна: без сети (или если сервер не ответил) `wave.start/next` подбирает из скачанного (сессия `offline:…`),
сеть пропала посреди волны — продолжает её же, не повторяя сыгранное.

**«Без сети»** (`forceOffline`): HTTP моста не уходит на сервер вовсе, плеер не спрашивает ссылки.

**Тактильный отклик** — `haptic(kind)`: `light` | `medium` | `heavy` | `soft` | `rigid` (удар), `selection`,
`success` | `warning` | `error`. Только iPhone (UIKit); на компьютерах ничего. Неизвестный `kind` — `invalid`.

**«Почувствуй бит»** (iPhone, `src/beat.rs` + `DalyWaveBeat.swift`): удары Core Haptics на сильных долях
того, что играет, по сетке из `/api/tracks/{id}/analysis`, с поправкой на задержку вывода; только пока
приложение на экране. `beat_state()` и `beat_set(enabled)` → `{ supported, enabled }` (`supported` — есть
Taptic Engine), `beat_check(trackId)` → `boolean` — есть ли у трека сетка. Выбор хранит интерфейс (`dw.beat`).

**Заставка для Mac** (`src/screensaver.rs`, `apps/shell/saver`): `screensaver_state()` и `screensaver_install()`
→ `{ available, installed }`; установка копирует `Contents/Resources/DalyWave.saver` в `~/Library/Screen Savers`
и открывает настройки заставки. Что играет, заставка спрашивает у приложения: `GET http://127.0.0.1:47815/now-playing`
→ `{ playing, title, artist, artworkUrl? }`.

**Проверка без входа** — `DALYWAVE_OFFLINE_SMOKE=<тестовый сервер>`: своя база, файлы тестового сервера
качаются тем же путём (iPhone — фоновой сессией), затем «Без сети», волна из скачанного и воспроизведение
из файлов. В журнал — `offline smoke: …`.

## Уведомления

Уведомления шлёт сервер (`/api/notifications*`): в центр уведомлений и по WebSocket
(`notification.new`) — всем, push — iPhone и iPad через APNs. Команды — `src/push.rs` и `src/notify.rs`.

| Команда | Аргументы | Результат |
|---|---|---|
| `push_state` | — | `{ supported: boolean, permission: 'default' \| 'granted' \| 'denied' }` — iPhone/iPad: разрешение системы (`default` — ещё не спрашивали); компьютеры — `supported: false` |
| `push_register` | `prompt: boolean` | `{ token: string, environment: 'sandbox' \| 'production' } \| null` — токен APNs (hex) для `POST /api/notifications/push`. `prompt: false` — окна разрешения не будет (пока не спрашивали — `null`); `true` — система спросит один раз. Запрещено, не iOS, нет ответа 10 с — `null`. Звать при каждом запуске после входа: токен может смениться |
| `push_set_badge` | `count: number` | — число непрочитанных на значке (iPhone/iPad) |
| `notify_show` | `id`, `title`, `body`, `link: string \| null` | `bool` — системное уведомление на компьютере, пока приложение открыто (звать, когда окно не в фокусе). Щелчок показывает окно и открывает `link` (`dalywave://…`; на Windows — только окно). macOS спросит разрешение при первом; iPhone/iPad и запуск без бандла — `false` |

Нажатие на push (iPhone/iPad) и на уведомление macOS приходит как ссылка `dalywave://…` — через
`links_take` / `deep-link:open`, как ссылки из «Поделиться» и виджетов: `dalywave://album/{id}`,
`dalywave://import`, `dalywave://rewind?year=2026`, `dalywave://notifications`, `dalywave://admin/disputed`.

## Распознать

macOS, iPhone и iPad: какая песня играет рядом — ShazamKit (`SHSession`) слушает микрофон через
`AVAudioEngine` (`src/recognize.rs`). На Windows команды отвечают `unsupported`; в интерфейсе
`bridge.recognizer` там и на сайте — `null`, кнопки нет.

```ts
interface RecognizeMatch {
  title: string; artist: string
  isrc: string | null; appleMusicId: string | null; shazamId: string | null; artworkUrl: string | null
}
```

1. `recognize.listen()` — первый раз система спрашивает доступ к микрофону («Чтобы узнать, какая песня
   играет рядом»). Пока слушаем, своя музыка на паузе (на iPhone сеанс на это время — `playAndRecord`
   с выводом в динамик). Не узнали, отменили, сбой — музыка продолжается; узнали — пауза остаётся.
2. Совпадение интерфейс отправляет серверу: `POST /api/recognize/resolve` → `Recognition` с треком
   каталога (или `track: null`) — он же попадает в «Распознанное» (`GET /api/recognize/history`).

| Код ошибки `listen()` | Что случилось |
|---|---|
| `no_match` | за 15 с не узнали |
| `permission` | нет доступа к микрофону |
| `no_microphone` | у устройства нет входа звука |
| `cancelled` | `recognize.cancel()` или новый `listen()` |
| `shazam` | сервис Shazam не ответил: нет сети или у App ID не включена служба ShazamKit (IOS.md) |
| `unsupported` | Windows, Linux |

**Открыть экран просят** пункт трея macOS «Распознать песню» и быстрое действие «Распознать» на
значке iPhone (долгое нажатие): окно выходит вперёд, приходит `recognize:open`. Если приложение
запустили этим действием, страница ещё не загружена — при запуске интерфейс зовёт
`recognize.takePending()` (обёртка `app/bridge/tauri/recognize.ts` делает это сама).

**Проверка без микрофона:** `DALYWAVE_RECOGNIZE_MOCK=1` — через 2,5 с совпадение «Midnight City — M83»
(`Название|Исполнитель|ISRC` — своё, `none` — «не узнали», `permission` — нет доступа);
`DALYWAVE_RECOGNIZE_OPEN=1` вместе с ним — экран открывается сам при запуске. Сайт в разработке —
`?recognize=match|none|listening`.

## Siri

iPhone и iPad, iOS 17+: фразы и действия «Команд» (App Intents) и запросы музыки SiriKit (в том числе
с HomePod). Пауза, «продолжи», «дальше», «назад» выполняет движок сам (как пульт). Волну и подборки
включает интерфейс: событие `siri:command` — сигнал забрать команды `siri_take` (после входа и на каждое
событие; первый вызов ещё и спрашивает разрешение Siri). Не забрал за 3 с или приложение в фоне —
оболочка включает сама (очередь с сервера прямо в движок), и команда из `siri_take` уже не придёт.
«Нравится» оболочка ставит на сервере сама и сообщает `liked` — отметить сердечко.

```ts
type SiriCommand =
  | { action: 'wave', mood?: WaveMood, activity?: WaveActivity, character?: WaveCharacter, language?: WaveLanguage }
  | { action: 'play', kind: 'album' | 'artist' | 'playlist' | 'track' | 'liked', id: string, title?: string, shuffle?: boolean }
  | { action: 'liked', trackId: string }
```

Подключено плагином `apps/ui/app/plugins/siri.client.ts`; устройство — `apps/shell/src/siri.rs`,
Swift — `gen/apple/Sources/dalywave-shell/DalyWaveIntents.swift` и `gen/apple/SiriExtension`.

## Виджеты iPhone

Виджеты WidgetKit (`apps/shell/src/widgets.rs`, Swift — `gen/apple/Widgets`, IOS.md «Виджеты»)
интерфейсу почти ничего не стоят: что играет, оболочка берёт у движка, недавнее и сочетания волны —
с сервера сама. Кнопки виджетов приходят тем же путём, что Siri: пауза и «дальше» — движок,
«Моя волна» и её сочетание — `siri:command` с `wave` (характер и язык — только у сочетаний). Нажатие
на альбом, плейлист или артиста в «Недавнем» — ссылка `dalywave://album/{id}` (`playlist`, `artist`):
`deep-link:open` → `links.take()` → экран (`useSharedLink`, `appRoute`).

## Фокусирование

Фильтр фокуса iOS 16+ (`gen/apple/Sources/dalywave-shell/DalyWaveFocus.swift`, `SetFocusFilterIntent`):
«Настройки» → «Фокусирование» → режим → «Фильтры фокусирования» → DalyWave → сочетание волны (те же,
что у виджета). Система зовёт фильтр при включении и выключении фокуса — и когда приложение не запущено:
Swift запоминает выбор и зовёт `dalywave_focus_changed` → событие `focus:wave`; при запуске и открытии
интерфейс спрашивает `focus_wave`. Применяет ли он сочетание — настройка «Фокусирование» (stores/focus.ts).

## Окно и приложение

- Закрытие окна (красная кнопка, ⌘W, Alt+F4, `window.close()`) прячет его в трей: музыка
  играет дальше. Вернуть — значок в Dock (macOS), трей «Показать DalyWave», повторный
  запуск приложения. Выход — трей «Выйти из DalyWave» или ⌘Q.
- Размер, место и развёрнутость окна запоминаются; минимум 960 × 600.
- Второй запуск не открывает второе окно — показывает первое.
- В фоне WebView на macOS не притормаживается (таймеры и отчёты о прослушивании идут).
- Ссылки на чужие сайты, по которым перешли внутри окна, открываются в браузере;
  явно — `openExternal(url)`.

## Загрузчик

Только macOS и Windows: приложение запускает загрузчик музыки (sidecar `dalywave-fetcher`,
`src/downloader.rs`) и показывает его. В интерфейсе — `bridge.downloader` (`null` на сайте и
телефонах), состояние — `useDownloader()`.

```ts
interface DownloaderSnapshot {
  enabled: boolean          // «Скачивать на этом Mac», по умолчанию true
  paused: boolean           // пауза пользователя, запоминается
  phase: 'disabled' | 'signed-out' | 'forbidden' | 'provisioning' | 'starting' | 'running'
       | 'stopping' | 'restarting' | 'token-rejected' | 'broken'
  status: DownloaderStatus | null   // последний `status` загрузчика (SIDECAR.md), пока работает
  server: string; workerName: string; logFile: string
  version: string | null; ytDlp: string | null
  error: { code: string, message: string } | null   // почему не работает
  retryAtMs: number | null  // restarting: следующая попытка
}
```

`running` — подробности в `status.state` (`idle`, `working` с `job`, `paused`, `offline`, `error`
с отдыхом до `retryAt`). Ход всего задания в процентах интерфейс и трей считают одинаково: поиск —
до 10 %, скачивание — до 65 %, проверка — до 80 %, отправка — до 100 %. Токен загрузчика приложение
заводит само (только администратору), хранит в связке ключей и интерфейсу не отдаёт.

## Обучение рекомендаций

Только macOS и Windows (`src/trainer.rs`): приложение берёт у сервера задание «обучить сеть вкуса»
(`POST /api/taste/training/claim` с токеном устройства), учит её тем же `crates/taste`, что и сервер,
в потоке с низким приоритетом и отправляет веса (`…/{id}/result`). Решает сервер: проверяет веса на
своих отложенных данных. Учит только на питании от сети и когда загрузка процессора ниже трети ядер;
первая попытка — через 5 минут после запуска, дальше — раз в час. В интерфейсе — `bridge.trainer`
(`null` на сайте и телефонах), команды `trainer_state`, `trainer_set_enabled(enabled: bool | null)`,
событие `trainer:state`:

```ts
interface TrainerSnapshot {
  enabled: boolean | null   // «Помогать учить рекомендации»; null — по умолчанию: учат только компьютеры
                            // администраторов (общую сеть), true — ещё и свою сеть слушателя
  phase: 'off' | 'idle' | 'on-battery' | 'busy' | 'signed-out' | 'fetching' | 'training' | 'sending'
  last: { atMs: number, outcome: 'active' | 'kept' | 'failed', rows: number, tookMs: number, note: string } | null
}
```

## Обновления

Только macOS и Windows. Сервер отвечает на `GET /api/updates/{target}/{arch}/{current_version}`
(`target` — `darwin` или `windows`, `arch` — `aarch64` или `x86_64`): `204` — обновлений нет;
`200` — JSON `{ version, notes, pub_date, url, signature }`, где `url` — подписанная ссылка R2, а
`signature` — содержимое файла `.sig` из сборки. Как выпуск попадает на сервер — README, «Выпуск».

Оболочка проверяет при запуске (через 20 с) и раз в 6 часов, найденное сразу качает в фоне в
папку кэша и проверяет подписью (`update:available` → `update:progress` → `update:ready`); файл
переживает перезапуск, повторно не качается. Затем — тихо: строка в настройках («О приложении»)
и пункт «Обновление готово — Перезапустить» в начале меню трея. Само приложение не
перезапускается никогда: обновление ставит `updates.install()` (кнопка «Перезапустить» или пункт
трея), а на macOS — ещё и обычный выход (⌘Q, «Выйти»; `installOnQuit: true`). На Windows
установщик NSIS закрывает приложение (загрузчик перед этим возвращает задание) и запускает новую
версию.

```ts
type UpdateStatus = 'idle' | 'checking' | 'downloading' | 'ready' | 'installing' | 'failed'
interface UpdateState {
  status: UpdateStatus
  currentVersion: string           // то же, что appVersion
  update: UpdateInfo | null        // найденная версия: downloading, ready, installing, сбой скачивания
  downloadedBytes: number | null
  totalBytes: number | null
  checkedAtMs: number | null       // последняя удачная проверка
  error: string | null             // failed: текст для журнала
  installOnQuit: boolean           // macOS
}
```

`updates.install()` до `ready` сначала скачивает (ход — `update:progress`); ошибка — `{ code: 'update' }`.

## Поделиться и статус в Discord

**Ссылки наружу.** «Поделиться» в меню трека, альбома, плейлиста и артиста даёт
`https://wave.daly.rest/{kind}/{id}`: на телефоне — системное меню (`navigator.share`), на
компьютере — буфер обмена через плагин Rust (`plugin:clipboard-manager|write_text`: WebView
пустил бы запись только по жесту, а пункт меню срабатывает после закрытия меню) и уведомление.
Превью в мессенджерах сервер отдаёт сам (Open Graph на `/{kind}/{id}`).

**«Поделиться → DalyWave».** iPhone и iPad: расширение `dalywave-share` (gen/apple/ShareExtension)
в системном меню «Поделиться» для ссылок и текста — открывает приложение ссылкой
`dalywave://share?url=…`. macOS — та же ссылка из любого места. Оболочка кладёт её в `links` и шлёт
`deep-link:open`; интерфейс забирает `links.take()` при запуске (приложение могли открыть самой
ссылкой — событие ушло раньше подписки) и на каждое событие, только после входа. Ещё вставка ссылки
в окно или в поле поиска и перетаскивание ссылки на окно. Ссылку разбирает сервер
(`POST /api/resolve-link`) — Deezer, Spotify, Apple Music, YouTube, YouTube Music, SoundCloud,
Bandcamp; своя ссылка просто открывает экран.

**Статус в Discord** (macOS, Windows) ведёт Rust по событиям движка, интерфейсу делать нечего: тип
«Слушает», название, исполнители, обложка (публичный адрес Deezer из `GET /api/share/track/{id}`),
значок DalyWave, отметки начала и конца — Discord рисует прогресс; кнопка «Слушать в DalyWave».
Пауза дольше 30 с — статус убирается. Discord не запущен — тишина, запустили позже — статус
появится сам (попытка раз в 15 с). id приложения Discord — `DALYWAVE_DISCORD_CLIENT_ID` (при
запуске или при сборке) или `plugins.discord.clientId` в `tauri.conf.json`; нет — `available:
false`, переключатель в настройках не показывается.

