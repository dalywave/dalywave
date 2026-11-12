-- «Недавно прослушанное» и «Неверная запись».

-- «Очистить историю» и «Убрать из истории» прячут прослушивание только с экрана истории: вкус, миксы,
-- «На повторе» и «Итоги года» его по-прежнему учитывают
ALTER TABLE play_events ADD COLUMN hidden_at timestamptz;
CREATE INDEX play_events_user_visible ON play_events (user_id, started_at DESC, id DESC) WHERE hidden_at IS NULL;

-- Жалобы слушателя на основной файл записи: что не так, какое видео было, спорная запись в админке
-- и новое скачивание. Видео из всех жалоб на запись больше не берутся
CREATE TABLE recording_reports (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    recording_id   uuid        NOT NULL REFERENCES recordings (id) ON DELETE CASCADE,
    track_id       uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    user_id        uuid        REFERENCES users (id) ON DELETE SET NULL,
    reason         text        NOT NULL CHECK (reason IN ('wrong_song', 'wrong_version', 'bad_quality', 'cut')),
    comment        text,
    file_id        uuid        REFERENCES audio_files (id) ON DELETE SET NULL,
    video_id       text,
    dispute_id     uuid        REFERENCES acquisitions (id) ON DELETE SET NULL,
    acquisition_id uuid        REFERENCES acquisitions (id) ON DELETE SET NULL,
    created_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX recording_reports_recording ON recording_reports (recording_id, created_at DESC);
