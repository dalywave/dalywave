-- Большой экран: Apple TV (платформа tvos, вид устройства «телевизор» в «Где играть»), вход на
-- телевизоре кодом с экрана (как device flow RFC 8628), режим вечеринки и копия AAC для AVPlayer.

ALTER TABLE devices DROP CONSTRAINT devices_platform_check;
ALTER TABLE devices ADD CONSTRAINT devices_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos'));

ALTER TABLE app_login_requests DROP CONSTRAINT app_login_requests_platform_check;
ALTER TABLE app_login_requests ADD CONSTRAINT app_login_requests_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos'));

ALTER TABLE play_events DROP CONSTRAINT play_events_platform_check;
ALTER TABLE play_events ADD CONSTRAINT play_events_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos'));

ALTER TABLE connect_devices DROP CONSTRAINT connect_devices_platform_check;
ALTER TABLE connect_devices ADD CONSTRAINT connect_devices_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos'));

ALTER TABLE connect_devices DROP CONSTRAINT connect_devices_kind_check;
ALTER TABLE connect_devices ADD CONSTRAINT connect_devices_kind_check
    CHECK (kind IN ('web', 'laptop', 'desktop', 'phone', 'tablet', 'watch', 'tv'));

-- Вход на телевизоре. Телевизор держит длинный секрет (на сервере — его SHA-256) и спрашивает по нему,
-- подтверждён ли вход; человек вводит или сканирует короткий код. Код живёт 10 минут и годится один раз:
-- сессия выдаётся при первом опросе после подтверждения.
CREATE TABLE tv_logins (
    id               uuid        PRIMARY KEY DEFAULT uuidv7(),
    device_code_hash bytea       NOT NULL UNIQUE,
    -- 8 знаков без похожих (0/O, 1/I/L), хранится без дефиса
    user_code        text        NOT NULL CHECK (user_code ~ '^[A-Z2-9]{8}$'),
    platform         text        NOT NULL CHECK (platform IN ('tvos', 'web')),
    device_name      text        NOT NULL CHECK (length(device_name) BETWEEN 1 AND 64),
    created_at       timestamptz NOT NULL DEFAULT now(),
    expires_at       timestamptz NOT NULL,
    -- Кто подтвердил и как он сам вошёл (сессия телевизора наследует способ входа)
    user_id          uuid        REFERENCES users (id) ON DELETE CASCADE,
    auth_method      text        CHECK (auth_method IN ('oidc', 'password')),
    approved_at      timestamptz,
    -- Сессия выдана: код больше ничего не даёт
    used_at          timestamptz,
    session_id       uuid        REFERENCES sessions (id) ON DELETE SET NULL
);

-- Живой код с экрана — один
CREATE UNIQUE INDEX tv_logins_user_code_key ON tv_logins (user_code) WHERE used_at IS NULL;
CREATE INDEX tv_logins_expires_idx ON tv_logins (expires_at);

-- Режим вечеринки: одна строка на человека. Идёт, пока active_until в будущем.
CREATE TABLE parties (
    user_id          uuid        PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    active_until     timestamptz,
    -- Волна: только энергичное
    energetic        boolean     NOT NULL DEFAULT false,
    -- Волна: без повторов артистов
    no_artist_repeat boolean     NOT NULL DEFAULT true,
    -- Переходы «Диджей» на устройстве, где играет звук
    dj               boolean     NOT NULL DEFAULT true,
    updated_at       timestamptz NOT NULL DEFAULT now()
);

-- Копия AAC 256 кбит/с для плееров без Opus (AVPlayer на Apple TV)
ALTER TABLE audio_renditions DROP CONSTRAINT audio_renditions_kind_check;
ALTER TABLE audio_renditions ADD CONSTRAINT audio_renditions_kind_check CHECK (kind IN ('opus64', 'aac256'));
UPDATE job_lanes SET title = 'Копии файлов' WHERE lane = 'transcode' AND title = 'Сжатие для сотовой';
