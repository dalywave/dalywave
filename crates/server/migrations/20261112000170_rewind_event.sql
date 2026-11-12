-- «Итоги года» как событие сервера: когда истории открыты и в каком стиле их показывать.
CREATE TABLE rewind_event (
    id         boolean     PRIMARY KEY DEFAULT true CHECK (id),
    -- wave — «Волна года», spread — «Разворот», record — «Пластинка», random — свой у каждого слушателя
    style      text        NOT NULL DEFAULT 'random' CHECK (style IN ('wave', 'spread', 'record', 'random')),
    -- auto — с 1 декабря по 15 января; open — открыто сейчас за year; closed — закрыто
    mode       text        NOT NULL DEFAULT 'auto' CHECK (mode IN ('auto', 'open', 'closed')),
    year       integer     CHECK (year >= 2000),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (mode <> 'open' OR year IS NOT NULL)
);
INSERT INTO rewind_event DEFAULT VALUES;

-- Кто уже посмотрел итоги года: до этого главная зовёт карточкой «Твои итоги готовы»
CREATE TABLE rewind_seen (
    user_id uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    year    integer     NOT NULL,
    seen_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, year)
);
