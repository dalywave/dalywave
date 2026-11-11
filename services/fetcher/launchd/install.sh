#!/usr/bin/env bash
# Ставит загрузчик DalyWave в автозапуск этого Mac (launchd, при входе пользователя).
#   services/fetcher/launchd/install.sh [https://wave.daly.rest]   — поставить или обновить
#   services/fetcher/launchd/install.sh --uninstall                — убрать
# Токен загрузчика (dwk_…) выдаёт админка: «Источники» → новый загрузчик. Он хранится только в
# ~/.config/dalywave/downloader-token с правами 600 и в репозиторий не попадает.
set -euo pipefail

LABEL=rest.daly.wave.downloader
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
FETCHER_DIR=$(dirname "$HERE")
PLIST=$HOME/Library/LaunchAgents/$LABEL.plist
TOKEN_FILE=$HOME/.config/dalywave/downloader-token
LOG=$HOME/Library/Logs/DalyWave/downloader.log
DOMAIN=gui/$(id -u)

if [ "${1:-}" = "--uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  echo "загрузчик убран из автозапуска"
  exit 0
fi
SERVER=${1:-https://wave.daly.rest}

if [ ! -s "$TOKEN_FILE" ]; then
  echo "Нет токена загрузчика. Создайте его в админке («Источники» → новый загрузчик) и сохраните:"
  echo "  mkdir -p ~/.config/dalywave && pbpaste > $TOKEN_FILE && chmod 600 $TOKEN_FILE"
  exit 1
fi
chmod 600 "$TOKEN_FILE"
case "$(tr -d '[:space:]' < "$TOKEN_FILE")" in
  dwk_*) ;;
  *) echo "в $TOKEN_FILE не токен загрузчика (dwk_…)"; exit 1 ;;
esac

missing=()
for tool in uv ffmpeg ffprobe fpcalc deno docker; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done
if [ ${#missing[@]} -gt 0 ]; then
  echo "не хватает: ${missing[*]} (brew install uv ffmpeg chromaprint deno; Docker Desktop)"
  exit 1
fi

# shellcheck source=pot.sh
. "$HERE/pot.sh"
ensure_pot || echo "внимание: сервис PO-токенов не запущен — загрузчик поднимет его сам, когда появится Docker"
(cd "$FETCHER_DIR" && uv sync --frozen --quiet)

# launchd не знает PATH из оболочки: собираем каталоги нужных программ
path_value=/usr/bin:/bin:/usr/sbin:/sbin
for tool in uv ffmpeg fpcalc deno docker; do
  dir=$(dirname "$(command -v "$tool")")
  case ":$path_value:" in *":$dir:"*) ;; *) path_value="$dir:$path_value" ;; esac
done

mkdir -p "$(dirname "$PLIST")" "$(dirname "$LOG")"
sed -e "s|__RUNNER__|$HERE/run.sh|g" \
    -e "s|__SERVER__|$SERVER|g" \
    -e "s|__WORKDIR__|$FETCHER_DIR|g" \
    -e "s|__PATH__|$path_value|g" \
    -e "s|__LOG__|$LOG|g" \
    "$HERE/$LABEL.plist" > "$PLIST"
plutil -lint "$PLIST" >/dev/null
launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
launchctl bootstrap "$DOMAIN" "$PLIST"
launchctl kickstart -k "$DOMAIN/$LABEL"
echo "загрузчик запущен → $SERVER"
echo "лог: $LOG"
echo "остановить: launchctl bootout $DOMAIN/$LABEL; убрать совсем: $0 --uninstall"
