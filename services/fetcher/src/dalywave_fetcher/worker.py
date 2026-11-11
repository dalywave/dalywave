"""«Домашний загрузчик»: тот же пакет на домашнем Mac забирает задания с сервера,
скачивает со своего адреса, кладёт файл по подписанной ссылке в R2 и отчитывается.

Протокол (серверную сторону реализует Rust-воркер, все запросы с
`Authorization: Bearer <токен загрузчика>`):

    POST {server}/api/fetcher/jobs/claim
        {"workerId": "...", "version": "0.1.0", "capabilities": {...}}
        ← 204 — заданий нет; 200 {"job": Job} (форма — `models.Job`)
    POST {server}/api/fetcher/jobs/{id}/result
        {"workerId": "...", "result": <ответ POST /v1/acquire>}
        ← 204

Задание выдаётся в аренду (`leaseExpiresAt`); не пришёл результат — сервер
возвращает его в очередь.
"""

from __future__ import annotations

import random
import socket
import sys
import time
from collections.abc import Callable
from typing import Any

import httpx

from . import __version__
from .acquire import SYSTEMIC_ERRORS, Fetcher
from .models import Job

# YouTube заподозрил бота — долгая пауза лучше, чем бан адреса.
SYSTEMIC_PAUSE_S = 15 * 60


def _log(msg: str) -> None:
    print(time.strftime("%H:%M:%S"), msg, file=sys.stderr, flush=True)


def run_worker(
    fetcher: Fetcher,
    server: str,
    token: str,
    *,
    worker_id: str | None = None,
    once: bool = False,
    poll_s: float = 30.0,
    cooldown_s: float = 5.0,
    log: Callable[[str], None] = _log,
    sleep: Callable[[float], None] = time.sleep,
    transport: httpx.BaseTransport | None = None,
) -> int:
    worker_id = worker_id or socket.gethostname()
    s = fetcher.settings
    hello = {
        "workerId": worker_id,
        "version": __version__,
        "capabilities": {
            "kinds": ["acquire"],
            "potProvider": s.pot_url is not None,
            "cookies": s.cookies_file is not None,
            "playerClients": list(s.player_clients),
        },
    }
    http = httpx.Client(
        base_url=server.rstrip("/"),
        headers={
            "Authorization": f"Bearer {token}",
            "User-Agent": f"DalyWave-fetcher/{__version__}",
        },
        timeout=30.0,
        transport=transport,
    )
    failures = 0
    with http:
        while True:
            try:
                response = http.post("/api/fetcher/jobs/claim", json=hello)
            except httpx.HTTPError as e:
                failures += 1
                log(f"сервер недоступен ({type(e).__name__}), жду")
                sleep(min(poll_s * 2**failures, 600))
                continue
            if response.status_code in (401, 403):
                log("сервер не принял токен загрузчика")
                return 2
            if response.status_code == 204:
                failures = 0
                if once:
                    return 0
                sleep(poll_s + random.uniform(0, poll_s / 3))
                continue
            if not response.is_success:
                failures += 1
                log(f"сервер ответил {response.status_code}, жду")
                sleep(min(poll_s * 2**failures, 600))
                continue
            failures = 0
            job = Job.model_validate(response.json()["job"])
            log(f"задание {job.id}: трек Deezer {job.deezer_track_id}")
            result = fetcher.acquire(
                job.deezer_track_id,
                upload=job.upload.target() if job.upload else None,
                max_attempts=job.max_attempts,
                prefer_video_ids=job.prefer_video_ids,
                exclude_video_ids=job.exclude_video_ids,
            )
            log(f"задание {job.id}: {result.status}")
            _report(http, job, worker_id, result.to_json(), log, sleep)
            if once:
                return 0
            code = (result.error or {}).get("code")
            sleep(SYSTEMIC_PAUSE_S if code in SYSTEMIC_ERRORS else cooldown_s)


def _report(
    http: httpx.Client,
    job: Job,
    worker_id: str,
    result: dict[str, Any],
    log: Callable[[str], None],
    sleep: Callable[[float], None],
) -> None:
    payload = {"workerId": worker_id, "result": result}
    for attempt in range(5):
        try:
            http.post(f"/api/fetcher/jobs/{job.id}/result", json=payload).raise_for_status()
            return
        except httpx.HTTPError as e:
            log(f"не удалось отправить результат ({type(e).__name__}), повтор")
            sleep(2**attempt)
    log(f"результат задания {job.id} потерян — сервер вернёт его в очередь по аренде")
