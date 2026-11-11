"""Клиент публичного API Deezer: ключ не нужен, лимит около 50 запросов за 5 секунд."""

from __future__ import annotations

import collections
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import httpx

BASE_URL = "https://api.deezer.com"
USER_AGENT = "DalyWave-fetcher/0.1 (+https://wave.daly.rest)"

# Коды ошибок из тела ответа Deezer (HTTP при этом 200).
_QUOTA_EXCEEDED = 4
_NO_DATA = 800


class DeezerError(Exception):
    def __init__(self, message: str, code: int | None = None) -> None:
        super().__init__(message)
        self.code = code


class DeezerNotFoundError(DeezerError):
    pass


class RateLimiter:
    """Скользящее окно: не больше `max_calls` вызовов за `period` секунд."""

    def __init__(
        self,
        max_calls: int = 40,
        period: float = 5.0,
        clock: Callable[[], float] = time.monotonic,
        sleep: Callable[[float], None] = time.sleep,
    ) -> None:
        self.max_calls = max_calls
        self.period = period
        self._clock = clock
        self._sleep = sleep
        self._calls: collections.deque[float] = collections.deque()
        self._lock = threading.Lock()

    def acquire(self) -> None:
        with self._lock:
            now = self._clock()
            self._purge(now)
            if len(self._calls) >= self.max_calls:
                self._sleep(self.period - (now - self._calls[0]))
                now = self._clock()
                self._purge(now)
            self._calls.append(now)

    def _purge(self, now: float) -> None:
        while self._calls and now - self._calls[0] >= self.period:
            self._calls.popleft()


@dataclass(frozen=True)
class DeezerTrack:
    id: int
    title: str
    title_short: str
    title_version: str
    duration_s: int
    isrc: str | None
    artist: str
    # Основной артист первым, затем остальные участники без повторов.
    artists: tuple[str, ...]
    album_id: int | None
    album_title: str | None
    track_position: int | None
    disk_number: int | None
    release_date: str | None
    explicit: bool
    preview_url: str | None

    @classmethod
    def from_api(cls, data: dict[str, Any]) -> DeezerTrack:
        main = (data.get("artist") or {}).get("name") or ""
        names = [main] + [c.get("name") or "" for c in data.get("contributors") or []]
        album = data.get("album") or {}
        return cls(
            id=int(data["id"]),
            title=data.get("title") or "",
            title_short=data.get("title_short") or data.get("title") or "",
            title_version=(data.get("title_version") or "").strip(),
            duration_s=int(data.get("duration") or 0),
            isrc=data.get("isrc") or None,
            artist=main,
            artists=tuple(dict.fromkeys(n for n in names if n)),
            album_id=album.get("id"),
            album_title=album.get("title"),
            track_position=data.get("track_position"),
            disk_number=data.get("disk_number"),
            release_date=data.get("release_date") or album.get("release_date"),
            explicit=bool(data.get("explicit_lyrics")),
            preview_url=data.get("preview") or None,
        )

    def to_json(self) -> dict[str, Any]:
        return {
            "deezerId": self.id,
            "title": self.title,
            "artist": self.artist,
            "artists": list(self.artists),
            "album": self.album_title,
            "albumId": self.album_id,
            "durationMs": self.duration_s * 1000,
            "isrc": self.isrc,
            "trackPosition": self.track_position,
            "diskNumber": self.disk_number,
            "releaseDate": self.release_date,
            "explicit": self.explicit,
        }


@dataclass(frozen=True)
class AlbumTrack:
    id: int
    title: str
    duration_s: int
    disk_number: int
    track_position: int


@dataclass(frozen=True)
class DeezerAlbum:
    id: int
    title: str
    artist: str
    nb_tracks: int
    release_date: str | None
    record_type: str | None
    explicit: bool
    tracks: tuple[AlbumTrack, ...]

    @property
    def year(self) -> int | None:
        if self.release_date and self.release_date[:4].isdigit():
            return int(self.release_date[:4])
        return None

    def index_of(self, track_id: int) -> int | None:
        """Сквозной номер трека в альбоме с нуля (диски идут подряд, как на YouTube Music)."""
        for i, t in enumerate(self.tracks):
            if t.id == track_id:
                return i
        return None


class DeezerClient:
    def __init__(
        self,
        http: httpx.Client | None = None,
        limiter: RateLimiter | None = None,
        base_url: str = BASE_URL,
    ) -> None:
        self._http = http or httpx.Client(
            headers={"User-Agent": USER_AGENT}, timeout=15.0, follow_redirects=True
        )
        self._limiter = limiter or RateLimiter()
        self._base = base_url.rstrip("/")

    def close(self) -> None:
        self._http.close()

    def get(self, path: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        url = path if path.startswith("http") else f"{self._base}/{path.lstrip('/')}"
        last_error: Exception | None = None
        for attempt in range(4):
            self._limiter.acquire()
            try:
                response = self._http.get(url, params=params)
            except httpx.TransportError as e:
                last_error = e
                time.sleep(1.0 + attempt)
                continue
            if response.status_code >= 500:
                last_error = DeezerError(f"HTTP {response.status_code}", response.status_code)
                time.sleep(1.0 + attempt)
                continue
            response.raise_for_status()
            data = response.json()
            error = data.get("error") if isinstance(data, dict) else None
            if not error:
                return data
            code = error.get("code")
            if code == _QUOTA_EXCEEDED:
                last_error = DeezerError("лимит запросов Deezer", code)
                time.sleep(5.0 * (attempt + 1))
                continue
            if code == _NO_DATA:
                raise DeezerNotFoundError(f"{path}: нет данных", code)
            raise DeezerError(error.get("message") or str(error), code)
        raise DeezerError(f"{path}: Deezer не ответил: {last_error}")

    def track(self, track_id: int) -> DeezerTrack:
        return DeezerTrack.from_api(self.get(f"track/{track_id}"))

    def track_by_isrc(self, isrc: str) -> DeezerTrack:
        return DeezerTrack.from_api(self.get(f"track/isrc:{isrc}"))

    def album(self, album_id: int) -> DeezerAlbum:
        data = self.get(f"album/{album_id}")
        # Во вложенном списке альбома максимум 25 треков и нет номеров дисков.
        tracks: list[AlbumTrack] = []
        page: dict[str, Any] | None = self.get(f"album/{album_id}/tracks", {"limit": 100})
        while page:
            tracks.extend(
                AlbumTrack(
                    id=int(t["id"]),
                    title=t.get("title") or "",
                    duration_s=int(t.get("duration") or 0),
                    disk_number=int(t.get("disk_number") or 1),
                    track_position=int(t.get("track_position") or 0),
                )
                for t in page.get("data") or []
            )
            next_url = page.get("next")
            page = self.get(next_url) if next_url else None
        tracks.sort(key=lambda t: (t.disk_number, t.track_position))
        return DeezerAlbum(
            id=int(data["id"]),
            title=data.get("title") or "",
            artist=(data.get("artist") or {}).get("name") or "",
            nb_tracks=int(data.get("nb_tracks") or len(tracks)),
            release_date=data.get("release_date"),
            record_type=data.get("record_type"),
            explicit=bool(data.get("explicit_lyrics")),
            tracks=tuple(tracks),
        )

    def artist(self, artist_id: int) -> dict[str, Any]:
        return self.get(f"artist/{artist_id}")

    def artist_top(self, artist_id: int, limit: int = 10) -> list[DeezerTrack]:
        data = self.get(f"artist/{artist_id}/top", {"limit": limit})
        return [DeezerTrack.from_api(t) for t in data.get("data") or []]

    def search(self, query: str, kind: str = "track", limit: int = 10) -> list[dict[str, Any]]:
        path = "search" if kind == "track" else f"search/{kind}"
        return list(self.get(path, {"q": query, "limit": limit}).get("data") or [])

    def download_preview(self, track: DeezerTrack, dest: Path) -> DeezerTrack:
        """Скачивает 30-секундное превью. Подписанная ссылка живёт ~15 минут:
        если она протухла, трек запрашивается заново за свежей."""
        current = track
        for attempt in range(2):
            if not current.preview_url:
                raise DeezerError("у трека нет превью")
            response = self._http.get(current.preview_url)
            if response.status_code in (403, 404, 410) and attempt == 0:
                current = self.track(track.id)
                continue
            response.raise_for_status()
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(response.content)
            return current
        raise DeezerError("не удалось скачать превью")
