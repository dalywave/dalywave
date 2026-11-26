-- Общий аккаунт публичного демо (DALYWAVE_DEMO_ACCOUNT): гости не видят устройств друг друга и не
-- переключают друг другу музыку. «Где играть» и «что играет» теперь хранятся по области (scope):
-- у обычного человека это его user_id, у гостя демо — его вход (id корневой сессии, см. ниже).

-- Сессия, выданная от другой сессии гостя демо (часы от iPhone, телевизор по коду с его сайта):
-- корневая сессия этого входа. У самого входа и у всех обычных сессий — NULL
ALTER TABLE sessions ADD COLUMN guest uuid;
ALTER TABLE tv_logins ADD COLUMN guest uuid;

ALTER TABLE connect_devices ADD COLUMN scope uuid;
UPDATE connect_devices SET scope = user_id;
ALTER TABLE connect_devices ALTER COLUMN scope SET NOT NULL;

-- Решения «где играть» — по той же области, что и устройства
ALTER TABLE connect_choices DROP CONSTRAINT connect_choices_user_id_from_id_fkey;
ALTER TABLE connect_choices DROP CONSTRAINT connect_choices_user_id_to_id_fkey;
ALTER TABLE connect_devices DROP CONSTRAINT connect_devices_pkey;
ALTER TABLE connect_devices ADD PRIMARY KEY (scope, id);
-- Админка «Сейчас» и «Доступ» ищут устройства человека во всех его областях
CREATE INDEX connect_devices_user ON connect_devices (user_id, last_seen_at DESC);
ALTER TABLE connect_choices RENAME COLUMN user_id TO scope;
ALTER TABLE connect_choices
    ADD FOREIGN KEY (scope, from_id) REFERENCES connect_devices (scope, id) ON DELETE CASCADE,
    ADD FOREIGN KEY (scope, to_id) REFERENCES connect_devices (scope, id) ON DELETE CASCADE;

ALTER TABLE playback_state ADD COLUMN scope uuid;
UPDATE playback_state SET scope = user_id;
ALTER TABLE playback_state ALTER COLUMN scope SET NOT NULL;
ALTER TABLE playback_state DROP CONSTRAINT playback_state_pkey;
ALTER TABLE playback_state ADD PRIMARY KEY (scope);
ALTER TABLE playback_state ALTER COLUMN user_id SET NOT NULL;
CREATE INDEX playback_state_user ON playback_state (user_id, updated_at DESC);
