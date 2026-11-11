#!/usr/bin/env bash
# Загрузчик DalyWave на этом Mac: `fetcher worker` берёт задания у сервера и качает со своего адреса.
# Запускает его launchd (install.sh) или `make downloader`. Токен — FETCHER_WORKER_TOKEN или файл
# ~/.config/dalywave/downloader-token (права 600); в репозиторий он не попадает.
#   run.sh [https://wave.daly.rest]
set -uo pipefail

SERVER=${1:-${DALYWAVE_SERVER:-https://wave.daly.rest}}
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
FETCHER_DIR=$(dirname "$HERE")
TOKEN_FILE=${DALYWAVE_TOKEN_FILE:-$HOME/.config/dalywave/downloader-token}

log() { printf '%s %s\n' "$(date '+%H:%M:%S')" "$*" >&2; }

if [ -z "${FETCHER_WORKER_TOKEN:-}" ]; then
  if [ ! -r "$TOKEN_FILE" ]; then
    log "нет токена загрузчика: положите dwk_… в $TOKEN_FILE (права 600)"
    sleep 60
    exit 78
  fi
  FETCHER_WORKER_TOKEN=$(tr -d '[:space:]' < "$TOKEN_FILE")
fi
export FETCHER_WORKER_TOKEN

# shellcheck source=pot.sh
. "$HERE/pot.sh"
if ensure_pot; then
  export FETCHER_POT_URL=${FETCHER_POT_URL:-http://127.0.0.1:4416}
else
  log "качаю без PO-токенов"
  unset FETCHER_POT_URL
fi
export FETCHER_DATA_DIR=${FETCHER_DATA_DIR:-$HOME/Library/Application Support/DalyWave/fetcher}
mkdir -p "$FETCHER_DATA_DIR"

cd "$FETCHER_DIR" || exit 1
log "загрузчик → $SERVER"
uv run --frozen fetcher worker --server "$SERVER" &
child=$!
trap 'kill -TERM "$child" 2>/dev/null' TERM INT
wait "$child"
code=$?
if [ "$code" -eq 2 ]; then
  # Сервер не принял токен: не стучимся каждые полминуты, пока его не заменят
  log "токен не принят — проверьте $TOKEN_FILE; повтор через 10 минут"
  sleep 600
fi
exit "$code"
