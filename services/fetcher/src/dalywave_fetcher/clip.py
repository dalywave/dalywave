"""Клип для ленты открытий: официальное видео трека на YouTube, 10–15 секунд вокруг припева,
480p без звука, H.264 в MP4 (`+faststart`) ~1–2 МБ.

Путь: поиск yt-dlp («артист название official video») → отбор официального клипа (название,
артист или канал, длительность, без lyric/live/cover…) → место припева в клипе (у клипа бывает
вступление) → скачивание только нужного куска видео без звука → точная нарезка и сжатие ffmpeg →
PUT по подписанной ссылке. Настройки YouTube (PO-токены, cookies, клиенты плеера) — те же, что у
скачивания аудио (`Downloader.options`).
"""

from __future__ import annotations

import json
import math
import re
import shutil
import subprocess
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import httpx
import yt_dlp
from yt_dlp.utils import YoutubeDLError, download_range_func

from . import tools
from .download import (
    RETRY_WITH_FALLBACK,
    Downloader,
    DownloadError,
    _Collector,
    classify_error,
)
from .normalize import (
    STRONG_MARKERS,
    clean_channel,
    fold,
    has_cyrillic,
    name_match,
    parse_title,
    split_names,
    text_similarity,
)

# Видео без звука, H.264 — его играют все браузеры и AVPlayer; 480p хватает телефону.
VIDEO_FORMAT = "bv*[height<=480][vcodec^=avc1]/bv*[height<=480]"
SEARCH_LIMIT = 8
# Ролик заметно короче трека — нарезка или тизер; длиннее больше чем на 2 минуты — фильм.
MIN_SHARE = 0.9
MAX_EXTRA_S = 120.0
# Вступление клипа до первой ноты: столько лишнего в начале считаем вступлением, а не растяжкой.
INTRO_MAX_S = 60.0
SAME_LENGTH_S = 3.0
# Запас скачивания с каждой стороны: ffmpeg режет поток копированием по ключевым кадрам.
MARGIN_S = 1.0
CRF = 27
CRF_STEP = 4
TARGET_BYTES = 2 * 1024 * 1024
MIN_TITLE_SIMILARITY = 0.82
# Меньше стольких байт на секунду после сжатия — в кадре почти ничего не движется: это обложка
# под звук, а не клип (живой клип 480p — 40–100 КБ/с, неподвижная картинка — 2–5 КБ/с).
STILL_BYTES_PER_S = 12_000


def is_still(size_bytes: int, length_ms: int) -> bool:
    return size_bytes < STILL_BYTES_PER_S * max(1, length_ms) / 1000


# Слова, которые выдают не клип: текст, звук, концерт, реакция… Мимо, если их нет в названии трека.
_NOT_A_CLIP = re.compile(
    r"\b(?:lyrics?|lyric\s+video|letra|official\s+audio|audio|visuali[sz]er|teaser|trailer|"
    r"snippet|preview|reaction|react|karaoke|cover|live|remix|sped\s*up|slowed|nightcore|"
    r"amv|gmv|fan\s*made|fanmade|fan\s+video|edit|"
    r"текст|слова|караоке|кавер|реакция|лайв|тизер|фан|1\s+hour|10\s+hours?)\b"
)
_OFFICIAL = re.compile(
    r"\b(?:official\s+(?:music\s+)?video|music\s+video|official\s+mv|mv|official\s+clip|"
    r"videoclip|video\s+oficial|клип|премьера\s+клипа|официальный\s+клип)\b"
)


@dataclass
class Pick:
    video_id: str
    title: str
    channel: str
    duration_s: float
    views: int
    score: float = 0.0
    reasons: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.reasons

    def to_json(self) -> dict[str, Any]:
        return {
            "videoId": self.video_id,
            "title": self.title,
            "channel": self.channel,
            "durationS": self.duration_s,
            "views": self.views,
            "score": round(self.score, 3),
            "rejected": self.reasons or None,
        }


def _entry(item: dict[str, Any]) -> Pick | None:
    video_id = item.get("id")
    if not video_id or item.get("ie_key") not in (None, "Youtube"):
        return None
    return Pick(
        video_id=str(video_id),
        title=str(item.get("title") or ""),
        channel=str(item.get("channel") or item.get("uploader") or ""),
        duration_s=float(item.get("duration") or 0),
        views=int(item.get("view_count") or 0),
    )


def judge(pick: Pick, title: str, artists: list[str], duration_s: float) -> Pick:
    """Годится ли ролик в клип трека; причины отказа — в `reasons`, сила — в `score`."""
    expected = parse_title(title, artists)
    got = parse_title(pick.title, artists)
    video_folded = fold(pick.title)
    track_folded = fold(title)
    folded_artists = [a for name in artists for a in split_names(name)]

    similarity = text_similarity(expected.base, got.base) if expected.base else 0.0
    if similarity < MIN_TITLE_SIMILARITY and f" {expected.base} " not in f" {video_folded} ":
        pick.reasons.append("title")

    channel = pick.channel
    if channel.casefold().endswith("- topic"):
        pick.reasons.append("topic_channel")
    channel_artist = fold(clean_channel(channel))
    by_channel = any(name_match(channel_artist, a) for a in folded_artists)
    in_title = any(name_match(p, a) for p in got.prefix_artists for a in folded_artists) or any(
        len(a) >= 3 and f" {a} " in f" {video_folded} " for a in folded_artists
    )
    if not (by_channel or in_title):
        pick.reasons.append("artist")

    extra = set(got.markers & STRONG_MARKERS) - set(expected.markers)
    words = {m.group(0) for m in _NOT_A_CLIP.finditer(video_folded)}
    words -= {m.group(0) for m in _NOT_A_CLIP.finditer(track_folded)}
    if extra or words:
        pick.reasons.append("version:" + ",".join(sorted(extra | words)))

    if not pick.duration_s:
        pick.reasons.append("duration_unknown")
    elif not (duration_s * MIN_SHARE <= pick.duration_s <= duration_s + MAX_EXTRA_S):
        pick.reasons.append("duration")

    pick.score = similarity
    if _OFFICIAL.search(video_folded):
        pick.score += 0.5
    if by_channel or "vevo" in channel.casefold():
        pick.score += 0.3
    if pick.views:
        pick.score += min(0.4, math.log10(pick.views + 1) / 25)
    if pick.duration_s and duration_s:
        pick.score -= min(0.3, abs(pick.duration_s - duration_s) / 600)
    return pick


def choose(
    items: list[dict[str, Any]],
    title: str,
    artists: list[str],
    duration_s: float,
    exclude: set[str] | frozenset[str] = frozenset(),
) -> tuple[Pick | None, list[Pick]]:
    """Лучший официальный клип из выдачи поиска и все кандидаты с причинами отказа."""
    picks: list[Pick] = []
    for item in items:
        pick = _entry(item)
        if pick is None:
            continue
        if pick.video_id in exclude:
            pick.reasons.append("excluded")
            picks.append(pick)
            continue
        picks.append(judge(pick, title, artists, duration_s))
    good = sorted((p for p in picks if p.ok), key=lambda p: p.score, reverse=True)
    return (good[0] if good else None), picks


def clip_start_ms(hook_ms: int, track_ms: int, video_ms: int, length_ms: int) -> int:
    """Где в клипе припев трека. Клип той же длины — там же; длиннее на вступление (до минуты) —
    сдвиг на вступление; иначе (растянут, сокращён) — пропорционально."""
    extra = video_ms - track_ms
    if abs(extra) <= SAME_LENGTH_S * 1000:
        start = hook_ms
    elif 0 < extra <= INTRO_MAX_S * 1000:
        start = hook_ms + extra
    else:
        start = round(hook_ms * video_ms / track_ms) if track_ms else hook_ms
    return max(0, min(start, video_ms - length_ms))


def search_queries(title: str, artists: list[str]) -> list[str]:
    head = f"{artists[0]} {title}" if artists else title
    second = "клип" if has_cyrillic(head) else "music video"
    return [f"{head} official video", f"{head} {second}"]


def probe(path: Path) -> dict[str, Any]:
    out = subprocess.run(
        [
            tools.exe("ffprobe"),
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate:format=duration",
            "-of",
            "json",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    try:
        data = json.loads(out.stdout or "{}")
    except ValueError:
        return {}
    stream = (data.get("streams") or [{}])[0]
    num, _, den = str(stream.get("r_frame_rate") or "0/1").partition("/")
    try:
        fps = float(num) / float(den or 1)
    except (ValueError, ZeroDivisionError):
        fps = 0.0
    try:
        duration = float((data.get("format") or {}).get("duration") or 0)
    except ValueError:
        duration = 0.0
    return {
        "width": stream.get("width"),
        "height": stream.get("height"),
        "fps": fps,
        "duration": duration,
    }


def encode(src: Path, dst: Path, offset_s: float, length_s: float, crf: int = CRF) -> None:
    """Точная нарезка и сжатие: без звука, до 480p, не больше 30 кадров/с, H.264 high, faststart."""
    info = probe(src)
    filters = ["scale=-2:'min(480,ih)'"]
    if info.get("fps", 0) > 30.5:
        filters.append("fps=30")
    cmd = [
        tools.exe("ffmpeg"),
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        str(src),
        "-ss",
        f"{max(0.0, offset_s):.3f}",
        "-t",
        f"{length_s:.3f}",
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-vf",
        ",".join(filters),
        "-c:v",
        "libx264",
        "-profile:v",
        "high",
        "-pix_fmt",
        "yuv420p",
        "-preset",
        "slow",
        "-crf",
        str(crf),
        "-map_metadata",
        "-1",
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        str(dst),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0 or not dst.is_file():
        raise DownloadError("encode_failed", proc.stderr.strip()[-500:] or "ffmpeg упал")


def encode_small(src: Path, dst: Path, offset_s: float, length_s: float) -> int:
    """Сжатие с CRF по умолчанию; больше ~2 МБ — ещё раз, заметнее. Возвращает CRF."""
    crf = CRF
    encode(src, dst, offset_s, length_s, crf)
    if dst.stat().st_size > TARGET_BYTES:
        crf += CRF_STEP
        encode(src, dst, offset_s, length_s, crf)
    return crf


class ClipMaker:
    """Клипы по заданиям сервера. Обращения к YouTube — через общий `Downloader`: те же
    настройки и та же пауза между запросами, что у скачивания аудио."""

    def __init__(self, downloader: Downloader) -> None:
        self.downloader = downloader

    @property
    def work_root(self) -> Path:
        return self.downloader.settings.data_dir / "clips"

    def search(self, query: str) -> list[dict[str, Any]]:
        opts = self.downloader.options(self.work_root, "search")
        collector = _Collector()
        opts.update(
            {
                "logger": collector,
                "extract_flat": "in_playlist",
                "skip_download": True,
            }
        )
        try:
            with self.downloader.paced(), yt_dlp.YoutubeDL(opts) as ydl:
                info = ydl.extract_info(f"ytsearch{SEARCH_LIMIT}:{query}", download=False)
        except YoutubeDLError as e:
            message = str(e).removeprefix("ERROR: ")
            text = " ".join([message, *collector.warnings])
            raise DownloadError(classify_error(text), message, collector.warnings) from e
        return list((info or {}).get("entries") or [])

    def fetch_range(self, video_id: str, start_s: float, end_s: float, out_dir: Path) -> Path:
        """Только кусок видео [start_s, end_s] без звука; при 403 — ещё раз другим клиентом."""
        try:
            return self._fetch_range(video_id, start_s, end_s, out_dir, None)
        except DownloadError as e:
            fallback = self.downloader.fallback_clients()
            if e.code not in RETRY_WITH_FALLBACK or not fallback:
                raise
            return self._fetch_range(video_id, start_s, end_s, out_dir, fallback)

    def _fetch_range(
        self,
        video_id: str,
        start_s: float,
        end_s: float,
        out_dir: Path,
        clients: tuple[str, ...] | None,
    ) -> Path:
        opts = self.downloader.options(out_dir, "raw", clients)
        collector = _Collector()
        opts.update(
            {
                "format": VIDEO_FORMAT,
                "logger": collector,
                "download_ranges": download_range_func(None, [(start_s, end_s)]),
                "force_keyframes_at_cuts": False,
            }
        )
        try:
            with self.downloader.paced(), yt_dlp.YoutubeDL(opts) as ydl:
                info = ydl.extract_info(
                    f"https://www.youtube.com/watch?v={video_id}", download=True
                )
        except YoutubeDLError as e:
            message = str(e).removeprefix("ERROR: ")
            text = " ".join([message, *collector.warnings])
            raise DownloadError(classify_error(text), message, collector.warnings) from e
        requested = ((info or {}).get("requested_downloads") or [info or {}])[0]
        path = Path(requested.get("filepath") or requested.get("_filename") or "")
        if not path.is_file():
            found = sorted(out_dir.glob("raw*"))
            if not found:
                raise DownloadError("unknown", "yt-dlp не сохранил файл", collector.warnings)
            path = found[0]
        return path

    def make(
        self,
        *,
        title: str,
        artists: list[str],
        duration_ms: int,
        hook_start_ms: int,
        length_ms: int,
        exclude_video_ids: list[str] | None = None,
        upload: Any = None,
        keep: Path | None = None,
    ) -> dict[str, Any]:
        """Клип целиком. Возвращает `ClipResult` сервера: status ready / none / error.

        `upload` — `UploadSpec` (PUT по подписанной ссылке); `keep` — куда скопировать готовый MP4
        (живая проверка без сервера)."""
        timings: dict[str, int] = {}
        details: dict[str, Any] = {"timingsMs": timings}
        started = time.monotonic()
        exclude = set(exclude_video_ids or [])
        duration_s = duration_ms / 1000
        best: Pick | None = None
        try:
            for query in search_queries(title, artists):
                items = self.search(query)
                best, picks = choose(items, title, artists, duration_s, exclude)
                details.setdefault("searches", []).append(
                    {"query": query, "candidates": [p.to_json() for p in picks]}
                )
                if best:
                    break
        except DownloadError as e:
            return self._error(e, details)
        timings["search"] = _ms(started)
        if best is None:
            return {"status": "none", "details": details}

        video_ms = round(best.duration_s * 1000)
        start_ms = clip_start_ms(hook_start_ms, duration_ms, video_ms, length_ms)
        details["chosen"] = best.to_json()
        self.work_root.mkdir(parents=True, exist_ok=True)
        work = Path(tempfile.mkdtemp(prefix="clip-", dir=self.work_root))
        try:
            t = time.monotonic()
            section_start = max(0.0, start_ms / 1000 - MARGIN_S)
            section_end = (start_ms + length_ms) / 1000 + MARGIN_S
            try:
                raw = self.fetch_range(best.video_id, section_start, section_end, work)
            except DownloadError as e:
                return self._error(e, details, best.video_id)
            timings["download"] = _ms(t)
            # Копированием поток режется по ключевому кадру до начала: лишнее — в начале файла
            got = probe(raw).get("duration") or 0.0
            offset = max(0.0, got - (section_end - start_ms / 1000))
            out = work / "clip.mp4"
            t = time.monotonic()
            try:
                crf = encode_small(raw, out, offset, length_ms / 1000)
            except DownloadError as e:
                return self._error(e, details, best.video_id)
            timings["encode"] = _ms(t)
            info = probe(out)
            size = out.stat().st_size
            details["crf"] = crf
            if is_still(size, length_ms):
                # Неподвижная картинка под звук (обложка на канале артиста) — не клип
                details["still"] = {"videoId": best.video_id, "sizeBytes": size}
                return {"status": "none", "details": details}
            if keep is not None:
                keep.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(out, keep)
            if upload is not None:
                t = time.monotonic()
                uploaded = self._upload(out, upload)
                timings["upload"] = _ms(t)
                details["upload"] = uploaded
                if not uploaded.get("ok"):
                    return {
                        "status": "error",
                        "videoId": best.video_id,
                        "error": f"upload_failed: {uploaded.get('error') or uploaded.get('status')}",
                        "details": details,
                    }
            timings["total"] = _ms(started)
            return {
                "status": "ready",
                "videoId": best.video_id,
                "videoDurationMs": video_ms,
                "clipStartMs": start_ms,
                "durationMs": round((info.get("duration") or length_ms / 1000) * 1000),
                "sizeBytes": size,
                "width": info.get("width"),
                "height": info.get("height"),
                "details": details,
            }
        finally:
            shutil.rmtree(work, ignore_errors=True)

    @staticmethod
    def _error(
        e: DownloadError, details: dict[str, Any], video_id: str | None = None
    ) -> dict[str, Any]:
        details["warnings"] = e.warnings[-5:]
        return {
            "status": "error",
            "videoId": video_id,
            "error": f"{e.code}: {e.message[:300]}",
            "details": details,
        }

    @staticmethod
    def _upload(path: Path, upload: Any) -> dict[str, Any]:
        headers = dict(upload.headers)
        headers.setdefault("Content-Type", "video/mp4")
        data = path.read_bytes()
        try:
            response = httpx.request(
                upload.method,
                upload.url,
                content=data,
                headers={**headers, "Content-Length": str(len(data))},
                timeout=httpx.Timeout(30.0, write=120.0),
            )
        except httpx.HTTPError as e:
            return {"ok": False, "status": None, "error": str(e)}
        return {
            "ok": response.is_success,
            "status": response.status_code,
            "error": None if response.is_success else response.text[:300],
        }


def error_code(result: dict[str, Any]) -> str | None:
    """Код сбоя из `error` («код: текст») — чтобы цикл узнал bot_check и остыл."""
    error = result.get("error")
    if result.get("status") != "error" or not error:
        return None
    return str(error).split(":", 1)[0].strip() or None


def _ms(started: float) -> int:
    return int((time.monotonic() - started) * 1000)
