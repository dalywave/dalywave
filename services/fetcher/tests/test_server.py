from __future__ import annotations

import threading
from pathlib import Path
from typing import Any

from fastapi.testclient import TestClient

from dalywave_fetcher.config import Settings
from dalywave_fetcher.server import cookies_status, create_app


class FakeResult:
    def __init__(self, status: str) -> None:
        self.status = status

    def to_json(self, max_candidates: int = 10) -> dict[str, Any]:
        return {
            "status": self.status,
            "chosen": None,
            "error": None,
            "attempts": [],
            "timingsMs": {"total": 1},
            "setup": {},
            "versions": {},
        }


class FakeFetcher:
    def __init__(self, tmp: Path) -> None:
        self.tmp = tmp
        self.started = threading.Event()
        self.release = threading.Event()
        self.release.set()
        self.calls: list[dict[str, Any]] = []

    def jobs_dir(self) -> Path:
        return self.tmp

    def acquire(self, track_id: int, out_dir: Path | None = None, **kw: Any) -> FakeResult:
        self.calls.append({"trackId": track_id, **kw})
        self.started.set()
        self.release.wait(5)
        return FakeResult("verified")


def make_client(tmp_path: Path) -> tuple[TestClient, FakeFetcher]:
    fetcher = FakeFetcher(tmp_path)
    app = create_app(Settings(data_dir=tmp_path), fetcher=fetcher)  # type: ignore[arg-type]
    return TestClient(app), fetcher


def test_healthz_without_pot(tmp_path: Path) -> None:
    client, _ = make_client(tmp_path)
    with client:
        data = client.get("/v1/healthz").json()
    assert data["ok"] is True
    assert data["pot"] == {"configured": False, "reachable": False}
    assert data["busy"] is False
    assert "ytDlp" in data["tools"]


def test_acquire_passes_upload_and_hints(tmp_path: Path) -> None:
    client, fetcher = make_client(tmp_path)
    body = {
        "deezerTrackId": 42,
        "maxAttempts": 2,
        "preferVideoIds": ["abc"],
        "upload": {"url": "https://r2.example/put", "headers": {"x-amz-meta-a": "1"}},
    }
    with client:
        response = client.post("/v1/acquire", json=body)
    assert response.status_code == 200
    assert response.json()["status"] == "verified"
    call = fetcher.calls[0]
    assert call["trackId"] == 42
    assert call["max_attempts"] == 2
    assert call["prefer_video_ids"] == ["abc"]
    assert call["upload"].url == "https://r2.example/put"
    assert call["upload"].headers == {"x-amz-meta-a": "1"}


def test_second_job_gets_busy(tmp_path: Path) -> None:
    client, fetcher = make_client(tmp_path)
    fetcher.release.clear()
    with client:
        first = threading.Thread(
            target=client.post, args=("/v1/acquire",), kwargs={"json": {"deezerTrackId": 1}}
        )
        first.start()
        assert fetcher.started.wait(5)
        response = client.post("/v1/acquire", json={"deezerTrackId": 2})
        fetcher.release.set()
        first.join(5)
    assert response.status_code == 409
    assert response.json() == {
        "error": {"code": "busy", "message": "Сервис занят другим заданием, повторите позже"}
    }


def test_validation_error_format(tmp_path: Path) -> None:
    client, _ = make_client(tmp_path)
    with client:
        response = client.post("/v1/acquire", json={"deezerTrackId": "x"})
    assert response.status_code == 422
    assert response.json()["error"]["code"] == "invalid_request"


def test_cookies_status_reads_expiry(tmp_path: Path) -> None:
    path = tmp_path / "cookies.txt"
    path.write_text(
        "# Netscape HTTP Cookie File\n"
        ".youtube.com\tTRUE\t/\tTRUE\t1893456000\t__Secure-3PSID\tx\n"
        "#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t1861920000\tLOGIN_INFO\ty\n"
        ".youtube.com\tTRUE\t/\tTRUE\t1700000000\tVISITOR_INFO1_LIVE\tz\n"
    )
    status = cookies_status(path)
    assert status["authCookies"] == 2
    assert status["expiresAt"] == "2029-01-01T00:00:00Z"
    assert cookies_status(None) == {"configured": False}
