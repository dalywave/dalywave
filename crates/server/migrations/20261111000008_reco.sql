-- Этап 2: анализ звука, индекс каталога вокруг вкуса («разведчик»), грани вкуса, волна, показы
-- рекомендаций, версии модели и набор умного офлайна. Векторы — halfvec (вдвое меньше места на диске),
-- поиск ближайших — HNSW по косинусному расстоянию.

-- По превью (30 с) — для каталога вокруг вкуса, по полному файлу — для скачанного. Полный анализ заменяет
-- анализ превью, а превью полный уже не перезаписывает
CREATE TABLE track_analysis (
    recording_id    uuid PRIMARY KEY REFERENCES recordings (id) ON DELETE CASCADE,
    source          text        NOT NULL CHECK (source IN ('preview', 'full')),
    -- Файл полного анализа: сетка долей привязана к нему, новый основной файл анализируется заново
    file_id         uuid        REFERENCES audio_files (id) ON DELETE SET NULL,
    models          jsonb       NOT NULL,           -- версии моделей
    duration_ms     integer,                        -- сколько звука разобрано
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
    approachability real,                           -- «массовость» (голова MTG): «Популярное / Редкое»
    engagement      real,                           -- «вовлечённость»: фон для работы или на первый план
    intro_end_ms    integer,
    outro_start_ms  integer,
    beats_ms        integer[],                      -- сетка долей (полный анализ)
    downbeats_ms    integer[],
    vocal_segments  jsonb,                          -- [[начало, конец], …] в мс
    energy_curve    smallint[],                     -- по секунде, 0..1000
    extra           jsonb       NOT NULL DEFAULT '{}', -- остальное из ответа ml: громкость, размер такта, …
    analyzed_at     timestamptz NOT NULL DEFAULT now(),
    rev             bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid            xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX track_analysis_sound ON track_analysis USING hnsw (sound halfvec_cosine_ops);
CREATE INDEX track_analysis_semantic ON track_analysis USING hnsw (semantic halfvec_cosine_ops);
CREATE INDEX track_analysis_source ON track_analysis (source, analyzed_at DESC);
CREATE TRIGGER track_analysis_rev BEFORE UPDATE ON track_analysis FOR EACH ROW EXECUTE FUNCTION bump_rev();

-- Настройки разведчика и рекомендаций: меняются в админке («Рекомендации»). Одна строка.
-- Остановить анализ или разведку — пауза полос «Анализ звука» и «Разведка каталога»
CREATE TABLE reco_settings (
    id             boolean     PRIMARY KEY DEFAULT true CHECK (id),
    -- Сколько треков в сутки разведчик отдаёт на анализ превью
    tracks_per_day integer     NOT NULL DEFAULT 3000 CHECK (tracks_per_day BETWEEN 0 AND 50000),
    -- Предел индекса (анализов по превью): диск сервера небольшой, дальние от вкуса вытесняются
    max_index      integer     NOT NULL DEFAULT 200000 CHECK (max_index BETWEEN 100 AND 2000000),
    -- «Смелость разведки»: сколько шагов по похожим артистам и во сколько раз падает вес на шаге
    max_hops       smallint    NOT NULL DEFAULT 2 CHECK (max_hops BETWEEN 0 AND 3),
    hop_decay      real        NOT NULL DEFAULT 0.5 CHECK (hop_decay > 0 AND hop_decay < 1),
    -- Часовой пояс слушателя: время суток у граней вкуса и в волне
    utc_offset_min smallint    NOT NULL DEFAULT 180 CHECK (utc_offset_min BETWEEN -720 AND 840),
    updated_at     timestamptz NOT NULL DEFAULT now()
);
INSERT INTO reco_settings DEFAULT VALUES;

-- Окрестность вкуса: артисты с весом близости. Семена (лайки, подписки, прослушивания) — вес от слушания,
-- похожие Deezer — с затуханием на каждом шаге. Пересчитывается каждым проходом разведчика
CREATE TABLE scout_artists (
    artist_id  uuid PRIMARY KEY REFERENCES artists (id) ON DELETE CASCADE,
    weight     real        NOT NULL,
    hops       smallint    NOT NULL,                -- 0 — семя
    via        uuid        REFERENCES artists (id) ON DELETE SET NULL, -- через кого найден
    scouted_at timestamptz,                         -- собраны похожие, популярное, релизы и радио
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX scout_artists_weight ON scout_artists (weight DESC);

-- Треки, найденные разведчиком: очередь на анализ превью и порядок вытеснения. Близость к вкусу —
-- base × вес артиста в окрестности (чарты и подборки — только base), поэтому она следует за вкусом сама
CREATE TABLE scout_tracks (
    track_id    uuid PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    artist_id   uuid        REFERENCES artists (id) ON DELETE SET NULL,
    source      text        NOT NULL CHECK (source IN ('top', 'release', 'radio', 'chart', 'editorial')),
    base        real        NOT NULL,               -- вид находки и популярность, 0..1
    found_at    timestamptz NOT NULL DEFAULT now(),
    queued_at   timestamptz,                        -- поставлен на анализ превью
    analyzed_at timestamptz,
    skipped     text                                -- no_preview, evicted
);
CREATE INDEX scout_tracks_pending ON scout_tracks (base DESC) WHERE analyzed_at IS NULL AND skipped IS NULL;
CREATE INDEX scout_tracks_artist ON scout_tracks (artist_id);

-- Грани вкуса: группы любимого (k-means по звучанию) со своими векторами и поправками по контексту
CREATE TABLE taste_facets (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id    uuid          NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    position   smallint      NOT NULL,              -- по весу: самая весомая — 0
    label      text          NOT NULL,
    centroid   halfvec(1280) NOT NULL,
    weight     real          NOT NULL,              -- доля вкуса, 0..1
    tracks     integer       NOT NULL DEFAULT 0,    -- сколько треков в грани
    context    jsonb         NOT NULL DEFAULT '{}', -- поправки по времени суток и устройству
    examples   uuid[]        NOT NULL DEFAULT '{}', -- треки-примеры
    artists    uuid[]        NOT NULL DEFAULT '{}', -- главные артисты грани: подпись и «Похоже на …»
    updated_at timestamptz   NOT NULL DEFAULT now()
);
CREATE INDEX taste_facets_user ON taste_facets (user_id, position);

CREATE TABLE wave_sessions (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id        uuid          NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id      uuid          REFERENCES devices (id) ON DELETE SET NULL,
    settings       jsonb         NOT NULL,            -- занятие, характер, настроение, язык, затравка
    state          jsonb         NOT NULL DEFAULT '{}', -- выданное, последний трек, шаг, пропуски подряд
    hits           halfvec(1280),                     -- «что сейчас заходит»
    misses         halfvec(1280),                     -- «что сейчас пропускается»
    seed_sound     halfvec(1280),                     -- затравка радио: трек, артист, альбом, плейлист
    seed_text      halfvec(512),                      -- затравка словами (CLAP)
    started_at     timestamptz   NOT NULL DEFAULT now(),
    last_active_at timestamptz   NOT NULL DEFAULT now()
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
    position        integer,                        -- номер в сессии или в выдаче
    -- Чем кончился показ: размеченные данные для метрик и будущей модели
    outcome         text        CHECK (outcome IN ('like', 'dislike', 'skip', 'not_now', 'more_like_this', 'completed')),
    outcome_at      timestamptz,
    shown_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX recommendation_impressions_user ON recommendation_impressions (user_id, shown_at DESC);
CREATE INDEX recommendation_impressions_session ON recommendation_impressions (wave_session_id, track_id)
    WHERE wave_session_id IS NOT NULL;

CREATE TABLE model_versions (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    kind         text        NOT NULL,              -- taste
    version      text        NOT NULL,
    metrics      jsonb       NOT NULL DEFAULT '{}',
    artifact_key text,
    active       boolean     NOT NULL DEFAULT false,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (kind, version)
);
CREATE UNIQUE INDEX model_versions_active ON model_versions (kind) WHERE active;
-- Пока своей нейросети нет, поток собирают правила: их версия пишется в показы
INSERT INTO model_versions (kind, version, active) VALUES ('taste', 'rules-1', true);

-- Набор для умного офлайна (этап 6): что сервер советует держать на устройстве
CREATE TABLE device_sets (
    device_id  uuid        NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    track_id   uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    kind       text        NOT NULL CHECK (kind IN ('favorite', 'fresh')),
    score      real        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid       xid8        NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (device_id, track_id)
);
CREATE TRIGGER device_sets_rev BEFORE UPDATE ON device_sets FOR EACH ROW EXECUTE FUNCTION bump_rev();

-- У набора устройства своего user_id нет: владелец — у устройства, ключ — (device_id, track_id)
CREATE OR REPLACE FUNCTION record_child_tombstone() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    owner uuid;
    key   jsonb;
BEGIN
    IF TG_TABLE_NAME = 'playlist_items' THEN
        SELECT user_id INTO owner FROM playlists WHERE id = OLD.playlist_id;
        key := jsonb_build_object('id', OLD.id, 'playlist_id', OLD.playlist_id);
    ELSIF TG_TABLE_NAME = 'device_sets' THEN
        SELECT user_id INTO owner FROM devices WHERE id = OLD.device_id;
        key := jsonb_build_object('device_id', OLD.device_id, 'track_id', OLD.track_id);
    ELSE
        SELECT user_id INTO owner FROM devices WHERE id = OLD.device_id;
        key := jsonb_build_object('id', OLD.id, 'device_id', OLD.device_id);
    END IF;
    IF owner IS NOT NULL THEN
        INSERT INTO tombstones (table_name, row_key, user_id) VALUES (TG_TABLE_NAME, key, owner);
    END IF;
    RETURN OLD;
END $$;
CREATE TRIGGER device_sets_gone AFTER DELETE ON device_sets FOR EACH ROW EXECUTE FUNCTION record_child_tombstone();

-- Отсев волны: последняя реакция «не нравится» / «снова можно» по треку и по артисту
CREATE INDEX feedback_events_user_track ON feedback_events (user_id, track_id, at DESC) WHERE track_id IS NOT NULL;
CREATE INDEX feedback_events_user_artist ON feedback_events (user_id, artist_id, at DESC) WHERE artist_id IS NOT NULL;
-- «Отдых» заслушанного: прослушивания трека человеком за последние недели
CREATE INDEX play_events_user_track ON play_events (user_id, track_id, started_at DESC);
-- Миксы дня и «Открытия недели» — плейлисты вида mix, их генератор и номер — в rules
CREATE INDEX playlists_generated ON playlists (user_id, (rules ->> 'generator')) WHERE kind = 'mix';
