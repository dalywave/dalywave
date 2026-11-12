"""Перенос музыки из YouTube Music для сервера: треки публичного плейлиста и имена видео по id
(видео из архива Google Takeout известны только по id).

Без входа: ни аккаунтов, ни cookies Google — те же запросы, что делает сайт YouTube Music для
гостя. Артиста и название из названия клипа сервер разбирает сам: здесь ответ ytmusicapi только
приводится к плоскому виду.
"""

from __future__ import annotations

import re
import threading
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from typing import Any

import requests
from ytmusicapi import YTMusic

# Имён видео за один запрос сервера и одновременных запросов к YouTube Music.
MAX_SONGS = 50
SONG_WORKERS = 4

VIDEO_ID = re.compile(r"^[A-Za-z0-9_-]{11}$")


class PlaylistNotFoundError(Exception):
    """Плейлист закрыт, удалён или это не плейлист."""


class LibraryUnavailableError(Exception):
    """YouTube Music не ответил: сеть, защита от ботов, сбой на их стороне."""


class YTLibrary:
    """ytmusicapi держит requests.Session — по экземпляру на поток."""

    def __init__(self, factory: Callable[[], YTMusic] | None = None) -> None:
        self._factory = factory or (lambda: YTMusic(language="en"))
        self._local = threading.local()

    def _yt(self) -> YTMusic:
        yt = getattr(self._local, "yt", None)
        if yt is None:
            yt = self._factory()
            self._local.yt = yt
        return yt

    def playlist(self, playlist_id: str) -> dict[str, Any]:
        try:
            raw = self._yt().get_playlist(playlist_id, limit=None)
        except requests.RequestException as exc:
            raise LibraryUnavailableError(str(exc)) from exc
        # Закрытый и несуществующий плейлист ytmusicapi не отличает: в ответе нет содержимого
        except (KeyError, IndexError, TypeError) as exc:
            raise PlaylistNotFoundError(playlist_id) from exc
        return playlist_json(raw)

    def songs(self, video_ids: list[str]) -> list[dict[str, Any]]:
        ids = [v for v in video_ids if VIDEO_ID.match(v)][:MAX_SONGS]
        with ThreadPoolExecutor(max_workers=SONG_WORKERS) as pool:
            return list(pool.map(self._song, ids))

    def _song(self, video_id: str) -> dict[str, Any]:
        try:
            raw = self._yt().get_song(video_id)
        except requests.RequestException as exc:
            # Сеть подвела — пусть сервер повторит пачку позже, а не сочтёт видео удалённым
            raise LibraryUnavailableError(str(exc)) from exc
        except (KeyError, IndexError, TypeError, ValueError):
            raw = None
        return song_json(video_id, raw)


def _names(items: list[dict[str, Any]] | None) -> list[str]:
    return [a["name"].strip() for a in items or [] if (a.get("name") or "").strip()]


def _ms(seconds: Any) -> int | None:
    try:
        value = int(seconds)
    except (TypeError, ValueError):
        return None
    return value * 1000 if value > 0 else None


def playlist_json(raw: dict[str, Any]) -> dict[str, Any]:
    """Ответ `get_playlist` → плоский плейлист для сервера."""
    tracks = []
    for t in raw.get("tracks") or []:
        title = (t.get("title") or "").strip()
        if not title:
            continue
        album = t.get("album")
        tracks.append(
            {
                "videoId": t.get("videoId"),
                "title": title,
                "artists": _names(t.get("artists")),
                "album": album.get("name") if isinstance(album, dict) else None,
                "durationMs": _ms(t.get("duration_seconds")),
                "videoType": t.get("videoType"),
            }
        )
    return {
        "playlistId": raw.get("id"),
        "title": (raw.get("title") or "").strip(),
        "description": (raw.get("description") or "").strip(),
        "trackCount": raw.get("trackCount"),
        "tracks": tracks,
    }


def song_json(video_id: str, raw: dict[str, Any] | None) -> dict[str, Any]:
    """Ответ `get_song` → имя видео. `found = False` — видео удалено или скрыто."""
    details = (raw or {}).get("videoDetails") or {}
    title = (details.get("title") or "").strip()
    author = (details.get("author") or "").strip()
    return {
        "videoId": video_id,
        "found": bool(title),
        "title": title,
        "artists": [author] if author else [],
        "durationMs": _ms(details.get("lengthSeconds")),
        "videoType": details.get("musicVideoType"),
    }
