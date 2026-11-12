-- Настройки слушателя, общие для всех его устройств: фильтр нецензурного и качество звука.
-- Одна строка на человека; синхронизируется с приложениями, как медиатека.

CREATE TABLE user_settings (
    user_id          uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    -- «Без нецензурного»: explicit не играет в волне, миксах и автопродолжении, в списках — серым
    hide_explicit    boolean     NOT NULL DEFAULT false,
    -- Качество по сети: Wi-Fi — high (lossless, если есть) | normal; сотовая — normal | saver (Opus 64)
    quality_wifi     text        NOT NULL DEFAULT 'high' CHECK (quality_wifi IN ('high', 'normal')),
    quality_cellular text        NOT NULL DEFAULT 'normal' CHECK (quality_cellular IN ('normal', 'saver')),
    quality_download text        NOT NULL DEFAULT 'high' CHECK (quality_download IN ('high', 'normal')),
    -- Порядок плейлистов в медиатеке и боковой панели
    playlist_sort    text        NOT NULL DEFAULT 'recent' CHECK (playlist_sort IN ('recent', 'title', 'custom')),
    updated_at       timestamptz NOT NULL DEFAULT now(),
    rev              bigint      NOT NULL DEFAULT nextval('sync_rev'),
    txid             xid8        NOT NULL DEFAULT pg_current_xact_id()
);
CREATE TRIGGER user_settings_rev BEFORE UPDATE ON user_settings FOR EACH ROW EXECUTE FUNCTION bump_rev();

-- Строки нет — значит, всё по умолчанию: читать можно без INSERT при входе
CREATE FUNCTION hide_explicit(uid uuid) RETURNS boolean LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT COALESCE((SELECT hide_explicit FROM user_settings WHERE user_id = uid), false)
$$;

-- Чистая версия explicit-трека: то же название и основной артист, без пометки, длительность рядом.
-- Готовая к воспроизведению — первой, затем популярная. Ищется по btree на нормализованном названии.
CREATE INDEX tracks_clean_title ON tracks (search_norm(title)) WHERE NOT explicit;

CREATE FUNCTION clean_version(tid uuid) RETURNS uuid LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT c.id
    FROM tracks t
    JOIN track_artists ta ON ta.track_id = t.id AND ta.position = 0
    JOIN tracks c ON search_norm(c.title) = search_norm(t.title) AND NOT c.explicit AND c.id <> t.id
    JOIN track_artists ca ON ca.track_id = c.id AND ca.position = 0 AND ca.artist_id = ta.artist_id
    LEFT JOIN recordings r ON r.id = c.recording_id
    WHERE t.id = tid AND t.explicit
      AND abs(c.duration_ms - t.duration_ms) <= 15000
      -- Ремикс или концертная запись — другая песня, а не чистая версия
      AND (c.title_version = t.title_version OR search_norm(c.title_version) ~ '(clean|edit|censored)')
    ORDER BY (r.audio_status = 'ready') DESC NULLS LAST, c.rank DESC NULLS LAST, c.id
    LIMIT 1
$$;
