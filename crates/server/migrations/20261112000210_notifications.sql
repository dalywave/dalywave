-- Уведомления: центр уведомлений человека, настройки по видам и тихие часы, подписки устройств на push
-- (APNs — iPhone и iPad, Web Push — сайт, позже FCM — Android). Рассылка — задачами очереди.

CREATE TABLE notifications (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind       text        NOT NULL CHECK (kind IN ('release', 'import', 'rewind', 'review', 'sources', 'test')),
    -- Группа: пока уведомление не разослано, новые события того же вида дописываются в него
    -- («3 новых релиза» вместо трёх уведомлений)
    group_key  text,
    title      text        NOT NULL,
    body       text        NOT NULL DEFAULT '',
    -- Путь сайта (/album/…, /import); приложения открывают его как dalywave://album/…
    link       text,
    image      text,
    -- События по отдельности: [{title, subtitle, link, image}]
    items      jsonb       NOT NULL DEFAULT '[]',
    count      integer     NOT NULL DEFAULT 1 CHECK (count >= 1),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    -- Разослано: окну приложения ушло событие, push поставлены. После этого группа закрыта
    sent_at    timestamptz,
    read_at    timestamptz
);
CREATE INDEX notifications_user ON notifications (user_id, created_at DESC, id DESC);
CREATE INDEX notifications_unread ON notifications (user_id) WHERE read_at IS NULL;
-- Открытая группа у человека одна на вид
CREATE UNIQUE INDEX notifications_open_group ON notifications (user_id, kind, group_key)
    WHERE sent_at IS NULL AND group_key IS NOT NULL;

-- Однократные события: релиз, итоги года, импорт. Второй раз уведомление о них не создаётся,
-- даже если первое уже удалено чисткой
CREATE TABLE notification_marks (
    user_id    uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    key        text        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, key)
);

-- Настройки; строки нет — всё по умолчанию
CREATE TABLE notification_settings (
    user_id        uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    -- Выключенные виды
    disabled_kinds text[]      NOT NULL DEFAULT '{}',
    -- Тихие часы: минуты от полуночи по часовому поясу человека; from > to — через полночь
    quiet_enabled  boolean     NOT NULL DEFAULT true,
    quiet_from     smallint    NOT NULL DEFAULT 1380 CHECK (quiet_from BETWEEN 0 AND 1439),
    quiet_to       smallint    NOT NULL DEFAULT 480 CHECK (quiet_to BETWEEN 0 AND 1439),
    -- Часовой пояс (минуты к UTC) последнего устройства; NULL — пояс сервиса (reco_settings)
    utc_offset_min smallint CHECK (utc_offset_min BETWEEN -840 AND 840),
    updated_at     timestamptz NOT NULL DEFAULT now()
);

-- Подписки устройств на push. Подписка живёт, пока жива сессия, из которой она пришла: выход или
-- отзыв устройства в «Доступе» её выключают. «Токен недействителен» от APNs или 404/410 от службы
-- Web Push — подписка выключается (disabled_at) и больше не используется
CREATE TABLE push_subscriptions (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id         uuid        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    session_id      uuid        REFERENCES sessions (id) ON DELETE CASCADE,
    channel         text        NOT NULL CHECK (channel IN ('apns', 'webpush', 'fcm')),
    -- APNs и FCM — токен устройства, Web Push — адрес подписки (endpoint)
    token           text        NOT NULL,
    -- Web Push: ключи шифрования браузера (base64url)
    p256dh          text,
    auth            text,
    -- APNs: среда сборки
    environment     text CHECK (environment IN ('sandbox', 'production')),
    name            text        NOT NULL DEFAULT '',
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    last_sent_at    timestamptz,
    disabled_at     timestamptz,
    disabled_reason text,
    UNIQUE (channel, token),
    CHECK (channel <> 'webpush' OR (p256dh IS NOT NULL AND auth IS NOT NULL))
);
CREATE INDEX push_subscriptions_user ON push_subscriptions (user_id) WHERE disabled_at IS NULL;
CREATE INDEX push_subscriptions_session ON push_subscriptions (session_id);

INSERT INTO job_lanes (lane, title, concurrency, min_interval_ms) VALUES ('notify', 'Уведомления', 4, 0)
ON CONFLICT (lane) DO NOTHING;
