"""Клипы ленты открытий: отбор официального клипа, место припева, сжатие, цикл загрузчика."""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path
from typing import Any

import httpx
import pytest

from dalywave_fetcher.clip import (
    ClipMaker,
    choose,
    clip_start_ms,
    encode_small,
    error_code,
    is_still,
    probe,
    search_queries,
)
from dalywave_fetcher.config import Settings
from dalywave_fetcher.download import Downloader
from dalywave_fetcher.worker import Control, Worker


def item(video_id: str, title: str, channel: str, duration: float, views: int = 0) -> dict:
    return {
        "id": video_id,
        "ie_key": "Youtube",
        "title": title,
        "channel": channel,
        "duration": duration,
        "view_count": views,
    }


BIRDS = [
    item(
        "V9PVRfjEBTI",
        "Billie Eilish - BIRDS OF A FEATHER (Official Music Video)",
        "Billie Eilish",
        231,
        1_007_778_441,
    ),
    item("geKxhmZL8ao", "Billie Eilish - BIRDS OF A FEATHER", "LatinHype", 211, 27_495_451),
    item(
        "YH6HJb_F-LE",
        "Billie Eilish – BIRDS OF A FEATHER (Live Performance from Songline)",
        "Billie Eilish",
        243,
        35_067_454,
    ),
    item("lyr", "Billie Eilish - BIRDS OF A FEATHER (Lyrics)", "7clouds", 210, 90_000_000),
    item("topic", "BIRDS OF A FEATHER", "Billie Eilish - Topic", 210, 500_000_000),
]


def test_official_video_wins_over_lyrics_live_and_topic() -> None:
    best, picks = choose(BIRDS, "BIRDS OF A FEATHER", ["Billie Eilish"], 210)
    assert best is not None and best.video_id == "V9PVRfjEBTI"
    reasons = {p.video_id: p.reasons for p in picks}
    assert any(r.startswith("version") for r in reasons["YH6HJb_F-LE"])
    assert any(r.startswith("version") for r in reasons["lyr"])
    assert "topic_channel" in reasons["topic"]


def test_excluded_and_wrong_duration_are_skipped() -> None:
    best, picks = choose(
        BIRDS, "BIRDS OF A FEATHER", ["Billie Eilish"], 210, exclude={"V9PVRfjEBTI"}
    )
    # Без официального клипа годится только перезалив без меток — он тоже клип трека
    assert best is not None and best.video_id == "geKxhmZL8ao"
    assert picks[0].reasons == ["excluded"]
    short = [item("t", "Billie Eilish - BIRDS OF A FEATHER (Official Video)", "Billie Eilish", 60)]
    best, picks = choose(short, "BIRDS OF A FEATHER", ["Billie Eilish"], 210)
    assert best is None and "duration" in picks[0].reasons


def test_cyrillic_artist_and_channel() -> None:
    items = [
        item("z1", "Zivert - Beverly Hills | Official Video", "Zivert", 205, 80_000_000),
        item("z2", "Другая песня - Beverly", "Someone", 200, 10),
    ]
    best, _ = choose(items, "Beverly Hills", ["Zivert"], 199)
    assert best is not None and best.video_id == "z1"
    items = [item("m", "Макс Корж — Мотылёк (клип)", "Макс Корж", 230, 1_000_000)]
    best, _ = choose(items, "Мотылёк", ["Макс Корж"], 225)
    assert best is not None
    assert search_queries("Мотылёк", ["Макс Корж"])[1].endswith("клип")
    assert search_queries("Blinding Lights", ["The Weeknd"])[1].endswith("music video")


def test_fan_videos_are_not_clips() -> None:
    # Живой прогон: на «ELA DANCA (Slowed)» нашёлся только AMV — это не официальный клип
    items = [
        item("a", "ELA DANCA (Slowed) - Sayfalse, 5opka, MellSher [AMV]", "Edits", 150, 50_000),
        item("f", "Sayfalse - ELA DANCA (Slowed) fan made video", "Fan", 150, 50_000),
    ]
    best, _ = choose(items, "ELA DANCA (Slowed)", ["Sayfalse"], 148)
    assert best is None


def test_a_still_picture_is_not_a_clip() -> None:
    # Живой прогон: «I Want Out» нашёлся роликом-обложкой — 23 КБ на 12 секунд
    assert is_still(23_556, 12_000)
    assert not is_still(629_119, 12_000)


def test_a_live_song_title_keeps_live_videos() -> None:
    items = [item("l", "Oasis - Live Forever (Official Video)", "Oasis", 280, 100)]
    best, _ = choose(items, "Live Forever", ["Oasis"], 277)
    assert best is not None


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
    encode_small(src, out, 1.5, 4.0)
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
    def __init__(self, result: dict[str, Any] | None = None) -> None:
        super().__init__(Downloader(Settings(data_dir=Path("data"))))
        self.calls: list[dict[str, Any]] = []
        self.result = result or {"status": "ready", "videoId": "V9PVRfjEBTI", "sizeBytes": 1}

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
    assert seen[1][1] == {"workerId": "mac"}
    assert seen[2][1]["result"]["status"] == "ready"
    call = clips.calls[0]
    assert call["hook_start_ms"] == 63_000 and call["exclude_video_ids"] == ["bad"]
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
