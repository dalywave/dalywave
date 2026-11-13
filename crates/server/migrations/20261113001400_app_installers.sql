-- Установщик для людей рядом с файлом автообновления: macOS — .dmg (обновляется приложение архивом
-- .app.tar.gz), Windows — тот же установщик NSIS, что и обновление. Его отдаёт `/api/downloads`
-- (меню аккаунта на сайте). Старые выпуски и конвейер без установщика — с NULL: автообновление
-- работает как раньше, а скачать такую версию с сайта нельзя.
ALTER TABLE app_releases
    ADD COLUMN installer_key text,
    ADD COLUMN installer_size bigint CHECK (installer_size > 0),
    ADD CONSTRAINT app_releases_installer_pair CHECK ((installer_key IS NULL) = (installer_size IS NULL));
