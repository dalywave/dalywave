# Загрузчик в приложении (sidecar) — договор для приложения

Домашний загрузчик (`fetcher worker`) живёт внутри приложения DalyWave для macOS как
sidecar Tauri: приложение запущено — загрузчик берёт задания у сервера и качает со своего
адреса; приложение закрыто — не качает ничего. Приложение показывает состояние загрузчика и
ставит его на паузу. Docker, Homebrew, uv и launchd на машине пользователя не нужны.

Документ — для того, кто встраивает загрузчик в `apps/shell`. Протокол с сервером — в
[README.md](README.md#домашний-загрузчик).

## Состав и сборка

```sh
services/fetcher/sidecar/build.sh           # → services/fetcher/dist/sidecar/
CLEAN=1 services/fetcher/sidecar/build.sh   # всё заново
```

Нужны (только для сборки): uv, Xcode Command Line Tools (clang, make), npm (Node ≥ 22).
Скачанное и собранное кэшируется в `dist/build/`; повторная сборка — около минуты, первая —
3–4 минуты. `dist/` в git не попадает. В конце сборка запускает загрузчик и проверяет, что он
нашёл и запустил свои программы, и пишет `dist/sidecar/MANIFEST.json` (версии, источники,
sha256, размеры).

| Файл в `dist/sidecar/` | Размер | Что это | Откуда | Лицензия |
|---|---|---|---|---|
| `dalywave-fetcher-aarch64-apple-darwin` | 29,9 МБ | сам загрузчик: Python 3.13.18 + yt-dlp 2026.10.26 с yt-dlp-ejs 0.8.0, ytmusicapi 1.12.3, плагин bgutil-ytdlp-pot-provider 2.0.2, numpy, httpx. PyInstaller 6.22.3, один файл | этот репозиторий, зависимости — из `uv.lock` (группа `sidecar`) | внутри: PSF, Unlicense, MIT, BSD, плагин bgutil — GPL-3.0 |
| `deno-aarch64-apple-darwin` | 81,5 МБ | Deno 2.9.8: JS-задачи YouTube (yt-dlp-ejs) и сервер PO-токенов | `github.com/denoland/deno/releases/download/v2.9.8/deno-aarch64-apple-darwin.zip`, sha256 `9f0bf660393da2eaff044cdc7444cb08c73f235849b61d224d825a194ce7c98e` | MIT |
| `ffmpeg-aarch64-apple-darwin`, `ffprobe-aarch64-apple-darwin` | 4,6 + 4,4 МБ | FFmpeg 9.0.2, своя сборка только для звука, статически, без внешних библиотек | `ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz`, sha256 `8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e` (совпадает с формулой Homebrew); флаги `configure` — в `build.sh` и `MANIFEST.json` | **LGPL-2.1+**: собран без `--enable-gpl`/`--enable-nonfree`, `ffmpeg -L` это подтверждает |
| `fpcalc-aarch64-apple-darwin` | 2,6 МБ | chromaprint 1.6.2, официальная статическая сборка | `github.com/acoustid/chromaprint/releases/download/v1.6.2/chromaprint-fpcalc-1.6.2-macos-arm64.tar.gz`, sha256 `f3bc56cb42d89a01237bb3f3629329e4ec75f7c1b0217acf6e9bf952ac47979b` | LGPL-2.1+ (chromaprint MIT + вшитый FFmpeg без GPL) |
| `resources/bgutil-server/` | 48,3 МБ, 3 528 файлов | сервер PO-токенов bgutil 2.0.2 (TypeScript, запускается нашим Deno) + `node_modules` по `package-lock.json` без install-скриптов; `canvas` 3.2.3 — готовая сборка | `github.com/Brainicism/bgutil-ytdlp-pot-provider/archive/refs/tags/2.0.2.tar.gz`, sha256 `1f2ae9ec39405b8e80397dfc2551f58deed09d8dff4b2b8fbc314df8e091c0a1`; canvas — `canvas-v3.2.3-napi-v7-darwin-arm64.tar.gz`, sha256 `38c296c9d81c05598db849fb8103543d6649fec7246c35197d9be8199c75116f` | GPL-3.0 (bgutil); MIT/ISC/BSD (`node_modules`); cairo/pango в canvas — LGPL/MPL |
| `licenses/`, `MANIFEST.json` | — | тексты лицензий FFmpeg, chromaprint, bgutil, Python; паспорт сборки | | |

Всего 171 МБ. Все программы зависят только от системных библиотек macOS (`otool -L`),
минимальная macOS — 11.

Лицензии: проект личный и не распространяется — обязательств нет. Если приложение когда-нибудь
будут раздавать: FFmpeg остаётся LGPL (исходники и флаги сборки указаны выше), а плагин bgutil
(GPL-3.0) загружается в процесс загрузчика, поэтому `dalywave-fetcher` как целое тогда
подпадает под GPL-3.0.

Почему так собрано:

- **PyInstaller, один файл.** Tauri `externalBin` переносит в пакет только отдельные файлы;
  папка onedir (`_internal/`) в `Contents/MacOS` не ложится, а связка python-build-standalone
  с запускающим скриптом не годится для Windows. Один файл — одинаково на macOS и Windows.
  Цена — распаковка во временную папку при запуске (~1,7 с) и два процесса: загрузчик PyInstaller
  и сам Python; сигналы первый передаёт второму. yt-dlp сам распространяется так же.
- **Плагины yt-dlp** ищутся как папки на `sys.path`, а не в архиве модулей — поэтому исходники
  `yt_dlp_plugins/` кладутся в сборку файлами (`--add-data`).
- **FFmpeg из исходников.** Готовых LGPL-сборок `ffmpeg`/`ffprobe` для macOS arm64 нет
  (evermeet, osxexperts, Martin Riedl, Homebrew — GPL; у acoustid/ffmpeg-build нет фильтров
  `ebur128`/`asplit` и формата `null`). Сборка — 1–2 минуты, громкость, истинный пик и
  огибающая совпадают с полным ffmpeg из Homebrew до последней цифры.

## Подключение к Tauri

Имена в `dist/sidecar/` — как ждёт `externalBin`: `<имя>-<target triple>`. В пакете они
окажутся в `Contents/MacOS/` без суффикса, ресурсы — в `Contents/Resources/`. Загрузчик сам ищет
`deno`, `ffmpeg`, `ffprobe`, `fpcalc` рядом с собой (с суффиксом и без), а сервер PO-токенов — в
`../Resources/bgutil-server`, `resources/bgutil-server` или `bgutil-server` рядом с собой.

`apps/shell/tauri.macos.conf.json` (пути — от `apps/shell/`):

```json
{
  "bundle": {
    "externalBin": [
      "../../services/fetcher/dist/sidecar/dalywave-fetcher",
      "../../services/fetcher/dist/sidecar/deno",
      "../../services/fetcher/dist/sidecar/ffmpeg",
      "../../services/fetcher/dist/sidecar/ffprobe",
      "../../services/fetcher/dist/sidecar/fpcalc"
    ],
    "resources": {
      "../../services/fetcher/dist/sidecar/resources/bgutil-server/": "bgutil-server/"
    }
  }
}
```

Перед `tauri build` (и `tauri dev`) должен быть выполнен `services/fetcher/sidecar/build.sh`.
После сборки проверить, что есть `DalyWave.app/Contents/Resources/bgutil-server/serve.ts`.
Запуск — из Rust через `tauri-plugin-shell` (2.x, в `Cargo.toml` его ещё нет;
`.plugin(tauri_plugin_shell::init())`). Права в capabilities не нужны: они ограничивают только JS.

```rust
use tauri::Manager;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

let pot_dir = app.path().resource_dir()?.join("bgutil-server");
let data_dir = app.path().app_local_data_dir()?.join("fetcher");
let log_file = app.path().app_log_dir()?.join("fetcher.log");
let (mut rx, child) = app
    .shell()
    .sidecar("dalywave-fetcher")?
    .args(["sidecar", "--worker-name", "Mac Захара (приложение)"])
    .args(["--pot-server-dir".as_ref(), pot_dir.as_os_str()])
    .args(["--data-dir".as_ref(), data_dir.as_os_str()])
    .args(["--log-file".as_ref(), log_file.as_os_str()])
    .env("DALYWAVE_DOWNLOADER_TOKEN", token) // из связки ключей
    .spawn()?;
// `child` хранить в состоянии приложения: команды — `child.write(b"{\"cmd\":\"pause\"}\n")`
tauri::async_runtime::spawn(async move {
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                if let Ok(event) = serde_json::from_slice::<serde_json::Value>(&line) {
                    // hello / status / job / ack / exit → трей
                }
            }
            CommandEvent::Terminated(done) => { /* done.code — код выхода */ break; }
            _ => {} // Stderr — лог, читать всё равно
        }
    }
});
```

Подпись: для локальной сборки (ad-hoc) ничего не нужно — так всё и проверено. Если приложение
подписывается с hardened runtime (Developer ID, нотаризация), sidecar-программам нужны
entitlements `com.apple.security.cs.allow-jit` и `com.apple.security.cs.allow-unsigned-executable-memory`
(V8 в Deno) и `com.apple.security.cs.disable-library-validation` (PyInstaller грузит свой
`libpython` из временной папки, Deno — `canvas.node`); это не проверялось. App Sandbox — выключен
(загрузчику нужны свои папки и запуск дочерних программ).

## Запуск

```
dalywave-fetcher sidecar [--server URL] [--worker-name ИМЯ] [--data-dir ПАПКА] [--log-file ФАЙЛ]
                         [--paused] [--pot-server-dir ПАПКА] [--tools-dir ПАПКА]… [--no-pot]
```

| Аргумент | По умолчанию | Что задаёт |
|---|---|---|
| `--server` | `$DALYWAVE_SERVER` или `https://wave.daly.rest` | сервер DalyWave |
| `--worker-name` | имя компьютера | имя загрузчика в админке («Источники»), 1–100 символов. Для этого Mac — «Mac Захара (приложение)»: так он не путается с загрузчиком из launchd |
| `--data-dir` | `~/Library/Application Support/rest.daly.wave/fetcher` (= `app_local_data_dir()/fetcher`) | кэш yt-dlp, временные файлы заданий, кэш Deno, `state.json` (счётчики за день и отдых после бот-проверки) |
| `--log-file` | `~/Library/Logs/rest.daly.wave/fetcher.log` (= `app_log_dir()/fetcher.log`) | лог; 5 МБ × 3 файла по кругу |
| `--paused` | — | запуститься на паузе. Пауза в загрузчике не запоминается: если пользователь оставил паузу, приложение передаёт этот флаг при следующем запуске |
| `--pot-server-dir` | ищется рядом с программой | папка `bgutil-server`; надёжнее передать `resource_dir()/bgutil-server` явно |
| `--tools-dir` | папка самой программы | где искать deno, ffmpeg, ffprobe, fpcalc |
| `--no-pot` | — | без сервиса PO-токенов |

Окружение:

- `DALYWAVE_DOWNLOADER_TOKEN` — токен `dwk_…` (обязателен). Приложение хранит его в связке
  ключей и передаёт только переменной окружения, не аргументом (аргументы видны в `ps`). Загрузчик
  сразу убирает переменную из своего окружения — дочерним Deno и ffmpeg она не достаётся.
  Принимается и старое имя `FETCHER_WORKER_TOKEN`.
- Необязательные `FETCHER_*` из [README](README.md#переменные-окружения) работают как у
  `fetcher worker` (`FETCHER_PROXY`, `FETCHER_SOURCE_ADDRESS`, `FETCHER_PLAYER_CLIENTS`, …), а
  `FETCHER_FFMPEG`, `FETCHER_FFPROBE`, `FETCHER_FPCALC`, `FETCHER_DENO` подменяют программы.

Рабочая папка процесса не важна.

## Протокол

Строки JSON, одна строка — одно сообщение, UTF-8, `\n` в конце. **stdout** — только события
загрузчика; **stdin** — команды приложения; **stderr** — тот же текстовый лог, что в файле.

Почему stdin/stdout, а не HTTP-порт с секретом: у sidecar Tauri они есть из коробки
(`CommandEvent::Stdout` по строке, `CommandChild::write`), не нужны ни порт, ни секрет, ни
поиск адреса, а закрытие stdin — надёжный признак, что приложения больше нет, даже если оно
упало или убито: загрузчик возвращает задание серверу и выходит. Так и выполняется «приложение
закрыто — ничего не качается».

Важно для приложения:

- Держать `CommandChild` всё время работы загрузчика: его удаление закрывает stdin, и загрузчик
  останавливается (так задумано).
- Читать события из канала `rx` постоянно, в том числе `Stderr`: канал Tauri ограничен, и, пока
  его не читают, загрузчик ждёт на записи. Строка `Stdout` может прийти с `\n` в конце — `serde_json`
  его переносит.
- Неизвестные поля и события пропускать: протокол будет расти (`hello.protocol` — версия, сейчас 1).

### События (stdout)

У каждого события есть `event` и `ts` (UTC, RFC 3339). Порядок: `hello` — первым, затем `status`
и остальные, `exit` — последним, после него строк нет.

**`hello`** — сразу после запуска (через ~1,7 с: распаковка PyInstaller):

```json
{"event":"hello","ts":"2026-11-11T13:25:49.800Z","protocol":1,"version":"0.2.0","ytDlp":"2026.10.26",
 "pid":41207,"server":"https://wave.daly.rest","workerName":"Mac Захара (приложение)",
 "dataDir":"/Users/…/Library/Application Support/rest.daly.wave/fetcher",
 "logFile":"/Users/…/Library/Logs/rest.daly.wave/fetcher.log",
 "tools":{"ffmpeg":{"path":"/Applications/DalyWave.app/Contents/MacOS/ffmpeg","version":"ffmpeg version 9.0.2 Copyright (c) 2000-2026 the FFmpeg developers"},
          "ffprobe":{"path":"…","version":"…"},"fpcalc":{"path":"…","version":"fpcalc version 1.6.2 (…)"},
          "deno":{"path":"…","version":"deno 2.9.8 (stable, release, aarch64-apple-darwin)"}}}
```

**`status`** — полное состояние; приходит при каждом изменении (одинаковые подряд не повторяются),
ход задания — не чаще 4 раз в секунду, и в ответ на команду `status`. Приложению достаточно
показывать последний `status`.

```json
{"event":"status","ts":"2026-11-11T11:00:30.112Z","state":"working","paused":false,
 "server":"https://wave.daly.rest","workerName":"Mac Захара (приложение)",
 "job":{"id":"01a1ca44-2432-77cc-ad23-095bf7dce3a6","deezerTrackId":4262016331,"track":"Another World",
        "artist":"CONTROL Resonant","stage":"downloading","progress":0.42,"attempt":1},
 "error":null,"retryAt":null,
 "counters":{"date":"2026-11-11","doneToday":3,"failedToday":1},
 "pot":{"state":"ready","version":"2.0.2"}}
```

| `state` | Что значит | Что показать |
|---|---|---|
| `starting` | только запущен: поднимается сервис PO-токенов (3–6 с), сервер ещё не спрошен | «Запуск…» |
| `idle` | на связи с сервером, ждёт задание (сервер держит запрос до 25 с, следующий — сразу) | «Ждёт заданий» |
| `working` | качает: `job` заполнен | трек, артист, этап, прогресс |
| `paused` | на паузе по просьбе пользователя, заданий не берёт | «На паузе» |
| `offline` | сервер недоступен (`error.code`: `server_unreachable` — нет связи, `server_error` — 5xx и прочие неожиданные ответы); следующая попытка в `retryAt` (5 с, затем вдвое дольше, до 5 минут) | «Нет связи с сервером» |
| `error` | нужна реакция или вынужденный отдых: `token_rejected` (следом `exit` с кодом 3), `no_token` (код 2), `tools_missing` (код 4); либо YouTube недоволен — `bot_check`, `rate_limited`, `http_403`, `po_token`, `network`, `cookies_missing`: отдых 15 минут до `retryAt`, потом сам продолжит (отдых переживает перезапуск) | текст `error.message`, время `retryAt` |
| `stopping` | останавливается: задание возвращается серверу | — |

- `paused` — желание пользователя. `paused: true` при `state: "working"` значит «доделывает
  текущий трек, потом пауза».
- `job` есть только при `working`: `track` и `artist` — `null` первые ~0,3 с (пока не ответил
  Deezer); `stage` — `matching` (Deezer, поиск на YouTube Music, превью) → `downloading` →
  `verifying` (отпечаток, затем громкость и огибающая) → `uploading` (в R2 и отчёт серверу);
  если кандидат не прошёл проверку, этапы `downloading`/`verifying` повторяются со следующим
  (`attempt` — номер кандидата). `progress` — доля 0..1 внутри этапа: у скачивания и загрузки — по
  байтам, у остальных — шагами.
- `counters` — за сегодняшний день по часам Mac, переживают перезапуск. `doneToday` — файл загружен
  и сервер принял отчёт; `failedToday` — задание кончилось без годного файла («Спорные», «нет в
  источниках», сбой). Возвращённые и потерянные задания не считаются нигде.
- `error` — `{"code": "…", "message": "текст по-русски"}` или `null`; `retryAt` — RFC 3339 или `null`.
- `pot.state`: `starting`, `ready` (`version`), `failed` (`message` — качает без токенов: обычным
  клиентам они не нужны, токены выручают после 403), `off` (`--no-pot`).

**`job`** — задание закончилось:

```json
{"event":"job","ts":"2026-11-11T11:00:39.529Z","id":"01a1ca44-2432-77cc-ad23-095bf7dce3a6",
 "deezerTrackId":4262016331,"track":"Another World","artist":"CONTROL Resonant",
 "outcome":"done","status":"verified","error":null,"elapsedMs":13690}
```

`outcome`: `done`, `failed`, `released` (прервано паузой или остановкой и сразу возвращено
серверу), `lost` (задание ушло из рук: аренда кончилась и его отдали другому, или отчёт так и не
дошёл — сервер вернёт задание в очередь по аренде). `status` — итог fetcher
(`verified`, `unverified`, `disputed`, `not_found`, `error`) или `null`, если задание прервано;
`error` — `{"code","message"}` из результата.

**`ack`** — ответ на каждую команду приложения, раньше вызванного ею `status`:

```json
{"event":"ack","ts":"2026-11-11T09:52:55.808Z","cmd":"pause","ok":true,"id":1}
{"event":"ack","ts":"2026-11-11T09:53:06.404Z","cmd":"dance","ok":false,"message":"неизвестная команда: 'dance'"}
```

**`exit`** — последняя строка перед выходом:

```json
{"event":"exit","ts":"2026-11-11T13:25:59.584Z","code":0,"reason":"signal 15","message":null}
```

`reason`: `stop` (команда), `stdin closed`, `signal N`, `worker exited`, `no_token`,
`tools_missing`, `token_rejected`, `crashed`.

### Команды (stdin)

Одна строка — один объект. Необязательное поле `id` (любое значение) возвращается в `ack`.

| Команда | Что делает |
|---|---|
| `{"cmd":"pause"}` | не брать новые задания; текущее — доделать (`state` остаётся `working` с `paused: true`), затем `paused` |
| `{"cmd":"pause","now":true}` | то же, но текущее задание прервать и сразу вернуть серверу (`job` с `outcome: "released"`) |
| `{"cmd":"resume"}` | снова брать задания (отдых после бот-проверки не отменяет) |
| `{"cmd":"status"}` | прислать `status` |
| `{"cmd":"stop"}` | остановиться: вернуть текущее задание серверу, выключить сервис PO-токенов, `exit` с кодом 0 |

Так же, как `stop`, загрузчик останавливается по SIGTERM, SIGINT, SIGHUP и при закрытии stdin —
без `ack`.

### Остановка и коды выхода

| Код | Когда | Что делать приложению |
|---|---|---|
| 0 | остановлен командой, сигналом или закрытием stdin | — |
| 1 | цикл загрузчика упал (подробности — в логе) | перезапустить с растущей паузой |
| 2 | нет токена (или неверные аргументы) | попросить токен |
| 3 | сервер не принял токен (отозван) | попросить новый токен, **не** перезапускать |
| 4 | свои программы не нашлись или не запускаются | сообщить о повреждённой установке |

При выходе из приложения: отправить `{"cmd":"stop"}` и дождаться `exit` (или
`CommandEvent::Terminated`), затем выходить. Без задания это ~0,05 с. С заданием — прерывание
и возврат: `POST /api/fetcher/jobs/{id}/release` (таймаут 5 с), затем, если сервер его не знает,
отчёт о сбое исполнителя (ещё до 5 с); если отчёт о готовом задании уже отправляется — ждём его
до 10 с. Разумный предел ожидания — 15 с; дальше — `child.kill()`.

`child.kill()` (SIGKILL) убивает только загрузчик PyInstaller; сам Python увидит закрытый stdin и
всё равно вернёт задание. Если же погибнет всё дерево процессов, сервер заберёт задание по аренде
(до 5 минут, попытка засчитается). Сервер PO-токенов выходит сам, как только закрылся его stdin
(обёртка `serve.ts`), — сирот не остаётся.

## Что происходит с текущим заданием при остановке

Сервер сейчас не умеет «отпустить» задание по просьбе исполнителя. Поэтому загрузчик сначала
пробует `POST /api/fetcher/jobs/{id}/release` `{"workerId": …, "reason": …}` (сейчас — 404), а затем
отчитывается результатом `{"status":"error","error":{"code":"network","message":"загрузчик
остановлен: …","interrupted":true}}`. `network` сервер считает сбоем исполнителя: задание сразу
возвращается в очередь, попытка не тратится, но взять его можно только через 2 минуты
(`REMOTE_POSTPONE`), и в журнале попыток оно будет с кодом `network`.

Чтобы задание уходило другому исполнителю мгновенно, серверу стоит добавить
`POST /api/fetcher/jobs/{id}/release` (право `downloader`, задание должно быть `running` и
`locked_by` этим загрузчиком) → `jobs::postpone_in(…, Duration::ZERO)` и `sync_with_job`, как
`result::give_back`; ответ 204, чужое задание — 409. Загрузчик уже его вызывает, пересборка не нужна.

## Сервис PO-токенов без Docker

Загрузчик запускает HTTP-сервер bgutil своим Deno:
`deno run --allow-env --allow-net --allow-ffi=<node_modules> --allow-read=<node_modules> serve.ts --port <случайный> --host 127.0.0.1`.
Порт — случайный свободный, только 127.0.0.1; адрес получает yt-dlp
(`youtubepot-bgutilhttp:base_url`). `serve.ts` — наша обёртка над `src/main.ts` bgutil: сервер
выходит, когда закрылся его stdin. Кэш Deno — в `--data-dir/deno`. Если сервер упал, загрузчик
поднимает его снова (не чаще раза в минуту); не поднялся — качает без токенов, `pot.state:
"failed"`.

Почему сервер, а не одноразовый скрипт bgutil (`generate_once.ts`): сервер держит BotGuard «тёплым»
(токен за доли секунды вместо запуска Deno, jsdom и BotGuard на каждый запрос), а yt-dlp передаёт
ему задачу BotGuard со страницы видео; скрипт её не получает. Тот же режим был у Docker-сервиса в
замере `bench/REPORT.md`.

Проверка: `dalywave-fetcher pot-check` поднимает сервер, получает два токена и просит у YouTube
потоки клиентом `mweb`, которому без токена их не отдают. На этом Mac: сервер готов за 3,1 с,
токены по 120 символов за 1,0 и 0,7 с, у yt-dlp 6 из 10 аудиоформатов со ссылками с `pot=`
(клиент `MWEB`).

## Логи и данные

- Лог — `--log-file` (по умолчанию `~/Library/Logs/rest.daly.wave/fetcher.log`, 5 МБ × 3),
  копия — в stderr. Токен в лог не попадает; запросы заданий (раз в ~25 с) не пишутся.
- `--data-dir`: `state.json` (счётчики, отдых), `yt-dlp-cache/`, `deno/` (кэш Deno), `jobs/`
  (временные файлы задания; удаляются после загрузки, забытые — через сутки).
- PyInstaller распаковывается в `$TMPDIR/_MEI…` и убирает за собой при выходе.

## Замеры на этом Mac (M-серия, macOS 27)

| Что | Значение |
|---|---|
| запуск до `hello` | 1,7–3 с |
| сервис PO-токенов | 3–6 с при первом запуске, затем `idle` через 0,2 с |
| память без задания | Python ~33 МБ, Deno (сервис PO-токенов) ~96 МБ, загрузчик PyInstaller < 1 МБ |
| задание на проде | «Another World» (CONTROL Resonant): 13,7 с, `verified`, Opus в MP4 загружен в R2, отчёт принят (204) |
| остановка без задания | 0,03–0,09 с по SIGTERM, `stop` или закрытию stdin |

## Чек-лист для приложения

1. Собрать `services/fetcher/sidecar/build.sh`; добавить `externalBin` и `resources` (выше).
2. Токен `dwk_…` — в связке ключей (вставляется в настройках; выдаёт админка: «Источники» →
   новый загрузчик); передавать в `DALYWAVE_DOWNLOADER_TOKEN`.
3. При старте приложения запустить `dalywave-fetcher sidecar --worker-name "Mac Захара (приложение)"
   --data-dir <app_local_data_dir>/fetcher --log-file <app_log_dir>/fetcher.log
   --pot-server-dir <resource_dir>/bgutil-server` (и `--paused`, если пользователь оставил паузу).
   Один экземпляр на приложение.
4. Читать `rx` постоянно; последний `status` — в меню в трее: состояние, трек/артист/этап/прогресс,
   «сегодня: N скачано, M не удалось», сервер и имя загрузчика, текст ошибки и `retryAt`.
5. Пункты меню: «Пауза» → `pause` (или `pause` с `now: true` — «остановить сейчас»),
   «Продолжить» → `resume`.
6. Выход из приложения: `stop` → ждать `exit` до 15 с → выходить.
7. По коду выхода: 1 — перезапуск с паузой; 2 и 3 — спросить токен, не перезапускать; 4 — ошибка
   установки.

## Windows x64 (позже)

`build.sh` сейчас только для macOS arm64. Для Windows (`x86_64-pc-windows-msvc`, сборка на самой
Windows — PyInstaller не собирает накрест):

- загрузчик — тот же PyInstaller onefile → `dalywave-fetcher-x86_64-pc-windows-msvc.exe`;
- Deno — `deno-x86_64-pc-windows-msvc.zip` 2.9.8, sha256 `cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf`;
- fpcalc — `chromaprint-fpcalc-1.6.2-windows-x86_64.zip`, sha256 `b74369726676670af3ab0eadff45f960cf969fdf4a87a971ba37eaf8d530e874`;
- FFmpeg — LGPL-сборка BtbN (`ffmpeg-n…-win64-lgpl-…`, в ней все нужные фильтры) или та же
  конфигурация через MSYS2;
- canvas — `canvas-v3.2.3-napi-v7-win32-x64.tar.gz`, sha256 `ba953cc8c38303ab94cc83461c7561506a5a3af37d6c678de4a47914d2d0bb48`;
- папки по умолчанию — `%LOCALAPPDATA%\rest.daly.wave\fetcher` и `…\logs`;
- SIGTERM в Windows нет: останавливать командой `stop` или закрытием stdin (работает так же).
