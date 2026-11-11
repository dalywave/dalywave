-- Активность (прослушивания и реакции), импорт и последнее состояние плеера.

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

-- ───────────────────────────── Импорт ─────────────────────────────

CREATE TABLE imports (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id            uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind               text        NOT NULL CHECK (kind IN ('text', 'csv', 'm3u', 'files', 'spotify_archive')),
    title              text        NOT NULL,
    status             text        NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'done', 'failed')),
    total              integer     NOT NULL DEFAULT 0,
    matched            integer     NOT NULL DEFAULT 0,
    unmatched          jsonb       NOT NULL DEFAULT '[]',
    target_playlist_id uuid        REFERENCES playlists (id) ON DELETE SET NULL,
    created_at         timestamptz NOT NULL DEFAULT now(),
    finished_at        timestamptz
);
CREATE INDEX imports_user ON imports (user_id, created_at DESC);

-- ───────────────────────────── Плеер (этап 3) ─────────────────────────────

-- Последнее состояние воспроизведения пользователя; живые команды идут по WebSocket
CREATE TABLE playback_state (
    user_id          uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    active_device_id uuid        REFERENCES devices (id) ON DELETE SET NULL,
    state            jsonb       NOT NULL,         -- очередь, позиция, режимы
    updated_at       timestamptz NOT NULL DEFAULT now()
);
