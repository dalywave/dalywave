"""Matcher целиком на подставных Deezer и YouTube Music — без сети."""

from __future__ import annotations

from typing import Any

from helpers import make_album, make_track

from dalywave_fetcher.deezer import DeezerAlbum, DeezerTrack
from dalywave_fetcher.match import Matcher, YTMusicClient

ALBUM_ITEM = {
    "title": "светлая сторона",
    "type": "Album",
    "year": "2024",
    "browseId": "MPREb_album",
    "artists": [{"name": "mzlff"}],
}
OMV = "MUSIC_VIDEO_TYPE_OMV"
ATV = "MUSIC_VIDEO_TYPE_ATV"


class FakeDeezer:
    def __init__(self, album: DeezerAlbum) -> None:
        self._album = album

    def album(self, album_id: int) -> DeezerAlbum:
        return self._album


class FakeYTM(YTMusicClient):
    def __init__(self) -> None:
        self.calls: list[tuple[str, str]] = []

    def search(self, query: str, kind: str, limit: int = 10) -> list[dict[str, Any]]:
        self.calls.append((kind, query))
        if kind == "albums":
            return [ALBUM_ITEM]
        if kind == "songs" and query.startswith("RU"):
            return [
                {
                    "videoId": "isrc_hit",
                    "title": "ящики",
                    "artists": [{"name": "mzlff"}],
                    "duration_seconds": 200,
                    "videoType": ATV,
                }
            ]
        if kind == "songs":
            return [
                {
                    "videoId": "song_atv",
                    "title": "ящики",
                    "artists": [{"name": "mzlff"}],
                    "album": {"name": "светлая сторона"},
                    "duration_seconds": 200,
                    "videoType": ATV,
                },
                {
                    "videoId": "other",
                    "title": "hate core",
                    "artists": [{"name": "mzlff"}],
                    "duration_seconds": 167,
                    "videoType": ATV,
                },
            ]
        if kind == "videos":
            return [
                {
                    "videoId": "clip",
                    "title": "mzlff - ящики (клип)",
                    "artists": [{"name": "MZLFF"}],
                    "duration_seconds": 215,
                    "videoType": OMV,
                }
            ]
        return []

    def album(self, browse_id: str) -> dict[str, Any]:
        self.calls.append(("album", browse_id))
        return {
            "title": "светлая сторона",
            "year": "2024",
            "trackCount": 3,
            "artists": [{"name": "mzlff"}],
            "audioPlaylistId": "OLAK_audio",
            "tracks": [
                {"videoId": "omv1", "title": "для тебя", "duration_seconds": 223, "videoType": OMV},
                {
                    "videoId": "omv2",
                    "title": "ящики",
                    "duration_seconds": 200,
                    "videoType": OMV,
                    "artists": [{"name": "mzlff"}, {"name": "BOOKER"}],
                },
                {
                    "videoId": "omv3",
                    "title": "hate core",
                    "duration_seconds": 167,
                    "videoType": OMV,
                },
            ],
        }

    def playlist(self, playlist_id: str) -> dict[str, Any]:
        self.calls.append(("playlist", playlist_id))
        return {
            "tracks": [
                {"videoId": "atv1", "title": "для тебя", "duration_seconds": 223, "videoType": ATV},
                {
                    "videoId": "atv2",
                    "title": "ящики",
                    "duration_seconds": 200,
                    "videoType": ATV,
                    "artists": [{"name": "mzlff"}, {"name": "BOOKER"}],
                },
                {
                    "videoId": "atv3",
                    "title": "hate core",
                    "duration_seconds": 167,
                    "videoType": ATV,
                },
            ]
        }


def mzlff_track() -> DeezerTrack:
    return make_track(
        id=102,
        title="ящики",
        artist="Mzlff",
        artists=("Mzlff", "Booker"),
        duration_s=199,
        isrc="RUAGT2454320",
        album_title="светлая сторона",
    )


def test_album_step_swaps_clip_for_audio_version() -> None:
    album = make_album([(101, "для тебя", 223), (102, "ящики", 199), (103, "hate core", 167)])
    ytm = FakeYTM()
    result = Matcher(FakeDeezer(album), ytm).match(mzlff_track())  # type: ignore[arg-type]

    best = result.best
    assert best is not None
    assert best.video_id == "atv2"
    assert best.source == "album"
    assert best.album_match is not None
    assert best.album_match["position"] == best.album_match["deezerPosition"] == 2
    ids = [c.video_id for c in result.candidates]
    assert ids.index("atv2") < ids.index("omv2")
    # Нашлось хорошее — видео не искали.
    assert ("videos", "Mzlff ящики") not in ytm.calls
    assert {s["name"] for s in result.steps} >= {"deezer_album", "album", "isrc", "song"}


def test_videos_are_searched_when_nothing_good() -> None:
    album = make_album([(1, "другое", 100)], title="другой альбом")
    ytm = FakeYTM()
    track = make_track(
        id=5,
        title="ящики",
        artist="Mzlff",
        artists=("Mzlff",),
        duration_s=214,
        isrc=None,
        album_title="другой альбом",
    )
    result = Matcher(FakeDeezer(album), ytm).match(track)  # type: ignore[arg-type]
    assert ("videos", "Mzlff ящики") in ytm.calls
    assert result.candidates[0].video_id == "clip"


def test_more_videos_adds_only_once() -> None:
    album = make_album([(101, "для тебя", 223), (102, "ящики", 199), (103, "hate core", 167)])
    matcher = Matcher(FakeDeezer(album), FakeYTM())  # type: ignore[arg-type]
    result = matcher.match(mzlff_track())
    new = matcher.more_videos(result)
    assert [c.video_id for c in new] == ["clip"]
    assert new[0].rejected == "duration"
    assert matcher.more_videos(result) == []
