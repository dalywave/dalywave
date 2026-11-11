-- Этап 6, умный офлайн: каталожные строки набора волны и закреплённых артистов уходят в приложения
-- синхронизацией, у набора устройства — время сборки.

-- Плюс к прежнему: треки набора волны устройств человека и дискография закреплённых на устройстве артистов
CREATE OR REPLACE FUNCTION user_track_ids(uid uuid) RETURNS SETOF uuid LANGUAGE sql STABLE AS $$
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
    UNION
    SELECT t.id FROM device_downloads dd JOIN devices d ON d.id = dd.device_id
    JOIN artist_releases ar ON ar.artist_id = dd.ref_id JOIN tracks t ON t.album_id = ar.album_id
    WHERE d.user_id = uid AND dd.kind = 'artist'
    UNION
    SELECT ds.track_id FROM device_sets ds JOIN devices d ON d.id = ds.device_id WHERE d.user_id = uid
$$;

-- Когда и под какой запас собран набор волны устройства
CREATE TABLE device_set_builds (
    device_id uuid        PRIMARY KEY REFERENCES devices (id) ON DELETE CASCADE,
    favorites integer     NOT NULL,
    fresh     integer     NOT NULL,
    built_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX device_sets_track ON device_sets (track_id);
CREATE INDEX device_downloads_device ON device_downloads (device_id);
