-- Медиатека: лайки, альбомы, подписки, «не рекомендовать», плейлисты и загрузки на устройства.

CREATE TABLE liked_tracks (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    track_id   uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid       xid8        NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (user_id, track_id)
);
CREATE INDEX liked_tracks_recent ON liked_tracks (user_id, created_at DESC);
CREATE INDEX liked_tracks_track ON liked_tracks (track_id);
CREATE TRIGGER liked_tracks_gone AFTER DELETE ON liked_tracks FOR EACH ROW EXECUTE FUNCTION record_tombstone('user_id', 'track_id');

CREATE TABLE saved_albums (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    album_id   uuid        NOT NULL REFERENCES albums (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid       xid8        NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (user_id, album_id)
);
CREATE INDEX saved_albums_recent ON saved_albums (user_id, created_at DESC);
CREATE TRIGGER saved_albums_gone AFTER DELETE ON saved_albums FOR EACH ROW EXECUTE FUNCTION record_tombstone('user_id', 'album_id');

-- Подписка на артиста: что докачивать на сервер; на устройства — через device_downloads
CREATE TABLE followed_artists (
    user_id         uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    artist_id       uuid        NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    download_policy jsonb       NOT NULL DEFAULT '{"albums": true, "eps": true, "singles": true, "compilations": false, "live": false, "appearsOn": false}',
    notify          boolean     NOT NULL DEFAULT true,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    rev             bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid            xid8        NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (user_id, artist_id)
);
CREATE INDEX followed_artists_artist ON followed_artists (artist_id);
CREATE TRIGGER followed_artists_rev BEFORE UPDATE ON followed_artists FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER followed_artists_gone AFTER DELETE ON followed_artists FOR EACH ROW EXECUTE FUNCTION record_tombstone('user_id', 'artist_id');

-- «Не рекомендовать»: трек или артист. Синхронизируется: волна без сети тоже должна их обходить
CREATE TABLE blocked_items (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind       text        NOT NULL CHECK (kind IN ('track', 'artist')),
    ref_id     uuid        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid       xid8        NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (user_id, kind, ref_id)
);
CREATE TRIGGER blocked_items_gone AFTER DELETE ON blocked_items FOR EACH ROW EXECUTE FUNCTION record_tombstone('user_id', 'kind', 'ref_id');

CREATE TABLE playlists (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id     uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title       text        NOT NULL,
    description text        NOT NULL DEFAULT '',
    kind        text        NOT NULL DEFAULT 'user' CHECK (kind IN ('user', 'smart', 'mix', 'system')),
    rules       jsonb,                              -- для умных плейлистов
    cover_key   text,
    pinned      boolean     NOT NULL DEFAULT false,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    rev         bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid        xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX playlists_user ON playlists (user_id, updated_at DESC);
CREATE TRIGGER playlists_rev BEFORE UPDATE ON playlists FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER playlists_gone AFTER DELETE ON playlists FOR EACH ROW EXECUTE FUNCTION record_tombstone('id', 'user_id');

-- Позиция — дробная: перестановка меняет одну строку
CREATE TABLE playlist_items (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    playlist_id uuid             NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    track_id    uuid             NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    position    double precision NOT NULL,
    added_at    timestamptz      NOT NULL DEFAULT now(),
    rev         bigint           NOT NULL DEFAULT nextval('sync_rev'),
    txid        xid8             NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX playlist_items_order ON playlist_items (playlist_id, position);
CREATE INDEX playlist_items_track ON playlist_items (track_id);
CREATE TRIGGER playlist_items_rev BEFORE UPDATE ON playlist_items FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER playlist_items_gone AFTER DELETE ON playlist_items FOR EACH ROW EXECUTE FUNCTION record_child_tombstone();

-- Что держать скачанным на конкретном устройстве
CREATE TABLE device_downloads (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    device_id  uuid        NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    kind       text        NOT NULL CHECK (kind IN ('track', 'album', 'playlist', 'artist', 'liked')),
    ref_id     uuid,                                -- NULL только для liked
    created_at timestamptz NOT NULL DEFAULT now(),
    rev        bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid       xid8        NOT NULL DEFAULT pg_current_xact_id(),
    CHECK ((kind = 'liked') = (ref_id IS NULL))
);
CREATE UNIQUE INDEX device_downloads_unique ON device_downloads (device_id, kind, coalesce(ref_id, '00000000-0000-0000-0000-000000000000'));
CREATE TRIGGER device_downloads_gone AFTER DELETE ON device_downloads FOR EACH ROW EXECUTE FUNCTION record_child_tombstone();

-- Каталожные строки, на которые ссылаются данные человека: их изменения уходят в приложения вместе с медиатекой
CREATE FUNCTION user_track_ids(uid uuid) RETURNS SETOF uuid LANGUAGE sql STABLE AS $$
    SELECT track_id FROM liked_tracks WHERE user_id = uid
    UNION
    SELECT i.track_id FROM playlist_items i JOIN playlists p ON p.id = i.playlist_id WHERE p.user_id = uid
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
$$;

CREATE FUNCTION user_album_ids(uid uuid) RETURNS SETOF uuid LANGUAGE sql STABLE AS $$
    SELECT album_id FROM saved_albums WHERE user_id = uid
    UNION
    SELECT t.album_id FROM tracks t WHERE t.id IN (SELECT user_track_ids(uid)) AND t.album_id IS NOT NULL
$$;

CREATE FUNCTION user_artist_ids(uid uuid) RETURNS SETOF uuid LANGUAGE sql STABLE AS $$
    SELECT artist_id FROM followed_artists WHERE user_id = uid
    UNION
    SELECT ref_id FROM blocked_items WHERE user_id = uid AND kind = 'artist'
    UNION
    SELECT a.artist_id FROM albums a WHERE a.id IN (SELECT user_album_ids(uid))
    UNION
    SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id IN (SELECT user_track_ids(uid))
$$;
