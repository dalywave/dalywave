-- Облегчённые копии файлов для сотовой сети («Экономия»): Opus 64 кбит/с в MP4. Делаются воркером при
-- первом запросе, лежат в хранилище вторым файлом записи и вычищаются по давности использования, когда
-- их общий объём больше предела. Копия привязана к файлу-источнику: сменился основной файл — старая
-- копия больше не выдаётся и уходит при следующей чистке.

CREATE TABLE audio_renditions (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    source_file_id uuid        NOT NULL REFERENCES audio_files (id) ON DELETE CASCADE,
    kind           text        NOT NULL CHECK (kind IN ('opus64')),
    storage_key    text        NOT NULL UNIQUE,
    codec          text        NOT NULL,
    container      text        NOT NULL,
    bitrate_kbps   integer     NOT NULL,
    duration_ms    integer     NOT NULL,
    size_bytes     bigint      NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now(),
    last_used_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_file_id, kind)
);
CREATE INDEX audio_renditions_used ON audio_renditions (last_used_at);

-- Ключи копий, которые надо стереть из хранилища: строка ушла каскадом вместе с файлом-источником,
-- а сам объект удалит воркер
CREATE TABLE storage_trash (
    storage_key text PRIMARY KEY,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE FUNCTION audio_renditions_trash() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO storage_trash (storage_key) VALUES (OLD.storage_key) ON CONFLICT DO NOTHING;
    RETURN OLD;
END $$;
CREATE TRIGGER audio_renditions_gone AFTER DELETE ON audio_renditions FOR EACH ROW EXECUTE FUNCTION audio_renditions_trash();

INSERT INTO job_lanes (lane, title, concurrency, min_interval_ms) VALUES ('transcode', 'Сжатие для сотовой', 1, 0)
ON CONFLICT (lane) DO NOTHING;
