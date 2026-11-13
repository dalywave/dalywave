#!/usr/bin/env bash
# Заставка DalyWave для Mac: swiftc → DalyWave.saver (arm64 + x86_64), подпись тем же ключом, что у
# приложения (Developer ID из APPLE_SIGNING_IDENTITY или первый в связке ключей, иначе ad-hoc), с
# hardened runtime. `make app` кладёт её в DalyWave.app/Contents/Resources, установку делает приложение
# (apps/shell/src/screensaver.rs).
#
#   bash apps/shell/saver/build.sh [папка-вывода]   # по умолчанию apps/shell/target/saver
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
shell_dir="$(dirname "$here")"
out="${1:-$shell_dir/target/saver}"
bundle="$out/DalyWave.saver"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$shell_dir/Cargo.toml" | head -n1)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

sources=("$here/DalyWaveSaver.swift" "$here/WaveShader.swift")
for arch in arm64 x86_64; do
  xcrun swiftc -O -module-name DalyWaveSaver -target "$arch-apple-macos13.0" \
    -emit-executable -Xlinker -bundle \
    -framework ScreenSaver -framework Metal -framework MetalKit -framework AppKit \
    -o "$work/DalyWave-$arch" "${sources[@]}"
done

rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
xcrun lipo -create -output "$bundle/Contents/MacOS/DalyWave" "$work/DalyWave-arm64" "$work/DalyWave-x86_64"
sed "s/__VERSION__/$version/g" "$here/Info.plist" > "$bundle/Contents/Info.plist"
for f in thumbnail.png thumbnail@2x.png; do
  [ -f "$here/$f" ] && cp "$here/$f" "$bundle/Contents/Resources/$f"
done

id="${APPLE_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -n1)}"
id="${id:--}"
args=(--force --options runtime --sign "$id")
[ "$id" != "-" ] && args+=(--timestamp)
codesign "${args[@]}" "$bundle"
codesign --verify --strict "$bundle"
echo "Заставка: $bundle (подпись: $id)"
