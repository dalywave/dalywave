-- Админка: настройки сервера и расписание полос.

-- Настройки сервера одной строкой. Подписка без своей политики докачки берёт эту
CREATE TABLE server_settings (
    id            boolean     PRIMARY KEY DEFAULT true CHECK (id),
    follow_policy jsonb       NOT NULL DEFAULT '{"albums": true, "eps": true, "singles": true, "compilations": false, "live": false, "appearsOn": false}',
    updated_at    timestamptz NOT NULL DEFAULT now()
);
INSERT INTO server_settings DEFAULT VALUES;

-- Окно работы полосы в минутах суток по часовому поясу владельца (reco_settings.utc_offset_min):
-- вне окна полоса ведёт себя как на паузе. Окно может переходить через полночь (с 23:00 до 07:00).
-- NULL в обоих — круглосуточно
ALTER TABLE job_lanes
    ADD COLUMN active_from  smallint CHECK (active_from BETWEEN 0 AND 1439),
    ADD COLUMN active_until smallint CHECK (active_until BETWEEN 0 AND 1439),
    ADD CONSTRAINT job_lanes_window CHECK (
        (active_from IS NULL AND active_until IS NULL)
        OR (active_from IS NOT NULL AND active_until IS NOT NULL AND active_from <> active_until)
    );

-- Открыта ли полоса с окном [from, until) прямо сейчас
CREATE FUNCTION lane_open(from_min smallint, until_min smallint) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT CASE
        WHEN from_min IS NULL OR until_min IS NULL THEN true
        ELSE (
            WITH m AS (
                SELECT ((EXTRACT(HOUR FROM now() AT TIME ZONE 'UTC') * 60
                         + EXTRACT(MINUTE FROM now() AT TIME ZONE 'UTC')
                         + COALESCE((SELECT utc_offset_min FROM reco_settings WHERE id), 0))::int % 1440 + 1440) % 1440 AS v
            )
            SELECT CASE WHEN from_min < until_min THEN v >= from_min AND v < until_min
                        ELSE v >= from_min OR v < until_min END
            FROM m
        )
    END
$$;
