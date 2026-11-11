"""Проверки с настоящими Deezer и YouTube. По умолчанию пропускаются:

uv run pytest -m network
FETCHER_NETWORK_DOWNLOAD=1 uv run pytest -m network   # ещё и скачать один трек
"""

from __future__ import annotations

import os
from pathlib import Path

import pytest

from dalywave_fetcher.acquire import Fetcher
from dalywave_fetcher.config import Settings
from dalywave_fetcher.deezer import DeezerClient

pytestmark = pytest.mark.network

# Mzlff — «ящики»: альбомный трек, в альбоме YouTube Music стоит клип вместо аудио.
TRACK_ID = 3040222091


def test_deezer_track_and_isrc() -> None:
    dz = DeezerClient()
    track = dz.track(TRACK_ID)
    assert track.isrc
    assert dz.track_by_isrc(track.isrc).id == TRACK_ID
    assert dz.album(track.album_id or 0).index_of(TRACK_ID) is not None


def test_match_finds_album_audio_version(tmp_path: Path) -> None:
    result = Fetcher(Settings(data_dir=tmp_path)).match(TRACK_ID)
    best = result.best
    assert best is not None
    assert best.source == "album"
    assert best.kind == "atv"
    assert best.score > 0.9


@pytest.mark.skipif(not os.environ.get("FETCHER_NETWORK_DOWNLOAD"), reason="скачивание выключено")
def test_acquire_verifies(tmp_path: Path) -> None:
    result = Fetcher(Settings.from_env()).acquire(TRACK_ID, tmp_path / "out")
    assert result.status == "verified", result.to_json()
    assert result.path is not None
    assert result.path.suffix == ".opus"
