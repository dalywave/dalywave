# Встановлення DalyWave на власний сервер

DalyWave працює як кілька контейнерів із готових образів (linux/amd64 і linux/arm64):

| Контейнер | Образ | Що робить |
|---|---|---|
| `api` | `ghcr.io/dalywave/server` | API, вебінтерфейс, посилання на файли для плеєрів |
| `worker` | `ghcr.io/dalywave/server` | фонові завдання: сканування папки з музикою, обкладинки, гучність, черга аналізу |
| `ml` | `ghcr.io/dalywave/ml` | аналіз звуку для хвилі, радіо, переходів «Діджей» і пошуку за звучанням |
| `db` | `pgvector/pgvector:pg18` | PostgreSQL із pgvector |
| `backup` | `postgres:18-alpine` | щоденний дамп бази даних, зберігаються останні 14 |

Каталог Deezer і завантажувач із YouTube за замовчуванням вимкнені (див. [Необов’язкові модулі](#необовязкові-модулі)).

## Вимоги

- Linux із Docker Engine і Docker Compose v2. Підійде й Raspberry Pi 5 або хмарна віртуальна машина на ARM.
- Пам’ять: близько 1 ГБ для сервера й бази даних, плюс 1,5–3 ГБ для `ml`. Якщо загалом менше 4 ГБ,
  запускайте без `ml` (`ML_REPLICAS=0`).
- Диск: кілька ГБ для образів, плюс база даних і файли сервера: обкладинки, кліпи й копії треків для
  повільних мереж і для форматів, які браузери не відтворюють (не більше `DALYWAVE_RENDITIONS_MAX_GB`,
  за замовчуванням 20 ГБ). Вашу музику сервер ніколи не копіює.
- Папка з музикою, файли якої може читати uid 10001 (достатньо, щоб їх могли читати всі).
- Доступ до інтернету під час першого запуску: образи завантажуються з ghcr.io, а `ml` отримує свої
  моделі MTG (23 МБ) з essentia.upf.edu — див. [Моделі аналізу звуку](#моделі-аналізу-звуку).
- Застосункам для iPhone, iPad, Apple TV і комп’ютера потрібен HTTPS: звичайний HTTP вони приймають
  лише для `localhost`. Див. [Зворотний проксі та HTTPS](#зворотний-проксі-та-https).

## Швидкий старт

1. Завантажте файл compose і приклад налаштувань:

   ```sh
   mkdir dalywave && cd dalywave
   curl -fsSLO https://raw.githubusercontent.com/dalywave/dalywave/main/infra/selfhost/docker-compose.yml
   curl -fsSL -o .env https://raw.githubusercontent.com/dalywave/dalywave/main/infra/selfhost/.env.example
   chmod 600 .env
   ```

2. Згенеруйте секрети в `.env` (на macOS пишіть `sed -i ''`):

   ```sh
   sed -i "s/^DALYWAVE_SECRET_KEY=.*/DALYWAVE_SECRET_KEY=$(openssl rand -hex 32)/" .env
   sed -i "s/^POSTGRES_PASSWORD=.*/POSTGRES_PASSWORD=$(openssl rand -hex 24)/" .env
   ```

3. Відредагуйте `.env`:
   - `DALYWAVE_PUBLIC_URL` — точна адреса, яку відкриватимуть люди, наприклад `http://192.168.1.20:8400`
     або `https://music.example.com`. Якщо вона відрізняється від тієї, що показує браузер, вхід не працюватиме.
   - `MUSIC_DIR` — ваша папка з музикою на цій машині, наприклад `/srv/music`.

4. Запустіть і перевірте сервер:

   ```sh
   docker compose up -d
   curl -fsS http://localhost:8400/api/healthz   # {"status":"ok",…}
   ```

   Перший запуск триває довше: крім завантаження образів, `ml` отримує свої моделі (від хвилини до
   кількох — essentia.upf.edu часом повільний) і доти має стан unhealthy. Музика вже грає; аналіз
   наздожене, щойно `ml` буде готовий (`docker compose logs -f ml`).

5. Одразу відкрийте `DALYWAVE_PUBLIC_URL` і створіть адміністратора на екрані «Ласкаво просимо». Доти
   цей екран відкритий для будь-кого, хто дістанеться до сервера.

6. `worker` сканує папку з музикою під час запуску, кожні 6 годин і в разі змін. Перебіг, пропущені
   файли й кнопка «Пересканувати» — в **Адмінка → Бібліотека**. Інших людей запрошуйте в **Адмінка →
   Запрошення та вхід**.

## Зворотний проксі та HTTPS

Файл compose публікує один порт зі звичайним HTTP і не має TLS. Поставте перед ним будь-який зворотний проксі:

- у `.env`: `HTTP_PORT=127.0.0.1:8400` (доступ матиме лише проксі) і
  `DALYWAVE_PUBLIC_URL=https://music.example.com`, потім `docker compose up -d`;
- проксі має пропускати перехід на WebSocket (`/api/ws`), дозволяти тіла запитів до 1 ГБ (завантаження
  файлів на сервер), не буферизувати відповіді (аудіо передається потоком із запитами Range) і
  задавати `X-Forwarded-For` з адресою клієнта (сервер використовує її для обмеження частоти запитів).

Caddy сам отримує сертифікат і робить усе перелічене:

```
music.example.com {
    reverse_proxy 127.0.0.1:8400
}
```

nginx:

```nginx
map $http_upgrade $connection_upgrade { default upgrade; '' close; }

server {
    listen 443 ssl;
    server_name music.example.com;
    # ssl_certificate …

    location / {
        proxy_pass http://127.0.0.1:8400;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_buffering off;
        proxy_read_timeout 1h;
        client_max_body_size 1g;
    }
}
```

Проксі, що працює в Docker, може під’єднатися до мережі `dalywave_default` і вказувати на `http://api:8400`.

## Вхід

Локальні облікові записи й екран першого запуску ввімкнено за замовчуванням (`DALYWAVE_LOCAL_AUTH=true`).
Будь-який провайдер OpenID Connect — Authentik, Keycloak, Pocket ID, Google та інші — додається
змінними `DALYWAVE_OIDC_<ID>_*` у `.env`; зареєструйте в провайдера адресу повернення
`<DALYWAVE_PUBLIC_URL>/api/auth/oidc/<id>/callback`. Ролі, запрошення й приклади для кожного
провайдера: [auth.uk.md](auth.uk.md).

## Необов’язкові модулі

Усі вони задаються в `.env`, після чого — `docker compose up -d`:

- **Каталог Deezer**: `DALYWAVE_DEEZER=true` — пошук у всьому каталозі, чарти, нові релізи, схожі
  виконавці; ваші файли зіставляються з ним.
- **Завантажувач**: завантаження з YouTube може порушувати його умови використання — вмикайте його лише
  для себе й усвідомлюючи це. `COMPOSE_PROFILES=downloader`, `DALYWAVE_DOWNLOADER=true` і
  `DALYWAVE_FETCHER_MODE=remote,server` запускають контейнери `fetcher` і `pot`.
- **Без аналізу звуку**: `ML_REPLICAS=0`. Музика грає, але хвиля перемішує вашу медіатеку, переходи
  «Діджей» і пошук за звучанням недоступні, а **Адмінка → Здоров’я** показує, що `ml` не працює.

### Моделі аналізу звуку

`ml` спирається на Discogs-EffNet і його класифікатори від Music Technology Group (Universitat
Pompeu Fabra), що поширюються за ліцензією **CC BY-NC-SA 4.0 — лише для некомерційного використання**.
Вони не входять до образу: під час першого запуску контейнер завантажує їх без змін з essentia.upf.edu
(20 файлів, 23 МБ) у том `ml-models`, перевіряє їхні sha256 і записує поруч `ATTRIBUTION.md`; наступні
запуски використовують том. Якщо завантаження не вдалося, контейнер завершує роботу, а Docker
перезапускає його, щоб спробувати знову. Для комерційного використання потрібна ліцензія від MTG — або
запускайте без `ml` (`ML_REPLICAS=0`). Інші моделі й програми в образі та їхні ліцензії:
`/usr/share/doc/dalywave-ml/`.

Як читається папка з музикою (теги, обкладинки, формати, перейменування): [music.uk.md](music.uk.md).
Зберігання файлів сервера в S3 (Cloudflare R2, MinIO) замість тому `data`: [storage.uk.md](storage.uk.md).

## Оновлення

```sh
docker compose pull
docker compose up -d
```

Під час запуску сервер виконує міграції бази даних; міграції йдуть лише вперед, тож спершу зробіть
резервну копію, а потім не повертайтеся до старішого образу. `IMAGE_TAG=latest` завжди вказує на
найновіший випуск; закріпіть версію, наприклад `IMAGE_TAG=0.1.19`, щоб оновлюватися лише тоді, коли ви
її зміните. Перед оновленням читайте примітки до випуску й заново завантажуйте `docker-compose.yml`,
коли там про це сказано.

## Резервні копії

**База даних.** Контейнер `backup` щодня записує `dalywave-YYYY-MM-DD.sql.gz` у том `backups` і
зберігає останні 14; **Адмінка → Резервні копії** показує їх і перевіряє. Регулярно копіюйте їх за
межі машини:

```sh
docker compose cp backup:/backup ./backups
```

**Файли сервера** (обкладинки, копії, кліпи, імпорт) лежать у томі `data`:

```sh
docker run --rm -v dalywave_data:/data:ro -v "$PWD":/out alpine tar czf /out/dalywave-data.tgz -C /data .
```

Резервні копії папки з музикою та `.env` робіть самі, як зазвичай; без того самого `DALYWAVE_SECRET_KEY`
усім доведеться увійти знову.

**Відновлення** на новому встановленні (з тим самим `.env`):

```sh
docker compose up -d db
docker compose exec -T db dropdb -U dalywave dalywave
docker compose exec -T db createdb -U dalywave dalywave
gunzip -c dalywave-2026-11-23.sql.gz | docker compose exec -T db psql -q -U dalywave -d dalywave
docker run --rm -v dalywave_data:/data -v "$PWD":/in alpine tar xzf /in/dalywave-data.tgz -C /data
docker compose up -d
```

## Розв’язання проблем

- `docker compose ps` — усі контейнери мають бути `running` (`api` — `healthy`).
- `docker compose logs -f api worker` — задайте `DALYWAVE_LOG_FORMAT=pretty`, щоб журнали було легше читати.
- Вхід не вдається одразу після введення пароля: `DALYWAVE_PUBLIC_URL` не збігається з адресою в
  браузері (схема, хост або порт).
- Медіатека порожня: перевірте `MUSIC_DIR` і права доступу до файлів; пропущені файли та причину видно
  в **Адмінка → Бібліотека**.
