"""Загрузчик внутри приложения: события, команды, пауза, возврат задания при остановке."""

from __future__ import annotations

import io
import json
import os
import signal
import subprocess
import sys
import threading
import time
from collections.abc import Callable, Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

import httpx
import pytest

from dalywave_fetcher import progress
from dalywave_fetcher.config import Settings
from dalywave_fetcher.sidecar import Counters, Emitter, Sidecar, token_from_env
from dalywave_fetcher.worker import Control, Worker

JOB = {
    "id": "0193f0c2-7a1e-7c3a-9b1d-2f4e5a6b7c8d",
    "kind": "acquire",
    "deezerTrackId": 3040222091,
    "maxAttempts": 3,
    "preferVideoIds": [],
    "excludeVideoIds": [],
    "upload": {"url": "https://r2.test/put?sig=1", "method": "PUT", "headers": {}},
    "leaseExpiresAt": "2026-11-11T11:10:00Z",
}


class Result:
    def __init__(self, status: str = "verified", uploaded: bool = True) -> None:
        self.status = status
        self.error = None
        self.upload = {"ok": uploaded}

    def to_json(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "deezerTrackId": 3040222091,
            "track": {"title": "ящики", "artist": "Mzlff"},
            "upload": self.upload,
            "error": self.error,
        }


class StagedFetcher:
    """Проходит этапы, как настоящий acquire; `hold` — ждать на скачивании, пока не прервут."""

    def __init__(self, hold: bool = False) -> None:
        self.settings = Settings(data_dir=Path("data"))
        self.hold = hold
        self.downloading = threading.Event()

    def acquire(self, track_id: int, **_: Any) -> Result:
        progress.report(progress.MATCHING, 0.0)
        progress.report(progress.MATCHING, 0.2, track="ящики", artist="Mzlff")
        progress.report(progress.DOWNLOADING, 0.0, attempt=1)
        self.downloading.set()
        while self.hold:
            progress.report(progress.DOWNLOADING, 0.5)
            time.sleep(0.01)
        progress.report(progress.DOWNLOADING, 1.0)
        progress.report(progress.VERIFYING, 1.0)
        progress.report(progress.UPLOADING, 1.0)
        return Result()


class Events:
    def __init__(self) -> None:
        self.stream = io.StringIO()
        self.emitter = Emitter(self.stream)

    def all(self) -> list[dict[str, Any]]:
        return [json.loads(line) for line in self.stream.getvalue().splitlines()]

    def of(self, kind: str) -> list[dict[str, Any]]:
        return [e for e in self.all() if e["event"] == kind]


def make(
    tmp_path: Path, handler: Callable[[httpx.Request], httpx.Response], fetcher: Any
) -> tuple[Sidecar, Worker, Events]:
    events = Events()
    control = Control()
    sidecar = Sidecar(
        events.emitter,
        server="https://wave.test",
        worker_name="Mac Захара (приложение)",
        counters=Counters(tmp_path / "state.json"),
        control=control,
    )
    worker = Worker(
        fetcher,
        "https://wave.test",
        "t",
        worker_id="Mac Захара (приложение)",
        log=lambda _: None,
        transport=httpx.MockTransport(handler),
        control=control,
        events=sidecar,
        cooldown_s=0.0,
    )
    sidecar.worker = worker
    return sidecar, worker, events


def test_job_reports_stages_and_counts_done(tmp_path: Path) -> None:
    claims = iter([httpx.Response(200, json={"job": JOB})])
    _, worker, events = make(tmp_path, lambda r: _serve(r, claims), StagedFetcher())
    assert worker.run(once=True) == 0
    working = [e for e in events.of("status") if e["state"] == "working"]
    stages = [e["job"]["stage"] for e in working]
    assert stages[0] == "matching"
    assert {"downloading", "verifying", "uploading"} <= set(stages)
    named = [e for e in working if e["job"]["track"]]
    assert named[0]["job"]["artist"] == "Mzlff"
    assert named[0]["server"] == "https://wave.test"
    assert named[0]["workerName"] == "Mac Захара (приложение)"
    (job,) = events.of("job")
    assert job["outcome"] == "done"
    assert job["track"] == "ящики"
    last = events.of("status")[-1]
    assert last["state"] == "idle"
    assert last["counters"]["doneToday"] == 1
    assert last["counters"]["failedToday"] == 0
    # Счётчики переживают перезапуск
    assert Counters(tmp_path / "state.json").done == 1


def test_failed_job_is_counted(tmp_path: Path) -> None:
    class Disputed(StagedFetcher):
        def acquire(self, track_id: int, **kw: Any) -> Result:
            return Result("disputed", uploaded=False)

    claims = iter([httpx.Response(200, json={"job": JOB})])
    _, worker, events = make(tmp_path, lambda r: _serve(r, claims), Disputed())
    worker.run(once=True)
    assert events.of("job")[0]["outcome"] == "failed"
    assert events.of("status")[-1]["counters"]["failedToday"] == 1


def _serve(
    request: httpx.Request,
    claims: Iterator[httpx.Response],
    seen: list[tuple[str, Any]] | None = None,
    release: int = 404,
) -> httpx.Response:
    body = json.loads(request.content or b"{}")
    if seen is not None:
        seen.append((request.url.path, body))
    path = request.url.path
    if path.endswith("/claim"):
        return next(claims, httpx.Response(204, headers={"Retry-After": "1"}))
    if path.endswith("/release"):
        return httpx.Response(release, json={"error": {"code": "not_found", "message": "нет"}})
    return httpx.Response(204)


@pytest.mark.parametrize("release", [404, 204])
def test_stop_hands_the_job_back_at_once(tmp_path: Path, release: int) -> None:
    seen: list[tuple[str, Any]] = []
    claims = iter([httpx.Response(200, json={"job": JOB})])
    fetcher = StagedFetcher(hold=True)
    sidecar, worker, events = make(tmp_path, lambda r: _serve(r, claims, seen, release), fetcher)
    thread = threading.Thread(target=worker.run, daemon=True)
    thread.start()
    assert fetcher.downloading.wait(5)
    sidecar.shutdown("stop")
    thread.join(5)
    assert not thread.is_alive()
    paths = [p for p, _ in seen]
    job_id = JOB["id"]
    assert f"/api/fetcher/jobs/{job_id}/release" in paths
    reports = [b for p, b in seen if p == f"/api/fetcher/jobs/{job_id}/result"]
    if release == 204:
        # Сервер умеет release — отчёт не нужен
        assert reports == []
    else:
        # Сервер без release: отчёт о сбое исполнителя, который не тратит попытку
        (report,) = reports
        assert report["result"]["status"] == "error"
        assert report["result"]["error"]["code"] == "network"
        assert report["result"]["error"]["interrupted"] is True
    (job,) = events.of("job")
    assert job["outcome"] == "released"
    assert events.of("status")[-1]["state"] == "stopping"
    assert Counters(tmp_path / "state.json").done == 0


def test_pause_finishes_the_current_job_then_waits(tmp_path: Path) -> None:
    seen: list[tuple[str, Any]] = []
    claims = iter([httpx.Response(200, json={"job": JOB})])
    fetcher = StagedFetcher(hold=True)
    sidecar, worker, events = make(tmp_path, lambda r: _serve(r, claims, seen), fetcher)
    thread = threading.Thread(target=worker.run, daemon=True)
    thread.start()
    assert fetcher.downloading.wait(5)
    assert sidecar.handle({"cmd": "pause", "id": 7})
    assert events.of("ack")[-1] == {**events.of("ack")[-1], "cmd": "pause", "ok": True, "id": 7}
    # Пока доделывает задание — «working», но уже с паузой
    assert events.of("status")[-1]["state"] == "working"
    assert events.of("status")[-1]["paused"] is True
    fetcher.hold = False
    _wait(lambda: events.of("status")[-1]["state"] == "paused")
    claims_made = [p for p, _ in seen if p.endswith("/claim")]
    assert len(claims_made) == 1
    assert events.of("job")[0]["outcome"] == "done"
    sidecar.handle({"cmd": "resume"})
    _wait(lambda: len([p for p, _ in seen if p.endswith("/claim")]) >= 2)
    sidecar.shutdown("stop")
    thread.join(5)


def test_pause_now_releases_the_job(tmp_path: Path) -> None:
    seen: list[tuple[str, Any]] = []
    claims = iter([httpx.Response(200, json={"job": JOB})])
    fetcher = StagedFetcher(hold=True)
    sidecar, worker, events = make(tmp_path, lambda r: _serve(r, claims, seen), fetcher)
    thread = threading.Thread(target=worker.run, daemon=True)
    thread.start()
    assert fetcher.downloading.wait(5)
    sidecar.handle({"cmd": "pause", "now": True})
    _wait(lambda: events.of("status")[-1]["state"] == "paused")
    assert events.of("job")[0]["outcome"] == "released"
    assert thread.is_alive()
    sidecar.shutdown("stop")
    thread.join(5)
    assert not thread.is_alive()


def test_offline_and_rejected_token(tmp_path: Path) -> None:
    replies = iter([httpx.ConnectError("нет сети"), httpx.Response(401)])

    def handler(_: httpx.Request) -> httpx.Response:
        reply = next(replies)
        if isinstance(reply, Exception):
            raise reply
        return reply

    _, worker, events = make(tmp_path, handler, StagedFetcher())
    worker.retry_base_s = 0.01
    assert worker.run() == 2
    states = [(e["state"], (e["error"] or {}).get("code")) for e in events.of("status")]
    assert ("offline", "server_unreachable") in states
    assert states[-1] == ("error", "token_rejected")
    offline = next(e for e in events.of("status") if e["state"] == "offline")
    assert offline["retryAt"]


def test_bot_check_cools_down_and_survives_restart(tmp_path: Path) -> None:
    class Blocked(StagedFetcher):
        def acquire(self, track_id: int, **kw: Any) -> Result:
            result = Result("error", uploaded=False)
            result.error = {"code": "bot_check", "message": "not a bot"}  # type: ignore[assignment]
            return result

    claims = iter([httpx.Response(200, json={"job": JOB})])
    sidecar, worker, events = make(tmp_path, lambda r: _serve(r, claims), Blocked())
    thread = threading.Thread(target=worker.run, daemon=True)
    thread.start()
    _wait(lambda: events.of("status")[-1]["state"] == "error")
    status = events.of("status")[-1]
    assert status["error"]["code"] == "bot_check"
    assert status["retryAt"]
    sidecar.shutdown("stop")
    thread.join(5)
    assert Counters(tmp_path / "state.json").cooldown_until > time.time() + 600


def test_unknown_command_and_status(tmp_path: Path) -> None:
    sidecar, _, events = make(tmp_path, lambda r: httpx.Response(204), StagedFetcher())
    assert sidecar.handle({"cmd": "dance"})
    assert events.of("ack")[-1]["ok"] is False
    assert sidecar.handle({"cmd": None, "raw": "pause"})
    assert events.of("ack")[-1]["message"] == "строка не JSON"
    assert sidecar.handle({"cmd": "status"})
    assert events.of("status")[-1]["state"] == "starting"
    assert sidecar.handle({"cmd": "stop"}) is False


def test_counters_reset_on_a_new_day(tmp_path: Path) -> None:
    path = tmp_path / "state.json"
    path.write_text(json.dumps({"date": "2000-01-01", "done": 5, "failed": 2}))
    counters = Counters(path)
    assert (counters.done, counters.failed) == (0, 0)


def test_token_is_not_inherited_by_child_processes(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("DALYWAVE_DOWNLOADER_TOKEN", " dwk_secret\n")
    monkeypatch.setenv("FETCHER_WORKER_TOKEN", "dwk_other")
    assert token_from_env() == "dwk_secret"
    assert "DALYWAVE_DOWNLOADER_TOKEN" not in os.environ
    assert "FETCHER_WORKER_TOKEN" not in os.environ


def test_internal_stop_is_not_acknowledged(tmp_path: Path) -> None:
    sidecar, _, events = make(tmp_path, lambda r: httpx.Response(204), StagedFetcher())
    assert sidecar.handle({"cmd": "stop", "reason": "signal 15", "internal": True}) is False
    assert events.of("ack") == []


class FakePot:
    """Сервис PO-токенов, который «упал» и поднимается на новом порту."""

    def __init__(self) -> None:
        self.port: int | None = 4416
        self.version = "2.0.2"
        self.running = False
        self.started = threading.Event()

    def alive(self) -> bool:
        return self.running

    def start(self) -> str:
        self.running = True
        self.port = 5000
        self.started.set()
        return "http://127.0.0.1:5000"


def test_crashed_pot_server_is_restarted_and_yt_dlp_gets_the_new_address(tmp_path: Path) -> None:
    sidecar, worker, events = make(tmp_path, lambda r: httpx.Response(204), StagedFetcher())
    pot = FakePot()
    sidecar.pot_server = pot  # type: ignore[assignment]
    sidecar.pot = {"state": "ready", "version": "2.0.2"}
    worker.fetcher.downloader = type("D", (), {"settings": worker.fetcher.settings})()
    sidecar.check_pot()
    assert pot.started.wait(5)
    _wait(lambda: events.of("status")[-1]["pot"]["state"] == "ready")
    assert worker.fetcher.settings.pot_url == "http://127.0.0.1:5000"
    assert worker.fetcher.downloader.settings.pot_url == "http://127.0.0.1:5000"
    # Живой сервер не трогаем, упавший — не чаще раза в минуту
    pot.started.clear()
    sidecar.check_pot()
    pot.running = False
    sidecar.check_pot()
    assert not pot.started.wait(0.2)


def test_pot_server_is_restarted_in_the_first_minute_after_boot(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """time.monotonic() считает от загрузки системы: на только что загруженной машине (раннер CI,
    автозапуск приложения) упавший сервер всё равно поднимается сразу."""
    import dalywave_fetcher.sidecar as module

    class JustBooted:
        def __getattr__(self, name: str) -> object:
            return getattr(time, name)

        @staticmethod
        def monotonic() -> float:
            return 5.0

    monkeypatch.setattr(module, "time", JustBooted())
    sidecar, worker, _ = make(tmp_path, lambda r: httpx.Response(204), StagedFetcher())
    pot = FakePot()
    sidecar.pot_server = pot  # type: ignore[assignment]
    worker.fetcher.downloader = type("D", (), {"settings": worker.fetcher.settings})()
    sidecar.check_pot()
    assert pot.started.wait(5)


def _wait(cond: Callable[[], bool], timeout: float = 5.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if cond():
            return
        time.sleep(0.01)
    raise AssertionError("не дождались")


# ───────────── процесс целиком: stdout, stdin, сигналы ─────────────


class _Server(BaseHTTPRequestHandler):
    def do_POST(self) -> None:
        self.rfile.read(int(self.headers.get("Content-Length") or 0))
        self.send_response(204)
        self.send_header("Retry-After", "1")
        self.end_headers()

    def log_message(self, *_: Any) -> None:
        pass


@pytest.fixture
def fake_server() -> Iterator[str]:
    server = ThreadingHTTPServer(("127.0.0.1", 0), _Server)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{server.server_port}"
    server.shutdown()


def _stub_tools(directory: Path) -> None:
    directory.mkdir()
    for name in ("ffmpeg", "ffprobe", "fpcalc", "deno"):
        tool = directory / name
        tool.write_text(f"#!/bin/sh\necho '{name} version 0-stub'\n")
        tool.chmod(0o755)


def _start(tmp_path: Path, server: str, *extra: str) -> subprocess.Popen[str]:
    _stub_tools(tmp_path / "bin")
    env = {k: v for k, v in os.environ.items() if not k.startswith(("FETCHER_", "DALYWAVE_"))}
    env["DALYWAVE_DOWNLOADER_TOKEN"] = "dwk_test"
    return subprocess.Popen(
        [
            sys.executable,
            "-m",
            "dalywave_fetcher.cli",
            "sidecar",
            "--server",
            server,
            "--worker-name",
            "тест",
            "--data-dir",
            str(tmp_path / "data"),
            "--log-file",
            str(tmp_path / "fetcher.log"),
            "--tools-dir",
            str(tmp_path / "bin"),
            "--no-pot",
            *extra,
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        env=env,
    )


def _read_until(
    proc: subprocess.Popen[str], cond: Callable[[dict[str, Any]], bool]
) -> list[dict[str, Any]]:
    assert proc.stdout is not None
    seen = []
    for line in proc.stdout:
        event = json.loads(line)
        seen.append(event)
        if cond(event):
            return seen
    raise AssertionError(f"не дождались события, были: {seen}")


@pytest.mark.skipif(sys.platform == "win32", reason="заглушки программ — shell-скрипты")
def test_process_speaks_json_and_stops_on_command(tmp_path: Path, fake_server: str) -> None:
    proc = _start(tmp_path, fake_server)
    assert proc.stdin is not None
    try:
        hello = _read_until(proc, lambda e: e["event"] == "hello")[-1]
        assert hello["protocol"] == 1
        assert hello["workerName"] == "тест"
        assert hello["tools"]["ffmpeg"]["version"] == "ffmpeg version 0-stub"
        _read_until(proc, lambda e: e["event"] == "status" and e["state"] == "idle")
        proc.stdin.write('{"cmd": "pause"}\n')
        proc.stdin.flush()
        _read_until(proc, lambda e: e["event"] == "status" and e["state"] == "paused")
        proc.stdin.write('{"cmd": "resume"}\n{"cmd": "stop"}\n')
        proc.stdin.flush()
        events = _read_until(proc, lambda e: e["event"] == "exit")
        assert events[-1]["code"] == 0
        assert events[-1]["reason"] == "stop"
        assert proc.wait(10) == 0
    finally:
        proc.kill()
    assert "загрузчик остановлен" in (tmp_path / "fetcher.log").read_text(encoding="utf-8")


@pytest.mark.skipif(sys.platform == "win32", reason="SIGTERM и заглушки — только POSIX")
def test_process_stops_on_sigterm_and_on_closed_stdin(tmp_path: Path, fake_server: str) -> None:
    proc = _start(tmp_path, fake_server)
    try:
        _read_until(proc, lambda e: e["event"] == "status" and e["state"] == "idle")
        proc.send_signal(signal.SIGTERM)
        events = _read_until(proc, lambda e: e["event"] == "exit")
        assert events[-1]["reason"] == f"signal {int(signal.SIGTERM)}"
        assert proc.wait(10) == 0
    finally:
        proc.kill()

    (tmp_path / "again").mkdir()
    proc = _start(tmp_path / "again", fake_server)
    assert proc.stdin is not None
    try:
        _read_until(proc, lambda e: e["event"] == "status" and e["state"] == "idle")
        proc.stdin.close()
        events = _read_until(proc, lambda e: e["event"] == "exit")
        assert events[-1]["reason"] == "stdin closed"
        assert proc.wait(10) == 0
    finally:
        proc.kill()


@pytest.mark.skipif(sys.platform == "win32", reason="заглушки программ — shell-скрипты")
def test_process_without_token_says_why(tmp_path: Path) -> None:
    _stub_tools(tmp_path / "bin")
    env = {k: v for k, v in os.environ.items() if not k.startswith(("FETCHER_", "DALYWAVE_"))}
    proc = subprocess.run(
        [
            sys.executable,
            "-m",
            "dalywave_fetcher.cli",
            "sidecar",
            "--data-dir",
            str(tmp_path / "data"),
            "--log-file",
            str(tmp_path / "fetcher.log"),
            "--tools-dir",
            str(tmp_path / "bin"),
        ],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        env=env,
        timeout=60,
        check=False,
    )
    assert proc.returncode == 2
    events = [json.loads(line) for line in proc.stdout.splitlines()]
    assert [e["event"] for e in events] == ["hello", "status", "exit"]
    assert events[1]["error"]["code"] == "no_token"
    assert events[-1]["reason"] == "no_token"
