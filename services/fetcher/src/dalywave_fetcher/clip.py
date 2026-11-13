"""Клип для ленты открытий: официальное видео трека на YouTube, 10–15 секунд вокруг припева,
480p без звука, H.264 в MP4 (`+faststart`) ~1–2 МБ.

Путь: поиск yt-dlp («артист название official video») → отбор официального клипа (название,
артист или канал, длительность, без lyric/live/cover…) → место припева в клипе (у клипа бывает
вступление) → ссылка на поток видео (yt-dlp) и скачивание только нужного куска своим ffmpeg →
точная нарезка и сжатие ffmpeg → PUT по подписанной ссылке. Настройки YouTube (PO-токены,
cookies, клиенты плеера) — те же, что у скачивания аудио (`Downloader.options`).

Кодировщик H.264 — системный: в приложении для macOS `h264_videotoolbox`, для Windows `h264_mf`
(своя сборка ffmpeg под LGPL, без libx264); в Docker и разработке — libx264 полного ffmpeg.
Какой работает, проверяется один раз при запуске (`video_support`): без https в ffmpeg или без
рабочего кодировщика загрузчик клипы не берёт.
"""

from __future__ import annotations

import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import httpx
import yt_dlp
from yt_dlp.utils import YoutubeDLError

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

# Поток видео до 480p: H.264, затем VP9. AV1 не берём — ffmpeg приложения его не декодирует.
# Сначала прямая ссылка (https), затем HLS; DASH по кусочкам ffmpeg не читает.
_CODECS = ("avc1", "vp09", "vp9")
VIDEO_FORMAT = "/".join(
    [f"bv*[height<=480][vcodec^={c}][protocol=https]" for c in _CODECS]
    + [f"bv*[height<=480][vcodec^={c}][protocol^=m3u8]" for c in _CODECS]
)
SEARCH_LIMIT = 8
# Ролик заметно короче трека — нарезка или тизер; длиннее больше чем на 2 минуты — фильм.
MIN_SHARE = 0.9
MAX_EXTRA_S = 120.0
# Вступление клипа до первой ноты: столько лишнего в начале считаем вступлением, а не растяжкой.
INTRO_MAX_S = 60.0
SAME_LENGTH_S = 3.0
# Запас скачивания с каждой стороны: ffmpeg режет поток копированием по ключевым кадрам.
MARGIN_S = 1.0
# Скачивание куска: ffmpeg ждёт сеть не дольше READ_TIMEOUT_S, весь кусок — не дольше FETCH_TIMEOUT_S.
READ_TIMEOUT_S = 30
FETCH_TIMEOUT_S = 180
ENCODE_TIMEOUT_S = 180
MAX_AVAILABLE_WAIT_S = 30.0
# libx264: качество; аппаратные кодировщики: средний битрейт (800 кбит/с × 12 с ≈ 1,2 МБ)
CRF = 27
CRF_STEP = 4
BITRATE_K = 800
SMALLER_BITRATE_K = 550
TARGET_BYTES = 2 * 1024 * 1024
# Сколько последних строк stderr ffmpeg попадает в ошибку задачи (её видно в админке)
STDERR_LINES = 3
MIN_TITLE_SIMILARITY = 0.82
# Меньше стольких байт на секунду после сжатия libx264 — в кадре почти ничего не движется: это
# обложка под звук, а не клип (живой клип 480p — 40–100 КБ/с, неподвижная картинка — 2–5 КБ/с).
# Аппаратные кодировщики держат битрейт и на неподвижной картинке — для них `is_frozen`.
STILL_BYTES_PER_S = 12_000
# Кадр стоит (freezedetect: средняя разница кадров не больше FREEZE_NOISE) такую долю отрывка
FREEZE_SHARE = 0.8
FREEZE_NOISE = 0.003


def is_still(size_bytes: int, length_ms: int) -> bool:
    return size_bytes < STILL_BYTES_PER_S * max(1, length_ms) / 1000


def is_frozen(src: Path, offset_s: float, length_s: float) -> bool:
    """Картинка в отрывке почти не меняется (обложка под звук) — по кадрам, а не по размеру
    файла. Не вышло проверить — считаем, что движется."""
    cmd = [
        tools.exe("ffmpeg"),
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "info",
        "-ss",
        f"{max(0.0, offset_s):.3f}",
        "-t",
        f"{length_s:.3f}",
        "-i",
        str(src),
        "-map",
        "0:v:0",
        "-vf",
        f"scale=-2:240,freezedetect=n={FREEZE_NOISE}:d={length_s * FREEZE_SHARE:.2f}",
        "-f",
        "null",
        "-",
    ]
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60, check=False)
    except (OSError, subprocess.SubprocessError):
        return False
    return proc.returncode == 0 and "freezedetect.freeze_start" in proc.stderr


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


_FFMPEG_PREFIX = re.compile(r"^\[[^\]]* @ 0x[0-9a-f]+\]\s*")
_URL = re.compile(r"(https?://[^/\s]+)/\S*")


def stderr_tail(text: str, lines: int = STDERR_LINES) -> str:
    """Последние строки stderr ffmpeg одной строкой — для ошибки задачи. Без адресов в памяти
    и без хвостов ссылок (в ссылке потока — IP загрузчика и подписи)."""
    tail = [line.strip() for line in text.strip().splitlines() if line.strip()][-lines:]
    return " | ".join(_URL.sub(r"\1/…", _FFMPEG_PREFIX.sub("", line)) for line in tail)


# ───────────── кодировщик ─────────────


def encoder_candidates(platform: str = sys.platform) -> list[str]:
    """Кодировщики H.264 по порядку: системный аппаратный (без GPL), затем libx264 полного
    ffmpeg (Docker, разработка). `-allow_sw 1` / программный MFT Windows — запас, если нет железа."""
    system = {"darwin": ["h264_videotoolbox"], "win32": ["h264_mf"]}.get(platform, [])
    return [*system, "libx264"]


def encoder_args(encoder: str, smaller: bool = False) -> list[str]:
    """Параметры кодировщика: main/high, yuv420p — играют все браузеры и AVPlayer."""
    if encoder == "libx264":
        crf = CRF + (CRF_STEP if smaller else 0)
        return ["-c:v", "libx264", "-profile:v", "high", "-preset", "slow", "-crf", str(crf)]
    rate = SMALLER_BITRATE_K if smaller else BITRATE_K
    args = ["-c:v", encoder, "-b:v", f"{rate}k", "-profile:v", "main"]
    if encoder == "h264_videotoolbox":
        # Предел скорости — от всплесков на резких сменах кадра; без железа — программный VT
        return [*args, "-maxrate", f"{rate * 3 // 2}k", "-allow_sw", "1", "-realtime", "0"]
    if encoder == "h264_mf":
        # Средний битрейт без набивки: неподвижная картинка остаётся маленькой (is_still)
        return [*args, "-rate_control", "u_vbr"]
    return args


@dataclass(frozen=True)
class VideoSupport:
    """Может ли ffmpeg делать клипы: `encoder` — рабочий кодировщик, иначе `problem` — почему нет."""

    encoder: str | None
    problem: str | None = None
    # Код ошибки задачи: no_encoder — нечем кодировать, no_https — ffmpeg не скачает кусок
    code: str = "no_encoder"


_support: dict[str, VideoSupport] = {}


def video_support(refresh: bool = False) -> VideoSupport:
    """Проверка ffmpeg один раз на процесс: https (кусок видео качает он) и кодировщик, который
    действительно кодирует — аппаратный бывает в списке, но не работает (виртуалка, CI)."""
    ffmpeg = tools.exe("ffmpeg")
    if refresh or ffmpeg not in _support:
        _support[ffmpeg] = _check_video(ffmpeg)
    return _support[ffmpeg]


def _check_video(ffmpeg: str) -> VideoSupport:
    try:
        protocols = _ffmpeg_list(ffmpeg, "-protocols")
        encoders = _ffmpeg_list(ffmpeg, "-encoders")
    except (OSError, subprocess.SubprocessError) as e:
        return VideoSupport(None, f"ffmpeg не запускается: {e}")
    if "https" not in protocols.split():
        return VideoSupport(None, "в ffmpeg нет https — обновите приложение", "no_https")
    listed = {parts[1] for line in encoders.splitlines() if len(parts := line.split()) > 1}
    failed: list[str] = []
    for encoder in encoder_candidates():
        if encoder not in listed:
            continue
        problem = _try_encoder(ffmpeg, encoder)
        if problem is None:
            return VideoSupport(encoder)
        failed.append(f"{encoder}: {problem}")
    reason = "; ".join(failed) if failed else "обновите приложение"
    return VideoSupport(None, f"нет видеокодировщика H.264 ({reason})")


def _ffmpeg_list(ffmpeg: str, what: str) -> str:
    out = subprocess.run(
        [ffmpeg, "-hide_banner", what], capture_output=True, text=True, timeout=20, check=False
    )
    return out.stdout


def _try_encoder(ffmpeg: str, encoder: str) -> str | None:
    """Десять серых кадров 256×144 через кодировщик в никуда; None — работает."""
    width, height, frames = 256, 144, 10
    raw = bytes([128]) * (width * height * 3 // 2 * frames)
    cmd = [
        ffmpeg,
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "yuv420p",
        "-s",
        f"{width}x{height}",
        "-r",
        "30",
        "-i",
        "pipe:0",
        *encoder_args(encoder),
        "-pix_fmt",
        "yuv420p",
        "-f",
        "null",
        "-",
    ]
    try:
        proc = subprocess.run(cmd, input=raw, capture_output=True, timeout=30, check=False)
    except (OSError, subprocess.SubprocessError) as e:
        return str(e)
    if proc.returncode == 0:
        return None
    return stderr_tail(proc.stderr.decode(errors="replace"), 2) or f"код {proc.returncode}"


def encode(
    src: Path,
    dst: Path,
    offset_s: float,
    length_s: float,
    encoder: str = "libx264",
    smaller: bool = False,
) -> None:
    """Точная нарезка и сжатие: без звука, до 480p, не больше 30 кадров/с, H.264, faststart."""
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
        *encoder_args(encoder, smaller),
        "-pix_fmt",
        "yuv420p",
        "-map_metadata",
        "-1",
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        str(dst),
    ]
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=ENCODE_TIMEOUT_S, check=False
        )
    except subprocess.TimeoutExpired as e:
        raise DownloadError(
            "encode_failed", f"{encoder}: ffmpeg не уложился в {e.timeout} с"
        ) from e
    if proc.returncode != 0 or not dst.is_file():
        tail = stderr_tail(proc.stderr) or f"код {proc.returncode}"
        raise DownloadError("encode_failed", f"{encoder}: {tail}")


def encode_small(
    src: Path, dst: Path, offset_s: float, length_s: float, encoder: str = "libx264"
) -> str:
    """Сжатие с обычными настройками; больше ~2 МБ — ещё раз, заметнее. Возвращает, с какими
    настройками сжато: `crf27`, `800k`…"""
    encode(src, dst, offset_s, length_s, encoder)
    smaller = dst.stat().st_size > TARGET_BYTES
    if smaller:
        encode(src, dst, offset_s, length_s, encoder, smaller=True)
    if encoder == "libx264":
        return f"crf{CRF + (CRF_STEP if smaller else 0)}"
    return f"{SMALLER_BITRATE_K if smaller else BITRATE_K}k"


def fetch_args(
    url: str,
    start_s: float,
    end_s: float,
    out: Path,
    headers: dict[str, str] | None = None,
    cookies: str | None = None,
) -> list[str]:
    """ffmpeg: только кусок [start_s, end_s] потока по ссылке, копированием, без звука, в MKV
    (в него ложатся и H.264, и VP9). Перед `-ss` ffmpeg начинает с ключевого кадра — лишнее
    в начале отрезает `encode`."""
    cmd = [
        tools.exe("ffmpeg"),
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-rw_timeout",
        str(READ_TIMEOUT_S * 1_000_000),
    ]
    if cookies:
        cmd += ["-cookies", cookies]
    if headers:
        # Каждый заголовок с \r\n в конце, иначе ffmpeg ругается (как делает yt-dlp)
        cmd += ["-headers", "".join(f"{k}: {v}\r\n" for k, v in headers.items())]
    cmd += [
        "-ss",
        f"{max(0.0, start_s):.3f}",
        "-t",
        f"{max(0.1, end_s - start_s):.3f}",
        "-i",
        url,
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-c",
        "copy",
        "-f",
        "matroska",
        str(out),
    ]
    return cmd


class ClipMaker:
    """Клипы по заданиям сервера. Обращения к YouTube — через общий `Downloader`: те же
    настройки и та же пауза между запросами, что у скачивания аудио."""

    def __init__(self, downloader: Downloader) -> None:
        self.downloader = downloader

    def support(self) -> VideoSupport:
        return video_support()

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
        """Ссылку на поток даёт yt-dlp, кусок качает ffmpeg сам: так его ошибка (stderr) видна
        в задаче, а 403 потока узнаётся и пробуется другим клиентом."""
        opts = self.downloader.options(out_dir, "raw", clients)
        collector = _Collector()
        opts.update({"format": VIDEO_FORMAT, "logger": collector, "skip_download": True})
        out = out_dir / "raw.mkv"
        env = None
        if proxy := self.downloader.settings.proxy:
            # Как у yt-dlp: ffmpeg берёт http-прокси из окружения (SOCKS он не умеет)
            env = {**os.environ, "http_proxy": proxy, "HTTP_PROXY": proxy}
        with self.downloader.paced():
            try:
                with yt_dlp.YoutubeDL(opts) as ydl:
                    info = ydl.extract_info(
                        f"https://www.youtube.com/watch?v={video_id}", download=False
                    )
                    fmt = ((info or {}).get("requested_formats") or [info or {}])[0]
                    url = str(fmt.get("url") or "")
                    cookies = (
                        "".join(
                            f"{c.name}={c.value}; path={c.path}; domain={c.domain};\r\n"
                            for c in ydl.cookiejar.get_cookies_for_url(url)
                        )
                        if url
                        else ""
                    )
            except YoutubeDLError as e:
                message = str(e).removeprefix("ERROR: ")
                text = " ".join([message, *collector.warnings])
                raise DownloadError(classify_error(text), message, collector.warnings) from e
            if not url:
                raise DownloadError("format_unavailable", "yt-dlp не дал ссылку на видео")
            # Ссылка YouTube оживает не сразу (`available_at`): раньше срока поток отвечает 403
            wait = float(fmt.get("available_at") or 0) - time.time()
            if wait > 0:
                time.sleep(min(wait + 0.5, MAX_AVAILABLE_WAIT_S))
            headers = fmt.get("http_headers") or (info or {}).get("http_headers") or {}
            cmd = fetch_args(url, start_s, end_s, out, headers, cookies or None)
            try:
                proc = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=FETCH_TIMEOUT_S,
                    env=env,
                    check=False,
                )
            except subprocess.TimeoutExpired as e:
                raise DownloadError(
                    "network", f"ffmpeg не скачал кусок за {e.timeout} с", collector.warnings
                ) from e
        if proc.returncode != 0 or not out.is_file() or out.stat().st_size == 0:
            tail = stderr_tail(proc.stderr) or f"код {proc.returncode}"
            code = classify_error(tail)
            raise DownloadError(
                "fetch_failed" if code == "unknown" else code,
                f"ffmpeg ({fmt.get('format_id')}, {fmt.get('vcodec')}): {tail}",
                collector.warnings,
            )
        return out

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
        support = self.support()
        details["encoder"] = support.encoder
        if support.encoder is None:
            # Не тратим запросы к YouTube: сделать клип всё равно нечем
            return {
                "status": "error",
                "error": f"{support.code}: {support.problem}"[:300],
                "details": details,
            }
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
                quality = encode_small(raw, out, offset, length_ms / 1000, support.encoder)
            except DownloadError as e:
                return self._error(e, details, best.video_id)
            timings["encode"] = _ms(t)
            info = probe(out)
            size = out.stat().st_size
            details["quality"] = quality
            if is_still(size, length_ms) or is_frozen(raw, offset, length_ms / 1000):
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
