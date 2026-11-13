#!/usr/bin/env bash
# Свой Postgres для тестов этой копии репозитория: tools/test-pg.sh cargo test --workspace
#
# #[sqlx::test] называет базу теста по его пути (`_sqlx_test_` + хэш «api::info::…»), а имена баз общие на
# весь сервер. Два прогона одних и тех же тестов на одном сервере — из двух worktree или двух терминалов —
# делят базы: один удаляет базу, в которой работает другой, или падает на «duplicate key … databases_pkey»
# ещё до начала теста (sqlx testing/mod.rs: «failed to connect to setup test database»). Поэтому у каждой
# копии — свой сервер: контейнер pgvector с данными в памяти, порт из пути к копии. Контейнер живёт, пока
# не остановят (`docker stop dalywave-testpg-…`), и переиспользуется следующими прогонами.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
hash="$(printf '%s' "$root" | shasum | cut -c1-8)"
name="dalywave-testpg-$hash"
port=$((5500 + 16#${hash:0:4} % 100))

if [ -z "$(docker ps -q -f "name=^${name}$")" ]; then
  docker rm -f "$name" >/dev/null 2>&1 || true
  docker run -d --rm --name "$name" \
    -e POSTGRES_USER=dalywave -e POSTGRES_PASSWORD=dalywave -e POSTGRES_DB=dalywave \
    -p "127.0.0.1:$port:5432" --tmpfs /var/lib/postgresql \
    pgvector/pgvector:pg18 \
    -c max_connections=200 -c fsync=off -c synchronous_commit=off -c full_page_writes=off >/dev/null
  # По TCP: пока идёт initdb, временный сервер слушает только сокет, и pg_isready без -h уже «готов»
  for _ in $(seq 1 60); do
    docker exec "$name" pg_isready -h 127.0.0.1 -U dalywave -q 2>/dev/null && break
    sleep 0.5
  done
fi

export DATABASE_URL="postgres://dalywave:dalywave@127.0.0.1:$port/dalywave"
export DALYWAVE_DATABASE_URL="$DATABASE_URL"
echo "Postgres для тестов: $name на :$port" >&2
if [ "$#" -gt 0 ]; then
  exec "$@"
fi
