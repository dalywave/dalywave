-- Клип вместо обложки: только официальный, с выбором администратора и личным «Скрыть клип».

-- Что за ролик нарезан и из каких кандидатов выбран (их видит администратор в «Клип»)
ALTER TABLE track_clips
    ADD COLUMN video_title   text,
    ADD COLUMN channel       text,
    -- Чей канал: artist / official / vevo / label; pinned — выбрал администратор
    ADD COLUMN source        text,
    ADD COLUMN score         real,
    -- До 8 последних кандидатов поиска: [{videoId, title, channel, durationS, views, source, score, rejected}]
    ADD COLUMN candidates    jsonb       NOT NULL DEFAULT '[]',
    ADD COLUMN candidates_at timestamptz,
    -- Клип проверен новым отбором (только официальное, канал автора первым). NULL — нарезан старым:
    -- воркер понемногу перепроверяет такие (video::clips::review)
    ADD COLUMN reviewed_at   timestamptz;

-- Выбор администратора: клип трека закреплён (video_id) или «без клипа» (video_id NULL). Автопоиск
-- его не перезаписывает; клип, ушедший по сроку хранения, нарезается заново из того же ролика
CREATE TABLE track_clip_pins (
    track_id  uuid PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    video_id  text,
    chosen_by uuid REFERENCES users (id) ON DELETE SET NULL,
    chosen_at timestamptz NOT NULL DEFAULT now()
);

-- «Скрыть клип» — личное: этот ролик у этого трека человеку не показывается. Это же — жалоба:
-- администратор видит треки, где скрыли клип, который сейчас нарезан и не подтверждён им позже
CREATE TABLE clip_hidden (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    track_id   uuid        NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    video_id   text        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, track_id, video_id)
);
CREATE INDEX clip_hidden_track ON clip_hidden (track_id, video_id);
