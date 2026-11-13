-- Подхват на другом устройстве и оверлей «Сейчас играет» для стрима.

-- «Подхват на другом устройстве»: off | suggest («Продолжить здесь?») | auto (сразу)
ALTER TABLE user_settings
    ADD COLUMN handoff text NOT NULL DEFAULT 'suggest' CHECK (handoff IN ('off', 'suggest', 'auto'));

-- Секретная ссылка на оверлей: одна на человека, только читать «сейчас играет». Токена в базе нет:
-- он выводится из секрета сервера и `nonce` (ссылку можно скопировать снова), а поиск — по хешу.
-- «Сменить ссылку» — новый nonce: прежняя перестаёт работать
CREATE TABLE overlay_links (
    user_id      uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    nonce        bytea       NOT NULL,
    token_hash   bytea       NOT NULL UNIQUE,
    -- Не показывать, что играет, пока идёт приватная сессия (private_sessions, played_privately)
    hide_private boolean     NOT NULL DEFAULT true,
    created_at   timestamptz NOT NULL DEFAULT now()
);
