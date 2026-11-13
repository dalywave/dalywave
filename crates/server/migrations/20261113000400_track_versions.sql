-- Версии одной песни, выпущенные самими исполнителями: оригинал, slowed, super slowed, sped up.
-- Звук сервис не обрабатывает — находит готовые релизы в каталоге и у Deezer (см. versions.rs).

-- Найденные связи: у трека — версии той же песни, по одной на вид. Пишется всей семьёй сразу
-- (каждый участник видит остальных), пересчитывается раз в две недели при открытии.
CREATE TABLE track_versions (
    track_id   uuid NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    version_id uuid NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    -- Вид версии `version_id`
    kind       text NOT NULL CHECK (kind IN ('original', 'slowed', 'super_slowed', 'sped_up')),
    PRIMARY KEY (track_id, version_id),
    CHECK (track_id <> version_id)
);
CREATE INDEX track_versions_version ON track_versions (version_id);

-- Когда версии трека искали у Deezer: свежие связи берутся из track_versions без запросов
CREATE TABLE track_version_checks (
    track_id   uuid PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    checked_at timestamptz NOT NULL DEFAULT now()
);

-- Выбор слушателя: у этого исполнителя он слушает slowed (или другую версию) — в следующий раз
-- плеер ставит ту же. Оригинал — это отсутствие строки.
CREATE TABLE version_choices (
    user_id   uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    artist_id uuid NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    kind      text NOT NULL CHECK (kind IN ('slowed', 'super_slowed', 'sped_up')),
    chosen_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, artist_id)
);
