-- Друзья и совместные плейлисты.
--
-- Друзья: запрос → принять (дружба — две строки, по одной на каждого) или отклонить (запрос остаётся
-- отклонённым: повторный не тревожит человека). Ссылка «Добавить в друзья» — постоянный код человека,
-- открывший её становится другом сразу. Приватность: «показывать друзьям, что я слушаю» и приватные
-- сессии — промежутки, сыгранное в которых друзья не видят.
--
-- Совместный плейлист: владелец — по-прежнему playlists.user_id, участники — playlist_members. Строки
-- плейлиста и его позиции уходят на устройства участников той же синхронизацией: у участника свои
-- место и «закреплён» (строка участника со своим номером изменения), вход участника поднимает номера
-- позиций плейлиста (они должны прийти на его устройства), выход — tombstone плейлиста для него.

CREATE TABLE friend_requests (
    from_user   uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    to_user     uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at  timestamptz NOT NULL DEFAULT now(),
    -- Отклонён: отправителю по-прежнему «запрос отправлен», получателя он больше не тревожит
    declined_at timestamptz,
    PRIMARY KEY (from_user, to_user),
    CHECK (from_user <> to_user)
);
CREATE INDEX friend_requests_to ON friend_requests (to_user) WHERE declined_at IS NULL;

-- Дружба взаимна: две строки (user_id → friend_id и обратно)
CREATE TABLE friendships (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    friend_id  uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, friend_id),
    CHECK (user_id <> friend_id)
);

CREATE TABLE social_settings (
    user_id         uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    -- «Показывать друзьям, что я слушаю»
    share_listening boolean     NOT NULL DEFAULT true,
    -- Код ссылки «Добавить в друзья»: 72 случайных бита в base64url
    invite_code     text UNIQUE CHECK (length(invite_code) BETWEEN 8 AND 32),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

-- Приватные сессии: идёт, пока now() < ended_at («до выключения» — конец через сто лет)
CREATE TABLE private_sessions (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    started_at timestamptz NOT NULL DEFAULT now(),
    ended_at   timestamptz NOT NULL,
    CHECK (ended_at >= started_at)
);
CREATE INDEX private_sessions_user ON private_sessions (user_id, ended_at DESC);

-- Сыграно в приватной сессии: друзья этого не видят
CREATE FUNCTION played_privately(uid uuid, at timestamptz) RETURNS boolean
LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT EXISTS (SELECT 1 FROM private_sessions s
                   WHERE s.user_id = uid AND at >= s.started_at AND at < s.ended_at)
$$;

-- ---------- Совместные плейлисты ----------

ALTER TABLE playlists
    ADD COLUMN collaborative   boolean NOT NULL DEFAULT false,
    -- Видят друзья на странице владельца (миксы — никогда)
    ADD COLUMN friends_can_see boolean NOT NULL DEFAULT true,
    -- Код ссылки-приглашения в совместный плейлист
    ADD COLUMN invite_code     text UNIQUE CHECK (length(invite_code) BETWEEN 8 AND 32);

ALTER TABLE playlist_items ADD COLUMN added_by uuid REFERENCES users (id) ON DELETE SET NULL;

CREATE TABLE playlist_members (
    playlist_id uuid             NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    user_id     uuid             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Место в медиатеке участника (вне папок, общая шкала с его папками) и «закреплён»
    position    double precision NOT NULL,
    pinned      boolean          NOT NULL DEFAULT false,
    joined_at   timestamptz      NOT NULL DEFAULT now(),
    rev         bigint           NOT NULL DEFAULT nextval('sync_rev'),
    txid        xid8             NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (playlist_id, user_id)
);
CREATE INDEX playlist_members_user ON playlist_members (user_id);
CREATE TRIGGER playlist_members_rev BEFORE UPDATE ON playlist_members FOR EACH ROW EXECUTE FUNCTION bump_rev();

-- Участник вышел (или его убрали, или плейлист удалён): на его устройствах плейлиста больше нет
CREATE FUNCTION record_member_tombstone() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO tombstones (table_name, row_key, user_id)
    VALUES ('playlists', jsonb_build_object('id', OLD.playlist_id), OLD.user_id);
    RETURN OLD;
END $$;
CREATE TRIGGER playlist_members_gone AFTER DELETE ON playlist_members FOR EACH ROW EXECUTE FUNCTION record_member_tombstone();

-- Позиция совместного плейлиста убрана — tombstone и владельцу, и каждому участнику (остальное —
-- как в 20261111000008_reco: позиции плейлиста, набор волны и загрузки устройства)
CREATE OR REPLACE FUNCTION record_child_tombstone() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    owner uuid;
    key   jsonb;
BEGIN
    IF TG_TABLE_NAME = 'playlist_items' THEN
        SELECT user_id INTO owner FROM playlists WHERE id = OLD.playlist_id;
        key := jsonb_build_object('id', OLD.id, 'playlist_id', OLD.playlist_id);
        IF owner IS NOT NULL THEN
            INSERT INTO tombstones (table_name, row_key, user_id)
            SELECT TG_TABLE_NAME, key, m.user_id FROM playlist_members m WHERE m.playlist_id = OLD.playlist_id;
        END IF;
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

-- Каталожные строки совместных плейлистов тоже уходят на устройства участника (остальное — как в
-- 20261111000040_offline)
CREATE OR REPLACE FUNCTION user_track_ids(uid uuid) RETURNS SETOF uuid LANGUAGE sql STABLE AS $$
    SELECT track_id FROM liked_tracks WHERE user_id = uid
    UNION
    SELECT i.track_id FROM playlist_items i JOIN playlists p ON p.id = i.playlist_id WHERE p.user_id = uid
    UNION
    SELECT i.track_id FROM playlist_items i JOIN playlist_members m ON m.playlist_id = i.playlist_id
    WHERE m.user_id = uid
    UNION
    SELECT t.id FROM saved_albums s JOIN tracks t ON t.album_id = s.album_id WHERE s.user_id = uid
    UNION
    SELECT ref_id FROM blocked_items WHERE user_id = uid AND kind = 'track'
    UNION
    SELECT dd.ref_id FROM device_downloads dd JOIN devices d ON d.id = dd.device_id
    WHERE d.user_id = uid AND dd.kind = 'track'
    UNION
    SELECT t.id FROM device_downloads dd JOIN devices d ON d.id = dd.device_id JOIN tracks t ON t.album_id = dd.ref_id
    WHERE d.user_id = uid AND dd.kind = 'album'
    UNION
    SELECT t.id FROM device_downloads dd JOIN devices d ON d.id = dd.device_id
    JOIN artist_releases ar ON ar.artist_id = dd.ref_id JOIN tracks t ON t.album_id = ar.album_id
    WHERE d.user_id = uid AND dd.kind = 'artist'
    UNION
    SELECT ds.track_id FROM device_sets ds JOIN devices d ON d.id = ds.device_id WHERE d.user_id = uid
$$;

-- ---------- Уведомления ----------

-- Новые виды: друзья и совместные плейлисты. Список видов держит сервер (перечисление в коде):
-- ограничение со списком мешало бы добавлять виды из разных веток
ALTER TABLE notifications DROP CONSTRAINT IF EXISTS notifications_kind_check;
