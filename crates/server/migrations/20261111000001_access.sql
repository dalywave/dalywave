-- Доступ: пользователи, устройства, сессии, вход приложений и журнал админки.

-- Векторы понадобятся рекомендациям; расширение включаем сразу, образ pgvector его содержит.
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE users (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    -- Пара (issuer, subject) из daly.rest. У локального владельца из аварийного входа обе пустые
    issuer text,
    subject text,
    email text,
    email_verified boolean NOT NULL DEFAULT false,
    name text NOT NULL DEFAULT '',
    avatar_url text,
    roles text[] NOT NULL DEFAULT '{}',
    -- admin выдан провайдером: снимается, когда провайдер перестанет его присылать
    idp_admin boolean NOT NULL DEFAULT false,
    disabled boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz,
    CONSTRAINT users_identity_pair CHECK ((issuer IS NULL) = (subject IS NULL))
);

CREATE UNIQUE INDEX users_issuer_subject_key ON users (issuer, subject);
CREATE UNIQUE INDEX users_email_key ON users (lower(email)) WHERE email IS NOT NULL;

CREATE TABLE devices (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name text NOT NULL,
    platform text NOT NULL
        CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android')),
    app_version text,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz,
    revoked_at timestamptz
);

CREATE INDEX devices_user_idx ON devices (user_id);

CREATE TABLE sessions (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id uuid REFERENCES devices (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('web', 'device')),
    -- Как человек подтвердил вход: через daly.rest или аварийным паролем
    auth_method text NOT NULL CHECK (auth_method IN ('oidc', 'password')),
    -- SHA-256 токена; сам токен на сервере не хранится
    token_hash bytea NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    user_agent text,
    ip inet,
    CONSTRAINT sessions_device_kind CHECK ((kind = 'device') = (device_id IS NOT NULL))
);

-- Раздел «Доступ» показывает живые сессии, в том числе одного пользователя
CREATE INDEX sessions_user_active_idx ON sessions (user_id) WHERE revoked_at IS NULL;
CREATE INDEX sessions_expires_idx ON sessions (expires_at);

-- Вход приложения через системный браузер: запрос, затем одноразовый код на 60 секунд
CREATE TABLE app_login_requests (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    redirect_uri text NOT NULL,
    state text NOT NULL,
    code_challenge text NOT NULL,
    device_name text NOT NULL,
    platform text NOT NULL
        CHECK (platform IN ('web', 'macos', 'windows', 'linux', 'ios', 'ipados', 'android')),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    user_id uuid REFERENCES users (id) ON DELETE CASCADE,
    code_hash bytea UNIQUE,
    code_expires_at timestamptz,
    used_at timestamptz,
    -- Сессия, выданная по коду: отзывается, если код предъявят повторно
    session_id uuid REFERENCES sessions (id) ON DELETE SET NULL
);

CREATE INDEX app_login_requests_expires_idx ON app_login_requests (expires_at);

-- Журнал админки. Порядок — по id: uuidv7 растёт со временем
CREATE TABLE audit_events (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    at timestamptz NOT NULL DEFAULT now(),
    actor_user_id uuid REFERENCES users (id) ON DELETE SET NULL,
    kind text NOT NULL,
    subject text,
    payload jsonb NOT NULL DEFAULT '{}'
);
