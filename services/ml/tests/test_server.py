import shutil
import threading
import wave
from pathlib import Path
from typing import Any

import numpy as np
import pytest
from fastapi.testclient import TestClient

from dalywave_ml.analysis import TooShortError, decode_f16, encode_f16
from dalywave_ml.config import Settings
from dalywave_ml.server import create_app

needs_ffmpeg = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нет ffmpeg")


class FakeAnalyzer:
    def __init__(self) -> None:
        self.calls: list[tuple[float, str]] = []
        self.gate: threading.Event | None = None

    def analyze(self, audio: Any, mode: str) -> dict[str, Any]:
        if self.gate is not None:
            self.gate.wait(5)
        if audio.duration_s < 2:
            raise TooShortError("слишком коротко")
        self.calls.append((audio.duration_s, mode))
        return {"source": mode, "bpm": 120.0, "timingsMs": {"effnet": 1}}

    def embed_text(self, text: str) -> dict[str, Any]:
        return {"semantic": encode_f16(np.ones(512) / np.sqrt(512)), "dim": 512, "query": text}


def write_wav(path: Path, seconds: float) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(22050)
        w.writeframes(np.zeros(int(22050 * seconds), dtype="<i2").tobytes())


@pytest.fixture
def setup(tmp_path):
    analyzer = FakeAnalyzer()
    settings = Settings(models_dir=tmp_path / "models", files_root=tmp_path.resolve(), threads=1)
    with TestClient(create_app(settings, analyzer)) as client:
        yield client, analyzer, tmp_path


def test_healthz(setup):
    client, _, _ = setup
    body = client.get("/v1/healthz").json()
    assert body["ok"] is True
    assert body["busy"] is False


@needs_ffmpeg
def test_analyze_by_path(setup):
    client, analyzer, root = setup
    write_wav(root / "a.wav", 3.0)
    r = client.post("/v1/analyze", json={"path": "a.wav", "mode": "preview"})
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["source"] == "preview"
    assert {"fetch", "decode", "effnet", "total"} <= body["timingsMs"].keys()
    assert analyzer.calls == [(pytest.approx(3.0, abs=0.01), "preview")]


@pytest.mark.parametrize(
    "payload",
    [
        {"mode": "full"},
        {"url": "https://x/a.mp3", "path": "a.wav"},
        {"url": "ftp://x/a.mp3"},
        {"path": "a.wav", "mode": "fast"},
        {"path": "a.wav", "extra": 1},
    ],
)
def test_invalid_requests(setup, payload):
    client, _, _ = setup
    r = client.post("/v1/analyze", json=payload)
    assert r.status_code == 422
    assert r.json()["error"]["code"] == "invalid_request"


def test_path_outside_root_forbidden(setup):
    client, _, _ = setup
    r = client.post("/v1/analyze", json={"path": "../../etc/passwd"})
    assert r.status_code == 403
    assert r.json()["error"]["code"] == "path_forbidden"


def test_path_disabled_without_root(tmp_path):
    settings = Settings(models_dir=tmp_path, files_root=None, threads=1)
    with TestClient(create_app(settings, FakeAnalyzer())) as client:
        r = client.post("/v1/analyze", json={"path": "/tmp/a.wav"})
    assert r.status_code == 403
    assert r.json()["error"]["code"] == "path_disabled"


def test_missing_file(setup):
    client, _, _ = setup
    r = client.post("/v1/analyze", json={"path": "nope.mp3"})
    assert r.status_code == 404


@needs_ffmpeg
def test_too_short_and_undecodable(setup):
    client, _, root = setup
    write_wav(root / "short.wav", 0.5)
    r = client.post("/v1/analyze", json={"path": "short.wav"})
    assert r.status_code == 422
    assert r.json()["error"]["code"] == "too_short"
    (root / "bad.mp3").write_bytes(b"\x00" * 100)
    r = client.post("/v1/analyze", json={"path": "bad.mp3"})
    assert r.json()["error"]["code"] == "undecodable"


@needs_ffmpeg
def test_second_analysis_is_busy(setup):
    client, analyzer, root = setup
    write_wav(root / "a.wav", 3.0)
    analyzer.gate = threading.Event()
    results: list[int] = []
    first = threading.Thread(
        target=lambda: results.append(
            client.post("/v1/analyze", json={"path": "a.wav"}).status_code
        )
    )
    first.start()
    for _ in range(100):
        if client.get("/v1/healthz").json()["busy"]:
            break
        threading.Event().wait(0.02)
    r = client.post("/v1/analyze", json={"path": "a.wav"})
    analyzer.gate.set()
    first.join()
    assert r.status_code == 409
    assert r.json()["error"]["code"] == "busy"
    assert results == [200]


def test_download_failure_is_502(setup):
    client, _, _ = setup
    r = client.post("/v1/analyze", json={"url": "http://127.0.0.1:9/a.mp3"})
    assert r.status_code == 502
    assert r.json()["error"]["code"] == "download_failed"


def test_embed_text(setup):
    client, _, _ = setup
    r = client.post("/v1/embed-text", json={"text": "спокойный инструментал"})
    assert r.status_code == 200
    vec = decode_f16(r.json()["semantic"])
    assert vec.shape == (512,)
    assert np.linalg.norm(vec) == pytest.approx(1.0, abs=1e-3)
    assert client.post("/v1/embed-text", json={"text": ""}).status_code == 422


def test_models_missing_is_503(tmp_path):
    settings = Settings(models_dir=tmp_path / "empty", files_root=tmp_path, threads=1)
    with TestClient(create_app(settings)) as client:
        health = client.get("/v1/healthz").json()
        assert health["ok"] is False
        assert health["error"]
        r = client.post("/v1/embed-text", json={"text": "calm"})
    assert r.status_code == 503
    assert r.json()["error"]["code"] == "models_missing"


def test_models_endpoint_lists_licenses(setup):
    client, _, _ = setup
    body = client.get("/v1/models").json()
    licenses = {m["key"]: m["license"] for m in body["models"]}
    assert licenses["effnet"] == "CC BY-NC-SA 4.0"
    assert licenses["clap"].startswith("CC0-1.0")
    assert licenses["beat_this"] == licenses["skey"] == "MIT"
    on_start = {m["key"]: m["onStart"] for m in body["models"]}
    assert on_start["effnet"] and not on_start["clap"]
    assert body["versions"]["sound"] == "discogs-effnet-bsdynamic-1"
