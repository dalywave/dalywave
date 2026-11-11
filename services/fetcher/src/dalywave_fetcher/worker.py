"""«Домашний загрузчик»: тот же пакет на домашнем Mac забирает задания с сервера,
скачивает со своего адреса, кладёт файл по подписанной ссылке в R2 и отчитывается.

Протокол (все запросы с `Authorization: Bearer <токен загрузчика dwk_…>`):

    POST {server}/api/fetcher/jobs/claim
        {"workerId": "...", "version": "0.2.0", "capabilities": {...}}
        ← 200 {"job": Job} (форма — `models.Job`); 204 — заданий нет: сервер уже подождал
          до ~25 секунд, `Retry-After` — через сколько секунд спросить снова
    POST {server}/api/fetcher/jobs/{id}/lease
        ← 200 {"leaseExpiresAt": ...} — аренда продлена; 409 — задание уже не наше
    POST {server}/api/fetcher/jobs/{id}/result
        {"workerId": "...", "result": <ответ POST /v1/acquire>}
        ← 204; 409 — задание уже не наше (аренда кончилась, его отменили)

Задание выдаётся в аренду (`leaseExpiresAt`): пока идёт скачивание, загрузчик продлевает её
раз в минуту. Не отчитался к сроку — сервер возвращает задание в очередь.
"""

from __future__ import annotations

import random
import socket
import sys
import threading
import time
from collections.abc import Callable
from typing import Any, Self

import httpx

from . import __version__
from .acquire import SYSTEMIC_ERRORS, Fetcher
from .download import YT_DLP_VERSION
from .models import Job

# YouTube заподозрил бота — долгая пауза лучше, чем бан адреса.
SYSTEMIC_PAUSE_S = 15 * 60
# Как часто продлевать аренду, пока идёт скачивание.
LEASE_RENEW_S = 60.0


def _log(msg: str) -> None:
    print(time.strftime("%H:%M:%S"), msg, file=sys.stderr, flush=True)


def _retry_after(response: httpx.Response) -> float | None:
    """`Retry-After` в секундах: так сервер говорит, когда спросить снова."""
    value = response.headers.get("Retry-After", "").strip()
    try:
        return max(0.0, float(value)) if value else None
    except ValueError:
        return None


class _LeaseKeeper:
    """Продлевает аренду задания в фоне, пока идёт скачивание: оно длится до нескольких минут."""

    def __init__(
        self,
        http: httpx.Client,
        job: Job,
        worker_id: str,
        every_s: float,
        log: Callable[[str], None],
    ) -> None:
        self._http = http
        self._job = job
        self._worker_id = worker_id
        self._every_s = every_s
        self._log = log
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name=f"lease-{job.id}", daemon=True)
        self.renewals = 0
        self.lost = False

    def __enter__(self) -> Self:
        self._thread.start()
        return self

    def __exit__(self, *_: object) -> None:
        self._stop.set()
        self._thread.join(timeout=5)

    def _run(self) -> None:
        while not self._stop.wait(self._every_s):
            try:
                response = self._http.post(
                    f"/api/fetcher/jobs/{self._job.id}/lease", json={"workerId": self._worker_id}
                )
            except httpx.HTTPError as e:
                self._log(f"аренда задания {self._job.id} не продлена ({type(e).__name__})")
                continue
            if response.status_code == 409:
                # Задание отдали другому: дорабатываем, но отчёт сервер уже не примет
                self.lost = True
                self._log(f"задание {self._job.id} уже не наше: аренда кончилась")
                return
            if response.is_success:
                self.renewals += 1


def run_worker(
    fetcher: Fetcher,
    server: str,
    token: str,
    *,
    worker_id: str | None = None,
    once: bool = False,
    poll_s: float = 30.0,
    cooldown_s: float = 5.0,
    lease_renew_s: float = LEASE_RENEW_S,
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
            "ytDlp": YT_DLP_VERSION,
        },
    }
    http = httpx.Client(
        base_url=server.rstrip("/"),
        headers={
            "Authorization": f"Bearer {token}",
            "User-Agent": f"DalyWave-fetcher/{__version__}",
        },
        # Сервер держит запрос задания до ~25 секунд, пока заданий нет
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
                # Сервер с long-poll уже подождал и сам говорит, когда спросить снова
                retry = _retry_after(response)
                sleep(retry if retry is not None else poll_s + random.uniform(0, poll_s / 3))
                continue
            if not response.is_success:
                failures += 1
                log(f"сервер ответил {response.status_code}, жду")
                sleep(min(poll_s * 2**failures, 600))
                continue
            failures = 0
            job = Job.model_validate(response.json()["job"])
            target = job.direct_url or job.deezer_track_id
            log(f"задание {job.id}: трек Deezer {job.deezer_track_id} ({target})")
            with _LeaseKeeper(http, job, worker_id, lease_renew_s, log):
                result = fetcher.acquire(
                    job.deezer_track_id,
                    upload=job.upload.target() if job.upload else None,
                    max_attempts=job.max_attempts,
                    prefer_video_ids=job.prefer_video_ids,
                    exclude_video_ids=job.exclude_video_ids,
                    direct_url=job.direct_url,
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
            response = http.post(f"/api/fetcher/jobs/{job.id}/result", json=payload)
        except httpx.HTTPError as e:
            log(f"не удалось отправить результат ({type(e).__name__}), повтор")
            sleep(2**attempt)
            continue
        if response.is_success:
            return
        if 400 <= response.status_code < 500 and response.status_code != 429:
            # Повтор не поможет: задание уже не наше или сервер отверг отчёт
            log(f"сервер не принял результат задания {job.id}: {response.status_code}")
            return
        log(f"сервер ответил {response.status_code} на результат, повтор")
        sleep(2**attempt)
    log(f"результат задания {job.id} потерян — сервер вернёт его в очередь по аренде")
