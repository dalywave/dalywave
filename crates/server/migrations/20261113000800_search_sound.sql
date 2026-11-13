-- Поиск «как слышится»: «капсайз» находит Capsize, «мзлф» — Mzlff, «скриптонит» — и Скриптонит, и
-- Scriptonite. Функции повторяют crates/textmatch шаг в шаг (text.rs — search_latin, sound.rs — sound_key);
-- тест tests/api/search.rs сверяет их на именах из базы. Менять — в обоих местах и новой миграцией.

-- Латиница по звучанию: «Скриптонит» — skriptonit, «Хаски» — haski. Остальное — как search_norm
CREATE FUNCTION search_latin(text) RETURNS text
    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT
    RETURN translate(
        replace(replace(replace(replace(replace(replace(replace(replace(replace(search_norm($1),
            'ё', 'е'), 'ж', 'zh'), 'х', 'h'), 'ц', 'ts'), 'ч', 'ch'), 'ш', 'sh'), 'щ', 'sh'), 'ю', 'yu'), 'я', 'ya'),
        'абвгдезийклмнопрстуфыэіїґєўъь',
        'abvgdeziiklmnoprstufyeiigeu');

-- Ключ звучания — только согласные упрощённого произношения: Capsize и «капсайз» — kps
CREATE FUNCTION sound_key(text) RETURNS text
    LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE STRICT AS $$
DECLARE
    s text := regexp_replace(search_latin($1), '[^a-z0-9]+', ' ', 'g');
BEGIN
    s := regexp_replace(s, '([a-z])\1+', '\1', 'g');
    s := replace(s, 'dzh', 'g');
    s := replace(s, 'dj', 'g');
    s := replace(s, 'zh', 'g');
    s := replace(s, 'tch', 'ch');
    s := replace(s, 'sch', 'sh');
    s := replace(s, 'ch', 'C');
    s := replace(s, 'ck', 'k');
    s := replace(s, 'ph', 'f');
    s := replace(s, 'th', 't');
    s := replace(s, 'kh', 'h');
    s := replace(s, 'gh', 'g');
    s := replace(s, 'sh', 's');
    s := replace(s, 'qu', 'kv');
    s := regexp_replace(s, 'c(?=[eiy])', 's', 'g');
    s := replace(s, 'c', 'k');
    s := replace(s, 'C', 'c');
    s := translate(s, 'qjz', 'kgs');
    s := replace(s, 'x', 'ks');
    s := regexp_replace(s, 'w(?=[aeiouy])', 'v', 'g');
    s := replace(s, 'w', 'u');
    s := regexp_replace(s, 'h(?![aeiouy])', '', 'g');
    s := translate(s, 'aeiouy', '');
    s := regexp_replace(s, '([a-z])\1+', '\1', 'g');
    RETURN btrim(regexp_replace(s, ' +', ' ', 'g'));
END $$;

-- Считаются при записи: поиск по медиатеке складывает их для трека с артистами, не пересчитывая
ALTER TABLE artists
    ADD COLUMN latin text GENERATED ALWAYS AS (search_latin(name)) STORED,
    ADD COLUMN sound text GENERATED ALWAYS AS (sound_key(name)) STORED;
ALTER TABLE albums
    ADD COLUMN latin text GENERATED ALWAYS AS (search_latin(title)) STORED,
    ADD COLUMN sound text GENERATED ALWAYS AS (sound_key(title)) STORED;
ALTER TABLE tracks
    ADD COLUMN latin text GENERATED ALWAYS AS (search_latin(title || ' ' || title_version)) STORED,
    ADD COLUMN sound text GENERATED ALWAYS AS (sound_key(title || ' ' || title_version)) STORED;

-- Артисты, уже известные серверу, по звучанию и по латинице с опечатками — без похода в Deezer
CREATE INDEX artists_sound ON artists (sound text_pattern_ops);
CREATE INDEX artists_latin_trgm ON artists USING gin (latin gin_trgm_ops);
