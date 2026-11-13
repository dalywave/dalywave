-- Сведения о музыке: чарты, кто сделал трек (MusicBrainz), об артисте (Википедия).

-- Чарт — треки каталога по местам. Собирается при обращении, если старше нескольких часов
CREATE TABLE charts (
    id         text PRIMARY KEY,
    track_ids  uuid[]      NOT NULL DEFAULT '{}',
    fetched_at timestamptz
);

-- Строка чужого чарта (Apple Music) → трек каталога. track_id NULL — не нашли, спросим позже
CREATE TABLE chart_matches (
    source      text        NOT NULL,
    external_id text        NOT NULL,
    track_id    uuid        REFERENCES tracks (id) ON DELETE CASCADE,
    checked_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source, external_id)
);

-- Кто сделал запись: люди MusicBrainz с ролями ([{"id", "name", "roles": [...]}]) по ISRC.
-- found = false — MusicBrainz записи не знает
CREATE TABLE recording_credits (
    isrc       text PRIMARY KEY,
    found      boolean     NOT NULL,
    people     jsonb       NOT NULL DEFAULT '[]',
    fetched_at timestamptz NOT NULL DEFAULT now()
);

-- Человек MusicBrainz → артист каталога по его ссылке на Deezer. checked_at NULL — ещё не смотрели
CREATE TABLE music_people (
    mbid       uuid PRIMARY KEY,
    name       text        NOT NULL,
    artist_id  uuid        REFERENCES artists (id) ON DELETE SET NULL,
    checked_at timestamptz
);

-- Об артисте: начало статьи Википедии и факты из Wikidata. about NULL — статьи нет
CREATE TABLE artist_about (
    artist_id   uuid PRIMARY KEY REFERENCES artists (id) ON DELETE CASCADE,
    wikidata_id text,
    about       jsonb,
    fetched_at  timestamptz NOT NULL DEFAULT now()
);
