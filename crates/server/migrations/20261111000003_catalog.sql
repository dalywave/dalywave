-- Каталог (копия Deezer, заполняется при первом обращении) и аудиофайлы.
-- Триггеры номера изменения срабатывают только на колонки, которые уходят в приложения:
-- обновление сроков свежести и счётчиков фанатов не гоняет синхронизацию.

CREATE TABLE artists (
    id                    uuid PRIMARY KEY DEFAULT uuidv7(),
    deezer_id             bigint UNIQUE,
    name                  text        NOT NULL,
    picture_key           text,                 -- копия в S3 (префикс, размеры — в имени файла); NULL, пока не скопирована
    picture_source        text,                 -- адрес у Deezer (1000×1000)
    picture_bytes         integer,              -- сколько занимают копии всех размеров
    fans                  integer,
    albums_count          integer,
    fetched_at            timestamptz,          -- карточка: имя, фото, счётчики (7 дней)
    top_fetched_at        timestamptz,          -- популярные треки (1 день)
    related_fetched_at    timestamptz,          -- похожие артисты (7 дней)
    albums_fetched_at     timestamptz,          -- список релизов (7 дней)
    discography_synced_at timestamptz,          -- последняя сверка релизов для подписки
    created_at            timestamptz NOT NULL DEFAULT now(),
    updated_at            timestamptz NOT NULL DEFAULT now(),
    rev                   bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid                  xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX artists_name_trgm ON artists USING gin (search_norm(name) gin_trgm_ops);
CREATE TRIGGER artists_rev BEFORE UPDATE ON artists FOR EACH ROW
    WHEN ((OLD.name, OLD.picture_key, OLD.picture_source) IS DISTINCT FROM (NEW.name, NEW.picture_key, NEW.picture_source))
    EXECUTE FUNCTION bump_rev();

CREATE TABLE albums (
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    deezer_id     bigint UNIQUE,
    upc           text,
    title         text        NOT NULL,
    artist_id     uuid        NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    record_type   text        NOT NULL DEFAULT 'album' CHECK (record_type IN ('album', 'ep', 'single', 'compile')),
    release_date  date,
    label         text,
    genres        text[]      NOT NULL DEFAULT '{}',
    explicit      boolean     NOT NULL DEFAULT false,
    tracks_count  integer,
    duration_ms   integer,
    fans          integer,
    cover_key     text,
    cover_source  text,
    cover_bytes   integer,
    cover_palette jsonb,                        -- основные цвета обложки для фона интерфейса: ["#rrggbb", …]
    fetched_at    timestamptz,                  -- когда загружен список треков (30 дней)
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    rev           bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid          xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX albums_artist_release ON albums (artist_id, release_date DESC);
CREATE INDEX albums_title_trgm ON albums USING gin (search_norm(title) gin_trgm_ops);
CREATE TRIGGER albums_rev BEFORE UPDATE ON albums FOR EACH ROW
    WHEN ((OLD.title, OLD.artist_id, OLD.record_type, OLD.release_date, OLD.label, OLD.genres, OLD.explicit,
           OLD.tracks_count, OLD.duration_ms, OLD.cover_key, OLD.cover_source, OLD.cover_palette)
          IS DISTINCT FROM
          (NEW.title, NEW.artist_id, NEW.record_type, NEW.release_date, NEW.label, NEW.genres, NEW.explicit,
           NEW.tracks_count, NEW.duration_ms, NEW.cover_key, NEW.cover_source, NEW.cover_palette))
    EXECUTE FUNCTION bump_rev();

-- Дискография артиста в том виде, как её отдаёт Deezer: туда входят и совместные релизы,
-- у которых главный артист альбома — другой (albums.artist_id один)
CREATE TABLE artist_releases (
    artist_id uuid NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    album_id  uuid NOT NULL REFERENCES albums (id) ON DELETE CASCADE,
    PRIMARY KEY (artist_id, album_id)
);
CREATE INDEX artist_releases_album ON artist_releases (album_id);

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
    rev               bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid              xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX recordings_status ON recordings (audio_status) WHERE audio_status <> 'none';
CREATE TRIGGER recordings_rev BEFORE UPDATE ON recordings FOR EACH ROW
    WHEN ((OLD.isrc, OLD.duration_ms, OLD.audio_status, OLD.primary_file_id)
          IS DISTINCT FROM (NEW.isrc, NEW.duration_ms, NEW.audio_status, NEW.primary_file_id))
    EXECUTE FUNCTION bump_rev();

CREATE TABLE tracks (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    deezer_id      bigint UNIQUE,
    isrc           text,
    -- NULL, пока ISRC неизвестен (трек пришёл из списка без кодов, например из популярного артиста)
    recording_id   uuid        REFERENCES recordings (id) ON DELETE SET NULL,
    title          text        NOT NULL,
    title_version  text        NOT NULL DEFAULT '',   -- «Slowed», «Remastered 2011» — часть названия у Deezer
    album_id       uuid        REFERENCES albums (id) ON DELETE SET NULL,
    disc_number    smallint    NOT NULL DEFAULT 1,
    track_position smallint,
    duration_ms    integer     NOT NULL,
    explicit       boolean     NOT NULL DEFAULT false,
    rank           integer,                           -- популярность у Deezer
    fetched_at     timestamptz,                       -- загружен полностью: ISRC и все участники
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz NOT NULL DEFAULT now(),
    rev            bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid           xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX tracks_album ON tracks (album_id, disc_number, track_position);
CREATE INDEX tracks_isrc ON tracks (isrc);
CREATE INDEX tracks_recording ON tracks (recording_id);
CREATE INDEX tracks_title_trgm ON tracks USING gin (search_norm(title) gin_trgm_ops);
CREATE TRIGGER tracks_rev BEFORE UPDATE ON tracks FOR EACH ROW
    WHEN ((OLD.isrc, OLD.recording_id, OLD.title, OLD.title_version, OLD.album_id, OLD.disc_number,
           OLD.track_position, OLD.duration_ms, OLD.explicit)
          IS DISTINCT FROM
          (NEW.isrc, NEW.recording_id, NEW.title, NEW.title_version, NEW.album_id, NEW.disc_number,
           NEW.track_position, NEW.duration_ms, NEW.explicit))
    EXECUTE FUNCTION bump_rev();

CREATE TABLE track_artists (
    track_id  uuid     NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    artist_id uuid     NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    role      text     NOT NULL DEFAULT 'main' CHECK (role IN ('main', 'featured')),
    position  smallint NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, artist_id)
);
CREATE INDEX track_artists_artist ON track_artists (artist_id);

-- Популярное и похожие для страницы артиста: кеш ответов Deezer в их порядке
CREATE TABLE artist_top_tracks (
    artist_id uuid     NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    position  smallint NOT NULL,
    track_id  uuid     NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    PRIMARY KEY (artist_id, position)
);

CREATE TABLE artist_related (
    artist_id  uuid     NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    position   smallint NOT NULL,
    related_id uuid     NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    PRIMARY KEY (artist_id, position)
);

-- ───────────────────────────── Аудио ─────────────────────────────

CREATE TABLE audio_files (
    id                uuid PRIMARY KEY DEFAULT uuidv7(),
    recording_id      uuid        NOT NULL REFERENCES recordings (id) ON DELETE CASCADE,
    storage_key       text        NOT NULL UNIQUE,
    codec             text        NOT NULL CHECK (codec IN ('opus', 'vorbis', 'aac', 'mp3', 'flac', 'alac', 'pcm')),
    container         text        NOT NULL CHECK (container IN ('ogg', 'webm', 'mp4', 'mp3', 'flac', 'wav')),
    lossless          boolean     NOT NULL DEFAULT false,
    bitrate_kbps      integer,
    sample_rate       integer,
    channels          smallint,
    bit_depth         smallint,
    duration_ms       integer     NOT NULL,
    size_bytes        bigint      NOT NULL,
    source            text        NOT NULL CHECK (source IN ('youtube', 'youtube_music', 'soundcloud', 'bandcamp', 'upload', 'other')),
    source_ref        text,                         -- id видео или ссылка
    source_title      text,                         -- как запись называлась в источнике
    loudness_lufs     real,                         -- интегральная громкость (EBU R128)
    true_peak_db      real,
    waveform          smallint[],                   -- огибающая для полосы прокрутки, 0..1000
    match_score       real,
    verified          boolean     NOT NULL DEFAULT false,
    verify_similarity real,
    verify_offset_ms  integer,
    acquired_by       text,                         -- server | device:<id> (загрузчик на Mac)
    created_at        timestamptz NOT NULL DEFAULT now(),
    rev               bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid              xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX audio_files_recording ON audio_files (recording_id);
CREATE TRIGGER audio_files_rev BEFORE UPDATE ON audio_files FOR EACH ROW EXECUTE FUNCTION bump_rev();

ALTER TABLE recordings
    ADD CONSTRAINT recordings_primary_file FOREIGN KEY (primary_file_id) REFERENCES audio_files (id) ON DELETE SET NULL;
