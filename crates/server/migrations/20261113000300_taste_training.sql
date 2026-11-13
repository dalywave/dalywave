-- Сеть вкуса учится не только на сервере: компьютеры с приложением (админов — общую, остальных —
-- по желанию свою) берут задание, учат на своём железе и присылают веса. Сервер сам проверяет их на
-- своей отложенной выборке и включает, только если сеть точнее правил и текущей версии.

-- Чья версия: NULL — общая для всех, иначе — своя сеть слушателя (её подбор предпочитает общей).
ALTER TABLE model_versions ADD COLUMN user_id uuid REFERENCES users (id) ON DELETE CASCADE;
-- Где обучена: на сервере или на компьютере (какой — имя устройства на момент обучения).
ALTER TABLE model_versions ADD COLUMN origin text NOT NULL DEFAULT 'server'
    CHECK (origin IN ('server', 'device'));
ALTER TABLE model_versions ADD COLUMN device_id uuid REFERENCES devices (id) ON DELETE SET NULL;
ALTER TABLE model_versions ADD COLUMN device_name text;

-- Активная общая — одна; своя у слушателя — тоже одна
DROP INDEX model_versions_active;
CREATE UNIQUE INDEX model_versions_active ON model_versions (kind) WHERE active AND user_id IS NULL;
CREATE UNIQUE INDEX model_versions_active_user ON model_versions (kind, user_id)
    WHERE active AND user_id IS NOT NULL;

-- Задание компьютеру: какая выборка (общая или слушателя), граница отложенной части на момент выдачи
-- (её строки компьютер не получает), описание обучения и чем кончилось.
CREATE TABLE taste_train_tasks (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    user_id      uuid REFERENCES users (id) ON DELETE CASCADE,
    device_id    uuid NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    requested_by uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    status       text NOT NULL DEFAULT 'leased'
        CHECK (status IN ('leased', 'done', 'failed', 'expired')),
    holdout_cut  timestamptz NOT NULL,
    rows         integer NOT NULL,
    spec         jsonb NOT NULL,
    result       jsonb,
    leased_at    timestamptz NOT NULL DEFAULT now(),
    finished_at  timestamptz
);
CREATE INDEX taste_train_tasks_device ON taste_train_tasks (device_id, leased_at DESC);
CREATE INDEX taste_train_tasks_scope ON taste_train_tasks (user_id, leased_at DESC);
