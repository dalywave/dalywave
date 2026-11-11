-- Синхронизация с приложениями и поиск без учёта регистра и диакритики.

CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS unaccent;

-- Нормализация для поиска: «Ёлка» = «елка», «Beyoncé» = «beyonce». unaccent() только STABLE, а индексу нужна
-- IMMUTABLE-функция; словарь указан явно и связывается при создании, поэтому результат зависит только от аргумента
CREATE FUNCTION search_norm(text) RETURNS text
    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
    RETURN lower(public.unaccent('public.unaccent'::regdictionary, $1));

-- Каждая синхронизируемая строка получает номер изменения из одной последовательности и id транзакции.
-- Номер задаёт порядок применения, а транзакция — полноту: номер выдаётся при записи, а видна строка
-- становится при коммите, поэтому одного номера мало (см. docs/data-model.md, «Синхронизация»).
CREATE SEQUENCE sync_rev;

CREATE FUNCTION bump_rev() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.rev := nextval('sync_rev');
    NEW.txid := pg_current_xact_id();
    RETURN NEW;
END $$;

CREATE TABLE tombstones (
    rev        bigint PRIMARY KEY DEFAULT nextval('sync_rev'),
    txid       xid8        NOT NULL DEFAULT pg_current_xact_id(),
    table_name text        NOT NULL,
    row_key    jsonb       NOT NULL,
    user_id    uuid,
    deleted_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX tombstones_user_rev ON tombstones (user_id, rev);

-- Аргументы триггера — имена ключевых колонок; user_id сохраняется, если колонка есть
CREATE FUNCTION record_tombstone() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    old_row jsonb := to_jsonb(OLD);
    key     jsonb := '{}'::jsonb;
    col     text;
BEGIN
    FOREACH col IN ARRAY TG_ARGV LOOP
        key := key || jsonb_build_object(col, old_row -> col);
    END LOOP;
    INSERT INTO tombstones (table_name, row_key, user_id)
    VALUES (TG_TABLE_NAME, key, (old_row ->> 'user_id')::uuid);
    RETURN OLD;
END $$;

-- У позиций плейлиста и загрузок устройства своего user_id нет: владелец — у плейлиста и устройства.
-- При каскадном удалении родителя его строки уже не видно: такой tombstone не нужен, приложение удаляет
-- детей вместе с родителем.
CREATE FUNCTION record_child_tombstone() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    owner uuid;
    key   jsonb;
BEGIN
    IF TG_TABLE_NAME = 'playlist_items' THEN
        SELECT user_id INTO owner FROM playlists WHERE id = OLD.playlist_id;
        key := jsonb_build_object('id', OLD.id, 'playlist_id', OLD.playlist_id);
    ELSE
        SELECT user_id INTO owner FROM devices WHERE id = OLD.device_id;
        key := jsonb_build_object('id', OLD.id, 'device_id', OLD.device_id);
    END IF;
    IF owner IS NOT NULL THEN
        INSERT INTO tombstones (table_name, row_key, user_id) VALUES (TG_TABLE_NAME, key, owner);
    END IF;
    RETURN OLD;
END $$;
