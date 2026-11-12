"""Перенос из YouTube Music: плейлист и имена видео. Ответы ytmusicapi — записанные (tests/fixtures),
без сети."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import requests
from fastapi.testclient import TestClient

from dalywave_fetcher.config import Settings
from dalywave_fetcher.library import YTLibrary, playlist_json, song_json
from dalywave_fetcher.server import create_app

FIXTURES = Path(__file__).parent / "fixtures"


def fixture(name: str) -> dict[str, Any]:
    return json.loads((FIXTURES / f"{name}.json").read_text(encoding="utf-8"))


class FakeYT:
    def __init__(self) -> None:
        self.songs: list[str] = []

    def get_playlist(self, playlist_id: str, limit: int | None = 100) -> dict[str, Any]:
        assert limit is None, "весь плейлист, а не первые сто"
        if playlist_id != "PLntVvZRJe0d-6aEQPLPW4C3VjzjP2XbG8":
            # Так ytmusicapi падает на закрытом и несуществующем плейлисте
            raise KeyError("Unable to find 'contents'")
        return fixture("ytmusic_playlist")

    def get_song(self, video_id: str) -> dict[str, Any]:
        self.songs.append(video_id)
        if video_id == "jNQXAC9IVRw":
            return fixture("ytmusic_song_zoo")
        if video_id == "dQw4w9WgXcQ":
            return {
                "playabilityStatus": {"status": "OK"},
                "videoDetails": {
                    "videoId": video_id,
                    "title": "Never Gonna Give You Up",
                    "author": "Rick Astley",
                    "lengthSeconds": "213",
                    "musicVideoType": "MUSIC_VIDEO_TYPE_OMV",
                },
            }
        if video_id == "netdownxxxx":
            raise requests.ConnectionError("нет сети")
        return {"playabilityStatus": {"status": "ERROR"}}


def make_client(tmp_path: Path) -> tuple[TestClient, FakeYT]:
    yt = FakeYT()
    app = create_app(
        Settings(data_dir=tmp_path),
        fetcher=object(),  # type: ignore[arg-type]
        library=YTLibrary(lambda: yt),  # type: ignore[arg-type, return-value]
    )
    return TestClient(app), yt


def test_playlist_is_flattened() -> None:
    p = playlist_json(fixture("ytmusic_playlist"))
    assert p["title"] == "Russian rap"
    assert p["trackCount"] == 72
    first = p["tracks"][0]
    assert first == {
        "videoId": "QoKpz_95OIM",
        "title": "4K - По Кайфу ★✯★",
        "artists": ["Mama I'm A Criminal"],
        "album": None,
        "durationMs": 189_000,
        "videoType": "MUSIC_VIDEO_TYPE_UGC",
    }


def test_song_names() -> None:
    zoo = song_json("jNQXAC9IVRw", fixture("ytmusic_song_zoo"))
    assert zoo["found"] is True
    assert zoo["videoType"] is None, "не музыка — сервер её пропустит"
    assert zoo["durationMs"] == 19_000
    gone = song_json("Gone0000001", {"playabilityStatus": {"status": "ERROR"}})
    assert gone == {
        "videoId": "Gone0000001",
        "found": False,
        "title": "",
        "artists": [],
        "durationMs": None,
        "videoType": None,
    }


def test_playlist_endpoint(tmp_path: Path) -> None:
    client, _ = make_client(tmp_path)
    with client:
        ok = client.post(
            "/v1/ytmusic/playlist", json={"playlistId": "PLntVvZRJe0d-6aEQPLPW4C3VjzjP2XbG8"}
        )
        assert ok.status_code == 200
        assert len(ok.json()["tracks"]) == 2
        closed = client.post("/v1/ytmusic/playlist", json={"playlistId": "PLclosed"})
        assert closed.status_code == 404
        assert closed.json()["error"]["code"] == "playlist_not_found"
        bad = client.post("/v1/ytmusic/playlist", json={"playlistId": "../etc"})
        assert bad.status_code == 422


def test_songs_endpoint(tmp_path: Path) -> None:
    client, yt = make_client(tmp_path)
    with client:
        res = client.post(
            "/v1/ytmusic/songs",
            json={"videoIds": ["dQw4w9WgXcQ", "jNQXAC9IVRw", "Gone0000001", "not-an-id"]},
        )
        assert res.status_code == 200
        songs = res.json()["songs"]
        assert [s["videoId"] for s in songs] == ["dQw4w9WgXcQ", "jNQXAC9IVRw", "Gone0000001"]
        assert songs[0]["artists"] == ["Rick Astley"]
        assert songs[0]["videoType"] == "MUSIC_VIDEO_TYPE_OMV"
        assert songs[2]["found"] is False
        assert "not-an-id" not in yt.songs
        # Сеть подвела — пачка целиком «повторите позже», а не «видео удалены»
        down = client.post("/v1/ytmusic/songs", json={"videoIds": ["netdownxxxx"]})
        assert down.status_code == 502
        assert down.json()["error"]["code"] == "ytmusic_unavailable"
        too_many = client.post("/v1/ytmusic/songs", json={"videoIds": ["dQw4w9WgXcQ"] * 51})
        assert too_many.status_code == 422
