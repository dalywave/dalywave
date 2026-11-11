-- Заготовки волны: под частые настройки слушателя (свои сочетания за месяц, сохранённые, «Моя волна»)
-- несколько подобранных треков держатся на сервере, чтобы волна начиналась сразу. Пересчёт — раз в час
-- (задача wave_pool): использованное, услышанное и устаревшее уходит, недостающее подбирается заново.
-- Подготовка начала самой сессии хранится в wave_sessions.state (prep) и миграции не требует.
CREATE TABLE wave_pools (
    user_id   uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- занятие:характер:настроение:язык[:темп бега]
    combo     text        NOT NULL,
    track_id  uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    settings  jsonb       NOT NULL,
    score     real,
    picked_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, combo, track_id)
);
-- Взятое в сессию уходит из заготовок всех настроек; услышанное и устаревшее — по сроку
CREATE INDEX wave_pools_track ON wave_pools (track_id);
CREATE INDEX wave_pools_picked ON wave_pools (picked_at);
-- Суточный предел заготовок: скачивания слушателя в полосе «Открытия»
CREATE INDEX acquisitions_requested ON acquisitions (requested_by, created_at DESC) WHERE requested_by IS NOT NULL;
