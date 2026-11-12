-- «Распознать»: история песен, которые приложение узнало через ShazamKit. Строка пишется всегда,
-- даже если трек в каталоге не нашёлся: человек видит, что слышал, и может поискать сам.
-- Название и артист — как их прислал Shazam, а не из каталога: история не меняется вместе с ним.
CREATE TABLE recognitions (
    id             uuid        PRIMARY KEY DEFAULT uuidv7(),
    user_id        uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- Трек могут убрать из каталога, а запись в истории остаётся
    track_id       uuid        REFERENCES tracks (id) ON DELETE SET NULL,
    title          text        NOT NULL,
    artist         text        NOT NULL,
    isrc           text,
    apple_music_id text,
    shazam_id      text,
    artwork_url    text,
    created_at     timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX recognitions_history ON recognitions (user_id, created_at DESC, id DESC);
CREATE INDEX recognitions_track ON recognitions (track_id) WHERE track_id IS NOT NULL;
