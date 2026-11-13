-- Клипы, упавшие у загрузчика без видео в ffmpeg (приложение до сборки ffmpeg с https и
-- кодировщиком H.264: «ffmpeg exited with code 8»), ищутся заново: «клипа нет» по таким трекам
-- забывается, и лента при следующем показе ставит задачу снова. Новые такие сбои сервер
-- откладывает, не запоминая (video::clips, TOOL_ERRORS).
DELETE FROM track_clips c
WHERE c.state = 'none'
  AND EXISTS (
      SELECT 1 FROM jobs j
      WHERE j.dedupe_key = 'clip:' || c.track_id::text
        AND j.kind = 'clip'
        AND j.status = 'failed'
        AND j.last_error LIKE '%ffmpeg exited with code%'
  );
