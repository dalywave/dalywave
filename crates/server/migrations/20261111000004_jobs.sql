-- Очередь задач на Postgres: полосы, задачи и история поиска аудио.

-- Полосы очереди: у каждой своя параллельность и пауза между задачами (вежливость к YouTube)
CREATE TABLE job_lanes (
    lane            text PRIMARY KEY,
    title           text        NOT NULL,
    paused          boolean     NOT NULL DEFAULT false,
    concurrency     smallint    NOT NULL DEFAULT 1 CHECK (concurrency BETWEEN 1 AND 64),
    min_interval_ms integer     NOT NULL DEFAULT 0 CHECK (min_interval_ms >= 0),
    -- Когда задача полосы последний раз взята в работу: от него отсчитывается min_interval_ms
    -- для всех исполнителей сразу, а не для каждого процесса отдельно
    last_claimed_at timestamptz,
    updated_at      timestamptz NOT NULL DEFAULT now()
);
INSERT INTO job_lanes (lane, title, concurrency, min_interval_ms) VALUES
    ('now',         'Сейчас',              2, 0),
    ('wave',        'Волна',               2, 2000),
    ('library',     'Медиатека',           1, 5000),
    ('discography', 'Дискографии',         1, 8000),
    ('discovery',   'Открытия',            1, 8000),
    ('analysis',    'Анализ звука',        2, 0),
    ('scout',       'Разведка каталога',   2, 300),
    ('mirror',      'Обложки',             4, 0),
    ('maintenance', 'Обслуживание',        1, 0);

CREATE TABLE jobs (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    kind         text        NOT NULL,             -- acquire, mirror_image, refresh_artist, sync_discography, …
    lane         text        NOT NULL REFERENCES job_lanes (lane),
    priority     smallint    NOT NULL DEFAULT 0,   -- больше — раньше
    payload      jsonb       NOT NULL DEFAULT '{}',
    status       text        NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'done', 'failed', 'cancelled')),
    attempts     smallint    NOT NULL DEFAULT 0,
    max_attempts smallint    NOT NULL DEFAULT 3,
    run_after    timestamptz NOT NULL DEFAULT now(),
    locked_by    text,
    locked_at    timestamptz,
    heartbeat_at timestamptz,
    last_error   text,
    result       jsonb,
    dedupe_key   text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    finished_at  timestamptz
);
CREATE INDEX jobs_dequeue ON jobs (lane, priority DESC, run_after, id) WHERE status = 'queued';
CREATE INDEX jobs_running ON jobs (heartbeat_at) WHERE status = 'running';
CREATE INDEX jobs_recent ON jobs (created_at DESC);
CREATE INDEX jobs_lane_status ON jobs (lane, status);
CREATE UNIQUE INDEX jobs_dedupe ON jobs (dedupe_key) WHERE dedupe_key IS NOT NULL AND status IN ('queued', 'running');

-- Поиск и скачивание одной записи; кандидаты с разбором оценки нужны разделу «Спорные»
CREATE TABLE acquisitions (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    recording_id uuid        NOT NULL REFERENCES recordings (id) ON DELETE CASCADE,
    track_id     uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    status       text        NOT NULL DEFAULT 'queued'
                 CHECK (status IN ('queued', 'matching', 'downloading', 'verifying', 'processing', 'ready', 'disputed', 'unavailable', 'failed', 'cancelled')),
    reason       text        NOT NULL CHECK (reason IN ('play', 'wave', 'like', 'playlist', 'album', 'subscription', 'discovery', 'import', 'manual', 'upgrade')),
    lane         text        NOT NULL REFERENCES job_lanes (lane),
    requested_by uuid        REFERENCES users (id) ON DELETE SET NULL,
    job_id       uuid        REFERENCES jobs (id) ON DELETE SET NULL,
    candidates   jsonb,
    chosen       jsonb,
    error        text,
    worker       text,
    attempts     smallint    NOT NULL DEFAULT 0,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    finished_at  timestamptz
);
CREATE UNIQUE INDEX acquisitions_active ON acquisitions (recording_id)
    WHERE status IN ('queued', 'matching', 'downloading', 'verifying', 'processing');
CREATE INDEX acquisitions_status ON acquisitions (status, updated_at DESC);
CREATE INDEX acquisitions_job ON acquisitions (job_id);
