"""Загрузка готового файла по подписанной ссылке — против локального HTTP-сервера."""

from __future__ import annotations

import threading
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import pytest
from helpers import make_candidate

from dalywave_fetcher.acquire import AcquireResult, Attempt, Fetcher, UploadTarget
from dalywave_fetcher.config import Settings
from dalywave_fetcher.download import DownloadResult

received: dict[str, Any] = {}


class PutHandler(BaseHTTPRequestHandler):
    def do_PUT(self) -> None:
        length = int(self.headers["Content-Length"])
        received["body"] = self.rfile.read(length)
        received["headers"] = dict(self.headers)
        received["path"] = self.path
        status = 403 if "expired" in self.path else 200
        self.send_response(status)
        self.send_header("ETag", '"abc123"')
        self.end_headers()

    def log_message(self, *args: Any) -> None:
        pass


@pytest.fixture
def server() -> Iterator[str]:
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), PutHandler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{httpd.server_address[1]}"
    httpd.shutdown()


def result_with_file(path: Path) -> AcquireResult:
    download = DownloadResult(
        video_id="vid00000001",
        path=path,
        container="mp4",
        codec="opus",
        original_container="webm",
        format_id="251",
        abr=130.0,
        asr=48000,
        channels=2,
        size_bytes=path.stat().st_size,
        original_size_bytes=None,
        duration_s=3.0,
        client="MWEB",
        po_token=True,
        title="t",
        channel="c",
    )
    attempt = Attempt(candidate=make_candidate(), download=download)
    return AcquireResult(status="verified", track_id=1, attempts=[attempt], chosen=attempt)


def test_upload_puts_file_with_content_type(server: str, tmp_path: Path) -> None:
    path = tmp_path / "a.m4a"
    path.write_bytes(b"\x00\x00\x00\x1cftypisom" + bytes(range(256)) * 40)
    fetcher = Fetcher(Settings(data_dir=tmp_path))
    target = UploadTarget(url=f"{server}/dalywave/audio/x?X-Amz-Signature=s", headers={"x-a": "1"})
    status = fetcher._upload(result_with_file(path), target)
    assert status == {"ok": True, "status": 200, "etag": '"abc123"', "error": None}
    assert received["body"] == path.read_bytes()
    assert received["headers"]["Content-Type"] == 'audio/mp4; codecs="Opus"'
    assert received["headers"]["x-a"] == "1"
    assert received["path"] == "/dalywave/audio/x?X-Amz-Signature=s"


def test_upload_reports_rejection(server: str, tmp_path: Path) -> None:
    path = tmp_path / "a.m4a"
    path.write_bytes(b"\x00\x00\x00\x1cftyp")
    fetcher = Fetcher(Settings(data_dir=tmp_path))
    status = fetcher._upload(result_with_file(path), UploadTarget(url=f"{server}/expired"))
    assert status["ok"] is False
    assert status["status"] == 403
