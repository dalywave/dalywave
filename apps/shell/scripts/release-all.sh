#!/usr/bin/env bash
# Выпуск везде одной командой (make release [NOTES="…"]): новая версия → коммит и push → сайт →
# Windows в CI (с публикацией) → TestFlight → Mac на сервер обновлений и в /Applications этого Mac →
# плагин Stream Deck → дождаться Windows. Рабочее дерево должно быть чистым: в коммит версии уходит только смена номера.
set -euo pipefail
cd "$(dirname "$0")/../../.."
unset CARGO_TARGET_DIR # иначе make app соберёт не туда, откуда берёт бандл

notes="${NOTES:-}"
[ -z "$(git status --porcelain)" ] || { echo "Есть незакоммиченные изменения — сначала закоммить их"; exit 1; }

make --no-print-directory bump-version
version="$(sed -n '/^\[package\]/,/^\[/s/^version = "\(.*\)"/\1/p' apps/shell/Cargo.toml | head -n1)"
git add -A
git commit -q -m "Версия $version${notes:+: $notes}"
git push -q

bash infra/deploy.sh

gh workflow run "Приложения" -f publish=true -f notes="$notes"
sleep 8
run="$(gh run list --workflow "Приложения" --limit 1 --json databaseId --jq '.[0].databaseId')"
echo "Windows: сборка $run в GitHub Actions"

make --no-print-directory testflight
make --no-print-directory release-mac NOTES="$notes"

osascript -e 'quit app "DalyWave"' 2>/dev/null || true
sleep 2
rm -rf /Applications/DalyWave.app
ditto apps/shell/target/release/bundle/macos/DalyWave.app /Applications/DalyWave.app
codesign --verify --deep /Applications/DalyWave.app
open /Applications/DalyWave.app
echo "Mac: в /Applications стоит $(defaults read /Applications/DalyWave.app/Contents/Info CFBundleShortVersionString)"

make --no-print-directory release-streamdeck NOTES="$notes"

if gh run watch "$run" --exit-status --interval 60 >/dev/null 2>&1; then
  echo "Windows: $version выпущена"
else
  echo "Windows: сборка $run не прошла — gh run view $run --log-failed"
  exit 1
fi
echo "Выпуск $version готов: сайт, TestFlight, Mac, Windows, Stream Deck"
