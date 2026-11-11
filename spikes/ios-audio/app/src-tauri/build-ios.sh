#!/bin/bash
# Сборка Rust-части для Xcode без запущенного `tauri ios dev/build`.
# Штатная фаза Tauri 2 (`cargo tauri ios xcode-script`) требует, чтобы рядом работал CLI;
# этот скрипт делает то же самое сам: cargo build нужной цели и копия libapp.a в Externals.
set -euo pipefail

# Xcode, запущенный из Dock, не видит PATH оболочки.
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

SRC_TAURI="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE="$(cd "$SRC_TAURI/../.." && pwd)"

if [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
  case "$(uname -m)" in
    arm64) TARGET=aarch64-apple-ios-sim; ARCH=arm64 ;;
    *) TARGET=x86_64-apple-ios; ARCH=x86_64 ;;
  esac
else
  TARGET=aarch64-apple-ios; ARCH=arm64
fi

PROFILE_FLAG=""
PROFILE_DIR=debug
case "${CONFIGURATION:-debug}" in
  [Rr]elease) PROFILE_FLAG="--release"; PROFILE_DIR=release ;;
esac

# SDKROOT Xcode указывает на iOS SDK — build-скрипты для хоста (macOS) с ним не соберутся.
# Целевым C-зависимостям (libopus через cmake, ring) SDK передаём явно.
SDK="${SDKROOT:-}"
unset SDKROOT
TRIPLE_ENV="$(echo "$TARGET" | tr '-' '_')"
if [ -n "$SDK" ]; then
  export "CFLAGS_${TRIPLE_ENV}=-isysroot $SDK"
  export "CXXFLAGS_${TRIPLE_ENV}=-isysroot $SDK"
fi
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"

cd "$SRC_TAURI"
# shellcheck disable=SC2086 — пустой флаг не должен превращаться в аргумент "".
cargo build --lib --target "$TARGET" $PROFILE_FLAG

OUT="${SRCROOT:-$SRC_TAURI/gen/apple}/Externals/$ARCH/${CONFIGURATION:-debug}"
mkdir -p "$OUT"
cp "$WORKSPACE/target/$TARGET/$PROFILE_DIR/libdw_spike_lib.a" "$OUT/libapp.a"
echo "Rust library for $TARGET ($PROFILE_DIR) -> $OUT/libapp.a"
