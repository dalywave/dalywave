-- «Итоги года»: посчитанный год слушателя. Кеш, а не источник правды: строка пересчитывается, когда
-- меняется отпечаток (число и время прослушиваний года, лайки), а в декабре — ещё и ночью заранее.
-- Хранятся только id и числа: треки, артисты и обложки подставляются свежими при ответе.
CREATE TABLE rewinds (
    user_id     uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    year        integer     NOT NULL CHECK (year BETWEEN 2000 AND 2200),
    fingerprint text        NOT NULL,
    data        jsonb       NOT NULL,
    computed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, year)
);
