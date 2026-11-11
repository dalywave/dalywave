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
| `locale` | язык меню в трее (`ru` / `en`), меняется `shell.setLocale()` |

## Заголовок окна

Системной рамки у окна нет: интерфейс рисует свою строку заголовка (`--titlebar-h`).

**iPhone и iPad.** Окна нет: WebView во весь экран, под вырезом и полоской «домой». Отступы —
`env(safe-area-inset-*)` (в интерфейсе `--safe-top`, `--safe-bottom`; в `<meta name="viewport">`
нужен `viewport-fit=cover`). Строка состояния светлая, системные окна — в тёмной теме.
Команды `window_*` и `update_*` там не существуют.

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

**Фон.** macOS: фон окна `#08090c` (`--ink-1`), прозрачности и системного размытия нет.
С материалом `sidebar` боковая панель становится `#1E1F24` — светлее содержимого
(`#121316`) и с оттенком обоев, а в «Полночи» она темнее (`#0D0E13`); снимок —
`design/shots/shell/vibrancy-sidebar-test.png`. Windows с
`backdrop: 'mica'`: у `html` и `body` фон прозрачный, панели — полупрозрачные
(`--glass`), тогда под ними видна Mica; при `none` — непрозрачный фон, как на сайте.

**Тема.** При выборе темы в настройках вызвать `shell.window.setTheme('dark' | 'light' | null)`
(`null` — «как в системе»): от неё зависят оттенок Mica, системные меню и полосы прокрутки,
а внутри WebView — `prefers-color-scheme`.

## Команды

| Вызов (`shell.…`) | Команда Tauri | Результат |
|---|---|---|
| `platformInfo()` | `platform_info` | `PlatformInfo` |
| `window.minimize()` | `window_minimize` | — |
| `window.toggleMaximize()` | `window_toggle_maximize` | `boolean` — развёрнуто ли теперь |
| `window.close()` | `window_close` | — окно в трей, музыка играет |
| `window.startDragging()` | `window_start_dragging` | — |
| `window.isMaximized()` | `window_is_maximized` | `boolean` |
| `window.setTheme(theme)` | `window_set_theme { theme }` | — |
| `window.setSnapArea(rect)` | `window_set_snap_area { rect }` | — (только Windows; `trackSnapArea` вызывает сам) |
| `auth.login()` | `auth_login` | `Session` — ждёт возврата из браузера до 10 минут |
| `auth.cancel()` | `auth_cancel` | `boolean` — был ли незаконченный вход |
| `auth.logout()` | `auth_logout` | — |
| `auth.getToken()` | `auth_token` | `string \| null` — токен устройства `dwd_…` |
| `auth.session()` | `auth_session` | `Session \| null` — без сети |
| `openExternal(url)` | `open_external { url }` | — http, https, mailto |
| `server.get()` / `server.set(url)` | `server_url_get` / `server_url_set { url }` | адрес сервера |
| `setLocale(locale)` | `shell_set_locale { locale }` | — |
| `updates.check()` | `update_check` | `UpdateInfo \| null` |
| `updates.install()` | `update_install` | — скачивает, ставит, перезапускает |
| `player.load(items, { start, startMs, autoplay })` | `player_load { items, start, startMs, autoplay }` | — новая очередь движка |
| `player.setUpcoming(items)` | `player_set_upcoming { items }` | — всё после текущего трека |
| `player.play()` / `pause()` / `toggle()` | `player_play` / `player_pause` / `player_toggle` | — |
| `player.seek(positionMs)` | `player_seek { positionMs }` | — |
| `player.next()` / `previous()` / `stop()` | `player_next` / `player_previous` / `player_stop` | — по очереди движка |
| `player.setVolume(volume)` | `player_set_volume { volume }` | — 0…1 |
| `player.setTransition(t)` | `player_set_transition { transition }` | — `{ mode: 'gapless' }` \| `{ mode: 'crossfade', ms }` |
| `player.setNormalize(enabled)` | `player_set_normalize { enabled }` | — выравнивание по LUFS |
| `player.setLevels(enabled)` | `player_set_levels { enabled }` | — события `player:level` |
| `player.state()` | `player_state` | `PlayerSnapshot` |
| `downloader.state()` | `downloader_state` | `DownloaderSnapshot` — macOS и Windows |
| `downloader.setEnabled(enabled)` | `downloader_set_enabled { enabled }` | `DownloaderSnapshot` |
| `downloader.setPaused(paused)` | `downloader_set_paused { paused }` | `DownloaderSnapshot` |
| `downloader.recreateToken()` | `downloader_recreate_token` | `DownloaderSnapshot` — только в `token-rejected` |
| `downloader.openLog()` | `downloader_open_log` | — журнал в «Консоли» (Windows — в Блокноте) |

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
| `server:changed` | `{ serverUrl }` | сменили адрес сервера |
| `update:available` | `UpdateInfo` | фоновая проверка нашла обновление (раз в 6 часов) |
| `update:progress` | `{ downloadedBytes, totalBytes }` | идёт `updates.install()` |
| `deep-link:open` | `{ url }` | ссылка `dalywave://…`, кроме возврата входа (на будущее) |
| `downloader:state` | `DownloaderSnapshot` | загрузчик на компьютере: каждая перемена, ход задания — до 4 раз в секунду |

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

**Ссылки.** Если ссылки нет или она истекла посреди трека (подписанная ссылка R2 живёт 6 часов;
хранилище отвечает 400/401/403/404/410), движок сам запрашивает `GET /api/tracks/{trackId}/stream`
с токеном устройства и продолжает с той же позиции — уже скачанное не качается заново. Ответ 202
(трек ещё скачивается) — `player:error` и следующий трек.

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
```

«Сейчас играет» (название, исполнитель, альбом, обложка, длительность, позиция, пауза) движок
публикует сам по своим событиям; `navigator.mediaSession` в приложении не задавать.

**Волна.** Анализатора WebAudio в приложении нет: `player.setLevels(true)` включает
`player:level` ~20 раз в секунду — RMS, пик и три полосы (до 150 Гц, 150–2000 Гц, выше 2 кГц)
до общей громкости. Включать, только пока волна на экране.

**iPhone.** Аудиосессия `.playback` (звук и при беззвучном режиме), фон — `UIBackgroundModes:
audio`. Звонок, Siri, будильник — пауза и продолжение, если iOS разрешит; наушники отключили —
пауза. В фоне `player:position` и `player:level` в WebView не уходят; при возвращении приложения
приходят свежие `player:state` и `player:position`.

**Смена устройства вывода.** macOS сама переводит поток на новое устройство по умолчанию;
устройство пропало — пауза; поток стал непригоден (другая частота, сброс медиасервисов iOS) —
вывод открывается заново, трек продолжается с той же позиции.

**Проверки без звука.** `DW_VOLUME=0` — громкость движка умножается на 0: всё работает, звука
нет. `DALYWAVE_SMOKE=http://127.0.0.1:8420` — очередь из тестового сервера
(`crates/client/audio/tools/serve.py`) играет сразу после запуска, в журнал — треки, позиция,
уровень, «Сейчас играет» и нагрузка.

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

## Обновления

Сервер (позже) отвечает на `GET /api/updates/{target}/{arch}/{current_version}`
(`target` — `darwin` или `windows`, `arch` — `aarch64` или `x86_64`): `204` — обновлений нет;
`200` — JSON `{ version, notes, pub_date, url, signature }`, где `signature` — содержимое файла
`.sig` из сборки. Найдя обновление, оболочка шлёт `update:available`; интерфейс показывает
предложение и вызывает `updates.install()`.
