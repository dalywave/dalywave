from __future__ import annotations

from typing import Any

import httpx
import pytest

from dalywave_fetcher.deezer import DeezerClient, DeezerNotFoundError, DeezerTrack, RateLimiter


class FakeClock:
    def __init__(self) -> None:
        self.now = 0.0
        self.slept: list[float] = []

    def time(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.slept.append(seconds)
        self.now += seconds


def test_rate_limiter_waits_when_window_is_full() -> None:
    clock = FakeClock()
    limiter = RateLimiter(max_calls=3, period=5.0, clock=clock.time, sleep=clock.sleep)
    for _ in range(3):
        limiter.acquire()
        clock.now += 0.5
    assert clock.slept == []
    limiter.acquire()
    assert clock.slept == [pytest.approx(3.5)]


TRACK = {
    "id": 3898084851,
    "title": "ZAKHELO (Slowed)",
    "title_short": "ZAKHELO",
    "title_version": "(Slowed)",
    "duration": 129,
    "isrc": "QT5M52651886",
    "track_position": 2,
    "disk_number": 1,
    "explicit_lyrics": False,
    "preview": "https://cdnt-preview.dzcdn.net/x.mp3",
    "artist": {"name": "Rushex"},
    "contributors": [{"name": "Rushex"}, {"name": "Montagem"}],
    "album": {"id": 7, "title": "ZAKHELO", "release_date": "2026-03-13"},
}


def test_track_from_api() -> None:
    t = DeezerTrack.from_api(TRACK)
    assert t.artists == ("Rushex", "Montagem")
    assert t.title_version == "(Slowed)"
    assert t.release_date == "2026-03-13"
    assert t.to_json()["durationMs"] == 129_000


def client_with(handler: Any) -> DeezerClient:
    http = httpx.Client(transport=httpx.MockTransport(handler))
    limiter = RateLimiter(max_calls=1000, period=1.0)
    return DeezerClient(http=http, limiter=limiter, base_url="https://api.test")


def test_not_found_error_code() -> None:
    client = client_with(
        lambda _: httpx.Response(200, json={"error": {"code": 800, "message": "no data"}})
    )
    with pytest.raises(DeezerNotFoundError):
        client.track(1)


def test_quota_error_is_retried(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("dalywave_fetcher.deezer.time.sleep", lambda _: None)
    calls = []

    def handler(request: httpx.Request) -> httpx.Response:
        calls.append(request.url.path)
        if len(calls) == 1:
            return httpx.Response(200, json={"error": {"code": 4, "message": "Quota"}})
        return httpx.Response(200, json=TRACK)

    assert client_with(handler).track(TRACK["id"]).id == TRACK["id"]
    assert len(calls) == 2


def test_album_follows_pagination_and_orders_discs() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == "/album/7":
            return httpx.Response(200, json={"id": 7, "title": "A", "nb_tracks": 3})
        if request.url.params.get("index") == "2":
            data = [{"id": 3, "title": "c", "duration": 1, "disk_number": 2, "track_position": 1}]
            return httpx.Response(200, json={"data": data})
        data = [
            {"id": 2, "title": "b", "duration": 1, "disk_number": 1, "track_position": 2},
            {"id": 1, "title": "a", "duration": 1, "disk_number": 1, "track_position": 1},
        ]
        return httpx.Response(
            200, json={"data": data, "next": "https://api.test/album/7/tracks?index=2"}
        )

    album = client_with(handler).album(7)
    assert [t.id for t in album.tracks] == [1, 2, 3]
    assert album.index_of(3) == 2
    assert album.index_of(99) is None
