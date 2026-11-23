#!/bin/bash
# Новая версия приложения: make bump-version VERSION=0.2.0, без VERSION — следующая исправленная
# (0.1.1 → 0.1.2): так получает номер каждая сборка для TestFlight и Mac.
#
# Источник версии один — [package] version в apps/shell/Cargo.toml: в tauri.conf.json версии нет,
# Tauri берёт её из Cargo.toml (она же — appVersion в platform_info и {{current_version}} в проверке
# обновлений). Скрипт меняет её и то, что из неё переписано для Xcode (MARKETING_VERSION в
# project.yml и в уже собранном project.pbxproj), и обновляет Cargo.lock оболочки.
set -euo pipefail
cd "$(dirname "$0")/.."

current="$(sed -n '/^\[package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
version="${1:-}"
if [ -z "$version" ]; then
  IFS=. read -r major minor patch <<<"${current%%-*}"
  version="$major.$minor.$((patch + 1))"
fi
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "Нужна версия SemVer: make bump-version VERSION=0.2.0" >&2
  exit 1
fi
if [ "$current" = "$version" ]; then
  echo "Версия уже $version"
  exit 0
fi

# Только первая строка version в [package]: у зависимостей свои версии
sed -i '' "/^\[package\]/,/^\[/s/^version = \"$current\"/version = \"$version\"/" Cargo.toml
sed -i '' "s/^\( *MARKETING_VERSION:\) .*/\1 $version/" gen/apple/project.yml
sed -i '' "s/MARKETING_VERSION = [^;]*;/MARKETING_VERSION = $version;/" gen/apple/dalywave-shell.xcodeproj/project.pbxproj
# Cargo.lock: запись самой оболочки — без сети и без обновления зависимостей
cargo metadata --format-version 1 --offline >/dev/null 2>&1 || cargo metadata --format-version 1 >/dev/null

echo "Версия приложения: $current → $version"
echo "Дальше: make release-mac (macOS), gh workflow run \"Приложения\" -f publish=true (Windows) и тег v$version (образы сервера)"
