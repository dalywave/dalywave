"""Поиск записи трека Deezer на YouTube Music и оценка кандидатов.

Порядок поиска: альбом целиком (сверка списка треков — самый точный путь), ISRC,
песни по «артист название», видео — только если лучше ничего не нашлось.
Оценка каждого кандидата раскладывается на составляющие: этот разбор показывает
раздел «Спорные» в админке.
"""

from __future__ import annotations

import math
import threading
import time
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from typing import Any, Literal

import requests
from ytmusicapi import YTMusic
from ytmusicapi.exceptions import YTMusicError

from .deezer import DeezerAlbum, DeezerClient, DeezerError, DeezerTrack
from .normalize import (
    STRONG_MARKERS,
    TitleParts,
    clean_channel,
    fold,
    name_match,
    parse_title,
    text_similarity,
)

WEIGHTS = {"duration": 0.30, "title": 0.30, "artist": 0.20, "album": 0.10, "source": 0.10}
STRONG_FACTOR = 0.3
MILD_FACTOR = 0.6
# Темп ultra/super slowed отличается от обычного slowed — это другая запись.
_STRONG = STRONG_MARKERS | {"ultra", "super", "extra", "mega", "hyper"}

REJECT_DURATION_S = 15
REJECT_TITLE = 0.45
TITLE_GATE_LOW = 0.45
TITLE_GATE_HIGH = 0.85
NO_ARTIST_GATE = 0.75
# Ниже этой оценки кандидата не скачиваем вовсе.
MIN_TRY_SCORE = 0.6
# С кандидатом не хуже этого видео уже не ищем.
GOOD_SCORE = 0.85

# manual — видео, выбранное в админке вручную.
SOURCE_PRIORITY = {"manual": -1, "album": 0, "isrc": 1, "song": 2, "video": 3}
SOURCE_KIND_SCORE = {"atv": 1.0, "omv": 0.75, "ugc": 0.35, "other": 0.35}

VideosMode = Literal["auto", "always", "never"]

# ytmusicapi падает по-разному: сеть, ответ сервера, изменившийся формат страницы.
YTM_ERRORS = (YTMusicError, requests.RequestException, KeyError, IndexError, TypeError, ValueError)


def source_kind(video_type: str | None) -> str:
    match video_type:
        case "MUSIC_VIDEO_TYPE_ATV":
            return "atv"
        case "MUSIC_VIDEO_TYPE_OMV":
            return "omv"
        case "MUSIC_VIDEO_TYPE_UGC":
            return "ugc"
        case _:
            return "other"


def duration_score(diff_s: float | None) -> float:
    """≤2 с — идеально, ≤5 с — хорошо, дальше быстро падает, >15 с — отказ."""
    if diff_s is None:
        return 0.5
    d = abs(diff_s)
    if d <= 2:
        return 1.0
    if d <= 5:
        return 1.0 - 0.1 * (d - 2) / 3
    if d <= REJECT_DURATION_S:
        return 0.9 - 0.6 * (d - 5) / 10
    return 0.0


def title_score(expected: TitleParts, got: TitleParts) -> tuple[float, float, float]:
    """(итог, сходство основы, сходство версии)."""
    base = text_similarity(expected.base, got.base)
    if expected.version and got.version:
        version = text_similarity(expected.version, got.version)
    elif expected.version or got.version:
        version = 0.6
    else:
        version = 1.0
    return 0.8 * base + 0.2 * version, base, version


def marker_penalties(expected: TitleParts, got: TitleParts) -> list[dict[str, Any]]:
    penalties = []
    for marker in sorted(got.markers - expected.markers):
        factor = STRONG_FACTOR if marker in _STRONG else MILD_FACTOR
        penalties.append({"marker": marker, "where": "candidate", "factor": factor})
    for marker in sorted(expected.markers - got.markers):
        factor = STRONG_FACTOR if marker in _STRONG else MILD_FACTOR
        penalties.append({"marker": marker, "where": "deezer", "factor": factor})
    return penalties


@dataclass(frozen=True)
class Expected:
    """То, что ищем: трек Deezer в удобном для сравнения виде."""

    parts: TitleParts
    artists: tuple[str, ...]
    duration_s: int
    album: TitleParts | None

    @classmethod
    def from_track(cls, track: DeezerTrack) -> Expected:
        title = track.title
        if track.title_version and fold(track.title_version) not in fold(title):
            title = f"{title} {track.title_version}"
        return cls(
            parts=parse_title(title),
            artists=tuple(fold(a) for a in track.artists),
            duration_s=track.duration_s,
            album=parse_title(track.album_title) if track.album_title else None,
        )

    @property
    def main_artist(self) -> str:
        return self.artists[0] if self.artists else ""


@dataclass
class Candidate:
    video_id: str
    title: str
    artists: list[str]
    album: str | None
    duration_s: int | None
    video_type: str | None
    source: str
    rank: int
    sources: list[str] = field(default_factory=list)
    album_match: dict[str, Any] | None = None
    score: float = 0.0
    breakdown: dict[str, Any] = field(default_factory=dict)
    rejected: str | None = None

    def __post_init__(self) -> None:
        if not self.sources:
            self.sources = [self.source]

    @property
    def kind(self) -> str:
        return source_kind(self.video_type)

    @property
    def url(self) -> str:
        if self.video_id.startswith(("http://", "https://")):
            return self.video_id
        return f"https://music.youtube.com/watch?v={self.video_id}"

    def sort_key(self) -> tuple[bool, float, int, int]:
        return (self.rejected is None, self.score, -SOURCE_PRIORITY[self.source], -self.rank)

    def to_json(self) -> dict[str, Any]:
        return {
            "videoId": self.video_id,
            "url": self.url,
            "title": self.title,
            "artists": self.artists,
            "album": self.album,
            "durationMs": self.duration_s * 1000 if self.duration_s is not None else None,
            "videoType": self.video_type,
            "kind": self.kind,
            "source": self.source,
            "sources": self.sources,
            "rank": self.rank,
            "score": round(self.score, 4),
            "rejected": self.rejected,
            "breakdown": self.breakdown,
            "albumMatch": self.album_match,
        }


def score_candidate(exp: Expected, cand: Candidate) -> Candidate:
    """Считает оценку 0..1 и её разбор; заполняет поля кандидата и возвращает его."""
    got = parse_title(cand.title, cand.artists)

    diff = None if cand.duration_s is None else cand.duration_s - exp.duration_s
    s_duration = duration_score(diff)

    s_title, base_sim, version_sim = title_score(exp.parts, got)

    cand_names = [fold(clean_channel(a)) for a in cand.artists]
    cand_names += list(got.featured) + list(got.prefix_artists)
    matched = [a for a in exp.artists if any(name_match(a, c) for c in cand_names)]
    if matched:
        # Основной артист весит вдвое больше приглашённых.
        main_hit = exp.main_artist in matched
        others_hit = len(matched) - int(main_hit)
        frac = (2 * int(main_hit) + others_hit) / (2 + len(exp.artists) - 1)
        s_artist = 0.5 + 0.5 * frac
    else:
        s_artist = 0.0

    if cand.album_match is not None:
        s_album = float(cand.album_match["albumScore"])
    elif cand.album and exp.album:
        s_album = text_similarity(exp.album.base, parse_title(cand.album).base)
    else:
        s_album = 0.5

    s_source = SOURCE_KIND_SCORE[cand.kind]

    components = {
        "duration": s_duration,
        "title": s_title,
        "artist": s_artist,
        "album": s_album,
        "source": s_source,
    }
    weighted = sum(WEIGHTS[k] * v for k, v in components.items())
    penalties = marker_penalties(exp.parts, got)
    factor = math.prod(p["factor"] for p in penalties)
    # Взвешенная сумма прощает чужое название: у любой песни того же артиста с похожей
    # длительностью набегает ~0,6. Поэтому название и артист ещё и «ворота».
    title_gate = min(
        1.0, max(0.0, (base_sim - TITLE_GATE_LOW) / (TITLE_GATE_HIGH - TITLE_GATE_LOW))
    )
    artist_gate = 1.0 if matched else NO_ARTIST_GATE
    total = weighted * factor * title_gate * artist_gate

    rejected = None
    if diff is not None and abs(diff) > REJECT_DURATION_S:
        rejected = "duration"
    elif base_sim < REJECT_TITLE:
        rejected = "title"

    cand.score = 0.0 if rejected else total
    cand.rejected = rejected
    cand.breakdown = {
        "weights": WEIGHTS,
        "duration": {
            "score": round(s_duration, 4),
            "diffMs": None if diff is None else diff * 1000,
        },
        "title": {
            "score": round(s_title, 4),
            "base": round(base_sim, 4),
            "version": round(version_sim, 4),
            "expected": exp.parts.base,
            "got": got.base,
            "expectedVersion": exp.parts.version or None,
            "gotVersion": got.version or None,
        },
        "artist": {
            "score": round(s_artist, 4),
            "matched": matched,
            "missing": [a for a in exp.artists if a not in matched],
            "candidate": cand_names,
        },
        "album": {"score": round(s_album, 4), "fromAlbumMatch": cand.album_match is not None},
        "source": {"score": s_source, "kind": cand.kind},
        "weighted": round(weighted, 4),
        "penalties": penalties,
        "penaltyFactor": round(factor, 4),
        "titleGate": round(title_gate, 4),
        "artistGate": artist_gate,
        "total": round(total, 4),
    }
    return cand


# --- YouTube Music -------------------------------------------------------------


class YTMusicClient:
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

    def search(self, query: str, kind: str, limit: int = 10) -> list[dict[str, Any]]:
        return list(self._yt().search(query, filter=kind, limit=limit))

    def album(self, browse_id: str) -> dict[str, Any]:
        return dict(self._yt().get_album(browse_id))

    def playlist(self, playlist_id: str) -> dict[str, Any]:
        return dict(self._yt().get_playlist(playlist_id, limit=None))


def _names(items: list[dict[str, Any]] | None) -> list[str]:
    return [a["name"] for a in items or [] if a.get("name")]


def _duration(item: dict[str, Any]) -> int | None:
    value = item.get("duration_seconds")
    return int(value) if value is not None else None


def candidate_from_item(item: dict[str, Any], source: str, rank: int) -> Candidate | None:
    """Песня или видео из выдачи поиска ytmusicapi → кандидат."""
    video_id = item.get("videoId")
    if not video_id or item.get("isAvailable") is False:
        return None
    album = item.get("album")
    return Candidate(
        video_id=video_id,
        title=item.get("title") or "",
        artists=_names(item.get("artists")),
        album=album.get("name") if isinstance(album, dict) else album,
        duration_s=_duration(item),
        video_type=item.get("videoType"),
        source=source,
        rank=rank,
    )


def _year_score(dz_year: int | None, yt_year: str | int | None) -> float:
    if dz_year is None or not yt_year or not str(yt_year).isdigit():
        return 0.5
    return 1.0 if abs(int(yt_year) - dz_year) <= 1 else 0.0


def album_search_score(
    exp: Expected, dz_album: DeezerAlbum, item: dict[str, Any]
) -> tuple[float, dict[str, Any]]:
    """Насколько альбом из выдачи YouTube Music похож на альбом Deezer (до сверки треков)."""
    title_sim = text_similarity(parse_title(dz_album.title).base, parse_title(item["title"]).base)
    yt_artists = [fold(a) for a in _names(item.get("artists"))]
    artist_ok = any(name_match(a, b) for a in exp.artists for b in yt_artists)
    year = _year_score(dz_album.year, item.get("year"))
    score = 0.6 * title_sim + 0.25 * float(artist_ok) + 0.15 * year
    return score, {"titleSimilarity": round(title_sim, 4), "artistMatch": artist_ok, "year": year}


def align_album_track(
    exp: Expected, dz_album: DeezerAlbum, dz_index: int | None, tracks: list[dict[str, Any]]
) -> tuple[int, float] | None:
    """Находит трек в списке альбома YouTube Music: название, длительность, позиция."""
    best: tuple[int, float] | None = None
    for i, t in enumerate(tracks):
        if not t.get("videoId") or t.get("isAvailable") is False:
            continue
        got = parse_title(t.get("title") or "")
        s_title, base_sim, _ = title_score(exp.parts, got)
        duration = _duration(t)
        diff = None if duration is None else duration - exp.duration_s
        if base_sim < 0.6 or (diff is not None and abs(diff) > REJECT_DURATION_S):
            continue
        position = 1.0 if dz_index == i else 0.0
        score = 0.55 * s_title + 0.3 * duration_score(diff) + 0.15 * position
        score *= math.prod(p["factor"] for p in marker_penalties(exp.parts, got))
        if best is None or score > best[1]:
            best = (i, score)
    return best


@dataclass
class MatchResult:
    track: DeezerTrack
    album: DeezerAlbum | None
    candidates: list[Candidate]
    steps: list[dict[str, Any]]
    elapsed_ms: int

    @property
    def best(self) -> Candidate | None:
        return self.candidates[0] if self.candidates and not self.candidates[0].rejected else None

    def viable(self, min_score: float = MIN_TRY_SCORE) -> list[Candidate]:
        return [c for c in self.candidates if not c.rejected and c.score >= min_score]

    def to_json(self) -> dict[str, Any]:
        return {
            "track": self.track.to_json(),
            "candidates": [c.to_json() for c in self.candidates],
            "steps": self.steps,
            "elapsedMs": self.elapsed_ms,
        }


class Matcher:
    def __init__(self, deezer: DeezerClient, ytm: YTMusicClient | None = None) -> None:
        self.deezer = deezer
        self.ytm = ytm or YTMusicClient()

    def match(self, track: DeezerTrack, videos: VideosMode = "auto") -> MatchResult:
        started = time.monotonic()
        exp = Expected.from_track(track)
        steps: list[dict[str, Any]] = []

        dz_album = None
        if track.album_id:
            step_started = time.monotonic()
            try:
                dz_album = self.deezer.album(track.album_id)
            except DeezerError as e:
                steps.append(_step("deezer_album", step_started, error=str(e)))
            else:
                steps.append(_step("deezer_album", step_started, results=dz_album.nb_tracks))

        # Поиски независимы, поэтому параллельно: так быстрее на 1–2 секунды.
        with ThreadPoolExecutor(max_workers=3) as pool:
            futures = [pool.submit(self._album_step, exp, track, dz_album)]
            if track.isrc:
                futures.append(pool.submit(self._isrc_step, track.isrc))
            futures.append(pool.submit(self._search_step, "song", _query(track)))
            results = [f.result() for f in futures]

        merged: dict[str, Candidate] = {}
        for candidates, step in results:
            steps.append(step)
            _merge(merged, candidates)
        for cand in merged.values():
            score_candidate(exp, cand)

        best = max((c.score for c in merged.values() if not c.rejected), default=0.0)
        if videos == "always" or (videos == "auto" and best < GOOD_SCORE):
            candidates, step = self._search_step("video", _query(track))
            steps.append(step)
            new = _merge(merged, candidates)
            for cand in new:
                score_candidate(exp, cand)

        ordered = sorted(merged.values(), key=Candidate.sort_key, reverse=True)
        return MatchResult(
            track=track,
            album=dz_album,
            candidates=ordered,
            steps=steps,
            elapsed_ms=_ms(started),
        )

    def more_videos(self, result: MatchResult) -> list[Candidate]:
        """Добавляет видео к уже найденному, если их ещё не искали. Возвращает новых."""
        if any(s["name"] == "video" for s in result.steps):
            return []
        exp = Expected.from_track(result.track)
        candidates, step = self._search_step("video", _query(result.track))
        result.steps.append(step)
        merged = {c.video_id: c for c in result.candidates}
        new = _merge(merged, candidates)
        for cand in new:
            score_candidate(exp, cand)
        result.candidates = sorted(merged.values(), key=Candidate.sort_key, reverse=True)
        return new

    # --- шаги ---

    def _search_step(self, source: str, query: str) -> tuple[list[Candidate], dict[str, Any]]:
        started = time.monotonic()
        kind = "songs" if source == "song" else "videos"
        try:
            items = self.ytm.search(query, kind, limit=10)
        except YTM_ERRORS as e:
            return [], _step(source, started, query=query, error=repr(e))
        cands = [candidate_from_item(it, source, rank) for rank, it in enumerate(items[:8])]
        found = [c for c in cands if c is not None]
        return found, _step(source, started, query=query, results=len(found))

    def _isrc_step(self, isrc: str) -> tuple[list[Candidate], dict[str, Any]]:
        started = time.monotonic()
        try:
            items = self.ytm.search(isrc, "songs", limit=5)
        except YTM_ERRORS as e:
            return [], _step("isrc", started, query=isrc, error=repr(e))
        # Поиск по ISRC отдаёт нужную песню первой, дальше — случайный шум.
        cands = [candidate_from_item(it, "isrc", rank) for rank, it in enumerate(items[:2])]
        found = [c for c in cands if c is not None]
        return found, _step("isrc", started, query=isrc, results=len(found))

    def _album_step(
        self, exp: Expected, track: DeezerTrack, dz_album: DeezerAlbum | None
    ) -> tuple[list[Candidate], dict[str, Any]]:
        started = time.monotonic()
        if dz_album is None:
            return [], _step("album", started, error="нет альбома Deezer")
        query = f"{track.artist} {dz_album.title}"
        try:
            items = self.ytm.search(query, "albums", limit=10)
        except YTM_ERRORS as e:
            return [], _step("album", started, query=query, error=repr(e))

        ranked = []
        for item in items[:6]:
            if not item.get("browseId"):
                continue
            score, info = album_search_score(exp, dz_album, item)
            if info["titleSimilarity"] >= 0.7 and info["artistMatch"]:
                ranked.append((score, info, item))
        ranked.sort(key=lambda x: x[0], reverse=True)

        dz_index = dz_album.index_of(track.id)
        found: list[Candidate] = []
        tried = []
        for pre_score, info, item in ranked[:2]:
            try:
                yt_album = self.ytm.album(item["browseId"])
            except YTM_ERRORS as e:
                tried.append({"browseId": item["browseId"], "error": repr(e)})
                continue
            tracks = list(yt_album.get("tracks") or [])
            aligned = align_album_track(exp, dz_album, dz_index, tracks)
            tried.append({"browseId": item["browseId"], "aligned": aligned is not None})
            if aligned is None:
                continue
            index, align_score = aligned
            yt_count = int(yt_album.get("trackCount") or len(tracks))
            count_match = max(
                0.0, 1.0 - abs(yt_count - dz_album.nb_tracks) / max(dz_album.nb_tracks, 1)
            )
            album_score = (
                0.5 * info["titleSimilarity"]
                + 0.2 * float(info["artistMatch"])
                + 0.15 * info["year"]
                + 0.15 * count_match
            )
            album_match = {
                "browseId": item["browseId"],
                "title": yt_album.get("title"),
                "year": yt_album.get("year"),
                "trackCount": yt_count,
                "deezerTrackCount": dz_album.nb_tracks,
                "position": index + 1,
                "deezerPosition": None if dz_index is None else dz_index + 1,
                "alignScore": round(align_score, 4),
                "albumScore": round(album_score, 4),
                "searchScore": round(pre_score, 4),
            }
            album_artists = _names(yt_album.get("artists"))
            for t in self._audio_versions(yt_album, tracks, index):
                cand = Candidate(
                    video_id=t["videoId"],
                    title=t.get("title") or "",
                    artists=_names(t.get("artists")) or album_artists,
                    album=yt_album.get("title"),
                    duration_s=_duration(t),
                    video_type=t.get("videoType"),
                    source="album",
                    rank=0,
                    album_match=album_match,
                )
                found.append(cand)
            break
        return found, _step("album", started, query=query, results=len(found), albums=tried)

    def _audio_versions(
        self, yt_album: dict[str, Any], tracks: list[dict[str, Any]], index: int
    ) -> list[dict[str, Any]]:
        """В списке альбома YouTube Music может стоять клип (OMV) вместо аудио (ATV).
        Аудиоверсии лежат в отдельном плейлисте альбома — берём трек оттуда."""
        track = tracks[index]
        if track.get("videoType") == "MUSIC_VIDEO_TYPE_ATV" or not yt_album.get("audioPlaylistId"):
            return [track]
        try:
            audio = list(self.ytm.playlist(yt_album["audioPlaylistId"]).get("tracks") or [])
        except YTM_ERRORS:
            return [track]
        if index < len(audio) and audio[index].get("videoId"):
            same = audio[index]
            same_title = text_similarity(
                fold(same.get("title") or ""), fold(track.get("title") or "")
            )
            if same_title >= 0.8:
                return [same, track]
        return [track]


def _query(track: DeezerTrack) -> str:
    parts = parse_title(track.title)
    title = track.title
    if parts.featured:
        # «(feat. X)» в запросе только мешает поиску.
        title = " ".join(w for w in [parts.base, parts.version] if w)
    return f"{track.artist} {title}".strip()


def _merge(merged: dict[str, Candidate], candidates: list[Candidate]) -> list[Candidate]:
    new = []
    for cand in candidates:
        existing = merged.get(cand.video_id)
        if existing is None:
            merged[cand.video_id] = cand
            new.append(cand)
            continue
        for source in cand.sources:
            if source not in existing.sources:
                existing.sources.append(source)
        if SOURCE_PRIORITY[cand.source] < SOURCE_PRIORITY[existing.source]:
            existing.source = cand.source
            existing.rank = cand.rank
        if existing.album_match is None and cand.album_match is not None:
            existing.album_match = cand.album_match
        if existing.album is None:
            existing.album = cand.album
        if existing.video_type is None:
            existing.video_type = cand.video_type
    return new


def _ms(started: float) -> int:
    return int((time.monotonic() - started) * 1000)


def _step(name: str, started: float, **info: Any) -> dict[str, Any]:
    return {"name": name, "ms": _ms(started), **{k: v for k, v in info.items() if v is not None}}
