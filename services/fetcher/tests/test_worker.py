"""Цикл «домашнего загрузчика» против подставного сервера."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import httpx

from dalywave_fetcher.config import Settings
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
        return FakeResult()


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
