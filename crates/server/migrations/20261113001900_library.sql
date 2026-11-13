-- Своя музыка из папок на сервере (DALYWAVE_MUSIC_DIRS). Файлы не копируются: файл записи — ключ
-- `library/<абсолютный путь>`, сервер только читает его. Что клиенты не играют как есть (ALAC, AIFF, Opus в Ogg,
-- WMA, APE…), перекодируется по запросу в копию `play`.

-- Источник файла записи: своя папка
ALTER TABLE audio_files DROP CONSTRAINT audio_files_source_check;
ALTER TABLE audio_files ADD CONSTRAINT audio_files_source_check
    CHECK (source IN ('youtube', 'youtube_music', 'soundcloud', 'bandcamp', 'upload', 'other', 'local'));
-- Кодек и контейнер, которых нет среди наших (WMA, APE, WavPack, AIFF): такой файл играет только копия
ALTER TABLE audio_files DROP CONSTRAINT audio_files_codec_check;
ALTER TABLE audio_files ADD CONSTRAINT audio_files_codec_check
    CHECK (codec IN ('opus', 'vorbis', 'aac', 'mp3', 'flac', 'alac', 'pcm', 'other'));
ALTER TABLE audio_files DROP CONSTRAINT audio_files_container_check;
ALTER TABLE audio_files ADD CONSTRAINT audio_files_container_check
    CHECK (container IN ('ogg', 'webm', 'mp4', 'mp3', 'flac', 'wav', 'other'));
-- Играют ли клиенты файл как есть (все браузеры и движок приложений). Нет — им отдаётся копия `play`
ALTER TABLE audio_files ADD COLUMN direct boolean NOT NULL DEFAULT true;

ALTER TABLE audio_renditions DROP CONSTRAINT audio_renditions_kind_check;
ALTER TABLE audio_renditions ADD CONSTRAINT audio_renditions_kind_check CHECK (kind IN ('opus64', 'aac256', 'play'));

-- Откуда запись каталога: копия Deezer или своя папка (теги файла)
ALTER TABLE artists ADD COLUMN source text NOT NULL DEFAULT 'deezer' CHECK (source IN ('deezer', 'local'));
ALTER TABLE albums ADD COLUMN source text NOT NULL DEFAULT 'deezer' CHECK (source IN ('deezer', 'local'));
ALTER TABLE tracks ADD COLUMN source text NOT NULL DEFAULT 'deezer' CHECK (source IN ('deezer', 'local'));
CREATE INDEX artists_local_name ON artists (lower(name)) WHERE source = 'local';
CREATE INDEX albums_local_title ON albums (artist_id, lower(title)) WHERE source = 'local';

-- Файлы своей музыки. Путь меняется при переименовании и переезде: строка (и трек с лайками) остаётся,
-- если отпечаток начала файла совпал.
CREATE TABLE library_files (
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    path          text        NOT NULL UNIQUE,      -- абсолютный путь на сервере
    root          text        NOT NULL,             -- из какой папки DALYWAVE_MUSIC_DIRS
    size_bytes    bigint      NOT NULL,
    mtime         timestamptz NOT NULL,
    -- SHA-256 первых 256 КБ и размера: узнаёт тот же файл после переезда, не читая его целиком
    fingerprint   text        NOT NULL,
    status        text        NOT NULL DEFAULT 'ok' CHECK (status IN ('ok', 'unsupported', 'error')),
    error         text,                             -- почему файл не прочитался
    format        text,                             -- что внутри: «FLAC 24/96», «MP3 320»
    track_id      uuid        REFERENCES tracks (id) ON DELETE SET NULL,
    audio_file_id uuid        REFERENCES audio_files (id) ON DELETE SET NULL,
    -- Теги как прочитаны: для пересборки каталога без чтения файла
    tags          jsonb       NOT NULL DEFAULT '{}',
    -- Громкость, огибающая и анализ звука посчитаны для этого содержимого (size, mtime)
    analyzed_at   timestamptz,
    seen_at       timestamptz NOT NULL DEFAULT now(),
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX library_files_fingerprint ON library_files (fingerprint);
CREATE INDEX library_files_track ON library_files (track_id);
CREATE INDEX library_files_status ON library_files (status) WHERE status <> 'ok';

-- Проходы сканера: прогресс для админки и итог
CREATE TABLE library_scans (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    kind        text        NOT NULL CHECK (kind IN ('full', 'changes')),
    status      text        NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'done', 'failed')),
    files_total integer     NOT NULL DEFAULT 0,     -- сколько аудиофайлов найдено в папках
    files_done  integer     NOT NULL DEFAULT 0,     -- сколько из них уже разобрано
    added       integer     NOT NULL DEFAULT 0,
    updated     integer     NOT NULL DEFAULT 0,
    moved       integer     NOT NULL DEFAULT 0,
    removed     integer     NOT NULL DEFAULT 0,
    failed      integer     NOT NULL DEFAULT 0,
    message     text,
    started_at  timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE INDEX library_scans_started ON library_scans (started_at DESC);

INSERT INTO job_lanes (lane, title, concurrency, min_interval_ms) VALUES
    ('local_scan', 'Своя музыка: папки', 1, 0),
    ('local_analyze', 'Своя музыка: громкость', 2, 0)
ON CONFLICT (lane) DO NOTHING;
