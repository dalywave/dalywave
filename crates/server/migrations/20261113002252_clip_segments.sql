-- Клип целиком со сценкой посреди или вырезанным куплетом: несколько кусков, у каждого своя прямая
-- «время клипа → время трека» ([{videoStartMs, videoEndMs, offsetMs, rate}]). NULL — кусок один
ALTER TABLE track_clips ADD COLUMN sync_segments jsonb;
