-- Плагин Stream Deck: платформа streamdeck (вход как у приложений — браузер и код на свой порт, пульт в «Где играть»)
-- и выпуски плагина рядом с приложениями — target streamdeck, arch universal (один файл для Mac и Windows).

ALTER TABLE devices DROP CONSTRAINT devices_platform_check;
ALTER TABLE devices ADD CONSTRAINT devices_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos', 'streamdeck'));

ALTER TABLE app_login_requests DROP CONSTRAINT app_login_requests_platform_check;
ALTER TABLE app_login_requests ADD CONSTRAINT app_login_requests_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos', 'streamdeck'));

ALTER TABLE play_events DROP CONSTRAINT play_events_platform_check;
ALTER TABLE play_events ADD CONSTRAINT play_events_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos', 'streamdeck'));

ALTER TABLE connect_devices DROP CONSTRAINT connect_devices_platform_check;
ALTER TABLE connect_devices ADD CONSTRAINT connect_devices_platform_check
    CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android', 'watchos', 'tvos', 'streamdeck'));

-- Плагин не обновляется плагином Tauri: подписи minisign у него нет (пустая строка), файл обновления —
-- он же установщик. Пары не смешиваются: streamdeck — только universal, и наоборот.
ALTER TABLE app_releases DROP CONSTRAINT app_releases_target_check;
ALTER TABLE app_releases ADD CONSTRAINT app_releases_target_check
    CHECK (target IN ('darwin', 'windows', 'linux', 'streamdeck'));
ALTER TABLE app_releases DROP CONSTRAINT app_releases_arch_check;
ALTER TABLE app_releases ADD CONSTRAINT app_releases_arch_check
    CHECK (arch IN ('aarch64', 'x86_64', 'i686', 'armv7', 'universal'));
ALTER TABLE app_releases ADD CONSTRAINT app_releases_streamdeck_pair
    CHECK ((target = 'streamdeck') = (arch = 'universal'));
ALTER TABLE app_releases ADD CONSTRAINT app_releases_signature_present
    CHECK (signature <> '' OR target = 'streamdeck');
