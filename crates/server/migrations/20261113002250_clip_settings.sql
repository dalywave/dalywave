-- Настройки клипов в админке: качество, кусок или клип целиком (в такт с треком), сроки и потолок
-- места, автопоиск. Прежнее поведение — значения по умолчанию.
ALTER TABLE server_settings
    ADD COLUMN clip_height          integer NOT NULL DEFAULT 480 CHECK (clip_height IN (360, 480, 720, 1080)),
    -- Клип целиком, а не кусок с припевом
    ADD COLUMN clip_whole           boolean NOT NULL DEFAULT false,
    -- Целиком — в такт с треком (сопоставление звука клипа с треком)
    ADD COLUMN clip_sync            boolean NOT NULL DEFAULT true,
    ADD COLUMN clip_length_s        integer NOT NULL DEFAULT 12 CHECK (clip_length_s BETWEEN 6 AND 30),
    ADD COLUMN clip_keep_days       integer NOT NULL DEFAULT 7 CHECK (clip_keep_days BETWEEN 1 AND 365),
    ADD COLUMN clip_keep_liked_days integer NOT NULL DEFAULT 60 CHECK (clip_keep_liked_days BETWEEN 1 AND 730),
    ADD COLUMN clip_max_gb          integer NOT NULL DEFAULT 3 CHECK (clip_max_gb BETWEEN 1 AND 2000),
    -- Искать клипы сами при показе трека
    ADD COLUMN clip_auto            boolean NOT NULL DEFAULT true;

-- Клип целиком и его сопоставление с треком: позиция трека = sync_offset_ms + позиция видео × sync_rate
ALTER TABLE track_clips
    ADD COLUMN whole          boolean NOT NULL DEFAULT false,
    ADD COLUMN sync_offset_ms integer,
    ADD COLUMN sync_rate      real CHECK (sync_rate BETWEEN 0.5 AND 2);
