-- Пробные проверки резервных копий базы из админки («Система → Резервные копии»): копию разбирает
-- целиком: распаковка (контрольная сумма gzip), заголовок и конец дампа, число таблиц и блоков данных
CREATE TABLE backup_checks (
    id         uuid PRIMARY KEY DEFAULT uuidv7(),
    name       text        NOT NULL,
    ok         boolean     NOT NULL,
    method     text        NOT NULL CHECK (method IN ('sql')),
    entries    bigint,
    tables     bigint,
    error      text,
    checked_by uuid REFERENCES users (id) ON DELETE SET NULL,
    checked_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX backup_checks_at ON backup_checks (checked_at DESC);
