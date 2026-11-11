-- Этап 1b: скачивание аудио. Домашние загрузчики, аренда задач внешними исполнителями, попытки скачивания.

-- Домашний загрузчик (`fetcher worker` на Mac владельца). Токен `dwk_…` даёт одно право — `downloader`:
-- брать задания и отчитываться. В базе только SHA-256 токена
CREATE TABLE downloaders (
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    name          text        NOT NULL,
    token_hash    bytea       NOT NULL UNIQUE,
    created_by    uuid        REFERENCES users (id) ON DELETE SET NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    revoked_at    timestamptz,
    -- Последнее обращение (задание, аренда, отчёт): по нему «загрузчик на связи»
    last_seen_at  timestamptz,
    last_claim_at timestamptz,
    worker_id     text,                         -- имя машины из запроса
    version       text,                         -- версия fetcher
    yt_dlp        text,                         -- версия yt-dlp, которую загрузчик сообщает
    capabilities  jsonb       NOT NULL DEFAULT '{}',
    last_ip       inet
);

-- Аренда у внешнего исполнителя: не отчитался к сроку — задача снова в очереди. Задачи воркера
-- аренды не имеют, их держит пульс (heartbeat_at)
ALTER TABLE jobs ADD COLUMN lease_expires_at timestamptz;

-- Ключ файла текущей попытки: выдаётся вместе с подписанной ссылкой при взятии задачи
ALTER TABLE acquisitions ADD COLUMN upload_key text;

-- SHA-256 файла со слов исполнителя (размер сервер сверяет сам): целостность и поиск дублей
ALTER TABLE audio_files ADD COLUMN sha256 text;

-- Каждая попытка и решение по ней: «Спорные», успех и сбои загрузчиков за сутки, доля успеха по дням
CREATE TABLE acquisition_attempts (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    acquisition_id uuid        NOT NULL REFERENCES acquisitions (id) ON DELETE CASCADE,
    job_id         uuid        REFERENCES jobs (id) ON DELETE SET NULL,
    executor       text        NOT NULL,         -- server | device:<id загрузчика>
    status         text        NOT NULL,         -- ответ fetcher: verified, unverified, disputed, not_found, error
    outcome        text        NOT NULL CHECK (outcome IN ('ready', 'disputed', 'unavailable', 'retry', 'failed', 'postponed')),
    error_code     text,
    error          text,
    video_id       text,
    yt_dlp         text,
    total_ms       integer,
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX acquisition_attempts_acquisition ON acquisition_attempts (acquisition_id, created_at DESC);
CREATE INDEX acquisition_attempts_executor ON acquisition_attempts (executor, created_at DESC);

-- Задачи acquire теперь выполняются: дискографии подписок качаются, полоса больше не на паузе
UPDATE job_lanes SET paused = false, updated_at = now() WHERE lane = 'discography';
