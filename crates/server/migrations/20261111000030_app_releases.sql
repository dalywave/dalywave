-- Выпуски приложений для автообновления (tauri-plugin-updater). Файл обновления (.app.tar.gz,
-- установщик NSIS) лежит в хранилище, здесь — его ключ и подпись из `.sig`. Регистрирует конвейер
-- сборки токеном выпуска; приложение получает самый новый выпуск своей платформы.
CREATE TABLE app_releases (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    target text NOT NULL CHECK (target IN ('darwin', 'windows', 'linux')),
    arch text NOT NULL CHECK (arch IN ('aarch64', 'x86_64', 'i686', 'armv7')),
    -- SemVer без «v»: порядок версий считает сервер, а не сортировка строк
    version text NOT NULL,
    storage_key text NOT NULL,
    size_bytes bigint NOT NULL CHECK (size_bytes > 0),
    signature text NOT NULL,
    notes text,
    published_at timestamptz NOT NULL DEFAULT now(),
    -- Повторная сборка той же версии заменяет выпуск; уникальность заодно индекс для проверки
    UNIQUE (target, arch, version)
);
