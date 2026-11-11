# fetcher — поиск и скачивание аудио

Сервис получает трек Deezer, находит ту же запись на YouTube Music, скачивает лучший
аудиопоток без перекодирования, сверяет отпечаток с 30-секундным превью Deezer, считает
громкость и огибающую и отвечает JSON. По желанию кладёт файл в R2 по подписанной ссылке.
Тот же пакет работает домашним загрузчиком на Mac (`fetcher worker`, ниже).

Python 3.13, uv, yt-dlp + Deno, ytmusicapi, ffmpeg, chromaprint. HTTP — FastAPI на `:8402`.
Прогоны на 20 треках: сводки — [`bench/results-*.md`](bench/), калибровка порога —
[`bench/calibration.json`](bench/calibration.json).

## Как это работает

```
Deezer: трек, альбом ──► поиск кандидатов ──► оценка ──► скачивание ──► проверка ──► громкость ──► JSON / R2
                          (YouTube Music)       0..1      (yt-dlp)      (отпечаток)   и огибающая
```

1. **Deezer** (`deezer.py`). Трек, альбом с полным списком треков и дисков, поиск,
   `track/isrc:…`. Ключ не нужен; клиент держит не больше 40 запросов за 5 секунд и
   повторяет запрос при ответе «лимит исчерпан». Ссылка на превью подписана и живёт
   ~15 минут, поэтому превью качается сразу после свежего запроса трека.
2. **Поиск** (`match.py`). Три шага идут параллельно, затем — видео, если нужно:
   - *альбом*: поиск «артист альбом» среди альбомов YouTube Music, сверка названия,
     артиста и года, затем список треков альбома: трек ищется по позиции, названию и
     длительности. В альбоме YouTube Music вместо аудио часто стоит клип (OMV) — тогда
     берётся тот же трек из аудиоплейлиста альбома (`audioPlaylistId`, официальное аудио
     «Topic», ATV);
   - *ISRC*: поиск по коду записи — YouTube Music отдаёт нужную песню первой;
   - *песни*: «артист название» с фильтром songs;
   - *видео*: только если лучший кандидат хуже 0,85 или аудиоверсии не прошли проверку.
3. **Оценка** — число 0..1 и разбор по составляющим (его показывает раздел «Спорные»):

   | Составляющая | Вес | Как считается |
   |---|---|---|
   | Длительность | 0,30 | ≤ 2 с — 1; ≤ 5 с — 0,9; к 15 с падает до 0,3; > 15 с — отказ |
   | Название | 0,30 | rapidfuzz по основе названия (без feat., «Official Video», «Remastered 2011», скобочного шума), 20 % — по версии в скобках; кириллица сравнивается и в транслите |
   | Артисты | 0,20 | основной и приглашённые; имена из канала («VEVO», «- Topic»), из «feat.» и из префикса «Артист - Песня» |
   | Альбом | 0,10 | совпадение альбома; у найденных через альбом — оценка сверки альбома |
   | Источник | 0,10 | ATV (официальное аудио) 1; клип артиста 0,75; прочее 0,35 |

   Поверх суммы — множители. **Метки версий** (live, remix, sped up, slowed, ultra/super
   slowed, reverb, nightcore, 8D, cover, karaoke, instrumental, acoustic, edit, extended…)
   с одной стороны: ×0,3 для сильных, ×0,6 для слабых. Если метка есть и в названии Deezer
   («(Slowed)»), slowed-версия — правильный ответ, а оригинал получает штраф.
   **Ворота названия**: при сходстве основы 0,45 → 0, 0,85 → 1 — иначе любая песня того
   же артиста похожей длины набирала бы ~0,6. **Ворота артиста**: ни одного совпадения —
   ×0,75. Отказ: длительность разошлась больше чем на 15 с или основа названия похожа
   меньше чем на 0,45. Ниже 0,6 кандидат не качается.
4. **Скачивание** (`download.py`). Формат `774/251/bestaudio[acodec=opus]/140/bestaudio`:
   Opus 256 кбит/с (только с Premium), обычный Opus ~130–160 кбит/с (itag 251), AAC как
   запасной. Файл не перекодируется: Opus и AAC перекладываются в MP4 (ISO BMFF, `.m4a`,
   `ffmpeg -c:a copy -movflags +faststart -f mp4`; `audio/mp4; codecs="Opus"` — код записи MP4 по
   RFC 6381: на `codecs="opus"` Safari отвечает «не умею»). Opus в MP4
   играют Safari, AVPlayer, Chrome, Firefox и движок приложений, а `faststart` ставит
   оглавление в начало — плеер начинает играть, не дочитав файл. Записываются itag, кодек, битрейт, частота,
   размер, исходный контейнер и клиент плеера (параметр `c=` ссылки потока), был ли
   PO-токен. JS-задачи YouTube решает Deno (`yt-dlp-ejs`), PO-токены выдаёт
   bgutil-ytdlp-pot-provider. Скачивания идут по одному, между ними пауза. Если поток
   ответил 403 или формат недоступен, тот же ролик пробуется другим набором клиентов.
   Перед потоком YouTube может требовать «досмотреть рекламу» — yt-dlp ждёт, в ответе это
   `adWait` (обычно 4–6 с — половина всего времени).
5. **Проверка** (`verify.py`). `fpcalc -raw` по превью и по файлу; превью прикладывается
   к треку со всеми сдвигами (шаг ~0,124 с), на каждом сдвиге считается доля несовпавших
   бит. Сходство = 1 − BER на лучшем сдвиге, `offsetS` — где в треке начинается превью.
   Порог `0.80` откалиброван: верные записи — от 0,92, чужие треки и другие записи той же
   песни — до 0,62 (`bench/calibration.json`). Не совпало —
   следующий кандидат; кандидаты кончились — «Спорные».
6. **Громкость и огибающая** (`analyze.py`). Один проход ffmpeg по выбранному файлу:
   `ebur128=peak=true` — интегральная громкость (EBU R128, LUFS) и истинный пик (dBFS),
   вторая ветка — моно 8 кГц для огибающей: RMS по 512 равным кускам, 0..1000 от самого
   громкого. ~0,8 с на трёхминутный трек. Не вышло — файл всё равно годится, причина —
   в `file.analysisError`.

Статусы ответа:

| `status` | Значение |
|---|---|
| `verified` | файл скачан, отпечаток совпал |
| `unverified` | проверить нечем (у трека нет превью в Deezer) — взят кандидат с оценкой ≥ 0,9; или видео выбрано вручную (`preferVideoIds`) и отпечаток с ним не сошёлся |
| `disputed` | кандидаты скачивались, но отпечаток не совпал ни разу → «Спорные» |
| `not_found` | подходящих кандидатов нет |
| `error` | сбой: `error.code` — `bot_check`, `http_403`, `po_token`, `rate_limited`, `geo_blocked`, `unavailable`, `format_unavailable`, `search_failed` (YouTube Music не ответил), `deezer_not_found`… |

## Запуск

```sh
cd services/fetcher
uv sync
uv run fetcher match 3040222091                      # кандидаты с разбором оценки
uv run fetcher acquire 3040222091 --out /tmp/track   # поиск → скачивание → проверка
uv run fetcher serve                                 # HTTP API на :8402
```

Сервис PO-токенов локально (порт только на 127.0.0.1). Рекомендуемая настройка —
клиенты по выбору yt-dlp плюс токены: после 403 тот же ролик сам повторяется через
`mweb` с GVS-токеном:

```sh
docker run -d --name dalywave-pot --restart unless-stopped -p 127.0.0.1:4416:4416 \
  brainicism/bgutil-ytdlp-pot-provider:2.0.2-deno
FETCHER_POT_URL=http://127.0.0.1:4416 uv run fetcher acquire …
```

Бенчмарк и калибровка порога (≥ 5 с между треками, по одному):

```sh
uv run fetcher bench --set bench/set.json --out bench/results-<метка>.json
# --hard докачивает другие записи тех же песен (концерты, ремиксы, sped up) — трудные негативы
uv run fetcher calibrate --audio-dir bench/out/<метка> --hard bench/hard-negatives.json \
  --out bench/calibration.json
```

Образ и бенчмарк в Docker (сеть с сервисом PO-токенов, порт наружу не открыт):

```sh
docker build -t dalywave-fetcher:0.2.0 .
export BENCH_UID=$(id -u) BENCH_GID=$(id -g)
BENCH_LABEL=docker-default-pot docker compose -f compose.bench.yml run --rm bench
BENCH_LABEL=docker-mweb-pot FETCHER_PLAYER_CLIENTS=mweb \
  docker compose -f compose.bench.yml run --rm bench
docker compose -f compose.bench.yml down
```

Проверки:

```sh
uv run ruff check . && uv run ruff format --check . && uv run pytest -q
uv run pytest -m network          # с настоящими Deezer и YouTube Music
```

Обновить yt-dlp (YouTube меняет защиту часто): `uv lock --upgrade-package yt-dlp`,
затем пересобрать образ.

## Переменные окружения

| Переменная | По умолчанию | Что задаёт |
|---|---|---|
| `FETCHER_POT_URL` | — | адрес bgutil-ytdlp-pot-provider, например `http://pot:4416`. Пусто — PO-токены не запрашиваются |
| `FETCHER_PLAYER_CLIENTS` | выбор yt-dlp | клиенты плеера через запятую: `mweb`, `tv`, `web_safari`, `default`… Рекомендуется не задавать: yt-dlp сам обновляет набор вслед за YouTube |
| `FETCHER_FALLBACK_CLIENTS` | авто | клиенты для повтора после 403: при заданных основных — `default`, иначе `mweb` при наличии PO-токенов; `none` — без повтора |
| `FETCHER_COOKIES` | — | файл cookies (Netscape) запасного аккаунта Google; yt-dlp получает копию в `FETCHER_DATA_DIR` |
| `FETCHER_DATA_DIR` | `data` (в образе `/data`) | кэш yt-dlp, копия cookies, временные файлы заданий |
| `FETCHER_PROXY` | — | прокси для yt-dlp (`socks5://…`) — запасной путь при блокировке |
| `FETCHER_SOURCE_ADDRESS` | — | исходящий адрес yt-dlp: `0.0.0.0` — только IPv4, `::` — только IPv6. YouTube судит о клиенте по адресу: на домашнем IPv4 `mweb` получал 403, на IPv6 — нет |
| `FETCHER_DENO` | `deno` из `PATH` | путь к Deno |
| `FETCHER_FFMPEG`, `FETCHER_FFPROBE`, `FETCHER_FPCALC` | из `PATH` | пути к ffmpeg, ffprobe, fpcalc. Собранный загрузчик (`dist/sidecar/`) берёт свои программы, лежащие рядом |
| `FETCHER_MIN_INTERVAL_S` | `3` | пауза между скачиваниями внутри процесса |
| `FETCHER_SELFTEST_TRACK` | `908604612` | трек Deezer для `/v1/selftest` |
| `FETCHER_WORKER_TOKEN` | — | токен домашнего загрузчика вместо `--token` |

## HTTP API

Поля JSON — camelCase, длительности — `…Ms`. Ошибка — статус и
`{"error": {"code": "snake_case", "message": "текст по-русски"}}`.
Скачивание — строго одно за раз: второй `acquire` или `selftest` получает `409 busy`.
`match` идёт под отдельным замком и не ждёт скачивания.

| Запрос | Тело | Ответ |
|---|---|---|
| `GET /v1/healthz` | — | версии yt-dlp, Deno, ffmpeg, fpcalc; доступность сервиса PO-токенов; срок cookies; занят ли сервис |
| `POST /v1/match` | `{"deezerTrackId": 3040222091, "videos": "auto"}` | `track`, `candidates[]` с `score` и `breakdown`, `steps[]` с временем шагов |
| `POST /v1/acquire` | `{"deezerTrackId": …, "upload": {"url": "<presigned PUT>", "headers": {}}, "maxAttempts": 3, "preferVideoIds": [], "excludeVideoIds": [], "directUrl": null}` | `status`, `chosen` (кандидат, `download`, `verify`), `file` (sha256, размер, контейнер, mime, громкость, пик, огибающая), `upload`, `attempts[]`, `candidates[]`, `timingsMs`, `versions`, `error` |
| `POST /v1/selftest` | `{"deezerTrackId": null}` | `ok`, `status`, `download`, `verify`, `timingsMs` — кнопка «проверить скачивание» |

`preferVideoIds` — видео, выбранные вручную в «Спорных» (пробуются первыми),
`excludeVideoIds` — отклонённые, `directUrl` — ссылка, вставленная в «Спорных» (SoundCloud,
Bandcamp, любой сайт yt-dlp): качается она, как выбор человека. С `upload` файл уходит
`PUT`-ом по подписанной ссылке (Content-Type — `audio/mp4; codecs="Opus"` и т. п.),
локальная копия удаляется. Без `upload` файл остаётся в `FETCHER_DATA_DIR/jobs/…`,
путь — в `file.path`.

## Домашний загрузчик

YouTube не отдаёт аудио адресу сервера (docs/decisions.md), поэтому качает домашний Mac:
загрузчик забирает задания с сервера, скачивает со своего адреса, кладёт файл по подписанной
ссылке в R2 и отчитывается. Серверную сторону делает API (`/api/fetcher/*`).

Основной путь — загрузчик внутри приложения для macOS: один файл `dalywave-fetcher` со своими
Deno, ffmpeg, fpcalc и сервисом PO-токенов, без Docker (`sidecar/build.sh` → `dist/sidecar/`);
приложение запускает его как sidecar и говорит с ним строками JSON. Договор — [SIDECAR.md](SIDECAR.md),
проверка своего сервиса PO-токенов — `dalywave-fetcher pot-check`. Ниже — запуск без приложения
(launchd).

Настройка на Mac:

1. Админка → «Источники» → новый загрузчик (или `POST /api/admin/downloaders {"name": "…"}`).
   Токен `dwk_…` показывается один раз — сохранить его вне репозитория:
   `mkdir -p ~/.config/dalywave && pbpaste > ~/.config/dalywave/downloader-token && chmod 600 ~/.config/dalywave/downloader-token`.
2. Нужны `uv`, `ffmpeg`, `chromaprint` (fpcalc), `deno` (`brew install …`) и Docker Desktop —
   в нём сервис PO-токенов `dalywave-pot` на `127.0.0.1:4416` (поднимается сам).
3. Разово: `make downloader` (из корня; `SERVER=…`, `TOKEN=…` по желанию). В автозапуск при
   входе: `services/fetcher/launchd/install.sh` (`--uninstall` — убрать). Лог —
   `~/Library/Logs/DalyWave/downloader.log`.

`launchd/run.sh` читает токен из файла (или `FETCHER_WORKER_TOKEN`), поднимает контейнер
PO-токенов и запускает `uv run fetcher worker --server …`. Все запросы — с
`Authorization: Bearer <токен загрузчика>`; токен открывает только эти три адреса.

**Взять задание** — `POST /api/fetcher/jobs/claim`:

```json
{
  "workerId": "dalynkaa-mac",
  "version": "0.2.0",
  "capabilities": {"kinds": ["acquire"], "potProvider": true, "cookies": false, "playerClients": [], "ytDlp": "2026.10.26"}
}
```

Заданий нет — сервер держит запрос до ~25 секунд и отвечает `204` с `Retry-After` (через
сколько секунд спросить снова). `401/403` — токен не принят (загрузчик останавливается),
`200`:

```json
{
  "job": {
    "id": "0193f0c2-7a1e-7c3a-9b1d-2f4e5a6b7c8d",
    "kind": "acquire",
    "deezerTrackId": 3040222091,
    "maxAttempts": 3,
    "preferVideoIds": [],
    "excludeVideoIds": [],
    "directUrl": null,
    "upload": {
      "url": "https://<account>.r2.cloudflarestorage.com/dalywave/audio/<recording>/<file>.m4a?X-Amz-…",
      "method": "PUT",
      "headers": {},
      "expiresAt": "2026-11-11T12:00:00Z"
    },
    "leaseExpiresAt": "2026-11-11T11:35:00Z"
  }
}
```

Content-Type в подпись ссылки не включается. Ссылка живёт 30 минут — дольше аренды.

**Продлить аренду** — `POST /api/fetcher/jobs/{id}/lease` раз в минуту, пока идёт скачивание
(в фоне, сам `fetcher worker`): `200 {"leaseExpiresAt": …}`, `409` — задание уже не наше.
Аренда — 5 минут; не продлили и не отчитались — сервер возвращает задание в очередь.

**Отчитаться** — `POST /api/fetcher/jobs/{id}/result`, ответ `204`:

```json
{
  "workerId": "dalynkaa-mac",
  "result": {
    "status": "verified",
    "deezerTrackId": 3040222091,
    "track": {"deezerId": 3040222091, "title": "ящики", "artist": "Mzlff", "durationMs": 199000, "isrc": "RUAGT2454320"},
    "chosen": {
      "candidate": {"videoId": "afkL3uLNeXA", "title": "ящики", "source": "album", "kind": "atv", "score": 1.0, "breakdown": {}},
      "download": {"itag": "251", "codec": "opus", "container": "mp4", "abrKbps": 136.1, "client": "MWEB", "poToken": true},
      "verify": {"verified": true, "similarity": 0.9761, "offsetS": 157.03, "threshold": 0.8}
    },
    "file": {"sha256": "40b9…", "sizeBytes": 3353582, "container": "mp4", "mimeType": "audio/mp4; codecs=\"Opus\"",
             "codec": "opus", "durationMs": 199700, "bitrateKbps": 136, "sampleRate": 48000, "channels": 2,
             "loudnessLufs": -8.4, "truePeakDb": 0.9, "waveform": [312, 640, "… 512 точек 0..1000"], "analysisError": null},
    "upload": {"ok": true, "status": 200, "etag": "\"…\""},
    "attempts": [],
    "candidates": [],
    "error": null,
    "timingsMs": {"deezer": 197, "match": 1340, "preview": 599, "download": 2983, "verify": 334, "analyze": 790, "upload": 410, "total": 6656},
    "versions": {"fetcher": "0.2.0", "ytDlp": "2026.10.26"}
  }
}
```

`result` — тот же объект, что отдаёт `POST /v1/acquire`. Сервер сам проверяет файл в
хранилище (есть ли, совпадает ли размер) и решает: принять, отдать в «Спорные», отметить
«нет в источниках» или повторить позже. `409` — задание уже не наше (аренда кончилась,
его отменили): повторять отчёт бессмысленно. При `error.code` из `bot_check`,
`rate_limited`, `http_403`, `po_token`, `network`, `cookies_missing` загрузчик делает
паузу 15 минут, а сервер откладывает задание, не тратя попытку.
