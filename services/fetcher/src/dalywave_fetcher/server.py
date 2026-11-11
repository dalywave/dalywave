"""HTTP API сервиса на :8402. Вызывается Rust-воркером изнутри docker-сети."""

from __future__ import annotations

import logging
import shutil
import tempfile
import threading
import time
from collections.abc import AsyncIterator, Iterator
from contextlib import asynccontextmanager, contextmanager
from pathlib import Path
from typing import Any

import httpx
from fastapi import FastAPI, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse

from . import __version__
from .acquire import Fetcher
from .config import Settings
from .deezer import DeezerError, DeezerNotFoundError
from .download import tool_versions
from .models import AcquireRequest, MatchRequest, SelftestRequest

log = logging.getLogger("dalywave_fetcher")


class BusyError(Exception):
    pass


def _error(status: int, code: str, message: str) -> JSONResponse:
    return JSONResponse(status_code=status, content={"error": {"code": code, "message": message}})


def pot_status(url: str | None) -> dict[str, Any]:
    if not url:
        return {"configured": False, "reachable": False}
    try:
        response = httpx.get(f"{url.rstrip('/')}/ping", timeout=3.0)
        data = response.json()
    except (httpx.HTTPError, ValueError) as e:
        return {"configured": True, "reachable": False, "error": type(e).__name__}
    return {"configured": True, "reachable": response.is_success, "version": data.get("version")}


# Куки, по которым видно, что сессия Google жива; самая ранняя дата — срок cookies.
_AUTH_COOKIES = {"SID", "__Secure-1PSID", "__Secure-3PSID", "LOGIN_INFO", "SAPISID"}


def cookies_status(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {"configured": False}
    if not path.is_file():
        return {"configured": True, "exists": False}
    expiries = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        parts = line.removeprefix("#HttpOnly_").split("\t")
        if (
            len(parts) == 7
            and "youtube.com" in parts[0]
            and parts[5] in _AUTH_COOKIES
            and parts[4].isdigit()
            and int(parts[4]) > 0
        ):
            expiries.append(int(parts[4]))
    expires = min(expiries) if expiries else None
    return {
        "configured": True,
        "exists": True,
        "authCookies": len(expiries),
        "expiresAt": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(expires)) if expires else None,
    }


def create_app(settings: Settings | None = None, fetcher: Fetcher | None = None) -> FastAPI:
    settings = settings or Settings.from_env()
    holder: dict[str, Fetcher] = {}
    # Скачивание — строго по одному: так нас реже замечает защита YouTube.
    # Поиск кандидатов не качает файлы и идёт под своим замком, чтобы «Спорные»
    # в админке не ждали массовую загрузку.
    job_lock = threading.Lock()
    match_lock = threading.Lock()

    @asynccontextmanager
    async def lifespan(_: FastAPI) -> AsyncIterator[None]:
        holder["fetcher"] = fetcher or Fetcher(settings)
        yield

    app = FastAPI(title="DalyWave fetcher", version=__version__, lifespan=lifespan)

    @contextmanager
    def exclusive(lock: threading.Lock) -> Iterator[Fetcher]:
        if not lock.acquire(blocking=False):
            raise BusyError
        try:
            yield holder["fetcher"]
        finally:
            lock.release()

    @app.exception_handler(BusyError)
    async def busy(_: Request, __: BusyError) -> JSONResponse:
        return _error(409, "busy", "Сервис занят другим заданием, повторите позже")

    @app.exception_handler(DeezerNotFoundError)
    async def not_found(_: Request, __: DeezerNotFoundError) -> JSONResponse:
        return _error(404, "deezer_not_found", "Такого трека нет в Deezer")

    @app.exception_handler(DeezerError)
    async def deezer_error(_: Request, exc: DeezerError) -> JSONResponse:
        return _error(502, "deezer_error", f"Deezer не ответил: {exc}")

    @app.exception_handler(Exception)
    async def unexpected(_: Request, exc: Exception) -> JSONResponse:
        log.exception("необработанная ошибка", exc_info=exc)
        return _error(500, "internal", f"Внутренняя ошибка сервиса: {type(exc).__name__}")

    @app.exception_handler(RequestValidationError)
    async def invalid(_: Request, exc: RequestValidationError) -> JSONResponse:
        first = exc.errors()[0] if exc.errors() else {}
        where = ".".join(str(p) for p in first.get("loc", ()))
        return _error(422, "invalid_request", f"Неверный запрос: {where} — {first.get('msg')}")

    @app.get("/v1/healthz")
    def healthz() -> dict[str, Any]:
        pot = pot_status(settings.pot_url)
        return {
            "ok": not settings.pot_url or pot["reachable"],
            "version": __version__,
            "busy": job_lock.locked(),
            "tools": tool_versions(settings),
            "pot": pot,
            "cookies": cookies_status(settings.cookies_file),
            "setup": settings.describe(),
        }

    @app.post("/v1/match")
    def match(req: MatchRequest) -> dict[str, Any]:
        with exclusive(match_lock) as f:
            return f.match(req.deezer_track_id, videos=req.videos).to_json()

    @app.post("/v1/acquire")
    def acquire(req: AcquireRequest) -> dict[str, Any]:
        with exclusive(job_lock) as f:
            result = f.acquire(
                req.deezer_track_id,
                max_attempts=req.max_attempts,
                upload=req.upload.target() if req.upload else None,
                prefer_video_ids=req.prefer_video_ids,
                exclude_video_ids=req.exclude_video_ids,
                direct_url=req.direct_url,
            )
            return result.to_json()

    @app.post("/v1/selftest")
    def selftest(req: SelftestRequest | None = None) -> dict[str, Any]:
        """Кнопка «проверить скачивание» в админке: один известный трек целиком."""
        track_id = (req.deezer_track_id if req else None) or settings.selftest_track_id
        with exclusive(job_lock) as f:
            work = Path(tempfile.mkdtemp(prefix="selftest-", dir=f.jobs_dir()))
            try:
                result = f.acquire(track_id, work)
            finally:
                shutil.rmtree(work, ignore_errors=True)
            data = result.to_json(max_candidates=3)
            chosen = data["chosen"] or {}
            return {
                "ok": result.status == "verified",
                "status": result.status,
                "deezerTrackId": track_id,
                "error": data["error"],
                "download": chosen.get("download"),
                "verify": chosen.get("verify"),
                "attempts": data["attempts"],
                "timingsMs": data["timingsMs"],
                "setup": data["setup"],
                "versions": data["versions"],
            }

    return app


def run(host: str = "0.0.0.0", port: int = 8402) -> None:
    import uvicorn

    uvicorn.run(create_app(), host=host, port=port, log_level="info")
