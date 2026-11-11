-- «Мой вкус»: ручки слушателя над рекомендациями — «меньше такого» и «сбросить вкус».

-- Сброс вкуса: прослушивания и лайки до reset_at грани вкуса больше не строят (медиатека остаётся)
CREATE TABLE taste_profiles (
    user_id    uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    reset_at   timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- С какого момента события слушателя строят вкус: последний сброс или «всегда»
CREATE FUNCTION taste_reset_at(uid uuid) RETURNS timestamptz
LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT COALESCE((SELECT reset_at FROM taste_profiles WHERE user_id = uid), '-infinity'::timestamptz)
$$;

-- «Меньше такого»: направление звучания (центр грани на момент нажатия), которое волна, миксы и
-- веса граней обходят. Своя таблица, а не колонка грани: ночной пересчёт собирает грани заново,
-- а приглушение должно пережить его
CREATE TABLE taste_dampers (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id    uuid          NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    label      text          NOT NULL,
    centroid   halfvec(1280) NOT NULL,
    strength   real          NOT NULL CHECK (strength > 0 AND strength <= 1),
    examples   uuid[]        NOT NULL DEFAULT '{}',
    created_at timestamptz   NOT NULL DEFAULT now(),
    updated_at timestamptz   NOT NULL DEFAULT now()
);
CREATE INDEX taste_dampers_user ON taste_dampers (user_id, updated_at DESC);

-- Насколько вектор приглушён «меньше такого» слушателя, 0..1: сила ближайшего приглушения × рампа по
-- сходству от lo (ничего) до hi (полностью). Треки сравниваются мягче (центры граней гладкие, а у
-- EffNet почти всё похоже друг на друга на 0,6–0,7), грани — строже
CREATE FUNCTION taste_damp(uid uuid, v halfvec, lo real, hi real) RETURNS real
LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT COALESCE(max(d.strength * LEAST(1, GREATEST(0, ((1 - (d.centroid <=> v)) - lo) / (hi - lo)))), 0)::real
    FROM taste_dampers d WHERE d.user_id = uid
$$;
