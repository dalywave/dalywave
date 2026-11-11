# Мост интерфейса к оболочке приложения

Договор между `apps/ui` и оболочкой Tauri (`apps/shell`) для macOS и Windows. Звук пока играет
в WebView; данные интерфейс берёт с сервера сам, с токеном устройства из оболочки.

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
В `Bridge` из `app/bridge/types.ts`: `kind: 'tauri'`, `platform: info.os`, `isDesktopApp: true`.

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

| Поле | Что значит |
|---|---|
| `windowControls` | `mac-inset` — кнопки окна рисует macOS поверх интерфейса; `windows` — интерфейс рисует свои; `none` — обычная рамка |
| `titleBar.height` | рекомендуемая высота своей строки заголовка, CSS-пиксели |
| `titleBar.controls` | где лежит «светофор» (от левого верхнего угла окна); на Windows — `null` |
| `backdrop` | `mica` — под прозрачными местами интерфейса системный фон Windows 11; иначе `none` |
| `dev` | приложение запущено через `cargo tauri dev` с dev-сервера интерфейса |
| `serverUrl` | адрес сервера при запуске; дальше — `shell.server.get()` и событие `server:changed` |
| `locale` | язык меню в трее (`ru` / `en`), меняется `shell.setLocale()` |

## Заголовок окна

Системной рамки у окна нет: интерфейс рисует свою строку заголовка (`--titlebar-h`).

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
| `nowPlaying.update(state)` | `now_playing_update { state }` | — |
| `openExternal(url)` | `open_external { url }` | — http, https, mailto |
| `server.get()` / `server.set(url)` | `server_url_get` / `server_url_set { url }` | адрес сервера |
| `setLocale(locale)` | `shell_set_locale { locale }` | — |
| `updates.check()` | `update_check` | `UpdateInfo \| null` |
| `updates.install()` | `update_install` | — скачивает, ставит, перезапускает |

## События (`shell.on(name, handler)`)

| Событие | Данные | Когда |
|---|---|---|
| `window:maximized` | `{ maximized }` | окно развернули или восстановили (кнопкой, Win+↑, привязкой) |
| `window:fullscreen` | `{ fullscreen }` | полноэкранный режим macOS |
| `window:focus` | `{ focused }` | окно получило или потеряло фокус |
| `window:caption` | `{ button: 'maximize', state: 'enter' \| 'leave' \| 'down' \| 'up' }` | Windows: мышь над кнопкой «развернуть» |
| `auth:changed` | `{ session: Session \| null }` | вход или выход |
| `media:command` | `MediaCommand` | медиаклавиши, наушники, Пункт управления, плашка Windows, трей |
| `server:changed` | `{ serverUrl }` | сменили адрес сервера |
| `update:available` | `UpdateInfo` | фоновая проверка нашла обновление (раз в 6 часов) |
| `update:progress` | `{ downloadedBytes, totalBytes }` | идёт `updates.install()` |
| `deep-link:open` | `{ url }` | ссылка `dalywave://…`, кроме возврата входа (на будущее) |

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
приложении страница живёт на `tauri://localhost` (macOS) или `https://tauri.localhost`
(Windows), поэтому адреса API — абсолютные: `https://wave.daly.rest/api/…`. Для этого серверу
нужен CORS для этих двух адресов (заголовок `Authorization`, предварительные запросы) —
**пока его нет, запросы из собранного приложения не пройдут.** Аудио из R2 по подписанным
ссылкам CORS уже разрешает.

**CSP собранного приложения:** скрипты — только свои (`'self'`, WASM разрешён, inline-скрипты
сборки Nuxt оболочка хеширует сама); стили — свои и inline; картинки, звук и запросы — `https:`,
`http://localhost:*`, `data:`, `blob:`; фреймов нет. Шрифты — только из сборки.

## «Сейчас играет» и медиаклавиши

`nowPlaying.update(state)` — при смене трека, запуске, паузе и перемотке (не каждую секунду:
позицию система досчитывает сама); `null` — ничего не играет.

```ts
interface NowPlaying {
  title: string; artist: string; album?: string | null
  artworkUrl?: string | null      // http(s) или file://
  durationMs?: number | null; positionMs?: number | null
  playing: boolean
}
```

Плашка — macOS (Пункт управления, экран блокировки) и Windows (SMTC); меню в трее
показывает «Исполнитель — Трек» и «Пауза» / «Играть». Команды приходят событием
`media:command` и выполняются плеером интерфейса:

```ts
type MediaCommand =
  | { action: 'play' | 'pause' | 'toggle' | 'next' | 'previous' | 'stop', source: 'system' | 'tray' }
  | { action: 'seek', positionMs: number, source: 'system' }     // ползунок в Пункте управления
  | { action: 'seekBy', offsetMs: number, source: 'system' }     // ±10 000, если система не сказала
```

Проверено на macOS 27: WebKit сам публикует плашку для `<audio>` в окне (заголовок страницы и
кнопки ±15 с), но после `nowPlaying.update` система показывает нашу, а медиаклавиши всегда
приходят в `media:command` — WebKit сам `<audio>` не останавливает. Поэтому `update` отправлять
сразу при запуске звука, а `navigator.mediaSession` в приложении не задавать.

## Окно и приложение

- Закрытие окна (красная кнопка, ⌘W, Alt+F4, `window.close()`) прячет его в трей: музыка
  играет дальше. Вернуть — значок в Dock (macOS), трей «Показать DalyWave», повторный
  запуск приложения. Выход — трей «Выйти из DalyWave» или ⌘Q.
- Размер, место и развёрнутость окна запоминаются; минимум 960 × 600.
- Второй запуск не открывает второе окно — показывает первое.
- В фоне WebView на macOS не притормаживается (таймеры и отчёты о прослушивании идут).
- Ссылки на чужие сайты, по которым перешли внутри окна, открываются в браузере;
  явно — `openExternal(url)`.

## Обновления

Сервер (позже) отвечает на `GET /api/updates/{target}/{arch}/{current_version}`
(`target` — `darwin` или `windows`, `arch` — `aarch64` или `x86_64`): `204` — обновлений нет;
`200` — JSON `{ version, notes, pub_date, url, signature }`, где `signature` — содержимое файла
`.sig` из сборки. Найдя обновление, оболочка шлёт `update:available`; интерфейс показывает
предложение и вызывает `updates.install()`.
