"""Клипы ленты открытий: отбор официального клипа, место припева, сжатие, цикл загрузчика."""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

import httpx
import pytest

from dalywave_fetcher import tools
from dalywave_fetcher.clip import (
    VIDEO_FORMAT,
    ClipMaker,
    VideoSupport,
    clip_start_ms,
    encode_small,
    encoder_args,
    encoder_candidates,
    error_code,
    fetch_args,
    is_frozen,
    is_still,
    probe,
    search_queries,
    stderr_tail,
    video_support,
)
from dalywave_fetcher.config import Settings
from dalywave_fetcher.download import Downloader, classify_error
from dalywave_fetcher.worker import Control, Worker


def test_search_queries_follow_the_language() -> None:
    assert search_queries("Мотылёк", ["Макс Корж"])[1].endswith("клип")
    assert search_queries("Blinding Lights", ["The Weeknd"])[1].endswith("music video")


def test_a_still_picture_is_not_a_clip() -> None:
    # Живой прогон: «I Want Out» нашёлся роликом-обложкой — 23 КБ на 12 секунд
    assert is_still(23_556, 12_000)
    assert not is_still(629_119, 12_000)


def test_clip_start_follows_the_intro() -> None:
    # Та же длина — там же
    assert clip_start_ms(60_000, 200_000, 201_000, 12_000) == 60_000
    # Вступление 21 с — припев позже на 21 с
    assert clip_start_ms(60_000, 210_000, 231_000, 12_000) == 81_000
    # Длиннее на 3 минуты — пропорционально
    assert clip_start_ms(60_000, 200_000, 380_000, 12_000) == 114_000
    # Короче трека — пропорционально, и не дальше конца ролика
    assert clip_start_ms(60_000, 200_000, 185_000, 12_000) == 55_500
    assert clip_start_ms(195_000, 200_000, 200_000, 12_000) == 188_000


def test_error_code() -> None:
    assert error_code({"status": "error", "error": "bot_check: Sign in"}) == "bot_check"
    assert error_code({"status": "none"}) is None


def test_encoder_per_platform_without_gpl() -> None:
    assert encoder_candidates("darwin") == ["h264_videotoolbox", "libx264"]
    assert encoder_candidates("win32") == ["h264_mf", "libx264"]
    assert encoder_candidates("linux") == ["libx264"]
    vt = encoder_args("h264_videotoolbox")
    assert vt[vt.index("-b:v") + 1] == "800k" and vt[vt.index("-allow_sw") + 1] == "1"
    assert vt[vt.index("-profile:v") + 1] == "main"
    assert "-b:v" in encoder_args("h264_mf") and "-crf" in encoder_args("libx264")
    smaller = encoder_args("h264_videotoolbox", smaller=True)
    assert smaller[smaller.index("-b:v") + 1] == "550k"


def test_video_format_skips_av1_and_dash() -> None:
    assert "av01" not in VIDEO_FORMAT and "dash" not in VIDEO_FORMAT
    assert VIDEO_FORMAT.startswith("bv*[height<=480][vcodec^=avc1][protocol=https]")


def test_ffmpeg_errors_are_short_and_classified() -> None:
    stderr = "\n".join(f"строка {i}" for i in range(10)) + "\n\n"
    assert stderr_tail(stderr) == "строка 7 | строка 8 | строка 9"
    assert classify_error("Server returned 403 Forbidden (access denied)") == "http_403"
    stderr = (
        "[in#0 @ 0x79e4c18000] Error opening input: Server returned 403 Forbidden\n"
        "Error opening input file https://rr5.googlevideo.com/videoplayback?ip=2a0d&sig=x.\n"
    )
    assert stderr_tail(stderr) == (
        "Error opening input: Server returned 403 Forbidden | "
        "Error opening input file https://rr5.googlevideo.com/…"
    )


def test_fetch_args_cut_a_piece_by_link() -> None:
    cmd = fetch_args(
        "https://rr1.googlevideo.com/videoplayback?x=1",
        61.0,
        75.0,
        Path("raw.mkv"),
        {"User-Agent": "UA", "Origin": "https://www.youtube.com"},
    )
    assert cmd[cmd.index("-ss") + 1] == "61.000" and cmd[cmd.index("-t") + 1] == "14.000"
    assert cmd.index("-ss") < cmd.index("-i")
    assert cmd[cmd.index("-headers") + 1] == "User-Agent: UA\r\nOrigin: https://www.youtube.com\r\n"
    assert cmd[-3:] == ["-f", "matroska", "raw.mkv"] and "-an" in cmd and "copy" in cmd


def fake_ffmpeg(tmp_path: Path, protocols: str, encoders: str, encode_rc: int = 0) -> str:
    """ffmpeg-заглушка: списки протоколов и кодировщиков, проверочное кодирование с кодом."""
    path = tmp_path / "ffmpeg"
    path.write_text(
        "#!/bin/sh\n"
        f'case "$2" in -protocols) printf "{protocols}";; -encoders) printf "{encoders}";;\n'
        f'*) cat >/dev/null; echo "Error: no device" >&2; exit {encode_rc};; esac\n'
    )
    path.chmod(0o755)
    return str(path)


@pytest.mark.skipif(sys.platform == "win32", reason="заглушка ffmpeg — shell-скрипт")
def test_video_support_finds_a_working_encoder(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    audio_only = fake_ffmpeg(tmp_path, "Input:\n  file\n  pipe\n", " A....D pcm_s16le  PCM\n")
    monkeypatch.setattr(tools, "_configured", {"ffmpeg": audio_only})
    support = video_support(refresh=True)
    assert support.encoder is None and support.code == "no_https"

    listed = " V....D h264_videotoolbox  VT\n V....D h264_mf  MF\n V....D libx264  x264\n"
    broken = fake_ffmpeg(tmp_path, "Input:\n  https\n", listed, encode_rc=1)
    monkeypatch.setattr(tools, "_configured", {"ffmpeg": broken})
    support = video_support(refresh=True)
    assert support.encoder is None and support.code == "no_encoder"
    assert "нет видеокодировщика" in (support.problem or "")
    assert "no device" in (support.problem or "")

    working = fake_ffmpeg(tmp_path, "Input:\n  https\n", listed)
    monkeypatch.setattr(tools, "_configured", {"ffmpeg": working})
    assert video_support(refresh=True).encoder == encoder_candidates()[0]


def test_without_an_encoder_a_clip_fails_at_once() -> None:
    class NoEncoder(ClipMaker):
        def support(self) -> VideoSupport:
            return VideoSupport(None, "нет видеокодировщика H.264 (обновите приложение)")

        def search(self, query: str) -> list[dict[str, Any]]:
            raise AssertionError("без кодировщика YouTube не трогаем")

    maker = NoEncoder(Downloader(Settings(data_dir=Path("data"))))
    result = maker.make(
        title="BIRDS OF A FEATHER",
        artists=["Billie Eilish"],
        duration_ms=210_000,
        hook_start_ms=63_000,
        length_ms=12_000,
    )
    assert result["status"] == "error" and error_code(result) == "no_encoder"
    assert "нет видеокодировщика" in result["error"]


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нет ffmpeg")
def test_encode_cuts_exactly_without_audio(tmp_path: Path) -> None:
    src = tmp_path / "src.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-nostdin",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=1280x720:rate=60:duration=8",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=stereo",
            "-shortest",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            str(src),
        ],
        check=True,
    )
    out = tmp_path / "clip.mp4"
    assert encode_small(src, out, 1.5, 4.0) == "crf27"
    info = probe(out)
    assert info["height"] == 480 and info["width"] % 2 == 0
    assert info["fps"] <= 30.5
    assert abs(info["duration"] - 4.0) < 0.2
    streams = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
            str(out),
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()
    assert streams == ["video"]
    assert out.stat().st_size < 2 * 1024 * 1024


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нет ffmpeg")
def test_a_frozen_picture_is_found_by_frames(tmp_path: Path) -> None:
    # Аппаратный кодировщик держит битрейт и на обложке — неподвижность видна только по кадрам
    def make(name: str, vf: str) -> Path:
        path = tmp_path / name
        subprocess.run(
            [
                "ffmpeg",
                "-nostdin",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x360:rate=25:duration=8",
                "-vf",
                vf,
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                str(path),
            ],
            check=True,
        )
        return path

    still = make("still.mp4", "select=eq(n\\,0),loop=300:1:0,setpts=N/25/TB")
    moving = make("moving.mp4", "null")
    assert is_frozen(still, 1.0, 6.0)
    assert not is_frozen(moving, 1.0, 6.0)
    assert not is_frozen(tmp_path / "missing.mp4", 0.0, 6.0)


# ───────────── цикл загрузчика ─────────────

CLIP_JOB = {
    "id": "0193f0c2-0000-7c3a-9b1d-2f4e5a6b7c8d",
    "trackId": "0193f0c2-1111-7c3a-9b1d-2f4e5a6b7c8d",
    "title": "BIRDS OF A FEATHER",
    "artists": ["Billie Eilish"],
    "durationMs": 210_000,
    "hookStartMs": 63_000,
    "lengthMs": 12_000,
    "isrc": "USUM72401994",
    "excludeVideoIds": ["bad"],
    "upload": {
        "url": "https://acc.r2.cloudflarestorage.com/dalywave/clips/x?X-Amz-Signature=s",
        "method": "PUT",
        "headers": {},
        "expiresAt": "2026-11-13T12:00:00Z",
    },
    "leaseExpiresAt": "2026-11-13T11:05:00Z",
}


class FakeClips(ClipMaker):
    def __init__(self, result: dict[str, Any] | None = None, encoder: str | None = "fake") -> None:
        super().__init__(Downloader(Settings(data_dir=Path("data"))))
        self.calls: list[dict[str, Any]] = []
        self.result = result or {"status": "ready", "videoId": "V9PVRfjEBTI", "sizeBytes": 1}
        self.encoder = encoder

    def support(self) -> VideoSupport:
        return VideoSupport(self.encoder, None if self.encoder else "нет видеокодировщика H.264")

    def make(self, **kw: Any) -> dict[str, Any]:
        self.calls.append(kw)
        return self.result


class NoAcquire:
    settings = Settings(data_dir=Path("data"))

    def acquire(self, *_: Any, **__: Any) -> Any:
        raise AssertionError("заданий скачивания нет")


def worker(handler: Any, clips: ClipMaker | None) -> Worker:
    return Worker(
        NoAcquire(),  # type: ignore[arg-type]
        "https://wave.test",
        "t",
        worker_id="mac",
        log=lambda _: None,
        transport=httpx.MockTransport(handler),
        control=Control(lambda _: None),
        clips=clips,
    )


def test_idle_server_hands_out_a_clip_and_gets_the_report() -> None:
    seen: list[tuple[str, dict[str, Any]]] = []

    def handler(request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content or b"{}")
        seen.append((request.url.path, body))
        if request.url.path == "/api/fetcher/jobs/claim":
            return httpx.Response(204, headers={"Retry-After": "0"})
        if request.url.path == "/api/fetcher/clips/claim":
            return httpx.Response(200, json={"job": CLIP_JOB})
        return httpx.Response(204)

    clips = FakeClips()
    assert worker(handler, clips).run(once=True) == 0
    paths = [p for p, _ in seen]
    assert paths == [
        "/api/fetcher/jobs/claim",
        "/api/fetcher/clips/claim",
        f"/api/fetcher/clips/{CLIP_JOB['id']}/result",
    ]
    assert seen[0][1]["capabilities"]["kinds"] == ["acquire", "clip"]
    assert seen[1][1] == {"workerId": "mac", "features": ["clip-modes"]}
    assert seen[2][1]["result"]["status"] == "ready"
    call = clips.calls[0]
    assert call["hook_start_ms"] == 63_000 and call["exclude_video_ids"] == ["bad"]
    assert call["mode"] == "auto" and call["label"] is None
    assert call["upload"].url.startswith("https://acc.r2.cloudflarestorage.com/")


def test_old_server_without_clips_is_not_asked_again() -> None:
    claims: list[str] = []
    main = iter([httpx.Response(204, headers={"Retry-After": "0"})] * 2 + [httpx.Response(401)])

    def handler(request: httpx.Request) -> httpx.Response:
        claims.append(request.url.path)
        if request.url.path == "/api/fetcher/clips/claim":
            return httpx.Response(404)
        return next(main)

    assert worker(handler, FakeClips()).run() == 2
    assert claims.count("/api/fetcher/clips/claim") == 1
    assert claims.count("/api/fetcher/jobs/claim") == 3


def test_bot_check_on_a_clip_cools_the_worker_down() -> None:
    replies = iter(
        [
            httpx.Response(204),
            httpx.Response(200, json={"job": CLIP_JOB}),
            httpx.Response(204),
        ]
    )

    def handler(request: httpx.Request) -> httpx.Response:
        return next(replies)

    w = worker(handler, FakeClips({"status": "error", "error": "bot_check: not a bot"}))
    w.control.stopped = False
    # Один круг: клип с ошибкой бота → пауза; второй круг цикл уже не делает (останавливаем)
    original = w._clip_turn

    def turn() -> str | None:
        code = original()
        w.control.stop()
        return code

    w._clip_turn = turn  # type: ignore[method-assign]
    w.run()
    assert w.cooldown_code == "bot_check"
    assert w.cooldown_until > 0


def test_without_a_downloader_clips_are_off() -> None:
    w = Worker(NoAcquire(), "https://wave.test", "t", log=lambda _: None)  # type: ignore[arg-type]
    assert w.clips is None
    assert w.hello()["capabilities"]["kinds"] == ["acquire"]


def test_ffmpeg_without_video_takes_no_clips() -> None:
    paths: list[str] = []
    kinds: list[list[str]] = []

    def handler(request: httpx.Request) -> httpx.Response:
        paths.append(request.url.path)
        kinds.append(json.loads(request.content or b"{}").get("capabilities", {}).get("kinds"))
        return httpx.Response(204, headers={"Retry-After": "0"})

    logs: list[str] = []
    w = worker(handler, FakeClips(encoder=None))
    w.log = logs.append
    assert w.run(once=True) == 0
    assert paths == ["/api/fetcher/jobs/claim"] and kinds == [["acquire"]]
    assert any("клипы выключены" in line for line in logs)
