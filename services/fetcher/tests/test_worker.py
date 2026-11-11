"""Цикл «домашнего загрузчика» против подставного сервера."""

from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

import httpx

from dalywave_fetcher.config import Settings
from dalywave_fetcher.download import YT_DLP_VERSION
from dalywave_fetcher.worker import run_worker

JOB = {
    "id": "0193f0c2-7a1e-7c3a-9b1d-2f4e5a6b7c8d",
    "kind": "acquire",
    "deezerTrackId": 3040222091,
    "maxAttempts": 2,
    "preferVideoIds": [],
    "excludeVideoIds": ["bad"],
    "upload": {
        "url": "https://acc.r2.cloudflarestorage.com/dalywave/audio/x?X-Amz-Signature=s",
        "method": "PUT",
        "headers": {},
        "expiresAt": "2026-11-11T12:00:00Z",
    },
    "leaseExpiresAt": "2026-11-11T11:10:00Z",
}


class FakeResult:
    status = "verified"
    error = None

    def to_json(self) -> dict[str, Any]:
        return {"status": "verified", "deezerTrackId": 3040222091}


class FakeFetcher:
    def __init__(self) -> None:
        self.settings = Settings(data_dir=Path("data"))
        self.calls: list[dict[str, Any]] = []

    def acquire(self, track_id: int, **kw: Any) -> FakeResult:
        self.calls.append({"trackId": track_id, **kw})
        time.sleep(self.busy_s)
        return FakeResult()

    busy_s = 0.0


def test_worker_claims_acquires_and_reports() -> None:
    seen: list[tuple[str, dict[str, Any], str]] = []

    def handler(request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content or b"{}")
        seen.append((request.url.path, body, request.headers["Authorization"]))
        if request.url.path == "/api/fetcher/jobs/claim":
            return httpx.Response(200, json={"job": JOB})
        return httpx.Response(204)

    fetcher = FakeFetcher()
    code = run_worker(
        fetcher,  # type: ignore[arg-type]
        "https://wave.test",
        "secret",
        worker_id="mac",
        once=True,
        sleep=lambda _: None,
        log=lambda _: None,
        transport=httpx.MockTransport(handler),
    )
    assert code == 0
    claim, report = seen
    assert claim[0] == "/api/fetcher/jobs/claim"
    assert claim[1]["workerId"] == "mac"
    assert claim[1]["capabilities"]["kinds"] == ["acquire"]
    assert claim[2] == "Bearer secret"
    assert report[0] == f"/api/fetcher/jobs/{JOB['id']}/result"
    assert report[1] == {"workerId": "mac", "result": FakeResult().to_json()}
    call = fetcher.calls[0]
    assert call["trackId"] == 3040222091
    assert call["max_attempts"] == 2
    assert call["exclude_video_ids"] == ["bad"]
    assert call["upload"].url.startswith("https://acc.r2.cloudflarestorage.com/")


def test_worker_stops_on_rejected_token() -> None:
    code = run_worker(
        FakeFetcher(),  # type: ignore[arg-type]
        "https://wave.test",
        "wrong",
        once=True,
        sleep=lambda _: None,
        log=lambda _: None,
        transport=httpx.MockTransport(lambda _: httpx.Response(401)),
    )
    assert code == 2


def test_worker_once_without_jobs() -> None:
    code = run_worker(
        FakeFetcher(),  # type: ignore[arg-type]
        "https://wave.test",
        "t",
        once=True,
        sleep=lambda _: None,
        log=lambda _: None,
        transport=httpx.MockTransport(lambda _: httpx.Response(204)),
    )
    assert code == 0


def test_worker_waits_as_long_as_the_server_says() -> None:
    # Сервер с long-poll уже подождал: спрашиваем снова через Retry-After, а не через 30 с
    replies = iter([httpx.Response(204, headers={"Retry-After": "1"}), httpx.Response(401)])
    sleeps: list[float] = []
    code = run_worker(
        FakeFetcher(),  # type: ignore[arg-type]
        "https://wave.test",
        "t",
        sleep=sleeps.append,
        log=lambda _: None,
        transport=httpx.MockTransport(lambda _: next(replies)),
    )
    assert code == 2
    assert sleeps == [1.0]


def test_worker_without_retry_after_polls_slowly() -> None:
    replies = iter([httpx.Response(204), httpx.Response(401)])
    sleeps: list[float] = []
    run_worker(
        FakeFetcher(),  # type: ignore[arg-type]
        "https://wave.test",
        "t",
        sleep=sleeps.append,
        log=lambda _: None,
        transport=httpx.MockTransport(lambda _: next(replies)),
    )
    assert 30.0 <= sleeps[0] <= 40.0


def test_worker_renews_the_lease_while_busy_and_reports_versions() -> None:
    seen: list[tuple[str, dict[str, Any]]] = []

    def handler(request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content or b"{}")
        seen.append((request.url.path, body))
        if request.url.path == "/api/fetcher/jobs/claim":
            return httpx.Response(200, json={"job": JOB})
        if request.url.path.endswith("/lease"):
            return httpx.Response(200, json={"leaseExpiresAt": "2026-11-11T11:15:00Z"})
        return httpx.Response(204)

    fetcher = FakeFetcher()
    fetcher.busy_s = 0.3
    code = run_worker(
        fetcher,  # type: ignore[arg-type]
        "https://wave.test",
        "t",
        worker_id="mac",
        once=True,
        lease_renew_s=0.05,
        sleep=lambda _: None,
        log=lambda _: None,
        transport=httpx.MockTransport(handler),
    )
    assert code == 0
    paths = [path for path, _ in seen]
    assert paths[0] == "/api/fetcher/jobs/claim"
    assert seen[0][1]["capabilities"]["ytDlp"] == YT_DLP_VERSION
    renewals = [p for p in paths if p == f"/api/fetcher/jobs/{JOB['id']}/lease"]
    assert len(renewals) >= 2
    assert paths[-1] == f"/api/fetcher/jobs/{JOB['id']}/result"


def test_rejected_report_is_not_retried() -> None:
    results: list[str] = []

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == "/api/fetcher/jobs/claim":
            return httpx.Response(200, json={"job": JOB})
        results.append(request.url.path)
        return httpx.Response(409, json={"error": {"code": "job_not_yours", "message": "x"}})

    run_worker(
        FakeFetcher(),  # type: ignore[arg-type]
        "https://wave.test",
        "t",
        once=True,
        sleep=lambda _: None,
        log=lambda _: None,
        transport=httpx.MockTransport(handler),
    )
    assert len(results) == 1


def test_direct_link_reaches_the_fetcher() -> None:
    job = {**JOB, "directUrl": "https://artist.bandcamp.com/track/x", "maxAttempts": 1}

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == "/api/fetcher/jobs/claim":
            return httpx.Response(200, json={"job": job})
        return httpx.Response(204)

    fetcher = FakeFetcher()
    run_worker(
        fetcher,  # type: ignore[arg-type]
        "https://wave.test",
        "t",
        once=True,
        sleep=lambda _: None,
        log=lambda _: None,
        transport=httpx.MockTransport(handler),
    )
    assert fetcher.calls[0]["direct_url"] == "https://artist.bandcamp.com/track/x"
    assert fetcher.calls[0]["max_attempts"] == 1
