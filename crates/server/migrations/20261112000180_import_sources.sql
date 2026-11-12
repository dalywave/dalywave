-- Перенос из «Яндекс Музыки» (плейлист по ссылке или лайки и плейлисты аккаунта) и YouTube Music
-- (плейлист по ссылке, архив Google Takeout). Позиции те же: yt:<id видео> и ym:<id трека> в source_ref
-- служат ключом кеша сопоставлений.

ALTER TABLE imports DROP CONSTRAINT imports_kind_check;
ALTER TABLE imports ADD CONSTRAINT imports_kind_check
    CHECK (kind IN ('text', 'csv', 'm3u', 'files', 'spotify_archive', 'yandex', 'youtube_playlist',
                    'youtube_takeout'));
