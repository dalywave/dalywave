"""Сборщики тестовых данных без сети."""

from __future__ import annotations

from typing import Any

from dalywave_fetcher.deezer import AlbumTrack, DeezerAlbum, DeezerTrack
from dalywave_fetcher.match import Candidate


def make_track(**overrides: Any) -> DeezerTrack:
    data: dict[str, Any] = {
        "id": 1,
        "title": "ZAKHELO",
        "title_short": "ZAKHELO",
        "title_version": "",
        "duration_s": 117,
        "isrc": "QT5M52636881",
        "artist": "Rushex",
        "artists": ("Rushex",),
        "album_id": 10,
        "album_title": "ZAKHELO",
        "track_position": 1,
        "disk_number": 1,
        "release_date": "2026-03-13",
        "explicit": False,
        "preview_url": "https://example.invalid/preview.mp3",
    }
    data.update(overrides)
    return DeezerTrack(**data)


def make_candidate(**overrides: Any) -> Candidate:
    data: dict[str, Any] = {
        "video_id": "vid00000001",
        "title": "ZAKHELO",
        "artists": ["Rushex"],
        "album": "ZAKHELO",
        "duration_s": 118,
        "video_type": "MUSIC_VIDEO_TYPE_ATV",
        "source": "song",
        "rank": 0,
    }
    data.update(overrides)
    return Candidate(**data)


def make_album(tracks: list[tuple[int, str, int]], **overrides: Any) -> DeezerAlbum:
    data: dict[str, Any] = {
        "id": 10,
        "title": "светлая сторона",
        "artist": "Mzlff",
        "nb_tracks": len(tracks),
        "release_date": "2024-10-11",
        "record_type": "album",
        "explicit": True,
        "tracks": tuple(
            AlbumTrack(id=tid, title=title, duration_s=dur, disk_number=1, track_position=i + 1)
            for i, (tid, title, dur) in enumerate(tracks)
        ),
    }
    data.update(overrides)
    return DeezerAlbum(**data)
