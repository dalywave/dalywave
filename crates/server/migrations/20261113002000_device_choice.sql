-- Где заиграет музыка, выбранная на другом устройстве (crates/server/src/choice.rs).
--
-- Настройки устройства видны и меняются с любого устройства:
--   choice       — «Когда выбираю музыку здесь»: автоматически, всегда здесь или там, где уже играет;
--   media_player — компьютер у колонок: пока звук на нём, выбор на других устройствах идёт сюда.
-- Присутствие — от устройства, где играет звук (раз в ~30 с, пока играет):
--   last_input_at — последний ввод (мышь, клавиатура, касание), input_source — откуда это известно
--   (система или только страница сайта), headphones — звук идёт в наушники, presence_at — когда
--   устройство сообщало о себе в последний раз (то есть ещё играло).
ALTER TABLE connect_devices
    ADD COLUMN choice        text        NOT NULL DEFAULT 'auto' CHECK (choice IN ('auto', 'here', 'there')),
    ADD COLUMN media_player  boolean     NOT NULL DEFAULT false,
    ADD COLUMN last_input_at timestamptz,
    ADD COLUMN input_source  text        CHECK (input_source IN ('system', 'page')),
    ADD COLUMN headphones    boolean     NOT NULL DEFAULT false,
    ADD COLUMN presence_at   timestamptz;

-- Пара «где выбрали → где уже играло»: последнее автоматическое решение (чтобы узнать поправку),
-- серия одинаковых поправок подряд и запомненное предпочтение (две поправки — на 12 часов).
CREATE TABLE connect_choices (
    user_id      uuid        NOT NULL,
    from_id      text        NOT NULL,
    to_id        text        NOT NULL,
    -- Последнее решение автоматики и когда; NULL — его уже поправили (или решений не было)
    decided      text        CHECK (decided IN ('here', 'there')),
    decided_at   timestamptz,
    -- Поправки подряд в одну сторону
    streak_dir   text        CHECK (streak_dir IN ('here', 'there')),
    streak       smallint    NOT NULL DEFAULT 0,
    prefer       text        CHECK (prefer IN ('here', 'there')),
    prefer_until timestamptz,
    PRIMARY KEY (user_id, from_id, to_id),
    FOREIGN KEY (user_id, from_id) REFERENCES connect_devices (user_id, id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, to_id) REFERENCES connect_devices (user_id, id) ON DELETE CASCADE
);

-- Поправка ищет последнее решение человека
CREATE INDEX connect_choices_recent ON connect_choices (user_id, decided_at DESC) WHERE decided_at IS NOT NULL;
