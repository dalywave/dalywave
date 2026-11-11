#!/bin/bash
# Пересобрать Xcode-проект iPhone и iPad из gen/apple/project.yml.
# xcodegen не знает тип документа Icon Composer (.icon) и пишет его как обёртку — тогда Xcode
# копирует значок как есть, а не компилирует его в каталог ресурсов. Тип исправляем после.
set -euo pipefail
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
cd "$(dirname "$0")/../gen/apple"
command -v xcodegen >/dev/null || { echo "нужен xcodegen: brew install xcodegen"; exit 1; }
# Папки, которых нет в git: libapp.a кладёт сборка, assets Tauri наполняет только своим CLI
mkdir -p Externals assets
xcodegen generate --quiet
sed -i '' 's/lastKnownFileType = wrapper\.icon;/lastKnownFileType = folder.iconcomposer.icon;/' dalywave-shell.xcodeproj/project.pbxproj
echo "Xcode-проект: $(pwd)/dalywave-shell.xcodeproj"
