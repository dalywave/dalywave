-- «Слушать вместе» (как Jam у Spotify): хозяин открывает сессию, гости заходят по ссылке с кодом,
-- называются именем (без аккаунта) и добавляют треки в очередь его активного устройства. Звук
-- играет у хозяина; гость может включить «Слушать у себя» — тот же трек с той же позиции.

CREATE TABLE jam_sessions (
    id               uuid        PRIMARY KEY DEFAULT uuidv7(),
    -- Хозяин: его активное устройство играет, его очередь общая
    user_id          uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Код из ссылки /jam/{code}: 96 случайных бит в base64url — его не подобрать
    code             text        NOT NULL UNIQUE CHECK (length(code) BETWEEN 12 AND 32),
    guests_can_add   boolean     NOT NULL DEFAULT true,
    -- Добавленное гостями ждёт одобрения хозяина
    approval         boolean     NOT NULL DEFAULT false,
    -- Гости могут слушать у себя (ссылка на аудио текущего и следующего трека)
    guests_can_listen boolean    NOT NULL DEFAULT true,
    created_at       timestamptz NOT NULL DEFAULT now(),
    -- Сессия без дел 6 часов закрыта сама
    last_activity_at timestamptz NOT NULL DEFAULT now(),
    ended_at         timestamptz
);

-- Открытая сессия у хозяина одна
CREATE UNIQUE INDEX jam_sessions_open_key ON jam_sessions (user_id) WHERE ended_at IS NULL;

CREATE TABLE jam_guests (
    id           uuid        PRIMARY KEY DEFAULT uuidv7(),
    jam_id       uuid        NOT NULL REFERENCES jam_sessions (id) ON DELETE CASCADE,
    name         text        NOT NULL CHECK (length(name) BETWEEN 1 AND 32),
    -- Вошёл со своим аккаунтом DalyWave: имя оттуда (гостю аккаунт не нужен)
    user_id      uuid        REFERENCES users (id) ON DELETE SET NULL,
    -- SHA-256 токена гостя `dwj_…`: он действует только в этой сессии и только на /api/jam/guest/*
    token_hash   bytea       NOT NULL UNIQUE,
    joined_at    timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    removed_at   timestamptz
);

CREATE INDEX jam_guests_jam_idx ON jam_guests (jam_id, joined_at);

-- Треки от гостей: ждут одобрения, добавлены в очередь или отклонены
CREATE TABLE jam_requests (
    id         uuid        PRIMARY KEY DEFAULT uuidv7(),
    jam_id     uuid        NOT NULL REFERENCES jam_sessions (id) ON DELETE CASCADE,
    guest_id   uuid        NOT NULL REFERENCES jam_guests (id) ON DELETE CASCADE,
    track_id   uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    status     text        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'added', 'declined')),
    created_at timestamptz NOT NULL DEFAULT now(),
    decided_at timestamptz
);

CREATE INDEX jam_requests_jam_idx ON jam_requests (jam_id, created_at DESC);
CREATE INDEX jam_requests_guest_idx ON jam_requests (guest_id, status);

-- Голоса «пропустить» за текущий трек
CREATE TABLE jam_skip_votes (
    jam_id   uuid        NOT NULL REFERENCES jam_sessions (id) ON DELETE CASCADE,
    track_id uuid        NOT NULL,
    guest_id uuid        NOT NULL REFERENCES jam_guests (id) ON DELETE CASCADE,
    at       timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (jam_id, track_id, guest_id)
);
