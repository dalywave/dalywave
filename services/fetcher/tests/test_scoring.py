from __future__ import annotations

from typing import Any

import pytest
from helpers import make_album, make_candidate, make_track

from dalywave_fetcher.deezer import DeezerTrack
from dalywave_fetcher.match import (
    MIN_TRY_SCORE,
    Expected,
    align_album_track,
    duration_score,
    score_candidate,
)


@pytest.mark.parametrize(
    ("diff", "expected"),
    [(0, 1.0), (2, 1.0), (-2, 1.0), (5, 0.9), (15, 0.3), (16, 0.0), (None, 0.5)],
)
def test_duration_score(diff: float | None, expected: float) -> None:
    assert duration_score(diff) == pytest.approx(expected)


def test_duration_score_is_monotonic() -> None:
    values = [duration_score(d / 2) for d in range(40)]
    assert values == sorted(values, reverse=True)


def score(track: DeezerTrack, **cand: Any) -> Any:
    return score_candidate(Expected.from_track(track), make_candidate(**cand))


def test_exact_atv_match_scores_one(track: DeezerTrack) -> None:
    c = score(track)
    assert c.rejected is None
    assert c.score == pytest.approx(1.0)
    assert set(c.breakdown) >= {"duration", "title", "artist", "album", "source", "penalties"}


def test_slowed_candidate_for_original_is_penalized(track: DeezerTrack) -> None:
    c = score(track, title="ZAKHELO (Slowed)", duration_s=129)
    assert c.score < MIN_TRY_SCORE
    assert {"marker": "slowed", "where": "candidate", "factor": 0.3} in c.breakdown["penalties"]


def test_slowed_deezer_track_wants_slowed_candidate() -> None:
    slowed = make_track(title="ZAKHELO (Slowed)", title_version="(Slowed)", duration_s=129)
    right = score(slowed, title="ZAKHELO (Slowed)", duration_s=129)
    original = score(slowed, title="ZAKHELO", duration_s=127)
    ultra = score(slowed, title="ZAKHELO (Ultra Slowed)", duration_s=130)
    assert right.score == pytest.approx(1.0)
    assert original.score < MIN_TRY_SCORE
    assert original.breakdown["penalties"][0]["where"] == "deezer"
    assert ultra.score < MIN_TRY_SCORE


def test_ultra_slowed_is_not_plain_slowed() -> None:
    ultra = make_track(title="MONTAGEM BATCHI (ULTRA SLOWED)", duration_s=97)
    assert score(ultra, title="MONTAGEM BATCHI (Ultra Slowed)", duration_s=97).score > 0.95
    assert score(ultra, title="MONTAGEM BATCHI (Slowed)", duration_s=99).score < MIN_TRY_SCORE


def test_live_and_remix_are_penalized_but_remaster_is_not() -> None:
    queen = make_track(
        title="Bohemian Rhapsody",
        artist="Queen",
        artists=("Queen",),
        duration_s=354,
        album_title="A Night At The Opera",
    )
    remaster = score(
        queen,
        title="Bohemian Rhapsody (Remastered 2011)",
        artists=["Queen"],
        album="A Night At The Opera (2011 Remaster)",
        duration_s=355,
    )
    live = score(
        queen,
        title="Bohemian Rhapsody (Live At Wembley Stadium)",
        artists=["Queen"],
        album="Live At Wembley",
        duration_s=351,
    )
    remix = score(queen, title="Bohemian Rhapsody (Remix)", artists=["Queen"], duration_s=356)
    assert remaster.score > 0.95
    assert live.score < MIN_TRY_SCORE
    assert remix.score < MIN_TRY_SCORE


def test_cover_by_other_artist_ranks_below_original() -> None:
    parker = make_track(
        title="Paint it Black",
        artist="Parker The Bandit",
        artists=("Parker The Bandit",),
        duration_s=159,
        album_title="Paint it Black",
    )
    own = score(
        parker,
        title="Paint it Black",
        artists=["Parker The Bandit"],
        album="Paint it Black",
        duration_s=160,
    )
    other = score(
        parker, title="Paint It Black", artists=["Some Band"], album="Covers", duration_s=161
    )
    assert own.score > 0.95
    assert other.score < own.score - 0.3
    assert other.breakdown["artistGate"] < 1


def test_duration_over_15s_is_rejected(track: DeezerTrack) -> None:
    c = score(track, duration_s=140)
    assert c.rejected == "duration"
    assert c.score == 0


def test_different_title_is_rejected_or_low(track: DeezerTrack) -> None:
    assert score(track, title="Completely Different Song").rejected == "title"
    # Похожее, но другое название того же артиста той же длины — ниже порога попытки.
    other = score(track, title="TRAKELLA", duration_s=116)
    assert other.score < MIN_TRY_SCORE


def test_official_video_scores_below_atv(track: DeezerTrack) -> None:
    atv = score(track)
    omv = score(
        track,
        title="Rushex - ZAKHELO (Official Video)",
        video_type="MUSIC_VIDEO_TYPE_OMV",
        album=None,
        source="video",
    )
    ugc = score(
        track,
        title="ZAKHELO",
        artists=["randomuser"],
        video_type="MUSIC_VIDEO_TYPE_UGC",
        album=None,
        source="video",
    )
    assert omv.score > MIN_TRY_SCORE
    assert atv.score > omv.score > ugc.score


def test_cyrillic_and_latin_names_match() -> None:
    molly = make_track(
        title="Нон стоп",
        artist="Пошлая Молли",
        artists=("Пошлая Молли",),
        duration_s=213,
        album_title="8 способов как бросить...",
    )
    c = score(molly, title="Non Stop", artists=["Poshlaya Molly"], album=None, duration_s=214)
    assert c.breakdown["artist"]["matched"] == ["пошлая молли"]
    assert c.score > 0.8


def test_featured_artists_count_in_artist_score() -> None:
    sunflower = make_track(
        title="Sunflower (Spider-Man: Into the Spider-Verse)",
        artist="Post Malone",
        artists=("Post Malone", "Swae Lee"),
        duration_s=158,
        album_title="Spider-Man: Into the Spider-Verse",
    )
    album = "Spider-Man: Into the Spider-Verse"
    title = "Sunflower (Spider-Man: Into the Spider-Verse)"
    both = score(
        sunflower, title=title, artists=["Post Malone", "Swae Lee"], album=album, duration_s=158
    )
    main_only = score(sunflower, title=title, artists=["Post Malone"], album=album, duration_s=158)
    assert both.breakdown["artist"]["score"] == pytest.approx(1.0)
    assert main_only.breakdown["artist"]["score"] < 1.0
    assert both.score > main_only.score > 0.9


def test_album_alignment_prefers_position_title_and_duration() -> None:
    album = make_album(
        [(101, "для тебя", 223), (102, "ящики", 199), (103, "hate core", 167)],
    )
    exp = Expected.from_track(
        make_track(
            id=102,
            title="ящики",
            artist="Mzlff",
            artists=("Mzlff", "Booker"),
            duration_s=199,
            album_title="светлая сторона",
        )
    )
    yt_tracks = [
        {"videoId": "a", "title": "для тебя", "duration_seconds": 223},
        {"videoId": "b", "title": "ящики", "duration_seconds": 200},
        {"videoId": "c", "title": "hate core", "duration_seconds": 167},
        {"videoId": None, "title": "ящики", "duration_seconds": 199},
    ]
    aligned = align_album_track(exp, album, album.index_of(102), yt_tracks)
    assert aligned is not None
    assert aligned[0] == 1
    assert (
        align_album_track(
            exp, album, 1, [{"videoId": "x", "title": "сейв", "duration_seconds": 199}]
        )
        is None
    )
