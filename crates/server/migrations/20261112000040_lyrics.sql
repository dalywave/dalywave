-- Тексты песен из LRCLIB: строки с временем (LRC) или простой текст. Берутся при первом запросе
-- и заранее для ближайших треков очереди (полоса «Тексты»). «Не нашли» живёт 30 дней, потом
-- спрашиваем снова: у LRCLIB тексты добавляют люди.

CREATE TABLE track_lyrics (
    track_id        uuid PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    -- Есть строки с временем: подсветка текущей строки
    synced          boolean     NOT NULL DEFAULT false,
    -- Инструментал по слову LRCLIB: текста нет и не будет («Без слов»)
    instrumental    boolean     NOT NULL DEFAULT false,
    -- [{"t_ms": 12340, "text": "…"}] по возрастанию времени; пустой текст — проигрыш
    lines           jsonb,
    -- Текст без времени (у синхронного — тоже, для поиска по строчке)
    plain           text,
    source          text        NOT NULL DEFAULT 'lrclib',
    source_id       bigint,
    fetched_at      timestamptz NOT NULL DEFAULT now(),
    -- Не нашли: до этого времени не спрашиваем
    not_found_until timestamptz,
    CHECK (NOT synced OR lines IS NOT NULL)
);
-- Поиск песни по строчке
CREATE INDEX track_lyrics_plain_trgm ON track_lyrics USING gin (search_norm(plain) gin_trgm_ops)
    WHERE plain IS NOT NULL;

INSERT INTO job_lanes (lane, title, concurrency, min_interval_ms) VALUES ('lyrics', 'Тексты', 1, 1000)
ON CONFLICT (lane) DO NOTHING;
