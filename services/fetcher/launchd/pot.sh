# shellcheck shell=bash
# Сервис PO-токенов (bgutil-ytdlp-pot-provider) в Docker на 127.0.0.1:4416: поднять, если не запущен.
# Подключается из run.sh и install.sh: `. pot.sh && ensure_pot`.

POT_NAME=${POT_NAME:-dalywave-pot}
POT_IMAGE=${POT_IMAGE:-brainicism/bgutil-ytdlp-pot-provider:2.0.2-deno}

ensure_pot() {
  local waited=0
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker не найден: сервис PO-токенов не запущен" >&2
    return 1
  fi
  # После входа в систему Docker Desktop поднимается не сразу
  until docker info >/dev/null 2>&1; do
    if [ "$waited" -ge "${POT_WAIT_S:-120}" ]; then
      echo "Docker не запущен: сервис PO-токенов не поднять" >&2
      return 1
    fi
    sleep 5
    waited=$((waited + 5))
  done
  if [ "$(docker inspect -f '{{.State.Running}}' "$POT_NAME" 2>/dev/null)" = "true" ]; then
    return 0
  fi
  if docker inspect "$POT_NAME" >/dev/null 2>&1; then
    docker start "$POT_NAME" >/dev/null
  else
    docker run -d --name "$POT_NAME" --restart unless-stopped \
      -p 127.0.0.1:4416:4416 "$POT_IMAGE" >/dev/null
  fi
}
