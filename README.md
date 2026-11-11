# DalyWave

Личный музыкальный сервис: каталог Deezer, своя медиатека на собственном сервере, волна с настроениями, переходы как у диджея, приложения для сайта, macOS, Windows, iPhone и iPad в одном стиле.

Адрес: https://wave.daly.rest · вход через daly.rest · план — [docs/plan.md](docs/plan.md)

## Устройство

```
apps/ui            интерфейс на Nuxt — сайт, админка и все приложения
crates/protocol    типы API — общие для сервера и приложений
crates/server      сервер на Rust: API, вход, каталог, задачи
services/fetcher   поиск и скачивание аудио (Python, yt-dlp)
design             макеты и данные для них
infra              docker compose, Dockerfile, выкладка
tools              скриншоты для проверки интерфейса, статика
docs               план, соглашения, решения, модель данных
```

## Локально

Нужны Docker, Rust (версия из `rust-toolchain.toml`), Bun, uv.

```sh
make up        # Postgres 18 + pgvector и S3 в Docker
make api       # API на http://localhost:8400
make ui        # интерфейс на http://localhost:3400 (проксирует /api)
make design    # макеты на http://localhost:4400
make test      # все тесты
make lint      # проверки стиля
```

Настройки — переменные `DALYWAVE_*`, список с пояснениями в `.env.example`. Локальные значения — в `.env` (не коммитится).

## Выкладка

`make deploy` передаёт текущий коммит на сервер по SSH и пересобирает образы там же (ARM64). Секреты сервера — в `~/dalywave/.env`, права 600.

## Правила

[docs/conventions.md](docs/conventions.md) — обязательно к прочтению: порты, переменные, стиль кода и API, проверка интерфейса скриншотами.
