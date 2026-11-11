-- Своя сеть вкуса (этап 2): ONNX версии лежит в хранилище (`artifact_key`) и копией здесь — его
-- загружают и API, и воркер при подборе волны, а хранилища у подбора под рукой нет. Около мегабайта.
ALTER TABLE model_versions ADD COLUMN artifact bytea;
CREATE INDEX model_versions_kind_time ON model_versions (kind, created_at DESC);
