# Оболочка приложения DalyWave (Tauri 2.12)

Приложение для macOS, Windows, iPhone и iPad вокруг интерфейса `apps/ui`. Звук — свой движок на
Rust (`crates/client/audio`). Договор с интерфейсом — [BRIDGE.md](BRIDGE.md), типизированные
обёртки — `bridge/shell.ts`. iPhone и iPad — [IOS.md](IOS.md).

```
src/lib.rs        плагины, окно, трей, команды
src/player.rs     звук: движок dalywave-audio, команды и события player:*, ссылки на файлы с сервера
src/media.rs      «Сейчас играет» и пульт системы по событиям движка (MediaPlayer, Windows — SMTC)
src/window.rs     окно без рамки, закрытие в трей, события окна
src/snap.rs       Windows 11: меню привязки окон над своей кнопкой «развернуть»
src/macos.rs      замер «светофора»
src/ios.rs        iPhone и iPad: окно входа поверх приложения (ASWebAuthenticationSession)
src/auth.rs       вход через браузер, dalywave://auth, токен в связке ключей
src/tray.rs       значок в строке меню и трее
src/downloader.rs загрузчик музыки для сервера (sidecar dalywave-fetcher): запуск, статус, пауза
src/updates.rs    автообновление: проверка, скачивание в фоне, «Обновление готово — Перезапустить»
src/settings.rs   адрес сервера, язык меню, загрузчик вкл/пауза (settings.json в папке настроек)
src/discord.rs    статус «Слушает DalyWave» в Discord (Rich Presence, только компьютеры)
src/links.rs      ссылки dalywave:// для интерфейса, в том числе «Поделиться → DalyWave»
gen/apple/        Xcode-проект iPhone и iPad (xcodegen, project.yml), расширение «Поделиться» (ShareExtension/)
scripts/          значки, сборка Rust для Xcode (build-ios.sh), пересборка Xcode-проекта
harness/          стенд: страница со всеми командами моста и заглушка сервера для входа
```

Отдельный cargo workspace: тяжёлые зависимости Tauri не попадают в `cargo test --workspace`
и в CI сервера. Движок подключён путём из своего workspace `crates/client`.

## Разработка

```sh
make ui                        # интерфейс на :3400 — отдельно, в своём терминале
make sidecar                   # загрузчик: services/fetcher/dist/sidecar (один раз, дальше — если устарел)
cd apps/shell && cargo tauri dev
```

Без `services/fetcher/dist/sidecar` приложение не соберётся: Tauri проверяет `externalBin` и
`resources` уже при `cargo build` и `cargo clippy` для macOS и Windows.

Окно грузит `http://localhost:3400`, сервер по умолчанию — тот же адрес (Nuxt проксирует
`/api` на :8400, `make api`). Вход в разработке на macOS возвращается не по `dalywave://`
(такие ссылки ведут в собранное приложение), а на `http://127.0.0.1:<порт>/callback` —
сервер разрешает оба адреса.

Переменные окружения: `DALYWAVE_SERVER_URL` — адрес сервера по умолчанию;
`DALYWAVE_AUTH_REDIRECT=loopback|deeplink` — способ возврата входа; в отладочной сборке
`DALYWAVE_AUTH_OPEN=print` — не открывать браузер, а записать адрес входа в журнал;
`DW_VOLUME=0` — громкость движка умножается на это число (проверки без звука);
`DALYWAVE_SMOKE=http://127.0.0.1:8420` — сразу после запуска играть очередь тестового сервера
(`make test-audio-server`), `DALYWAVE_SMOKE_ALBUM` — один набор из его `manifest.json`.
Журнал: `~/Library/Logs/rest.daly.wave/DalyWave.log` (Windows — `%LOCALAPPDATA%\rest.daly.wave\logs`,
iOS — `Library/Application Support/rest.daly.wave/logs` в контейнере приложения).

Проверки: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`; для iOS —
`cargo clippy --lib --target aarch64-apple-ios-sim --features tauri/custom-protocol -- -D warnings`
(с переменными SDK из `scripts/build-ios.sh`). Движок — `make audio-test`. Переходы «Диджея» на
реальных треках — без колонок, в WAV: `cargo run --release --example dj_render -- <треки> <вывод> a:b@0.5 …`
в `crates/client` (треки и их анализ — в каталоге с `index.json`, см. заголовок примера).

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

Звук на стенде — без звука и без входа: тестовое аудио `make test-audio-server` (:8420), оболочка с
`DW_VOLUME=0`, затем `curl -d player-demo localhost:3499/command` (ещё `player-gapless`,
`player-deferred` — без ссылок, их выдаёт заглушка `/api/tracks/{id}/stream`, `player-next`,
`player-seek`, `player-xfade`, `player-levels-on`, `player-state`). События `player:*` — в отчётах.

## Загрузчик музыки

YouTube не отдаёт записи серверу, поэтому их качает компьютер администратора. Приложение для macOS и
Windows само и есть загрузчик: пока оно запущено (живёт в строке меню или трее), оно берёт задания
у сервера; закрыли — не качает ничего. Внутри — sidecar `dalywave-fetcher` с Deno, FFmpeg, fpcalc и
сервером PO-токенов; договор — [services/fetcher/SIDECAR.md](../../services/fetcher/SIDECAR.md),
команды и событие для интерфейса — [BRIDGE.md](BRIDGE.md#загрузчик).

- **Включение.** Включён по умолчанию («Скачивать на этом Mac» / «на этом ПК» — в настройках и в
  трее) и работает, пока выполнен вход. При первом запуске приложение само заводит себе токен
  загрузчика `POST /api/admin/downloaders` с токеном устройства — только администратору, у
  остальных «Доступен администратору». Токен `dwk_…` — в связке ключей (Windows — «Диспетчер
  учётных данных»), запись `downloader-token <адрес сервера>`. В админке («Источники») загрузчик
  называется «Mac <имя компьютера>» или «PC <имя компьютера>».
- **Статус.** Трей: подменю «Загрузчик: качает «Трек» — Артист · 45 %» / «ждёт заданий» / «на
  паузе» / «нет связи с сервером» с пунктами «Пауза»/«Продолжить» и «Скачивать на этом Mac».
  Интерфейс: раздел настроек и строка в подвале боковой панели. Пауза запоминается.
- **Сбои.** Упал — перезапуск через 2, 5, 10 … 300 с. Сервер не принял токен (отозвали в админке) —
  «Пересоздать токен» в настройках. Программы не запускаются — «Установка повреждена».
- **Выход.** ⌘Q, «Выйти» в трее, перезапуск после обновления: `stop`, до 15 с на возврат задания
  серверу, затем принудительно вся группа процессов (Job Object на Windows) — сирот не остаётся.
- **Файлы.** macOS: данные — `~/Library/Application Support/DalyWave/fetcher`, журнал —
  `~/Library/Logs/DalyWave/fetcher.log`. Windows: `%LOCALAPPDATA%\DalyWave\fetcher` и
  `%LOCALAPPDATA%\DalyWave\logs\fetcher.log`. Ход работы — и в журнале приложения (`downloader: …`).
- **Подпись macOS.** Программы загрузчика в `Contents/MacOS` Tauri подписывает тем же ключом и с
  `entitlements.plist` (hardened runtime: JIT для Deno, библиотеки PyInstaller и canvas.node).
- **Разработка без процесса.** Сайт в разработке изображает загрузчик моком:
  `http://localhost:3400/settings?downloader=working` (`idle`, `paused`, `offline`, `resting`,
  `disabled`, `token`, `forbidden` …) — `apps/ui/app/bridge/mock/downloader.ts`.

## Сборка macOS

```sh
make app
```

Собирает загрузчик, если его нет или он старше исходников fetcher (`make sidecar` →
`services/fetcher/sidecar/build.sh`, первый раз — 3–4 минуты), интерфейс (`bun run build` в
`apps/ui`), затем `DalyWave.app`, `.dmg` и архив
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

## Выпуск

Обновления раздаёт наш сервер (`crates/server/src/routes/updates.rs`, решение — в
`docs/decisions.md`): архив обновления лежит в R2, выпуски — в таблице `app_releases`. Приложение
спрашивает `GET /api/updates/{target}/{arch}/{версия}` при запуске (через 20 с) и раз в 6 часов,
найденное качает в фоне и проверяет подписью, затем показывает «Обновление готово — Перезапустить»
в настройках («О приложении») и в трее. Само оно не перезапускается: ставит обновление по
«Перезапустить», на macOS — ещё и при обычном выходе (⌘Q, «Выйти» в трее). На Windows установщик
NSIS при этом закрывает приложение и открывает новую версию.

**Версия** — одна: `[package] version` в `Cargo.toml` (в `tauri.conf.json` её нет, Tauri берёт её
отсюда; это и `appVersion` в `platform_info`). Новая — `make bump-version VERSION=0.2.0`: заодно
`MARKETING_VERSION` Xcode-проекта и `Cargo.lock`.

**Шаги выпуска:**

1. `make bump-version VERSION=0.2.0`, коммит.
2. macOS: `make release-mac NOTES="Что нового"` — сборка с подписью обновлений (`make app`),
   архив `DalyWave.app.tar.gz` с `.sig` уходит в R2 по ссылке от сервера, выпуск регистрируется
   (`apps/shell/scripts/release.ts`). Другой сервер — `SERVER=http://localhost:8400`.
3. Windows: тег `git tag v0.2.0 && git push origin v0.2.0` — CI («Приложения») собирает установщик и
   выпускает его так же (тег обязан совпадать с версией в `Cargo.toml`). Без тега — Run workflow с
   флажком «Выпустить на сервер обновлений». macOS CI выпускает, только если в секретах есть
   сертификат Developer ID, иначе — шаг 2.

Повторный выпуск той же версии заменяет прежний. Снять выпуск — удалить строку `app_releases` на
сервере: приложения, ещё не поставившие его, перестанут его предлагать.

**Токен выпусков** — `DALYWAVE_RELEASE_TOKEN`, один на сервер и конвейер; ключа R2 конвейеру не нужно.
Завести один раз (значение не печатается):

```sh
ssh ubuntu@129.159.27.88 'cd ~/dalywave && echo "DALYWAVE_RELEASE_TOKEN=$(openssl rand -hex 32)" >> .env && chmod 600 .env'
# на сервере: docker compose … up -d api; затем то же значение — в связку ключей Mac и в GitHub
security add-generic-password -s dalywave-release-token -a "$USER" -w   # спросит значение
```

В GitHub — секрет `DALYWAVE_RELEASE_TOKEN` (Settings → Secrets → Actions) рядом с
`TAURI_SIGNING_PRIVATE_KEY` и `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

## Статус в Discord

«Слушает DalyWave»: трек, исполнители, обложка, полоса прогресса и кнопка «Слушать в DalyWave».
Нужно приложение Discord — один раз, минуты две:

1. https://discord.com/developers/applications → **New Application** → имя **DalyWave** (это имя
   Discord пишет в «Слушает …») → согласиться с условиями → **Create**.
2. **General Information** → **App Icon** → загрузить `design/brand/icon-1024.png` → **Save Changes**.
3. Там же скопировать **Application ID** (число) и вписать в `tauri.conf.json`:
   `"plugins": { "discord": { "clientId": "<Application ID>" } }`. Это не секрет — его видит любой
   клиент Discord. Для проверки без пересборки — переменная `DALYWAVE_DISCORD_CLIENT_ID`.
4. Собрать приложение. В «Настройках» появится «Интеграции → Статус в Discord» (по умолчанию включён).
   В Discord: «Настройки» → «Конфиденциальность активности» → «Показывать текущую активность» —
   включено.

Картинки в Rich Presence загружать не нужно: обложка — публичный адрес Deezer, значок —
`https://wave.daly.rest/icon-512.png`. На паузе дольше 30 с статус пропадает; Discord запустили позже —
статус появится сам.

## iPhone и iPad

`make ios` — Xcode-проект `gen/apple/dalywave-shell.xcodeproj`, дальше Run в Xcode. Подробно — [IOS.md](IOS.md).

## Значки

`scripts/icons.sh` пересобирает всё из `design/brand` (после `bun tools/brand.mjs`):

- PNG, `icon.ico` и плитки Microsoft Store — `cargo tauri icon design/brand/icon-1024.png`;
- `icon.icns` — из `icon-macos-1024.png` (сетка Apple с полями и тенью) — для macOS до 26;
- `DalyWave.icon` — документ Icon Composer, общий для macOS и iOS: фон `layers/background.svg` и
  знак (стекло); в тёмной теме — знак в градиенте на тёмном (`icon-ios-dark.svg`), в тонированной
  и прозрачной фон скрыт и знак красит система. Для iOS до 26 — каталог
  `gen/apple/Assets.xcassets/DalyWave.appiconset` (светлая, тёмная и тонированная 1024 из
  `design/brand`). При сборке Tauri компилирует его `actool`
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
установщик NSIS для текущего пользователя, ссылки `dalywave://` регистрирует он же. Загрузчик
собирается там же: задание «FFmpeg для Windows» (Ubuntu) кросс-собирает LGPL-сборку FFmpeg только
для звука (`services/fetcher/sidecar/ffmpeg-windows.sh`), затем на Windows
`services/fetcher/sidecar/build-windows.ps1` собирает `dalywave-fetcher.exe` (PyInstaller), кладёт
Deno, fpcalc и сервер PO-токенов и проверяет запуск; Tauri упаковывает всё в установщик.
Подписи кода для Windows нет — SmartScreen предупредит при первом запуске.

**Обновления.** Установленное приложение обновляется само (раздел «Выпуск»): тег `v*` собирает и
выпускает новую версию, после «Перезапустить» установщик ставит её поверх.

**Запуск сборки.** GitHub → Actions → «Приложения» → Run workflow (ветка main). Через ~25 минут
в итогах запуска — артефакт `DalyWave-x86_64-pc-windows-msvc` с `DalyWave_0.1.0_x64-setup.exe`.

**Установка на ПК** (Windows 10/11 x64):

1. Скачать артефакт, распаковать, запустить `DalyWave_…_x64-setup.exe`. SmartScreen: «Подробнее»
   → «Выполнить в любом случае». Ставится в `%LOCALAPPDATA%\DalyWave`, без прав администратора.
2. Открыть DalyWave → «Войти через daly.rest» → вход в браузере, возврат в приложение.
3. Загрузчик запустится сам (включён по умолчанию): приложение заведёт свой токен и появится в
   админке («Источники») как «PC <имя компьютера>».
4. Если Защитник Windows поместит `dalywave-fetcher.exe` в карантин (одиночные exe PyInstaller
   бывают ложными срабатываниями): «Безопасность Windows» → «Журнал защиты» → «Восстановить» и
   добавить папку `%LOCALAPPDATA%\DalyWave` в исключения.

**Статус загрузчика.** Значок DalyWave в трее (у часов, возможно под стрелкой «^») → правый щелчок
→ «Загрузчик: …» — состояние, «Пауза»/«Продолжить», «Скачивать на этом ПК». В приложении —
строка «Загрузчик на этом ПК» внизу боковой панели и раздел в настройках (счётчик за сегодня,
«Журнал»). Закрытие окна прячет приложение в трей — загрузчик продолжает работать; «Выйти из
DalyWave» в трее останавливает его. Журнал — `%LOCALAPPDATA%\DalyWave\logs\fetcher.log`, журнал
приложения — `%LOCALAPPDATA%\rest.daly.wave\logs\DalyWave.log` (строки `downloader: …`).

Ручная проверка на Windows 11: Mica под прозрачными местами, меню привязки окон при наведении
на свою кнопку «развернуть» (стенд), трей, медиаклавиши и плашка SMTC, вход по
`dalywave://auth`, токены в «Диспетчере учётных данных», нет мелькающих окон консоли при запуске
загрузчика.
