-- Встраиваемый плеер для чужих сайтов (/embed/{kind}/{id}).

-- «Можно встраивать»: плейлист открывается без входа только с разрешения владельца
ALTER TABLE playlists ADD COLUMN embeddable boolean NOT NULL DEFAULT false;

-- Отрывки 30 с с припева (AAC в MP4): без входа отдаётся только отрывок, не трек целиком.
-- Нарезается воркером при первом запросе; привязан к файлу-источнику — ушёл файл, уходит и отрывок
-- (объект в хранилище стирает чистка копий через storage_trash)
CREATE TABLE embed_clips (
    track_id       uuid PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    source_file_id uuid        NOT NULL REFERENCES audio_files (id) ON DELETE CASCADE,
    storage_key    text        NOT NULL UNIQUE,
    start_ms       integer     NOT NULL,
    length_ms      integer     NOT NULL,
    size_bytes     bigint      NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX embed_clips_source ON embed_clips (source_file_id);
CREATE TRIGGER embed_clips_gone AFTER DELETE ON embed_clips FOR EACH ROW EXECUTE FUNCTION audio_renditions_trash();
