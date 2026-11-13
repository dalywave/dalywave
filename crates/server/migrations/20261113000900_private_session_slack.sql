-- Приватная сессия — с запасом в минуту с обеих сторон.
--
-- Начало и конец сессии ставят часы сервера (now() базы), а время прослушивания присылает устройство по своим
-- часам. Часы расходятся: у телефона — на секунды, у базы в docker и сервера API — на миллисекунды. Трек,
-- включённый сразу после «Приватной сессии», по часам устройства мог оказаться «до» неё и показаться друзьям.
-- Приватность ошибается в сторону скрытого: минута до начала и после конца тоже не видна.

CREATE OR REPLACE FUNCTION played_privately(uid uuid, at timestamptz) RETURNS boolean
LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT EXISTS (SELECT 1 FROM private_sessions s
                   WHERE s.user_id = uid
                     AND at >= s.started_at - interval '1 minute'
                     AND at < s.ended_at + interval '1 minute')
$$;
