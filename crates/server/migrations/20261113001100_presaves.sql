-- Предсохранение: анонсированный релиз ляжет в медиатеку в день выхода (по часовому поясу слушателя),
-- его треки встанут на скачивание, придёт уведомление «Вышел альбом — уже в медиатеке».
CREATE TABLE album_presaves (
    user_id      uuid        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    album_id     uuid        NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    -- День выхода настал и треки появились: альбом в медиатеке
    delivered_at timestamptz,
    -- Релиз пропал из источника до выхода (отменён)
    cancelled_at timestamptz,
    PRIMARY KEY (user_id, album_id)
);

-- Ждущие выхода — их обходит обслуживание воркера раз в час
CREATE INDEX album_presaves_pending ON album_presaves (album_id)
    WHERE delivered_at IS NULL AND cancelled_at IS NULL;
