-- «Как подобран»: след подбора волны и карта звучания (2D-координаты треков и граней вкуса).

-- След одного подбора (`reco::wave::select`): источники, отсев с числами, ~50 лучших вариантов с
-- оценками и вероятностями сети, что выбрано и почему. Пишется фоном после выдачи, живёт 30 дней
-- (чистка — `reco::schedule`). Формат `data` — `reco::trace` (поле `v`).
CREATE TABLE wave_traces (
    id              uuid PRIMARY KEY,
    user_id         uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    wave_session_id uuid        NOT NULL REFERENCES wave_sessions (id) ON DELETE CASCADE,
    step            integer     NOT NULL,
    data            jsonb       NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX wave_traces_created ON wave_traces (created_at);

-- Показ — из какого подбора. Без внешнего ключа: след пишется фоном и может появиться чуть позже показа
ALTER TABLE recommendation_impressions ADD COLUMN trace_id uuid;

-- Карта звучания: UMAP по векторам Discogs-EffNet раз в сутки (`reco::map`), выровненный к прошлой карте;
-- новые треки между пересчётами — средним десяти ближайших соседей
ALTER TABLE track_analysis
    ADD COLUMN map_x real,
    ADD COLUMN map_y real;

-- Грань на карте: центр (среднее ближайших к центру грани треков) и радиус облака
ALTER TABLE taste_facets
    ADD COLUMN map_x real,
    ADD COLUMN map_y real,
    ADD COLUMN map_r real;
