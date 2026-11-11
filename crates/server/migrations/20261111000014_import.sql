-- Импорт музыки: архив данных Spotify, списки («Артист — Трек», CSV, M3U) и свои аудиофайлы.
-- Позиции импорта сопоставляются с каталогом Deezer в фоне (полоса «Импорт»), затем попадают
-- в медиатеку. Прослушивания из архива пишутся в play_events с source = 'import'.

INSERT INTO job_lanes (lane, title, concurrency, min_interval_ms) VALUES ('import', 'Импорт', 2, 0)
ON CONFLICT (lane) DO NOTHING;

ALTER TABLE imports DROP CONSTRAINT imports_status_check;
ALTER TABLE imports ADD CONSTRAINT imports_status_check
    CHECK (status IN ('running', 'done', 'failed', 'cancelled'));
ALTER TABLE imports
    -- uploading (файлы ещё грузятся) → matching → applying → done
    ADD COLUMN phase      text        NOT NULL DEFAULT 'matching'
        CHECK (phase IN ('uploading', 'matching', 'applying', 'done')),
    -- Сколько позиций уже разобрано (найдено, не найдено или пропущено) из total
    ADD COLUMN processed  integer     NOT NULL DEFAULT 0,
    -- Куда класть найденное: {"target": "likes" | "playlist" | "library", "playlistTitle": …}
    ADD COLUMN options    jsonb       NOT NULL DEFAULT '{}',
    -- Плейлисты архива: [{name, description, items: [id позиции, …], playlistId}]
    ADD COLUMN playlists  jsonb       NOT NULL DEFAULT '[]',
    -- Итог: {likes, playlists, playlistTracks, albums, artists, plays, files, upgraded}
    ADD COLUMN stats      jsonb       NOT NULL DEFAULT '{}',
    ADD COLUMN error      text,
    ADD COLUMN size_bytes bigint,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now();

-- Позиция импорта: трек, альбом, артист или загруженный файл
CREATE TABLE import_items (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    import_id   uuid        NOT NULL REFERENCES imports (id) ON DELETE CASCADE,
    kind        text        NOT NULL CHECK (kind IN ('track', 'album', 'artist', 'file')),
    position    integer     NOT NULL,
    -- Чем раньше сопоставить: медиатека раньше истории, частое раньше редкого
    priority    integer     NOT NULL DEFAULT 0,
    artist      text        NOT NULL DEFAULT '',
    title       text        NOT NULL DEFAULT '',
    album       text,
    duration_ms integer,
    isrc        text,
    -- spotify:track:…, имя файла, строка списка
    source_ref  text,
    liked       boolean     NOT NULL DEFAULT false,
    saved       boolean     NOT NULL DEFAULT false,
    followed    boolean     NOT NULL DEFAULT false,
    plays       integer     NOT NULL DEFAULT 0,
    status      text        NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'matched', 'unmatched', 'skipped')),
    track_id    uuid        REFERENCES tracks (id) ON DELETE SET NULL,
    album_id    uuid        REFERENCES albums (id) ON DELETE SET NULL,
    artist_id   uuid        REFERENCES artists (id) ON DELETE SET NULL,
    confidence  real,
    -- auto | cache | manual
    resolved_by text,
    -- Лучшие кандидаты Deezer с оценкой — для «проверьте» в интерфейсе
    candidates  jsonb       NOT NULL DEFAULT '[]',
    -- Загруженный файл: ключ в хранилище, размер, теги и формат, итог обработки
    file_key    text,
    file_size   bigint,
    file_meta   jsonb,
    -- Уже разложено в медиатеку (повторное применение ничего не дублирует)
    applied     boolean     NOT NULL DEFAULT false,
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX import_items_queue ON import_items (import_id, status, priority DESC, position);
CREATE INDEX import_items_track ON import_items (track_id) WHERE track_id IS NOT NULL;

-- История прослушиваний архива до сопоставления; после применения остаются только несопоставленные
CREATE TABLE import_plays (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    import_id  uuid        NOT NULL REFERENCES imports (id) ON DELETE CASCADE,
    item_id    uuid        NOT NULL REFERENCES import_items (id) ON DELETE CASCADE,
    ended_at   timestamptz NOT NULL,
    played_ms  integer     NOT NULL,
    end_reason text        NOT NULL,
    shuffle    boolean,
    platform   text
);
CREATE INDEX import_plays_item ON import_plays (item_id);
CREATE INDEX import_plays_import ON import_plays (import_id);

-- Кеш сопоставлений: spotify:track:… или нормализованное «артист|название» → Deezer. Повторный импорт
-- того же архива и одинаковые треки в разных файлах не тратят запросы к Deezer
CREATE TABLE import_matches (
    key        text PRIMARY KEY,
    kind       text        NOT NULL CHECK (kind IN ('track', 'album', 'artist')),
    -- NULL — не нашли (повторяем не раньше чем через неделю)
    deezer_id  bigint,
    confidence real,
    candidates jsonb       NOT NULL DEFAULT '[]',
    manual     boolean     NOT NULL DEFAULT false,
    matched_at timestamptz NOT NULL DEFAULT now()
);

-- Прослушивания из архива: питают вкус, но не «Продолжить»
ALTER TABLE play_events DROP CONSTRAINT play_events_source_check;
ALTER TABLE play_events ADD CONSTRAINT play_events_source_check
    CHECK (source IN ('stream', 'offline', 'import'));
-- Платформа прослушивания, когда устройства нет (архив Spotify): для поправок граней вкуса
ALTER TABLE play_events ADD COLUMN platform text
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android'));
