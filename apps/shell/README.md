# Оболочка приложения DalyWave (Tauri 2.12)

Окно приложения для macOS и Windows вокруг интерфейса `apps/ui`. Договор с интерфейсом —
[BRIDGE.md](BRIDGE.md), типизированные обёртки — `bridge/shell.ts`.

```
src/lib.rs        плагины, окно, трей, команды
src/window.rs     окно без рамки, закрытие в трей, события окна
src/snap.rs       Windows 11: меню привязки окон над своей кнопкой «развернуть»
src/macos.rs      замер «светофора»
src/auth.rs       вход через браузер, dalywave://auth, токен в связке ключей
src/media.rs      «Сейчас играет» и медиаклавиши (souvlaki: MPNowPlayingInfoCenter, SMTC)
src/tray.rs       значок в строке меню и трее
src/updates.rs    автообновление
src/settings.rs   адрес сервера и язык меню (settings.json в папке настроек)
harness/          стенд: страница со всеми командами моста и заглушка сервера для входа
```

Отдельный cargo workspace: тяжёлые зависимости Tauri не попадают в `cargo test --workspace`
и в CI сервера.

## Разработка

```sh
make ui                        # интерфейс на :3400 — отдельно, в своём терминале
cd apps/shell && cargo tauri dev
```

Окно грузит `http://localhost:3400`, сервер по умолчанию — тот же адрес (Nuxt проксирует
`/api` на :8400, `make api`). Вход в разработке на macOS возвращается не по `dalywave://`
(такие ссылки ведут в собранное приложение), а на `http://127.0.0.1:<порт>/callback` —
сервер разрешает оба адреса.

Переменные окружения: `DALYWAVE_SERVER_URL` — адрес сервера по умолчанию;
`DALYWAVE_AUTH_REDIRECT=loopback|deeplink` — способ возврата входа; в отладочной сборке
`DALYWAVE_AUTH_OPEN=print` — не открывать браузер, а записать адрес входа в журнал.
Журнал: `~/Library/Logs/rest.daly.wave/DalyWave.log` (Windows — `%LOCALAPPDATA%\rest.daly.wave\logs`).

Проверки: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

## Стенд

Без готового интерфейса — страница с кнопками всех команд, своими кнопками окна для Windows
и линиями выравнивания по «светофору»:

```sh
bun apps/shell/harness/server.ts       # стенд :3499, заглушка сервера :3498
cd apps/shell && DALYWAVE_SERVER_URL=http://localhost:3498 \
  cargo tauri dev --config '{"build":{"devUrl":"http://localhost:3499"}}'
curl -d login localhost:3499/command   # команда странице из терминала
```

Отчёты страницы — `$TMPDIR/dalywave-harness/reports.jsonl`. На Windows так же проверяются
Mica и меню привязки окон.

## Сборка macOS

```sh
make app
```

Собирает интерфейс (`bun run build` в `apps/ui`), затем `DalyWave.app`, `.dmg` и архив
обновления с подписью в `apps/shell/target/release/bundle/`. Подпись — первая
«Developer ID Application» из связки ключей (или `APPLE_SIGNING_IDENTITY`), без неё — ad-hoc.
Нотаризации нет: на Mac, где его собрали, приложение запускается сразу, на чужом — через «Открыть» в
контекстном меню. Голый `cargo tauri build` тоже работает, если задать переменные подписи.

Ссылки `dalywave://` система отдаёт одной из зарегистрированных копий приложения. Если копий
несколько (смонтированный DMG, старая копия, сборка из `target/`) и ссылка ушла в незапущенную,
та закрывается как второй экземпляр раньше, чем macOS передаст ей ссылку, — вход ждёт до
таймаута. Поэтому после установки DMG извлекать, а лишние копии удалять
(`…/LaunchServices.framework/Support/lsregister -u <путь к .app>` в
`/System/Library/Frameworks/CoreServices.framework/Frameworks/`). Обходной путь —
`DALYWAVE_AUTH_REDIRECT=loopback`: возврат на 127.0.0.1 вместо ссылки.

**Ключ подписи обновлений** — вне репозитория: `~/.tauri/dalywave-updater.key` (права 600),
публичная часть — `plugins.updater.pubkey` в `tauri.conf.json`. Пароль к ключу — в связке
ключей входа: `security find-generic-password -s dalywave-updater-key -w`. Для CI оба значения
кладутся в секреты `TAURI_SIGNING_PRIVATE_KEY` (содержимое файла) и
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Потеря ключа — конец автообновлений у установленных
копий: держите резервную копию в менеджере паролей.

## Значки

`scripts/icons.sh` пересобирает всё из `design/brand` (после `bun tools/brand.mjs`):

- PNG, `icon.ico` и плитки Microsoft Store — `cargo tauri icon design/brand/icon-1024.png`;
- `icon.icns` — из `icon-macos-1024.png` (сетка Apple с полями и тенью) — для macOS до 26;
- `DalyWave.icon` — документ Icon Composer: фон `layers/background.svg` и знак (стекло);
  в тёмной теме — знак в градиенте на тёмном (`icon-ios-dark.svg`), в тонированной и
  прозрачной фон скрыт и знак красит система. При сборке Tauri компилирует его `actool`
  (нужен Xcode 26+) в `Contents/Resources/Assets.car` и прописывает `CFBundleIconName`;
  `icon.icns` остаётся запасным `CFBundleIconFile`. Документ открывается в Icon Composer.app,
  превью тем: `ictool DalyWave.icon --export-preview macOS Dark 512 512 1 out.png`
  (`ictool` лежит в Icon Composer.app/Contents/Executables).
- Значок в строке меню — шаблон `icons/tray-template.png` из `icons/tray.svg`, в трее
  Windows — `icons/tray-windows.png`.

Знак в документе — растр 1024: штрих из SVG стекло Icon Composer принимает за замкнутую
фигуру и рисует лишнюю грань.

## Windows

Сборка — на GitHub Actions (`.github/workflows/desktop.yml`, вручную или по тегу `v*`):
установщик NSIS для текущего пользователя, ссылки `dalywave://` регистрирует он же.
Локально проверено, что код компилируется и проходит clippy под `x86_64-pc-windows-msvc`;
запуск — на ПК с Windows 11. Что проверить там: Mica под прозрачными местами, меню привязки
окон при наведении на свою кнопку «развернуть» (стенд), трей, медиаклавиши и плашка SMTC,
вход по `dalywave://auth`, токен в «Диспетчере учётных данных». Подписи кода для Windows пока
нет — SmartScreen предупредит при первом запуске.
