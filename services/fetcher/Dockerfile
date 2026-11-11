# DalyWave fetcher: yt-dlp + Deno + ffmpeg + chromaprint. Собирается под arm64 и amd64.
#   docker build -t dalywave-fetcher:0.2.0 services/fetcher

# --- Deno: официальный бинарник под архитектуру сборки, с проверкой sha256 ---
FROM python:3.13-slim AS deno
# Версия как в yt-dlp[pin-deno]; минимально нужна 2.3.
ARG DENO_VERSION=2.9.8
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && rm -rf /var/lib/apt/lists/*
RUN set -eu; \
    case "$(uname -m)" in \
      x86_64) target=x86_64-unknown-linux-gnu ;; \
      aarch64) target=aarch64-unknown-linux-gnu ;; \
      *) echo "неизвестная архитектура $(uname -m)"; exit 1 ;; \
    esac; \
    base="https://github.com/denoland/deno/releases/download/v${DENO_VERSION}/deno-${target}.zip"; \
    curl -fsSL -o /tmp/deno.zip "$base"; \
    curl -fsSL -o /tmp/deno.zip.sha256sum "$base.sha256sum"; \
    echo "$(awk '{print $1}' /tmp/deno.zip.sha256sum)  /tmp/deno.zip" | sha256sum -c -; \
    unzip -q /tmp/deno.zip -d /usr/local/bin; \
    /usr/local/bin/deno --version

# --- Python-окружение: uv нужен только при сборке ---
FROM python:3.13-slim AS build
COPY --from=ghcr.io/astral-sh/uv:0.13.7 /uv /usr/local/bin/uv
ENV UV_COMPILE_BYTECODE=1 \
    UV_LINK_MODE=copy \
    UV_PYTHON_DOWNLOADS=never \
    UV_PROJECT_ENVIRONMENT=/app/.venv
WORKDIR /app
COPY pyproject.toml uv.lock README.md ./
RUN uv sync --frozen --no-dev --no-install-project
COPY src ./src
RUN uv sync --frozen --no-dev --no-editable

# --- Итоговый образ ---
FROM python:3.13-slim
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PATH="/app/.venv/bin:${PATH}" \
    FETCHER_DATA_DIR=/data
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg libchromaprint-tools ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=deno /usr/local/bin/deno /usr/local/bin/deno
COPY --from=build /app/.venv /app/.venv
RUN useradd --create-home --uid 10001 fetcher \
    && mkdir -p /data \
    && chown fetcher:fetcher /data
USER fetcher
WORKDIR /app
VOLUME ["/data"]
EXPOSE 8402

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8402/v1/healthz', timeout=8)"]

CMD ["uvicorn", "dalywave_fetcher.server:create_app", "--factory", "--host", "0.0.0.0", "--port", "8402"]
