#!/usr/bin/env bash
# Значки приложения из design/brand (запускать после `bun tools/brand.mjs`; нужен macOS).
#   apps/shell/scripts/icons.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
brand="$root/design/brand"
icons="$root/apps/shell/icons"
render="$root/apps/shell/scripts/render-svg.swift"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Общий набор: PNG, ICO для Windows, плитки Microsoft Store
cargo tauri icon "$brand/icon-1024.png" -o "$icons"
# Значки iPhone и Android соберёт этап iOS из своего каталога ресурсов
rm -rf "$icons/android" "$icons/ios"

# macOS до 26: icns из варианта по сетке Apple — поля и тень уже нарисованы
cargo tauri icon "$brand/icon-macos-1024.png" -o "$tmp/macos" >/dev/null
cp "$tmp/macos/icon.icns" "$icons/icon.icns"

# macOS 26+: документ Icon Composer — светлая, тёмная, тонированная и прозрачная темы из слоёв.
# Знак — растр: штрих в SVG стекло Icon Composer принимает за замкнутую фигуру и рисует лишнюю грань.
doc="$icons/DalyWave.icon/Assets"
mkdir -p "$doc"
cp "$brand/layers/background.svg" "$doc/background.svg"
swift "$render" "$brand/layers/glyph.svg" 1024 1024 "$doc/glyph.png"
swift "$render" "$brand/icon-ios-dark.svg" 1024 1024 "$doc/glyph-dark.png"

# Строка меню macOS: шаблон (чёрный знак на прозрачном), система сама красит под тему
swift "$render" "$icons/tray.svg" 69 48 "$icons/tray-template.png"
# Трей Windows: цветной значок со скруглением
sips -z 32 32 "$brand/icon-rounded-512.png" --out "$icons/tray-windows.png" >/dev/null

echo "icons: $icons"
