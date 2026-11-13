#!/usr/bin/env bash
# FFmpeg и ffprobe для загрузчика в приложении Windows: кросс-сборка на Linux (mingw-w64) из тех
# же исходников и с теми же флагами, что build.sh для macOS — звук и клипы ленты, LGPL 2.1+,
# статически, без внешних DLL. Результат — папка с ffmpeg.exe, ffprobe.exe и лицензиями; её принимает
# build-windows.ps1 (-Ffmpeg). В CI — задание ffmpeg-windows в .github/workflows/desktop.yml.
#
#   services/fetcher/sidecar/ffmpeg-windows.sh <папка результата>
#
# Нужны (Ubuntu 24.04): gcc (configure собирает им вспомогательные программы), mingw-w64,
# libz-mingw-w64-dev, nasm, make, curl, xz-utils.
set -euo pipefail

OUT=${1:?папка результата}
# Абсолютный путь: дальше скрипт переходит в папку исходников, и относительный указывал бы не туда
mkdir -p "$OUT" && OUT=$(cd "$OUT" && pwd)
WORK=${WORK:-$(mktemp -d)}
FFMPEG_VERSION=9.0.2
# Совпадает с build.sh (и формулой Homebrew ffmpeg 9.0.2)
FFMPEG_SHA256=8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e
FFMPEG_URL=https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz
CROSS=x86_64-w64-mingw32-

# Те же компоненты, что FFMPEG_FLAGS в build.sh (меняются вместе): звук и клипы ленты. Вместо
# системных библиотек macOS — системные Windows: TLS через SChannel, кодировщик H.264 — Media
# Foundation (h264_mf; без видеокарты Windows берёт свой программный; без d3d11va mfenc.c не
# собирается). Кросс-сборка под Windows и статическая компоновка libgcc и zlib.
# shellcheck disable=SC2054 # запятые — списки компонентов configure, а не элементы массива
FLAGS=(
  --enable-cross-compile --target-os=mingw32 --arch=x86_64 --cross-prefix=$CROSS
  --enable-static --disable-shared
  --disable-autodetect --enable-zlib --enable-schannel --enable-mediafoundation --enable-d3d11va
  --disable-doc --disable-debug
  --disable-programs --enable-ffmpeg --enable-ffprobe
  --disable-avdevice
  --disable-everything
  --enable-protocol=file,pipe,http,https,tcp,tls,httpproxy,crypto
  --enable-demuxer=aac,ac3,aiff,ape,asf,au,caf,dsf,dts,eac3,flac,loas,matroska,mlp,mov,mp3,mpegts,ogg,truehd,tta,w64,wav,wv,xwma,hls,rawvideo
  --enable-muxer=mp4,ipod,mp3,flac,ogg,null,pcm_s16le,adts,wav,matroska
  --enable-decoder=aac,aac_fixed,aac_latm,ac3,eac3,alac,ape,flac,mp1,mp1float,mp2,mp2float,mp3,mp3float,opus,vorbis,wavpack,tta,wmav1,wmav2,wmapro,wmalossless,dca,mlp,truehd,dsd_lsbf,dsd_msbf,dsd_lsbf_planar,dsd_msbf_planar
  --enable-decoder='pcm_*'
  --enable-decoder=h264,hevc,vp8,vp9,rawvideo
  --enable-encoder=pcm_s16le,wrapped_avframe,h264_mf
  --enable-parser=aac,aac_latm,ac3,dca,flac,mlp,mpegaudio,opus,vorbis,h264,hevc,vp8,vp9
  --enable-bsfs
  --enable-filter=aformat,anull,aresample,asplit,atrim,ebur128,format,null,trim
  --enable-filter=scale,fps,crop,setpts,freezedetect
  --extra-ldflags=-static
)

for tool in curl sha256sum tar make nasm gcc "${CROSS}gcc"; do
  command -v "$tool" >/dev/null 2>&1 || { echo "ошибка: для сборки нужен $tool" >&2; exit 1; }
done

mkdir -p "$WORK" "$OUT"
tarball=$WORK/ffmpeg-$FFMPEG_VERSION.tar.xz
if ! echo "$FFMPEG_SHA256  $tarball" | sha256sum -c - >/dev/null 2>&1; then
  curl -fsSL --retry 3 -o "$tarball" "$FFMPEG_URL"
  echo "$FFMPEG_SHA256  $tarball" | sha256sum -c - >/dev/null || { echo "ошибка: sha256 не совпала: $FFMPEG_URL" >&2; exit 1; }
fi
src=$WORK/ffmpeg-$FFMPEG_VERSION
rm -rf "$src"
tar xf "$tarball" -C "$WORK"
cd "$src"
./configure "${FLAGS[@]}" >"$WORK/configure.log" 2>&1 || { tail -40 "$WORK/configure.log"; exit 1; }
grep -q "^License: LGPL version 2.1 or later" "$WORK/configure.log" || {
  echo "ошибка: FFmpeg настроен не под LGPL" >&2
  grep "^License" "$WORK/configure.log" >&2
  exit 1
}
# Клипы: без https и кодировщика H.264 загрузчик их не возьмёт (clip.video_support)
for component in H264_MF_ENCODER HTTPS_PROTOCOL; do
  grep -q "^#define CONFIG_$component 1" config_components.h || {
    echo "ошибка: configure выключил $component (нет заголовков mingw?)" >&2
    exit 1
  }
done
make -j"$(nproc)" >"$WORK/make.log" 2>&1 || { tail -60 "$WORK/make.log"; exit 1; }

cp ffmpeg.exe ffprobe.exe "$OUT/"
"${CROSS}strip" "$OUT/ffmpeg.exe" "$OUT/ffprobe.exe"
cp COPYING.LGPLv2.1 LICENSE.md "$OUT/"
echo "$FFMPEG_VERSION ${FLAGS[*]}" >"$OUT/BUILD.txt"
# Только системные DLL Windows: KERNEL32, msvcrt/ucrt, bcrypt и подобные — ничего из mingw
"${CROSS}objdump" -p "$OUT/ffmpeg.exe" | grep "DLL Name" | tee "$WORK/dlls.txt"
if grep -qiE "libgcc|libwinpthread|libstdc|zlib1|libz" "$WORK/dlls.txt"; then
  echo "ошибка: ffmpeg.exe тянет DLL mingw" >&2
  exit 1
fi
ls -la "$OUT"
