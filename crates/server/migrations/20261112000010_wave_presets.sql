-- Сохранённые сочетания настроек волны: карточки под «Моей волной», запуск в одно нажатие.
-- Синхронизируются с приложениями, как медиатека; заготовки волны (wave_pools) держатся и под них.

CREATE TABLE wave_presets (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id    uuid             NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name       text             NOT NULL CHECK (char_length(name) BETWEEN 1 AND 40),
    icon       text             NOT NULL CHECK (icon ~ '^[a-z0-9-]{1,40}$'),
    settings   jsonb            NOT NULL,
    -- Дробная, как у позиций плейлиста: перестановка меняет одну строку
    position   double precision NOT NULL,
    created_at timestamptz      NOT NULL DEFAULT now(),
    rev        bigint           NOT NULL DEFAULT nextval('sync_rev'),
    txid       xid8             NOT NULL DEFAULT pg_current_xact_id()
);
CREATE INDEX wave_presets_user ON wave_presets (user_id, position);
CREATE TRIGGER wave_presets_rev BEFORE UPDATE ON wave_presets FOR EACH ROW EXECUTE FUNCTION bump_rev();
CREATE TRIGGER wave_presets_gone AFTER DELETE ON wave_presets FOR EACH ROW EXECUTE FUNCTION record_tombstone('id', 'user_id');

-- Три сочетания для начала: ряд под волной не пустой с первого входа. Удалённые не возвращаются —
-- засевается только новый человек (и все, кто был до этой миграции)
CREATE FUNCTION seed_wave_presets(uid uuid) RETURNS void LANGUAGE sql AS $$
    INSERT INTO wave_presets (user_id, name, icon, settings, position) VALUES
        (uid, 'Работа', 'laptop', '{"activity": "work", "character": "mix", "mood": "calm", "language": "none"}', 1024),
        (uid, 'Дорога', 'car', '{"activity": "road", "character": "fav", "mood": "energetic", "language": "ru"}', 2048),
        (uid, 'Ночь', 'moon-stars', '{"activity": "rest", "character": "rare", "mood": "sad", "language": "any"}', 3072)
$$;

CREATE FUNCTION seed_wave_presets_for_new_user() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM seed_wave_presets(NEW.id);
    RETURN NEW;
END $$;
CREATE TRIGGER users_wave_presets AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION seed_wave_presets_for_new_user();

SELECT seed_wave_presets(id) FROM users;
