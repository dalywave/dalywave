"""HTTP API на :8403. Вызывается Rust-воркером изнутри docker-сети; в базу сервис не ходит."""

from __future__ import annotations

import logging
import platform
import tempfile
import threading
import time
from collections.abc import AsyncIterator, Iterator
from contextlib import asynccontextmanager, contextmanager
from pathlib import Path
from typing import Any, Literal

import httpx
import onnxruntime as ort
from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from pydantic import BaseModel, ConfigDict, Field, model_validator

from . import __version__
from .analysis import Analyzer, TooShortError
from .audio import AudioError, decode, ffmpeg_version
from .config import Settings
from .download import read_lock, verify
from .manifest import MODELS, versions

log = logging.getLogger("dalywave_ml")


class ApiError(Exception):
    def __init__(self, status: int, code: str, message: str) -> None:
        super().__init__(message)
        self.status = status
        self.code = code
        self.message = message


class AnalyzeRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    url: str | None = Field(
        default=None, description="ссылка на файл (подписанная R2, превью Deezer)"
    )
    path: str | None = Field(default=None, description="файл внутри ML_FILES_ROOT")
    mode: Literal["preview", "full"] = "full"

    @model_validator(mode="after")
    def one_source(self) -> AnalyzeRequest:
        if (self.url is None) == (self.path is None):
            raise ValueError("нужно ровно одно из полей url и path")
        if self.url is not None and not self.url.startswith(("http://", "https://")):
            raise ValueError("url должен начинаться с http:// или https://")
        return self


class EmbedTextRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    text: str = Field(min_length=1, max_length=500)


def _error(status: int, code: str, message: str) -> JSONResponse:
    return JSONResponse(status_code=status, content={"error": {"code": code, "message": message}})


def fetch(url: str, dest: Path, settings: Settings) -> None:
    """Скачивает файл целиком во временный файл; R2 и Deezer отдают его быстро."""
    limit = settings.max_download_mb * 1024 * 1024
    size = 0
    try:
        with httpx.stream(
            "GET", url, follow_redirects=True, timeout=settings.download_timeout_s
        ) as r:
            if r.status_code != 200:
                raise ApiError(502, "download_failed", f"Файл не скачался: HTTP {r.status_code}")
            with dest.open("wb") as f:
                for chunk in r.iter_bytes(1 << 16):
                    size += len(chunk)
                    if size > limit:
                        raise ApiError(
                            413, "too_large", f"Файл больше {settings.max_download_mb} МБ"
                        )
                    f.write(chunk)
    except httpx.HTTPError as e:
        raise ApiError(502, "download_failed", f"Файл не скачался: {type(e).__name__}") from e


def resolve_path(raw: str, settings: Settings) -> Path:
    if settings.files_root is None:
        raise ApiError(403, "path_disabled", "Чтение файлов по пути выключено (нет ML_FILES_ROOT)")
    path = Path(raw)
    if not path.is_absolute():
        path = settings.files_root / path
    path = path.resolve()
    if not path.is_relative_to(settings.files_root):
        raise ApiError(403, "path_forbidden", "Путь вне разрешённой папки")
    if not path.is_file():
        raise ApiError(404, "not_found", "Файл не найден")
    return path


def create_app(settings: Settings | None = None, analyzer: Analyzer | None = None) -> FastAPI:
    settings = settings or Settings.from_env()
    state: dict[str, Any] = {"analyzer": analyzer, "error": None, "problems": None}
    # Анализ — строго один за раз на процесс: модели и так занимают все ядра.
    analysis_lock = threading.Lock()
    text_lock = threading.Lock()

    @asynccontextmanager
    async def lifespan(_: FastAPI) -> AsyncIterator[None]:
        if state["analyzer"] is None:
            started = time.perf_counter()
            try:
                state["analyzer"] = Analyzer(settings)
                log.info("модели загружены за %.1f с", time.perf_counter() - started)
            except Exception as e:  # сервис поднимается и честно отвечает в healthz
                log.exception("модели не загрузились")
                state["error"] = f"{type(e).__name__}: {e}"
        yield

    app = FastAPI(title="DalyWave ml", version=__version__, lifespan=lifespan)

    def get_analyzer() -> Analyzer:
        if state["analyzer"] is None:
            raise ApiError(503, "models_missing", "Модели не загружены: см. /v1/healthz")
        return state["analyzer"]

    @contextmanager
    def exclusive() -> Iterator[None]:
        if not analysis_lock.acquire(blocking=False):
            raise ApiError(409, "busy", "Сервис занят другим анализом, повторите позже")
        try:
            yield
        finally:
            analysis_lock.release()

    @app.exception_handler(ApiError)
    async def api_error(_: Request, exc: ApiError) -> JSONResponse:
        return _error(exc.status, exc.code, exc.message)

    @app.exception_handler(RequestValidationError)
    async def invalid(_: Request, exc: RequestValidationError) -> JSONResponse:
        first = exc.errors()[0] if exc.errors() else {}
        where = ".".join(str(p) for p in first.get("loc", ()) if p != "body")
        message = str(first.get("msg", "")).removeprefix("Value error, ")
        return _error(422, "invalid_request", f"Неверный запрос: {where} — {message}".strip())

    @app.exception_handler(Exception)
    async def unexpected(_: Request, exc: Exception) -> JSONResponse:
        log.exception("необработанная ошибка", exc_info=exc)
        return _error(500, "internal", f"Внутренняя ошибка сервиса: {type(exc).__name__}")

    @app.get("/v1/healthz")
    def healthz() -> dict[str, Any]:
        return {
            "ok": state["analyzer"] is not None,
            "version": __version__,
            "busy": analysis_lock.locked(),
            "modelsLoaded": state["analyzer"] is not None,
            "error": state["error"],
            "runtime": {
                "python": platform.python_version(),
                "onnxruntime": ort.__version__,
                "ffmpeg": ffmpeg_version(),
                "machine": platform.machine(),
            },
            "setup": settings.describe(),
        }

    @app.get("/v1/models")
    def models() -> dict[str, Any]:
        lock = read_lock(settings.models_dir)
        # Суммы ~600 МБ файлов считаются один раз: модели во время работы не меняются.
        if state["problems"] is None:
            state["problems"] = verify(settings.models_dir)
        return {
            "versions": versions(),
            "problems": state["problems"],
            "models": [
                {
                    "key": m.key,
                    "name": m.name,
                    "version": m.version,
                    "license": m.license,
                    "source": m.source,
                    "files": [
                        {
                            "path": f.path,
                            "url": f.url,
                            "sourceSha256": f.sha256,
                            "sha256": (lock.get(f.path) or {}).get("sha256"),
                            "size": (lock.get(f.path) or {}).get("size"),
                            "fp16Storage": f.fp16_storage,
                        }
                        for f in m.files
                    ],
                }
                for m in MODELS
            ],
        }

    @app.post("/v1/analyze")
    def analyze(req: AnalyzeRequest) -> dict[str, Any]:
        analyzer = get_analyzer()
        with exclusive():
            started = time.perf_counter()
            with tempfile.TemporaryDirectory(dir=settings.work_dir, prefix="ml-") as tmp:
                if req.url is not None:
                    source = Path(tmp) / "input"
                    fetch(req.url, source, settings)
                else:
                    source = resolve_path(req.path or "", settings)
                fetched = time.perf_counter()
                try:
                    audio = decode(source, max_seconds=settings.max_duration_s)
                except AudioError as e:
                    raise ApiError(422, "undecodable", f"Файл не читается: {e}") from e
            decoded = time.perf_counter()
            try:
                result = analyzer.analyze(audio, req.mode)
            except TooShortError as e:
                raise ApiError(422, "too_short", f"Слишком короткий звук: {e}") from e
            result["timingsMs"] = {
                "fetch": round((fetched - started) * 1000),
                "decode": round((decoded - fetched) * 1000),
                **result["timingsMs"],
                "total": round((time.perf_counter() - started) * 1000),
            }
            log.info(
                "анализ %s: %.1f с звука за %d мс",
                req.mode,
                audio.duration_s,
                result["timingsMs"]["total"],
            )
            return result

    @app.post("/v1/embed-text")
    def embed_text(req: EmbedTextRequest) -> dict[str, Any]:
        analyzer = get_analyzer()
        with text_lock:
            return analyzer.embed_text(req.text)

    return app
