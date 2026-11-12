-- Apple Watch: платформа watchos, вид устройства «часы» в «Где играть» и сопряжённые устройства.
-- Своего входа у часов нет: сессию им выдаёт уже вошедший iPhone (POST /api/auth/app/pair).
-- Отзыв iPhone отзывает и его часы.

ALTER TABLE devices ADD COLUMN paired_from uuid REFERENCES devices (id) ON DELETE CASCADE;
CREATE INDEX devices_paired_from_idx ON devices (paired_from) WHERE paired_from IS NOT NULL;

ALTER TABLE devices DROP CONSTRAINT devices_platform_check;
ALTER TABLE devices ADD CONSTRAINT devices_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos'));

ALTER TABLE app_login_requests DROP CONSTRAINT app_login_requests_platform_check;
ALTER TABLE app_login_requests ADD CONSTRAINT app_login_requests_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos'));

ALTER TABLE play_events DROP CONSTRAINT play_events_platform_check;
ALTER TABLE play_events ADD CONSTRAINT play_events_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos'));

ALTER TABLE connect_devices DROP CONSTRAINT connect_devices_platform_check;
ALTER TABLE connect_devices ADD CONSTRAINT connect_devices_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos'));

ALTER TABLE connect_devices DROP CONSTRAINT connect_devices_kind_check;
ALTER TABLE connect_devices ADD CONSTRAINT connect_devices_kind_check
    CHECK (kind IN ('web', 'laptop', 'desktop', 'phone', 'tablet', 'watch'));
