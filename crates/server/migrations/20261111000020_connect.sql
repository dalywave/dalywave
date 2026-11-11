-- Управление между устройствами (этап 3). Каждое приложение и вкладка сайта — устройство «пульта»:
-- у вкладки нет строки в devices (она входит cookie сайта), поэтому список — отдельная таблица
-- с id, который придумывает сам клиент (постоянный у приложения, на время жизни у вкладки).
CREATE TABLE connect_devices (
    user_id      uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id           text        NOT NULL CHECK (length(id) BETWEEN 1 AND 64),
    -- Приложение с токеном устройства: имя берётся оттуда, отзыв устройства убирает и строку
    device_id    uuid        REFERENCES devices (id) ON DELETE CASCADE,
    name         text        NOT NULL,
    kind         text        NOT NULL CHECK (kind IN ('web', 'laptop', 'desktop', 'phone', 'tablet')),
    platform     text        NOT NULL
        CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android')),
    app_version  text,
    capabilities jsonb       NOT NULL DEFAULT '{}',
    -- Открытое соединение, которому принадлежит устройство; NULL — не подключено. При переподключении
    -- новое соединение забирает строку, и закрытие старого её уже не трогает
    connection   uuid,
    -- «В сети» = соединение есть и устройство напоминало о себе последние 15 секунд
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    created_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, id)
);

-- Активное устройство — id из connect_devices (вкладка сайта тоже бывает активной). Старая колонка
-- active_device_id (только приложения) остаётся пустой
ALTER TABLE playback_state ADD COLUMN active_connect_id text;
