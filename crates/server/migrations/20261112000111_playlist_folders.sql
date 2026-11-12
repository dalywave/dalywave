-- Папки плейлистов и свой порядок. Папка — один уровень, без вложенных. В «Своём порядке» папки и
-- плейлисты вне папок делят одну шкалу позиций; внутри папки — своя. Позиции дробные, как у треков
-- плейлиста: перестановка меняет одну строку.

CREATE TABLE playlist_folders (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id    uuid             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title      text             NOT NULL CHECK (char_length(title) BETWEEN 1 AND 100),
    position   double precision NOT NULL,
    -- Свёрнута ли в боковой панели и медиатеке — общее для всех устройств
    collapsed  boolean          NOT NULL DEFAULT false,
    created_at timestamptz      NOT NULL DEFAULT now(),
    updated_at timestamptz      NOT NULL DEFAULT now(),
    rev        bigint           NOT NULL DEFAULT nextval('sync_rev'),
    txid       xid8             NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX playlist_folders_user ON playlist_folders (user_id, position);
CREATE TRIGGER playlist_folders_rev BEFORE UPDATE ON playlist_folders FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER playlist_folders_gone AFTER DELETE ON playlist_folders FOR EACH ROW EXECUTE FUNCTION record_tombstone('id', 'user_id');

-- Удалили папку — плейлисты выходят из неё (SET NULL поднимает их rev триггером playlists_rev)
ALTER TABLE playlists
    ADD COLUMN folder_id uuid REFERENCES playlist_folders (id) ON DELETE SET NULL,
    ADD COLUMN position  double precision;
CREATE INDEX playlists_folder ON playlists (folder_id) WHERE folder_id IS NOT NULL;

-- Свой порядок для уже созданных: новые сверху, как было
UPDATE playlists p SET position = x.rn * 1024.0
FROM (SELECT id, row_number() OVER (PARTITION BY user_id ORDER BY pinned DESC, updated_at DESC, id DESC) AS rn
      FROM playlists) x
WHERE p.id = x.id;
ALTER TABLE playlists ALTER COLUMN position SET NOT NULL;

-- Новый плейлист (свой, микс, импорт, «Итоги года») без позиции встаёт первым вне папок
CREATE FUNCTION playlists_place() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.position IS NULL THEN
        NEW.position := COALESCE(LEAST(
            (SELECT min(position) FROM playlists WHERE user_id = NEW.user_id AND folder_id IS NULL),
            (SELECT min(position) FROM playlist_folders WHERE user_id = NEW.user_id)), 1024) - 1024;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER playlists_place BEFORE INSERT ON playlists FOR EACH ROW EXECUTE FUNCTION playlists_place();

-- «Недавние»: когда в последний раз играл плейлист
CREATE INDEX play_events_user_context ON play_events (user_id, context_id, started_at DESC)
    WHERE context_type IN ('playlist', 'mix');
