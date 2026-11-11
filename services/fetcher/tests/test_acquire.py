"""Логика попыток и итоговых статусов acquire на подставных клиентах — без сети."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import numpy as np
import pytest
from helpers import make_candidate, make_track

from dalywave_fetcher import acquire as acquire_module
from dalywave_fetcher.acquire import Fetcher
from dalywave_fetcher.analyze import Analysis, AnalyzeError
from dalywave_fetcher.config import Settings
from dalywave_fetcher.deezer import DeezerTrack
from dalywave_fetcher.download import DownloadError, DownloadResult
from dalywave_fetcher.match import Candidate, MatchResult
from dalywave_fetcher.verify import Fingerprint, VerifyResult


class FakeDeezer:
    def __init__(self, track: DeezerTrack) -> None:
        self._track = track

    def track(self, track_id: int) -> DeezerTrack:
        return self._track

    def download_preview(self, track: DeezerTrack, dest: Path) -> DeezerTrack:
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(b"ID3")
        return track


class FakeMatcher:
    def __init__(
        self,
        candidates: list[Candidate],
        videos: list[Candidate] | None = None,
        steps: list[dict[str, Any]] | None = None,
    ) -> None:
        self.candidates = candidates
        self.videos = videos or []
        self.steps = steps or []

    def match(self, track: DeezerTrack, videos: str = "auto") -> MatchResult:
        return MatchResult(track, None, list(self.candidates), list(self.steps), 0)

    def more_videos(self, result: MatchResult) -> list[Candidate]:
        new = [c for c in self.videos if c not in result.candidates]
        result.candidates.extend(new)
        return new


class FakeDownloader:
    def __init__(self, errors: dict[str, str] | None = None) -> None:
        self.errors = errors or {}
        self.calls: list[str] = []

    def download(self, video_id: str, out_dir: Path, basename: str) -> DownloadResult:
        self.calls.append(video_id)
        if video_id in self.errors:
            raise DownloadError(self.errors[video_id], f"сбой {video_id}")
        out_dir.mkdir(parents=True, exist_ok=True)
        path = out_dir / f"{basename}.opus"
        path.write_bytes(b"OggS" + video_id.encode())
        return DownloadResult(
            video_id=video_id,
            path=path,
            container="ogg",
            codec="opus",
            original_container="webm",
            format_id="251",
            abr=130.0,
            asr=48000,
            channels=2,
            size_bytes=path.stat().st_size,
            original_size_bytes=None,
            duration_s=117.0,
            client="MWEB",
            po_token=True,
            title=video_id,
            channel=None,
        )


def cand(video_id: str, score: float, source: str = "song") -> Candidate:
    c = make_candidate(video_id=video_id, source=source)
    c.score = score
    return c


@pytest.fixture
def matching(monkeypatch: pytest.MonkeyPatch) -> set[str]:
    """Видео, чей отпечаток «совпадает» с превью."""
    good: set[str] = set()
    monkeypatch.setattr(
        acquire_module,
        "fingerprint",
        lambda path: Fingerprint(items=np.zeros(10, dtype=np.uint32), duration_s=30.0),
    )

    def fake_verify(preview: Fingerprint, path: Path, threshold: float) -> VerifyResult:
        ok = path.stem in good
        return VerifyResult(
            verified=ok, similarity=0.95 if ok else 0.55, offset_s=10.0, threshold=threshold
        )

    monkeypatch.setattr(acquire_module, "verify", fake_verify)
    # Громкость и огибающая без ffmpeg: файлы подставные
    monkeypatch.setattr(acquire_module, "analyze", lambda path: Analysis(-9.0, -0.5, [500] * 512))
    return good


def run(
    tmp_path: Path,
    candidates: list[Candidate],
    *,
    errors: dict[str, str] | None = None,
    videos: list[Candidate] | None = None,
    steps: list[dict[str, Any]] | None = None,
    preview: bool = True,
    **kw: Any,
) -> tuple[Any, FakeDownloader]:
    track = make_track(preview_url="https://p" if preview else None)
    downloader = FakeDownloader(errors)
    fetcher = Fetcher(
        Settings(data_dir=tmp_path / "data"),
        deezer=FakeDeezer(track),  # type: ignore[arg-type]
        matcher=FakeMatcher(candidates, videos, steps),  # type: ignore[arg-type]
        downloader=downloader,  # type: ignore[arg-type]
    )
    return fetcher.acquire(1, tmp_path / "out", **kw), downloader


def test_second_candidate_verified_and_first_file_removed(
    tmp_path: Path, matching: set[str]
) -> None:
    matching.add("b")
    result, dl = run(tmp_path, [cand("a", 0.99), cand("b", 0.9), cand("c", 0.8)])
    assert result.status == "verified"
    assert dl.calls == ["a", "b"]
    assert result.path == tmp_path / "out" / "b.opus"
    assert not (tmp_path / "out" / "a.opus").exists()
    assert result.file_sha256
    assert result.to_json()["chosen"]["verify"]["verified"] is True


def test_videos_are_tried_after_audio_fails(tmp_path: Path, matching: set[str]) -> None:
    matching.add("v")
    result, dl = run(tmp_path, [cand("a", 0.9)], videos=[cand("v", 0.7, "video")])
    assert result.status == "verified"
    assert dl.calls == ["a", "v"]


def test_disputed_when_nothing_matches(tmp_path: Path, matching: set[str]) -> None:
    result, _ = run(tmp_path, [cand("a", 0.9), cand("b", 0.8)])
    assert result.status == "disputed"
    assert result.path is None
    assert len(result.attempts) == 2


def test_low_scores_are_not_downloaded(tmp_path: Path, matching: set[str]) -> None:
    result, dl = run(tmp_path, [cand("a", 0.4)])
    assert result.status == "not_found"
    assert dl.calls == []


def test_unavailable_video_is_skipped(tmp_path: Path, matching: set[str]) -> None:
    matching.add("b")
    result, _ = run(tmp_path, [cand("a", 0.95), cand("b", 0.9)], errors={"a": "unavailable"})
    assert result.status == "verified"
    assert result.attempts[0].error.code == "unavailable"


def test_bot_check_stops_immediately(tmp_path: Path, matching: set[str]) -> None:
    result, dl = run(tmp_path, [cand("a", 0.95), cand("b", 0.9)], errors={"a": "bot_check"})
    assert result.status == "error"
    assert result.error["code"] == "bot_check"
    assert dl.calls == ["a"]


def test_second_403_stops(tmp_path: Path, matching: set[str]) -> None:
    errors = {"a": "http_403", "b": "http_403"}
    result, dl = run(tmp_path, [cand("a", 0.95), cand("b", 0.9), cand("c", 0.8)], errors=errors)
    assert result.status == "error"
    assert result.error["code"] == "http_403"
    assert dl.calls == ["a", "b"]


def test_all_downloads_failed_is_error_not_disputed(tmp_path: Path, matching: set[str]) -> None:
    result, _ = run(tmp_path, [cand("a", 0.95)], errors={"a": "geo_blocked"})
    assert result.status == "error"
    assert result.error["code"] == "geo_blocked"


def test_no_preview_takes_only_very_good_candidate(tmp_path: Path, matching: set[str]) -> None:
    result, dl = run(tmp_path, [cand("a", 0.95), cand("b", 0.9)], preview=False)
    assert result.status == "unverified"
    assert dl.calls == ["a"]
    assert result.error["code"] == "no_preview"
    result, _ = run(tmp_path, [cand("a", 0.8)], preview=False)
    assert result.status == "disputed"


def test_manual_pick_wins_even_without_fingerprint_match(
    tmp_path: Path, matching: set[str]
) -> None:
    result, dl = run(tmp_path, [cand("a", 0.95)], prefer_video_ids=["manualvid01"])
    assert dl.calls == ["manualvid01"]
    assert result.status == "unverified"
    assert result.chosen.candidate.source == "manual"
    assert result.chosen.verify.verified is False


def test_excluded_videos_are_skipped(tmp_path: Path, matching: set[str]) -> None:
    matching.add("b")
    result, dl = run(tmp_path, [cand("a", 0.95), cand("b", 0.9)], exclude_video_ids=["a"])
    assert dl.calls == ["b"]
    assert result.status == "verified"


def test_search_outage_is_an_error(tmp_path: Path, matching: set[str]) -> None:
    steps = [
        {"name": "album", "ms": 1, "error": "ConnectionError()"},
        {"name": "song", "ms": 1, "error": "ConnectionError()"},
    ]
    result, _ = run(tmp_path, [], steps=steps)
    assert result.status == "error"
    assert result.error["code"] == "search_failed"


def test_chosen_file_carries_loudness_and_waveform(tmp_path: Path, matching: set[str]) -> None:
    matching.add("a")
    result, _ = run(tmp_path, [cand("a", 0.95)])
    file = result.to_json()["file"]
    assert file["loudnessLufs"] == -9.0
    assert file["truePeakDb"] == -0.5
    assert len(file["waveform"]) == 512
    assert file["analysisError"] is None
    assert file["bitrateKbps"] == 130
    assert "analyze" in result.timings_ms


def test_failed_analysis_keeps_the_file(
    tmp_path: Path, matching: set[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    def broken(path: Path) -> Analysis:
        raise AnalyzeError("ffmpeg: сломанный файл")

    monkeypatch.setattr(acquire_module, "analyze", broken)
    matching.add("a")
    result, _ = run(tmp_path, [cand("a", 0.95)])
    assert result.status == "verified"
    file = result.to_json()["file"]
    assert file["waveform"] is None
    assert file["analysisError"] == "ffmpeg: сломанный файл"


def test_direct_url_is_a_manual_choice(tmp_path: Path, matching: set[str]) -> None:
    url = "https://soundcloud.com/lowborn/light-me-up"
    result, dl = run(tmp_path, [cand("a", 0.95)], direct_url=url, max_attempts=1)
    assert dl.calls == [url]
    assert result.status == "unverified"
    assert result.chosen.candidate.source == "manual"
    chosen = result.to_json()["chosen"]
    assert chosen["candidate"]["videoId"] == url
    assert chosen["candidate"]["url"] == url
    # Файл назван хешем ссылки: в ней есть «/» и «?»
    assert result.path.name.startswith("url-")
