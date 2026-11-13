-- Раздел «Пользователи» админки: имя, заданное вручную, время блокировки, быстрые выборки по человеку.

ALTER TABLE users
    -- Имя задал администратор: провайдер при входе его больше не меняет
    ADD COLUMN name_locked boolean NOT NULL DEFAULT false,
    -- Когда заблокирован (вход запрещён, сессии отозваны)
    ADD COLUMN disabled_at timestamptz;

UPDATE users SET disabled_at = updated_at WHERE disabled;

-- Скачивания, которые вызвал человек, и приглашения на его странице. Заодно удаление человека
-- (ON DELETE SET NULL) не просматривает таблицы целиком
CREATE INDEX acquisitions_requested_by ON acquisitions (requested_by, created_at DESC) WHERE requested_by IS NOT NULL;
CREATE INDEX invites_created_by ON invites (created_by) WHERE created_by IS NOT NULL;
CREATE INDEX invites_used_by ON invites (used_by) WHERE used_by IS NOT NULL;
CREATE INDEX audit_events_actor ON audit_events (actor_user_id) WHERE actor_user_id IS NOT NULL;
CREATE INDEX playlist_items_added_by ON playlist_items (added_by) WHERE added_by IS NOT NULL;
