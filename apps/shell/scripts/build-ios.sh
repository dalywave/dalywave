#!/bin/bash
# Фаза «Build Rust Code» Xcode-проекта iPhone и iPad (gen/apple/project.yml).
#
# Штатная фаза Tauri 2 (`cargo tauri ios xcode-script`) работает только при запущенном
# `cargo tauri ios dev/build`, а приложение ставится кнопкой Run в Xcode. Скрипт делает
# то же сам: собирает интерфейс, если он устарел, затем Rust под нужную цель и кладёт libapp.a
# туда, где его ждёт линковщик Xcode.
set -euo pipefail

# Xcode, запущенный из Dock, не видит PATH оболочки: cargo — в ~/.cargo/bin, bun — в Homebrew.
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

SHELL_DIR="$(cd "$(dirname "$0")/.." && pwd)"
UI_DIR="$(cd "$SHELL_DIR/../ui" && pwd)"

# 1. Интерфейс: tauri::generate_context! вшивает apps/ui/.output/public в бинарник
INDEX="$UI_DIR/.output/public/index.html"
stale=""
if [ ! -f "$INDEX" ]; then
  stale="нет сборки"
elif [ -n "$(find "$UI_DIR/app" "$UI_DIR/public" "$UI_DIR/i18n" "$UI_DIR/nuxt.config.ts" "$UI_DIR/package.json" -newer "$INDEX" -print -quit 2>/dev/null)" ]; then
  stale="исходники новее сборки"
fi
if [ -n "$stale" ]; then
  echo "Интерфейс: $stale — bun run build"
  command -v bun >/dev/null || { echo "error: нужен bun (brew install oven-sh/bun/bun)"; exit 1; }
  (cd "$UI_DIR" && { [ -d node_modules ] || bun install --frozen-lockfile; } && bun run build)
fi

# 2. Rust
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
# Целевым C-зависимостям (libopus через cmake) SDK и компилятор передаём явно: если путь к
# компилятору меняется между запусками из терминала и из Xcode, CMake сбрасывает свой кэш.
SDK="${SDKROOT:-}"
unset SDKROOT
TRIPLE_ENV="$(echo "$TARGET" | tr '-' '_')"
if [ -z "$SDK" ]; then
  if [ "${PLATFORM_NAME:-}" = "iphonesimulator" ]; then
    SDK="$(xcrun --sdk iphonesimulator --show-sdk-path)"
  else
    SDK="$(xcrun --sdk iphoneos --show-sdk-path)"
  fi
fi
export "CFLAGS_${TRIPLE_ENV}=-isysroot $SDK"
export "CXXFLAGS_${TRIPLE_ENV}=-isysroot $SDK"
export "CC_${TRIPLE_ENV}=$(xcrun -f clang)"
export "CXX_${TRIPLE_ENV}=$(xcrun -f clang++)"
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"

cd "$SHELL_DIR"
# custom-protocol: страница из сборки, а не с dev-сервера (иначе — белый экран)
# shellcheck disable=SC2086 — пустой флаг не должен превращаться в аргумент "".
cargo build --lib --target "$TARGET" $PROFILE_FLAG --features tauri/custom-protocol

OUT="${SRCROOT:-$SHELL_DIR/gen/apple}/Externals/$ARCH/${CONFIGURATION:-debug}"
mkdir -p "$OUT"
# CARGO_TARGET_DIR — своя папка сборки (параллельные проверки, второй worktree)
cp "${CARGO_TARGET_DIR:-$SHELL_DIR/target}/$TARGET/$PROFILE_DIR/libdalywave_shell_lib.a" "$OUT/libapp.a"
echo "Rust: $TARGET ($PROFILE_DIR) -> $OUT/libapp.a"
