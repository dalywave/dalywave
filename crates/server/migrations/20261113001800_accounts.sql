-- Вход без привязки к daly.rest: несколько провайдеров OIDC, свои аккаунты (логин и пароль),
-- приглашения и одноразовые ссылки сброса пароля.
--
-- Совместимо с прежним кодом: users.issuer и users.subject остаются и заполняются при создании
-- человека через провайдера, как раньше. Новый код ищет человека по user_identities.

-- Входы через провайдеров: у человека их может быть несколько (свой аккаунт + daly.rest + Google)
CREATE TABLE user_identities (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    issuer text NOT NULL,
    subject text NOT NULL,
    -- Почта, которую провайдер прислал при последнем входе (для списка в настройках)
    email text,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz,
    UNIQUE (issuer, subject)
);

CREATE INDEX user_identities_user_idx ON user_identities (user_id);

INSERT INTO user_identities (user_id, issuer, subject, email, created_at, last_login_at)
SELECT id, issuer, subject, email, created_at, last_login_at
FROM users
WHERE issuer IS NOT NULL;

-- Свой аккаунт: логин (в нижнем регистре) и хеш пароля argon2id
ALTER TABLE users
    ADD COLUMN username text,
    ADD COLUMN password_hash text,
    ADD COLUMN password_changed_at timestamptz,
    -- Какой провайдер (issuer) выдал admin: снимает его только он, вход через другой провайдер роль не трогает
    ADD COLUMN idp_admin_issuer text;

CREATE UNIQUE INDEX users_username_key ON users (username) WHERE username IS NOT NULL;
ALTER TABLE users ADD CONSTRAINT users_username_lower CHECK (username = lower(username));

-- До этой миграции admin мог выдать только daly.rest — тот провайдер, через который человек вошёл
UPDATE users SET idp_admin_issuer = issuer WHERE idp_admin;

-- Приглашение: ссылка с ролями и сроком. В базе — только SHA-256 токена
CREATE TABLE invites (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    token_hash bytea NOT NULL UNIQUE,
    roles text[] NOT NULL DEFAULT '{}',
    -- Подпись для админа: кому отправили
    note text,
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    used_at timestamptz,
    used_by uuid REFERENCES users (id) ON DELETE SET NULL,
    revoked_at timestamptz
);

CREATE INDEX invites_expires_idx ON invites (expires_at);

-- Одноразовая ссылка сброса (или первой установки) пароля, которую выдаёт админ
CREATE TABLE password_resets (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    created_by uuid REFERENCES users (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    used_at timestamptz
);

CREATE INDEX password_resets_user_idx ON password_resets (user_id);

-- Как человек вошёл в браузере, начав вход приложения: сессия приложения наследует способ
ALTER TABLE app_login_requests ADD COLUMN auth_method text
    CHECK (auth_method IN ('oidc', 'password', 'local'));

-- Новый способ подтверждения входа: свой аккаунт (логин и пароль)
ALTER TABLE sessions DROP CONSTRAINT sessions_auth_method_check;
ALTER TABLE sessions ADD CONSTRAINT sessions_auth_method_check
    CHECK (auth_method IN ('oidc', 'password', 'local'));
ALTER TABLE tv_logins DROP CONSTRAINT tv_logins_auth_method_check;
ALTER TABLE tv_logins ADD CONSTRAINT tv_logins_auth_method_check
    CHECK (auth_method IN ('oidc', 'password', 'local'));
