# DalyWave ml: анализ звучания на CPU (onnxruntime), модели внутри образа.
# Собирается под arm64 (сервер) и amd64:
#   docker build -t dalywave-ml:0.1.0 services/ml

ARG PYTHON_IMAGE=python:3.13-slim

# --- ffmpeg только для звука: демультиплексоры и декодеры, без сети и видео ---
# Пакет ffmpeg из Debian тянет ~420 МБ библиотек; этот бинарник — несколько мегабайт.
FROM ${PYTHON_IMAGE} AS ffmpeg
ARG FFMPEG_VERSION=8.1.3
# Подпись релиза проверена ключом FFmpeg (FCF9 86EA 15E6 E293 A564 4F10 B432 2F04 D676 58D8).
ARG FFMPEG_SHA256=7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends build-essential ca-certificates curl xz-utils; \
    curl -fsSL -o /tmp/ffmpeg.tar.xz "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"; \
    echo "${FFMPEG_SHA256}  /tmp/ffmpeg.tar.xz" | sha256sum -c -; \
    mkdir /tmp/src; \
    tar -xJf /tmp/ffmpeg.tar.xz -C /tmp/src --strip-components=1; \
    cd /tmp/src; \
    ./configure --prefix=/opt/ffmpeg \
        --disable-everything --disable-autodetect --disable-doc --disable-debug \
        --disable-network --disable-ffplay --disable-ffprobe --disable-avdevice --disable-swscale \
        --enable-protocol=file,pipe \
        --enable-demuxer=aac,aiff,caf,flac,matroska,mov,mp3,ogg,w64,wav \
        --enable-decoder=aac,aac_latm,alac,flac,mp3,mp3float,opus,vorbis,pcm_f32be,pcm_f32le,pcm_s16be,pcm_s16le,pcm_s24be,pcm_s24le,pcm_s32be,pcm_s32le,pcm_u8 \
        --enable-parser=aac,aac_latm,flac,mpegaudio,opus,vorbis \
        --enable-encoder=pcm_f32le \
        --enable-muxer=wav \
        --enable-filter=abuffer,abuffersink,aformat,anull,aresample,atrim,buffer,buffersink,null; \
    make -j"$(nproc)"; \
    make install; \
    /opt/ffmpeg/bin/ffmpeg -hide_banner -version | head -1; \
    rm -rf /tmp/src /tmp/ffmpeg.tar.xz

# --- CLAP: ONNX из исходного чекпойнта LAION (веса на Hugging Face сломаны, см. README) ---
# Чекпойнт 2,35 ГБ и torch нужны только здесь и удаляются в том же слое; в образ идут
# два ONNX-файла (~390 МБ, веса в float16). Слой зависит только от export/ и пересобирается
# лишь при правке экспортёра.
FROM ${PYTHON_IMAGE} AS clap
COPY --from=ghcr.io/astral-sh/uv:0.13.7 /uv /usr/local/bin/uv
ENV UV_LINK_MODE=copy \
    UV_PYTHON_DOWNLOADS=never
WORKDIR /export
COPY export/ ./export/
RUN set -eux; \
    uv venv /opt/export; \
    uv pip install --python /opt/export/bin/python \
        --index-url https://download.pytorch.org/whl/cpu torch==2.11.0; \
    uv pip install --python /opt/export/bin/python -r export/requirements.txt; \
    /opt/export/bin/python export/export_clap.py --out /models; \
    rm -rf /opt/export /root/.cache

# --- Остальные модели: качаются с проверкой sha256 ---
# Слой зависит только от манифеста и загрузчика, а не от остального кода: правка анализа
# не перекачивает модели.
FROM ${PYTHON_IMAGE} AS models
COPY --from=ghcr.io/astral-sh/uv:0.13.7 /uv /usr/local/bin/uv
ENV UV_LINK_MODE=copy \
    UV_PYTHON_DOWNLOADS=never \
    UV_PROJECT_ENVIRONMENT=/opt/dl
WORKDIR /dl
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-install-project --no-default-groups
COPY src/dalywave_ml/__init__.py src/dalywave_ml/manifest.py src/dalywave_ml/download.py ./dalywave_ml/
COPY --from=clap /models /models
RUN /opt/dl/bin/python -m dalywave_ml.download /models

# --- Python-окружение сервиса: uv нужен только при сборке ---
FROM ${PYTHON_IMAGE} AS build
COPY --from=ghcr.io/astral-sh/uv:0.13.7 /uv /usr/local/bin/uv
ENV UV_COMPILE_BYTECODE=1 \
    UV_LINK_MODE=copy \
    UV_PYTHON_DOWNLOADS=never \
    UV_PROJECT_ENVIRONMENT=/app/.venv
WORKDIR /app
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-install-project --no-default-groups
COPY README.md ./
COPY src ./src
RUN uv sync --frozen --no-default-groups --no-editable

# --- Итоговый образ ---
FROM ${PYTHON_IMAGE}
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PATH="/app/.venv/bin:${PATH}" \
    ML_MODELS_DIR=/app/models
COPY --from=ffmpeg /opt/ffmpeg/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=models /models /app/models
COPY --from=build /app/.venv /app/.venv
RUN useradd --create-home --uid 10001 ml
USER ml
WORKDIR /app
EXPOSE 8403

HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8403/v1/healthz', timeout=8)"]

CMD ["ml", "serve", "--host", "0.0.0.0", "--port", "8403"]
