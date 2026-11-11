#!/usr/bin/env bash
# Загрузчик DalyWave для приложения (sidecar Tauri) → services/fetcher/dist/sidecar/:
#   dalywave-fetcher-<triple>          Python 3.13 + yt-dlp (+ejs) + ytmusicapi + плагин bgutil (PyInstaller, один файл)
#   deno-<triple>                      JS-задачи YouTube (yt-dlp-ejs) и сервис PO-токенов
#   ffmpeg-<triple>, ffprobe-<triple>  своя сборка FFmpeg только для звука, LGPL 2.1+
#   fpcalc-<triple>                    chromaprint, официальная статическая сборка
#   resources/bgutil-server/           сервер PO-токенов bgutil: TypeScript + node_modules
#   licenses/, MANIFEST.json           лицензии; версии, источники, sha256 и размеры
# Имена — как ждёт Tauri externalBin: `<имя>-<target triple>`. Договор с приложением — SIDECAR.md.
#
#   services/fetcher/sidecar/build.sh           собрать (скачанное и собранное берётся из dist/build/)
#   CLEAN=1 services/fetcher/sidecar/build.sh   собрать всё заново
#
# Для сборки нужны uv, Xcode Command Line Tools (clang, make) и npm (Node ≥ 22). На машине
# пользователя ничего из этого не нужно.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
FETCHER=$(dirname "$HERE")
DIST=$FETCHER/dist
BUILD=$DIST/build
OUT=$DIST/sidecar
DOWNLOADS=$BUILD/downloads

case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) TRIPLE=aarch64-apple-darwin ;;
  *)
    echo "сборка пока только для macOS arm64; Windows x64 — см. SIDECAR.md" >&2
    exit 1
    ;;
esac

# ── Версии и контрольные суммы (sha256 — архивов в том виде, как их отдаёт источник) ──
PYTHON_VERSION=3.13.18
DENO_VERSION=2.9.8
DENO_SHA256=9f0bf660393da2eaff044cdc7444cb08c73f235849b61d224d825a194ce7c98e
FFMPEG_VERSION=9.0.2
# Совпадает с формулой Homebrew ffmpeg 9.0.2
FFMPEG_SHA256=8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e
CHROMAPRINT_VERSION=1.6.2
FPCALC_SHA256=f3bc56cb42d89a01237bb3f3629329e4ec75f7c1b0217acf6e9bf952ac47979b
# Исходники chromaprint — только ради текста лицензии: в архиве fpcalc его нет
CHROMAPRINT_SRC_SHA256=d54ada62c16b2db396d3251ec54fc7a58909c163131a1cfcf7fa6407dbc14360
# Сервер — той же версии, что плагин в uv.lock: плагин сверяет версии по /ping
BGUTIL_VERSION=2.0.2
BGUTIL_SHA256=1f2ae9ec39405b8e80397dfc2551f58deed09d8dff4b2b8fbc314df8e091c0a1
CANVAS_VERSION=3.2.3
CANVAS_SHA256=38c296c9d81c05598db849fb8103543d6649fec7246c35197d9be8199c75116f
MACOS_MIN=11.0

DENO_URL=https://github.com/denoland/deno/releases/download/v$DENO_VERSION/deno-$TRIPLE.zip
FFMPEG_URL=https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz
FPCALC_URL=https://github.com/acoustid/chromaprint/releases/download/v$CHROMAPRINT_VERSION/chromaprint-fpcalc-$CHROMAPRINT_VERSION-macos-arm64.tar.gz
CHROMAPRINT_SRC_URL=https://github.com/acoustid/chromaprint/releases/download/v$CHROMAPRINT_VERSION/chromaprint-$CHROMAPRINT_VERSION.tar.gz
BGUTIL_URL=https://github.com/Brainicism/bgutil-ytdlp-pot-provider/archive/refs/tags/$BGUTIL_VERSION.tar.gz
CANVAS_URL=https://github.com/Automattic/node-canvas/releases/download/v$CANVAS_VERSION/canvas-v$CANVAS_VERSION-napi-v7-darwin-arm64.tar.gz

# FFmpeg только для звука: перекладка в MP4 (-c copy), ffprobe, громкость EBU R128 и огибающая
# (asplit, ebur128, aresample, вывод в null и s16le). Без --enable-gpl/--enable-nonfree и без
# внешних библиотек — сборка остаётся под LGPL 2.1+. Сеть FFmpeg не нужна: качает yt-dlp.
# shellcheck disable=SC2054 # запятые — списки компонентов configure, а не элементы массива
FFMPEG_FLAGS=(
  --enable-static --disable-shared
  --disable-autodetect --enable-zlib
  --disable-doc --disable-debug --disable-network
  --disable-programs --enable-ffmpeg --enable-ffprobe
  --disable-avdevice --disable-swscale
  --disable-everything
  --enable-protocol=file,pipe
  --enable-demuxer=aac,ac3,aiff,ape,asf,au,caf,dsf,dts,eac3,flac,loas,matroska,mlp,mov,mp3,mpegts,ogg,truehd,tta,w64,wav,wv,xwma
  --enable-muxer=mp4,ipod,mp3,flac,ogg,null,pcm_s16le,adts,wav
  --enable-decoder=aac,aac_fixed,aac_latm,ac3,eac3,alac,ape,flac,mp1,mp1float,mp2,mp2float,mp3,mp3float,opus,vorbis,wavpack,tta,wmav1,wmav2,wmapro,wmalossless,dca,mlp,truehd,dsd_lsbf,dsd_msbf,dsd_lsbf_planar,dsd_msbf_planar
  --enable-decoder='pcm_*'
  --enable-encoder=pcm_s16le,wrapped_avframe
  --enable-parser=aac,aac_latm,ac3,dca,flac,mlp,mpegaudio,opus,vorbis
  --enable-bsfs
  --enable-filter=aformat,anull,aresample,asplit,atrim,ebur128,format,null,trim
  "--extra-cflags=-mmacosx-version-min=$MACOS_MIN"
  "--extra-ldflags=-mmacosx-version-min=$MACOS_MIN"
)

log() { printf '\033[1m==> %s\033[0m\n' "$*" >&2; }
die() {
  echo "ошибка: $*" >&2
  exit 1
}

sha_ok() { echo "$2  $1" | shasum -a 256 -c - >/dev/null 2>&1; }

# fetch <url> <sha256> <имя файла> — скачать в dist/build/downloads и сверить sha256
fetch() {
  local url=$1 sum=$2 file=$DOWNLOADS/$3
  if [ -f "$file" ] && sha_ok "$file" "$sum"; then
    return
  fi
  log "скачиваю $url"
  curl -fsSL --retry 3 -o "$file.part" "$url"
  sha_ok "$file.part" "$sum" || die "sha256 не совпала: $url"
  mv "$file.part" "$file"
}

for tool in uv curl shasum unzip make clang npm; do
  command -v "$tool" >/dev/null 2>&1 || die "для сборки нужен $tool"
done

if [ "${CLEAN:-}" = 1 ]; then
  rm -rf "$BUILD" "$OUT"
fi
mkdir -p "$DOWNLOADS" "$OUT/resources" "$OUT/licenses"

# ── deno ──
build_deno() {
  log "deno $DENO_VERSION"
  fetch "$DENO_URL" "$DENO_SHA256" "deno-$DENO_VERSION-$TRIPLE.zip"
  rm -rf "$BUILD/deno" && mkdir -p "$BUILD/deno"
  unzip -q -o "$DOWNLOADS/deno-$DENO_VERSION-$TRIPLE.zip" -d "$BUILD/deno"
  install -m 755 "$BUILD/deno/deno" "$OUT/deno-$TRIPLE"
}

# ── fpcalc ──
build_fpcalc() {
  log "fpcalc (chromaprint $CHROMAPRINT_VERSION)"
  fetch "$FPCALC_URL" "$FPCALC_SHA256" "fpcalc-$CHROMAPRINT_VERSION-macos-arm64.tar.gz"
  rm -rf "$BUILD/fpcalc" && mkdir -p "$BUILD/fpcalc"
  tar xzf "$DOWNLOADS/fpcalc-$CHROMAPRINT_VERSION-macos-arm64.tar.gz" -C "$BUILD/fpcalc"
  local bin
  bin=$(find "$BUILD/fpcalc" -type f -name fpcalc | head -1)
  [ -n "$bin" ] || die "в архиве chromaprint нет fpcalc"
  install -m 755 "$bin" "$OUT/fpcalc-$TRIPLE"
  fetch "$CHROMAPRINT_SRC_URL" "$CHROMAPRINT_SRC_SHA256" "chromaprint-$CHROMAPRINT_VERSION.tar.gz"
  mkdir -p "$OUT/licenses/chromaprint"
  tar xzf "$DOWNLOADS/chromaprint-$CHROMAPRINT_VERSION.tar.gz" -C "$OUT/licenses/chromaprint" \
    --strip-components 1 "chromaprint-$CHROMAPRINT_VERSION/LICENSE.md"
}

# ── ffmpeg и ffprobe: сборка из исходников ──
build_ffmpeg() {
  local src=$BUILD/ffmpeg-$FFMPEG_VERSION
  local stamp="$FFMPEG_VERSION ${FFMPEG_FLAGS[*]}"
  if [ -x "$src/ffmpeg" ] && [ "$(cat "$src/.stamp" 2>/dev/null)" = "$stamp" ]; then
    log "ffmpeg $FFMPEG_VERSION: уже собран"
  else
    log "ffmpeg $FFMPEG_VERSION: сборка из исходников (пара минут)"
    fetch "$FFMPEG_URL" "$FFMPEG_SHA256" "ffmpeg-$FFMPEG_VERSION.tar.xz"
    rm -rf "$src"
    tar xf "$DOWNLOADS/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$BUILD"
    (
      cd "$src"
      ./configure "${FFMPEG_FLAGS[@]}" >"$BUILD/ffmpeg-configure.log" 2>&1 ||
        die "configure FFmpeg упал: $BUILD/ffmpeg-configure.log"
      make -j"$(sysctl -n hw.ncpu)" >"$BUILD/ffmpeg-make.log" 2>&1 || die "сборка FFmpeg упала: $BUILD/ffmpeg-make.log"
    )
    echo "$stamp" >"$src/.stamp"
  fi
  # `ffmpeg` и `ffprobe` в дереве сборки уже без отладочных символов (с ними — *_g)
  install -m 755 "$src/ffmpeg" "$OUT/ffmpeg-$TRIPLE"
  install -m 755 "$src/ffprobe" "$OUT/ffprobe-$TRIPLE"
  "$OUT/ffmpeg-$TRIPLE" -hide_banner -L | grep -q "Lesser General Public License" ||
    die "FFmpeg собрался не под LGPL"
  mkdir -p "$OUT/licenses/ffmpeg"
  cp "$src/COPYING.LGPLv2.1" "$src/LICENSE.md" "$OUT/licenses/ffmpeg/"
}

# ── сервер PO-токенов bgutil ──
build_bgutil() {
  local src=$BUILD/bgutil-ytdlp-pot-provider-$BGUTIL_VERSION dst=$BUILD/bgutil-server
  local stamp="$BGUTIL_VERSION $CANVAS_VERSION"
  fetch "$BGUTIL_URL" "$BGUTIL_SHA256" "bgutil-ytdlp-pot-provider-$BGUTIL_VERSION.tar.gz"
  fetch "$CANVAS_URL" "$CANVAS_SHA256" "canvas-v$CANVAS_VERSION-darwin-arm64.tar.gz"
  if [ "$(cat "$dst/.stamp" 2>/dev/null)" = "$stamp" ]; then
    log "bgutil-server $BGUTIL_VERSION: уже собран"
  else
    log "bgutil-server $BGUTIL_VERSION: исходники и node_modules"
    rm -rf "$src" "$dst"
    tar xzf "$DOWNLOADS/bgutil-ytdlp-pot-provider-$BGUTIL_VERSION.tar.gz" -C "$BUILD"
    mkdir -p "$dst"
    cp -R "$src/server/src" "$src/server/types" "$dst/"
    cp "$src/server/package.json" "$src/server/package-lock.json" "$src/server/tsconfig.json" "$dst/"
    cp "$src/LICENSE" "$dst/LICENSE"
    # Только зависимости из package-lock (sha512 сверяет npm) и без их install-скриптов:
    # единственный нужный — сборка canvas — заменён готовой сборкой с GitHub ниже
    (cd "$dst" && npm ci --omit=dev --ignore-scripts --no-audit --no-fund --loglevel=error)
    mkdir -p "$dst/node_modules/canvas"
    tar xzf "$DOWNLOADS/canvas-v$CANVAS_VERSION-darwin-arm64.tar.gz" -C "$dst/node_modules/canvas"
    [ -f "$dst/node_modules/canvas/build/Release/canvas.node" ] || die "в сборке canvas нет canvas.node"
    # Для запуска не нужны: карты исходников, типы TypeScript, описания, браузерная сборка
    # youtubei.js (серверу Deno отдаётся dist/src/platform/deno.js). Лицензии остаются.
    find "$dst/node_modules" -type f \( -name '*.map' -o -name '*.d.ts' -o -name '*.d.mts' \
      -o -name '*.d.cts' -o \( -iname '*.md' ! -iname 'LICEN[CS]E*' ! -iname 'COPYING*' \) \) -delete
    rm -rf "$dst/node_modules/youtubei.js/bundle"
    echo "$stamp" >"$dst/.stamp"
  fi
  rm -rf "$OUT/resources/bgutil-server"
  cp -R "$dst" "$OUT/resources/bgutil-server"
  rm -f "$OUT/resources/bgutil-server/.stamp"
  # Своя точка входа и deno.json: сервер выходит вместе с загрузчиком, lock-файл не пишется
  cp "$HERE/pot/serve.ts" "$HERE/pot/deno.json" "$OUT/resources/bgutil-server/"
  mkdir -p "$OUT/licenses/bgutil-ytdlp-pot-provider"
  cp "$dst/LICENSE" "$OUT/licenses/bgutil-ytdlp-pot-provider/LICENSE"
}

# ── сам загрузчик: PyInstaller, один файл ──
build_python() {
  log "dalywave-fetcher: Python $PYTHON_VERSION, PyInstaller"
  export UV_PYTHON_INSTALL_DIR=$BUILD/python
  uv python install --quiet "$PYTHON_VERSION"
  local venv=$BUILD/venv
  (
    cd "$FETCHER"
    # Пакет переустанавливается всегда: без этого uv оставит прежнюю копию той же версии
    UV_PROJECT_ENVIRONMENT=$venv uv sync --quiet --frozen --no-dev --group sidecar \
      --no-editable --reinstall-package dalywave-fetcher --managed-python --python "$PYTHON_VERSION"
  )
  local py=$venv/bin/python site plugin
  site=$("$py" -I -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')
  diff -rq -x __pycache__ "$FETCHER/src/dalywave_fetcher" "$site/dalywave_fetcher" >/dev/null ||
    die "в окружении сборки не тот dalywave_fetcher, что в src/"
  plugin=$("$py" -I -c 'import importlib.metadata as m; print(m.version("bgutil-ytdlp-pot-provider"))')
  [ "$plugin" = "$BGUTIL_VERSION" ] || die "плагин bgutil $plugin, а сервер $BGUTIL_VERSION — обновите BGUTIL_VERSION"
  # Плагины yt-dlp ищутся как папки на sys.path, а не в архиве модулей: кладём исходники рядом
  rm -rf "$BUILD/plugins" && mkdir -p "$BUILD/plugins"
  (cd "$site" && find yt_dlp_plugins -name '*.py' -print0 | xargs -0 tar cf - | tar xf - -C "$BUILD/plugins")
  "$venv/bin/pyinstaller" --noconfirm --clean --log-level WARN --onefile \
    --name "dalywave-fetcher-$TRIPLE" \
    --distpath "$OUT" --workpath "$BUILD/pyinstaller" --specpath "$BUILD/pyinstaller" \
    --add-data "$BUILD/plugins/yt_dlp_plugins:yt_dlp_plugins" \
    --collect-data ytmusicapi \
    --exclude-module fastapi --exclude-module uvicorn --exclude-module starlette \
    --exclude-module tkinter --exclude-module pytest \
    "$HERE/entry.py"
  mkdir -p "$OUT/licenses/python"
  cp "$BUILD"/python/cpython-"$PYTHON_VERSION"-*/lib/python3.13/LICENSE.txt "$OUT/licenses/python/" 2>/dev/null || true
}

build_deno
build_fpcalc
build_ffmpeg
build_bgutil
build_python

# ── проверка и паспорт сборки ──
log "проверка"
smoke=$BUILD/smoke
rm -rf "$smoke" && mkdir -p "$smoke"
code=0
env -u DALYWAVE_DOWNLOADER_TOKEN -u FETCHER_WORKER_TOKEN \
  "$OUT/dalywave-fetcher-$TRIPLE" sidecar --data-dir "$smoke/data" --log-file "$smoke/fetcher.log" \
  --no-pot </dev/null >"$smoke/events.jsonl" 2>"$smoke/stderr.log" || code=$?
[ "$code" = 2 ] || die "проверочный запуск: код $code вместо 2 (нет токена) — $smoke"

FFMPEG_CONFIGURE="${FFMPEG_FLAGS[*]}"
export PYTHON_VERSION DENO_VERSION DENO_URL DENO_SHA256 FFMPEG_VERSION FFMPEG_URL FFMPEG_SHA256 \
  FFMPEG_CONFIGURE CHROMAPRINT_VERSION FPCALC_URL FPCALC_SHA256 BGUTIL_VERSION BGUTIL_URL \
  BGUTIL_SHA256 CANVAS_VERSION CANVAS_URL CANVAS_SHA256
"$BUILD/venv/bin/python" -I - "$OUT" "$smoke/events.jsonl" "$TRIPLE" <<'PY'
import hashlib, importlib.metadata as md, json, os, sys, time
from pathlib import Path

out, events, triple = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
hello = next(json.loads(l) for l in events.read_text().splitlines() if '"hello"' in l)
broken = [name for name, t in hello["tools"].items() if not t["version"] or str(out) not in (t["path"] or "")]
if broken:
    sys.exit(f"не найдены или не запускаются свои программы: {broken} — {hello['tools']}")

def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()

def tree(path):
    files = [p for p in path.rglob("*") if p.is_file()]
    return {"files": len(files), "size": sum(p.stat().st_size for p in files)}

env = os.environ
files = {}
for name in ("dalywave-fetcher", "deno", "ffmpeg", "ffprobe", "fpcalc"):
    path = out / f"{name}-{triple}"
    files[path.name] = {"size": path.stat().st_size, "sha256": sha(path)}
files[f"dalywave-fetcher-{triple}"].update(
    version=hello["version"],
    python=env["PYTHON_VERSION"],
    contents={p: md.version(p) for p in ("yt-dlp", "yt-dlp-ejs", "ytmusicapi", "bgutil-ytdlp-pot-provider", "numpy", "httpx", "pyinstaller")},
    license="своя часть — без лицензии (личный проект); внутри: Python (PSF), yt-dlp (Unlicense), ytmusicapi (MIT), numpy (BSD), плагин bgutil (GPL-3.0)",
)
files[f"deno-{triple}"].update(version=env["DENO_VERSION"], source=env["DENO_URL"], sourceSha256=env["DENO_SHA256"], license="MIT")
for name in ("ffmpeg", "ffprobe"):
    files[f"{name}-{triple}"].update(
        version=env["FFMPEG_VERSION"], source=env["FFMPEG_URL"], sourceSha256=env["FFMPEG_SHA256"],
        configure=env["FFMPEG_CONFIGURE"], license="LGPL-2.1-or-later (собран без --enable-gpl и --enable-nonfree)",
    )
files[f"fpcalc-{triple}"].update(
    version=env["CHROMAPRINT_VERSION"], source=env["FPCALC_URL"], sourceSha256=env["FPCALC_SHA256"],
    license="LGPL-2.1-or-later (chromaprint MIT + статически вшитый FFmpeg без GPL)",
)
files["resources/bgutil-server"] = {
    **tree(out / "resources" / "bgutil-server"),
    "version": env["BGUTIL_VERSION"], "source": env["BGUTIL_URL"], "sourceSha256": env["BGUTIL_SHA256"],
    "canvas": {"version": env["CANVAS_VERSION"], "source": env["CANVAS_URL"], "sourceSha256": env["CANVAS_SHA256"]},
    "license": "GPL-3.0-only (bgutil), node_modules — MIT/BSD/ISC; canvas — MIT, внутри cairo/pango (LGPL/MPL)",
}
manifest = {
    "target": triple,
    "builtAt": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "tools": hello["tools"],
    "files": files,
}
(out / "MANIFEST.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
total = sum(f["size"] for f in files.values())
for name, f in files.items():
    print(f"{f['size'] / 1e6:8.1f} МБ  {name}")
print(f"{total / 1e6:8.1f} МБ  всего")
PY
log "готово: $OUT"
