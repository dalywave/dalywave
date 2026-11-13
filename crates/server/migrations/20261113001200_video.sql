-- Видео без звука: живые обложки альбомов из Apple Music и клипы ленты открытий.

-- Живая обложка альбома. Нет обложки — тоже строка (state = 'none'): воркер перепроверяет её раз в месяц
CREATE TABLE album_motion (
    album_id       uuid PRIMARY KEY REFERENCES albums (id) ON DELETE CASCADE,
    state          text        NOT NULL CHECK (state IN ('ready', 'none')),
    apple_album_id text,
    square_key     text UNIQUE,
    tall_key       text UNIQUE,
    size_bytes     bigint      NOT NULL DEFAULT 0,
    checked_at     timestamptz NOT NULL DEFAULT now(),
    error          text
);

-- Клип трека: отрывок официального видео вокруг припева. Живёт неделю с последнего показа (трек
-- в любимых — дольше), общий объём ограничен — старое вытесняется. Нет клипа — тоже строка
CREATE TABLE track_clips (
    track_id         uuid PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    state            text        NOT NULL CHECK (state IN ('ready', 'none')),
    video_id         text,
    storage_key      text UNIQUE,
    size_bytes       bigint      NOT NULL DEFAULT 0,
    duration_ms      integer,
    clip_start_ms    integer,
    video_duration_ms integer,
    width            integer,
    height           integer,
    -- Ролики, которые отвергли вручную или по ошибке: при повторном поиске не предлагаются
    rejected_videos  text[]      NOT NULL DEFAULT '{}',
    checked_at       timestamptz NOT NULL DEFAULT now(),
    last_used_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX track_clips_used ON track_clips (last_used_at) WHERE state = 'ready';

-- Файлы ушедших строк стирает воркер (storage_trash — из «Сжатия для сотовой»)
CREATE FUNCTION video_trash() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME = 'album_motion' THEN
        INSERT INTO storage_trash (storage_key)
        SELECT k FROM unnest(ARRAY[OLD.square_key, OLD.tall_key]) k
        WHERE k IS NOT NULL AND (TG_OP = 'DELETE' OR k NOT IN (coalesce(NEW.square_key, ''), coalesce(NEW.tall_key, '')))
        ON CONFLICT DO NOTHING;
    ELSIF OLD.storage_key IS NOT NULL AND (TG_OP = 'DELETE' OR OLD.storage_key IS DISTINCT FROM NEW.storage_key) THEN
        INSERT INTO storage_trash (storage_key) VALUES (OLD.storage_key) ON CONFLICT DO NOTHING;
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER album_motion_gone AFTER DELETE OR UPDATE ON album_motion FOR EACH ROW EXECUTE FUNCTION video_trash();
CREATE TRIGGER track_clips_gone AFTER DELETE OR UPDATE ON track_clips FOR EACH ROW EXECUTE FUNCTION video_trash();

INSERT INTO job_lanes (lane, title, concurrency, min_interval_ms) VALUES
    ('motion', 'Живые обложки', 1, 500),
    ('clips',  'Клипы',         1, 2000)
ON CONFLICT (lane) DO NOTHING;
