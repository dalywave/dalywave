-- Модель данных DalyWave после этапа 0 (пользователи, сессии, устройства — в миграциях этапа 0).
-- Это эталон для миграций этапов 1–6: таблицы переносятся в crates/server/migrations по мере реализации.
-- Пояснения — в docs/data-model.md.

CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS unaccent;

-- ───────────────────────────── Синхронизация с приложениями ─────────────────────────────
-- Каждая синхронизируемая строка получает номер изменения из одной последовательности.
-- Приложение просит «всё после номера N», удаления приходят из tombstones.

CREATE SEQUENCE IF NOT EXISTS sync_rev;

CREATE OR REPLACE FUNCTION bump_rev() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.rev := nextval('sync_rev');
    RETURN NEW;
END $$;

CREATE TABLE tombstones (
    rev        bigint PRIMARY KEY DEFAULT nextval('sync_rev'),
    table_name text        NOT NULL,
    row_key    jsonb       NOT NULL,
    user_id    uuid,
    deleted_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX tombstones_user_rev ON tombstones (user_id, rev);

-- Аргументы триггера — имена ключевых колонок; user_id сохраняется, если колонка есть
CREATE OR REPLACE FUNCTION record_tombstone() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    old_row jsonb := to_jsonb(OLD);
    key     jsonb := '{}'::jsonb;
    col     text;
BEGIN
    FOREACH col IN ARRAY TG_ARGV LOOP
        key := key || jsonb_build_object(col, old_row -> col);
    END LOOP;
    INSERT INTO tombstones (table_name, row_key, user_id)
    VALUES (TG_TABLE_NAME, key, (old_row ->> 'user_id')::uuid);
    RETURN OLD;
END $$;

-- ───────────────────────────── Каталог (Deezer, кешируется при первом обращении) ─────────────────────────────

CREATE TABLE artists (
    id                     uuid PRIMARY KEY DEFAULT uuidv7(),
    deezer_id              bigint UNIQUE,
    name                   text        NOT NULL,
    picture_key            text,                 -- копия в S3; NULL, пока не скопирована
    picture_source         text,                 -- адрес у Deezer
    fans                   integer,
    albums_count           integer,
    fetched_at             timestamptz,          -- последнее полное обновление карточки
    discography_synced_at  timestamptz,          -- последняя сверка релизов (подписки)
    created_at             timestamptz NOT NULL DEFAULT now(),
    updated_at             timestamptz NOT NULL DEFAULT now(),
    rev                    bigint      NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX artists_name_trgm ON artists USING gin (lower(name) gin_trgm_ops);
CREATE TRIGGER artists_rev BEFORE UPDATE ON artists FOR EACH ROW EXECUTE FUNCTION bump_rev();

CREATE TABLE albums (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    deezer_id      bigint UNIQUE,
    upc            text,
    title          text        NOT NULL,
    artist_id      uuid        NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    record_type    text        NOT NULL DEFAULT 'album' CHECK (record_type IN ('album', 'ep', 'single', 'compile')),
    release_date   date,
    label          text,
    genres         text[]      NOT NULL DEFAULT '{}',
    explicit       boolean     NOT NULL DEFAULT false,
    tracks_count   integer,
    duration_ms    integer,
    fans           integer,
    cover_key      text,
    cover_source   text,
    cover_palette  jsonb,                        -- основные цвета обложки для фона интерфейса
    fetched_at     timestamptz,                  -- когда загружен список треков
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz NOT NULL DEFAULT now(),
    rev            bigint      NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX albums_artist_release ON albums (artist_id, release_date DESC);
CREATE INDEX albums_title_trgm ON albums USING gin (lower(title) gin_trgm_ops);
CREATE TRIGGER albums_rev BEFORE UPDATE ON albums FOR EACH ROW EXECUTE FUNCTION bump_rev();

-- Запись (фонограмма). Треки с одним ISRC — сингл и он же в альбоме — делят одну запись и один файл.
CREATE TABLE recordings (
    id                uuid PRIMARY KEY DEFAULT uuidv7(),
    isrc              text UNIQUE,
    duration_ms       integer     NOT NULL,
    audio_status      text        NOT NULL DEFAULT 'none'
                      CHECK (audio_status IN ('none', 'queued', 'acquiring', 'ready', 'disputed', 'unavailable', 'failed')),
    primary_file_id   uuid,                      -- внешний ключ добавлен ниже, после audio_files
    status_changed_at timestamptz NOT NULL DEFAULT now(),
    created_at        timestamptz NOT NULL DEFAULT now(),
    rev               bigint      NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX recordings_status ON recordings (audio_status) WHERE audio_status <> 'none';
CREATE TRIGGER recordings_rev BEFORE UPDATE ON recordings FOR EACH ROW EXECUTE FUNCTION bump_rev();

CREATE TABLE tracks (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    deezer_id       bigint UNIQUE,
    isrc            text,
    recording_id    uuid        REFERENCES recordings (id) ON DELETE SET NULL,
    title           text        NOT NULL,
    title_version   text        NOT NULL DEFAULT '',   -- «Slowed», «Remastered 2011» — часть названия у Deezer
    album_id        uuid        REFERENCES albums (id) ON DELETE SET NULL,
    disc_number     smallint    NOT NULL DEFAULT 1,
    track_position  smallint,
    duration_ms     integer     NOT NULL,
    explicit        boolean     NOT NULL DEFAULT false,
    rank            integer,                           -- популярность у Deezer
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    rev             bigint      NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX tracks_album ON tracks (album_id, disc_number, track_position);
CREATE INDEX tracks_isrc ON tracks (isrc);
CREATE INDEX tracks_recording ON tracks (recording_id);
CREATE INDEX tracks_title_trgm ON tracks USING gin (lower(title) gin_trgm_ops);
CREATE TRIGGER tracks_rev BEFORE UPDATE ON tracks FOR EACH ROW EXECUTE FUNCTION bump_rev();

CREATE TABLE track_artists (
    track_id  uuid     NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    artist_id uuid     NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    role      text     NOT NULL DEFAULT 'main' CHECK (role IN ('main', 'featured')),
    position  smallint NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, artist_id)
);
CREATE INDEX track_artists_artist ON track_artists (artist_id);

-- ───────────────────────────── Аудио ─────────────────────────────

CREATE TABLE audio_files (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    recording_id       uuid        NOT NULL REFERENCES recordings (id) ON DELETE CASCADE,
    storage_key        text        NOT NULL UNIQUE,
    codec              text        NOT NULL CHECK (codec IN ('opus', 'vorbis', 'aac', 'mp3', 'flac', 'alac', 'pcm')),
    container          text        NOT NULL CHECK (container IN ('ogg', 'webm', 'mp4', 'mp3', 'flac', 'wav')),
    lossless           boolean     NOT NULL DEFAULT false,
    bitrate_kbps       integer,
    sample_rate        integer,
    channels           smallint,
    bit_depth          smallint,
    duration_ms        integer     NOT NULL,
    size_bytes         bigint      NOT NULL,
    source             text        NOT NULL CHECK (source IN ('youtube', 'youtube_music', 'soundcloud', 'bandcamp', 'upload', 'other')),
    source_ref         text,                         -- id видео или ссылка
    source_title       text,                         -- как запись называлась в источнике
    loudness_lufs      real,                         -- интегральная громкость (EBU R128)
    true_peak_db       real,
    waveform           smallint[],                   -- огибающая для полосы прокрутки, 0..1000
    match_score        real,
    verified           boolean     NOT NULL DEFAULT false,
    verify_similarity  real,
    verify_offset_ms   integer,
    acquired_by        text,                         -- server | device:<id> (загрузчик на Mac)
    created_at         timestamptz NOT NULL DEFAULT now(),
    rev                bigint      NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX audio_files_recording ON audio_files (recording_id);
CREATE TRIGGER audio_files_rev BEFORE UPDATE ON audio_files FOR EACH ROW EXECUTE FUNCTION bump_rev();

ALTER TABLE recordings
    ADD CONSTRAINT recordings_primary_file FOREIGN KEY (primary_file_id) REFERENCES audio_files (id) ON DELETE SET NULL;

-- ───────────────────────────── Задачи ─────────────────────────────

-- Полосы очереди: у каждой своя параллельность и пауза между задачами (вежливость к YouTube)
CREATE TABLE job_lanes (
    lane            text PRIMARY KEY,
    title           text        NOT NULL,
    paused          boolean     NOT NULL DEFAULT false,
    concurrency     smallint    NOT NULL DEFAULT 1,
    min_interval_ms integer     NOT NULL DEFAULT 0,
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
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    kind          text        NOT NULL,             -- acquire, analyze, mirror_cover, sync_discography, scout, ...
    lane          text        NOT NULL REFERENCES job_lanes (lane),
    priority      smallint    NOT NULL DEFAULT 0,   -- больше — раньше
    payload       jsonb       NOT NULL DEFAULT '{}',
    status        text        NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'done', 'failed', 'cancelled')),
    attempts      smallint    NOT NULL DEFAULT 0,
    max_attempts  smallint    NOT NULL DEFAULT 3,
    run_after     timestamptz NOT NULL DEFAULT now(),
    locked_by     text,
    locked_at     timestamptz,
    heartbeat_at  timestamptz,
    last_error    text,
    result        jsonb,
    dedupe_key    text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    finished_at   timestamptz
);
CREATE INDEX jobs_dequeue ON jobs (lane, priority DESC, run_after, id) WHERE status = 'queued';
CREATE INDEX jobs_running ON jobs (heartbeat_at) WHERE status = 'running';
CREATE INDEX jobs_recent ON jobs (created_at DESC);
CREATE UNIQUE INDEX jobs_dedupe ON jobs (dedupe_key) WHERE dedupe_key IS NOT NULL AND status IN ('queued', 'running');

-- Поиск и скачивание одной записи; кандидаты с разбором оценки нужны разделу «Спорные»
CREATE TABLE acquisitions (
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    recording_id  uuid        NOT NULL REFERENCES recordings (id) ON DELETE CASCADE,
    track_id      uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    status        text        NOT NULL DEFAULT 'queued'
                  CHECK (status IN ('queued', 'matching', 'downloading', 'verifying', 'processing', 'ready', 'disputed', 'unavailable', 'failed', 'cancelled')),
    reason        text        NOT NULL CHECK (reason IN ('play', 'wave', 'like', 'playlist', 'album', 'subscription', 'discovery', 'import', 'manual', 'upgrade')),
    lane          text        NOT NULL REFERENCES job_lanes (lane),
    requested_by  uuid        REFERENCES users (id) ON DELETE SET NULL,
    job_id        uuid        REFERENCES jobs (id) ON DELETE SET NULL,
    candidates    jsonb,
    chosen        jsonb,
    error         text,
    worker        text,
    attempts      smallint    NOT NULL DEFAULT 0,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    finished_at   timestamptz
);
CREATE UNIQUE INDEX acquisitions_active ON acquisitions (recording_id)
    WHERE status IN ('queued', 'matching', 'downloading', 'verifying', 'processing');
CREATE INDEX acquisitions_status ON acquisitions (status, updated_at DESC);

-- ───────────────────────────── Медиатека ─────────────────────────────

CREATE TABLE liked_tracks (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    track_id   uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    PRIMARY KEY (user_id, track_id)
);
CREATE INDEX liked_tracks_recent ON liked_tracks (user_id, created_at DESC);
CREATE TRIGGER liked_tracks_gone AFTER DELETE ON liked_tracks FOR EACH ROW EXECUTE FUNCTION record_tombstone('user_id', 'track_id');

CREATE TABLE saved_albums (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    album_id   uuid        NOT NULL REFERENCES albums (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    PRIMARY KEY (user_id, album_id)
);
CREATE TRIGGER saved_albums_gone AFTER DELETE ON saved_albums FOR EACH ROW EXECUTE FUNCTION record_tombstone('user_id', 'album_id');

-- Подписка на артиста: что докачивать на сервер; на устройства — через device_downloads
CREATE TABLE followed_artists (
    user_id         uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    artist_id       uuid        NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    download_policy jsonb       NOT NULL DEFAULT '{"albums": true, "eps": true, "singles": true, "compilations": false, "live": false, "appearsOn": false}',
    notify          boolean     NOT NULL DEFAULT true,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    rev             bigint      NOT NULL DEFAULT nextval('sync_rev'),
    PRIMARY KEY (user_id, artist_id)
);
CREATE TRIGGER followed_artists_rev BEFORE UPDATE ON followed_artists FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER followed_artists_gone AFTER DELETE ON followed_artists FOR EACH ROW EXECUTE FUNCTION record_tombstone('user_id', 'artist_id');

-- «Не рекомендовать»: трек или артист
CREATE TABLE blocked_items (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind       text        NOT NULL CHECK (kind IN ('track', 'artist')),
    ref_id     uuid        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, kind, ref_id)
);

CREATE TABLE playlists (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id     uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title       text        NOT NULL,
    description text        NOT NULL DEFAULT '',
    kind        text        NOT NULL DEFAULT 'user' CHECK (kind IN ('user', 'smart', 'mix', 'system')),
    rules       jsonb,                              -- для умных плейлистов
    cover_key   text,
    pinned      boolean     NOT NULL DEFAULT false,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    rev         bigint      NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX playlists_user ON playlists (user_id, updated_at DESC);
CREATE TRIGGER playlists_rev BEFORE UPDATE ON playlists FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER playlists_gone AFTER DELETE ON playlists FOR EACH ROW EXECUTE FUNCTION record_tombstone('id', 'user_id');

-- Позиция — дробная: перестановка меняет одну строку
CREATE TABLE playlist_items (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    playlist_id uuid             NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    track_id    uuid             NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    position    double precision NOT NULL,
    added_at    timestamptz      NOT NULL DEFAULT now(),
    rev         bigint           NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX playlist_items_order ON playlist_items (playlist_id, position);
CREATE TRIGGER playlist_items_rev BEFORE UPDATE ON playlist_items FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER playlist_items_gone AFTER DELETE ON playlist_items FOR EACH ROW EXECUTE FUNCTION record_tombstone('id', 'playlist_id');

-- Что держать скачанным на конкретном устройстве
CREATE TABLE device_downloads (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    device_id  uuid        NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    kind       text        NOT NULL CHECK (kind IN ('track', 'album', 'playlist', 'artist', 'liked')),
    ref_id     uuid,                                -- NULL только для liked
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    CHECK ((kind = 'liked') = (ref_id IS NULL))
);
CREATE UNIQUE INDEX device_downloads_unique ON device_downloads (device_id, kind, coalesce(ref_id, '00000000-0000-0000-0000-000000000000'));
CREATE TRIGGER device_downloads_gone AFTER DELETE ON device_downloads FOR EACH ROW EXECUTE FUNCTION record_tombstone('id', 'device_id');

-- ───────────────────────────── Активность ─────────────────────────────

-- Одно прослушивание; client_event_id делает повторную отправку из офлайна безопасной
CREATE TABLE play_events (
    id                uuid PRIMARY KEY DEFAULT uuidv7(),
    client_event_id   uuid        NOT NULL UNIQUE,
    user_id           uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id         uuid        REFERENCES devices (id) ON DELETE SET NULL,
    track_id          uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    started_at        timestamptz NOT NULL,
    played_ms         integer     NOT NULL,
    position_start_ms integer     NOT NULL DEFAULT 0,
    duration_ms       integer     NOT NULL,
    end_reason        text        NOT NULL CHECK (end_reason IN ('completed', 'skipped', 'previous', 'stopped', 'error', 'interrupted', 'transferred')),
    context_type      text        CHECK (context_type IN ('album', 'playlist', 'artist', 'wave', 'radio', 'mix', 'search', 'library', 'queue', 'remote')),
    context_id        text,
    wave_session_id   uuid,
    listening_session uuid,                         -- подряд идущие прослушивания с паузами меньше 30 минут
    source            text        NOT NULL DEFAULT 'stream' CHECK (source IN ('stream', 'offline')),
    transition        text,
    output            text,                         -- headphones, speaker, bluetooth, airplay
    created_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX play_events_user_time ON play_events (user_id, started_at DESC);
CREATE INDEX play_events_track ON play_events (track_id, started_at DESC);

CREATE TABLE feedback_events (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    client_event_id uuid        NOT NULL UNIQUE,
    user_id         uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    track_id        uuid        REFERENCES tracks (id) ON DELETE CASCADE,
    artist_id       uuid        REFERENCES artists (id) ON DELETE CASCADE,
    kind            text        NOT NULL CHECK (kind IN ('like', 'unlike', 'dislike', 'undislike', 'not_now', 'more_like_this', 'playlist_add', 'queue_add', 'replay')),
    context_type    text,
    context_id      text,
    wave_session_id uuid,
    at              timestamptz NOT NULL
);
CREATE INDEX feedback_events_user_time ON feedback_events (user_id, at DESC);

-- ───────────────────────────── Анализ звука и рекомендации (этап 2) ─────────────────────────────

-- По превью (30 с) для всего каталога вокруг вкуса, по полному файлу — для скачанного
CREATE TABLE track_analysis (
    recording_id    uuid PRIMARY KEY REFERENCES recordings (id) ON DELETE CASCADE,
    source          text        NOT NULL CHECK (source IN ('preview', 'full')),
    models          jsonb       NOT NULL,           -- версии моделей
    sound           halfvec(1280),                  -- Discogs-EffNet: звучание
    semantic        halfvec(512),                   -- CLAP: общее пространство звука и текста
    bpm             real,
    bpm_confidence  real,
    key_camelot     text,                           -- 8A, 11B …
    key_confidence  real,
    energy          real,
    danceability    real,
    valence         real,
    moods           jsonb,                          -- happy, sad, aggressive, relaxed, party
    voice           real,                           -- вероятность вокала
    language        text,
    styles          jsonb,                          -- верхние стили Discogs с вероятностями
    intro_end_ms    integer,
    outro_start_ms  integer,
    beats_ms        integer[],                      -- сетка долей (полный анализ)
    downbeats_ms    integer[],
    vocal_segments  jsonb,                          -- [[начало, конец], …] в мс
    energy_curve    smallint[],                     -- по секунде, 0..1000
    analyzed_at     timestamptz NOT NULL DEFAULT now(),
    rev             bigint      NOT NULL DEFAULT nextval('sync_rev')
);
CREATE INDEX track_analysis_sound ON track_analysis USING hnsw (sound halfvec_cosine_ops);
CREATE INDEX track_analysis_semantic ON track_analysis USING hnsw (semantic halfvec_cosine_ops);
CREATE TRIGGER track_analysis_rev BEFORE UPDATE ON track_analysis FOR EACH ROW EXECUTE FUNCTION bump_rev();

-- Грани вкуса: группы любимого со своими векторами и весами по контексту
CREATE TABLE taste_facets (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id     uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    label       text        NOT NULL,
    centroid    halfvec(1280) NOT NULL,
    weight      real        NOT NULL,
    context     jsonb       NOT NULL DEFAULT '{}',  -- веса по времени суток, устройству
    examples    uuid[]      NOT NULL DEFAULT '{}',  -- треки-примеры
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX taste_facets_user ON taste_facets (user_id);

CREATE TABLE wave_sessions (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id        uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id      uuid        REFERENCES devices (id) ON DELETE SET NULL,
    settings       jsonb       NOT NULL,            -- занятие, характер, настроение, язык, затравка
    state          jsonb       NOT NULL DEFAULT '{}', -- векторы «заходит» и «пропускается»
    started_at     timestamptz NOT NULL DEFAULT now(),
    last_active_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX wave_sessions_user ON wave_sessions (user_id, last_active_at DESC);

CREATE TABLE recommendation_impressions (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id         uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    track_id        uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    surface         text        NOT NULL CHECK (surface IN ('wave', 'radio', 'mix', 'similar', 'autoplay', 'discovery')),
    wave_session_id uuid        REFERENCES wave_sessions (id) ON DELETE SET NULL,
    model_version   text,
    score           real,
    reason          jsonb,                          -- для подписи «почему этот трек»
    shown_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX recommendation_impressions_user ON recommendation_impressions (user_id, shown_at DESC);

CREATE TABLE model_versions (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    kind        text        NOT NULL,               -- taste
    version     text        NOT NULL,
    metrics     jsonb       NOT NULL DEFAULT '{}',
    artifact_key text,
    active      boolean     NOT NULL DEFAULT false,
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (kind, version)
);
CREATE UNIQUE INDEX model_versions_active ON model_versions (kind) WHERE active;

-- Набор для умного офлайна: что сервер советует держать на устройстве
CREATE TABLE device_sets (
    device_id   uuid        NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    track_id    uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    kind        text        NOT NULL CHECK (kind IN ('favorite', 'fresh')),
    score       real        NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    rev         bigint      NOT NULL DEFAULT nextval('sync_rev'),
    PRIMARY KEY (device_id, track_id)
);
CREATE TRIGGER device_sets_gone AFTER DELETE ON device_sets FOR EACH ROW EXECUTE FUNCTION record_tombstone('device_id', 'track_id');

-- ───────────────────────────── Управление между устройствами (этап 3) ─────────────────────────────

-- Последнее состояние воспроизведения пользователя; живые команды идут по WebSocket
CREATE TABLE playback_state (
    user_id           uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    active_device_id  uuid        REFERENCES devices (id) ON DELETE SET NULL,
    state             jsonb       NOT NULL,         -- очередь, позиция, режимы
    updated_at        timestamptz NOT NULL DEFAULT now()
);

-- ───────────────────────────── Импорт ─────────────────────────────

CREATE TABLE imports (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id     uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind        text        NOT NULL CHECK (kind IN ('text', 'csv', 'm3u', 'files', 'spotify_archive')),
    title       text        NOT NULL,
    status      text        NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'done', 'failed')),
    total       integer     NOT NULL DEFAULT 0,
    matched     integer     NOT NULL DEFAULT 0,
    unmatched   jsonb       NOT NULL DEFAULT '[]',
    target_playlist_id uuid REFERENCES playlists (id) ON DELETE SET NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
