"""Скачивание лучшего аудиопотока через yt-dlp без перекодирования.

Opus из WebM и AAC перекладываются в MP4 (ISO BMFF, `.m4a`, `-movflags +faststart`) —
меняется только контейнер. Opus в MP4 играют Safari, AVPlayer, Chrome, Firefox и движок
приложений; `faststart` ставит оглавление в начало, и плеер начинает играть, не дочитав файл.
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import threading
import time
import urllib.parse
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yt_dlp
from yt_dlp.utils import YoutubeDLError
from yt_dlp.version import __version__ as YT_DLP_VERSION

from . import progress, tools
from .config import Settings

# 774 — Opus 256 кбит/с, только с YouTube Music Premium; 251 — Opus ~130–160 кбит/с;
# 140 — AAC 128 кбит/с на случай, когда Opus не отдают.
FORMAT = "774/251/bestaudio[acodec=opus]/140/bestaudio"

# Подстрока сообщения yt-dlp → код ошибки для отчётов и админки. Порядок важен.
_ERROR_PATTERNS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("bot_check", ("not a bot", "unusual traffic")),
    ("age_restricted", ("confirm your age", "age-restricted", "inappropriate for some users")),
    ("geo_blocked", ("available in your country", "blocked it in your country")),
    ("rate_limited", ("http error 429", "too many requests", "rate-limit")),
    ("http_403", ("http error 403", "403: forbidden")),
    ("po_token", ("po token", "po_token", "potoken")),
    ("drm", ("drm protected",)),
    ("format_unavailable", ("requested format is not available", "no video formats")),
    (
        "unavailable",
        (
            "video unavailable",
            "private video",
            "has been removed",
            "is not available",
            "account associated with this video has been terminated",
        ),
    ),
    ("network", ("timed out", "connection reset", "temporary failure", "name resolution")),
)


# Сбои, после которых тот же ролик стоит попробовать другим клиентом плеера.
RETRY_WITH_FALLBACK = frozenset({"http_403", "format_unavailable", "po_token"})


def classify_error(message: str) -> str:
    text = message.lower()
    for code, needles in _ERROR_PATTERNS:
        if any(n in text for n in needles):
            return code
    return "unknown"


class DownloadError(Exception):
    def __init__(self, code: str, message: str, warnings: list[str] | None = None) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.warnings = warnings or []

    def to_json(self) -> dict[str, Any]:
        return {"code": self.code, "message": self.message, "warnings": self.warnings[-5:]}


@dataclass
class DownloadResult:
    video_id: str
    path: Path
    container: str
    codec: str
    original_container: str
    format_id: str
    abr: float | None
    asr: int | None
    channels: int | None
    size_bytes: int
    original_size_bytes: int | None
    duration_s: float | None
    client: str | None
    po_token: bool
    title: str | None
    channel: str | None
    timings_ms: dict[str, int] = field(default_factory=dict)
    warnings: list[str] = field(default_factory=list)
    # Скачано со второй попытки другим клиентом плеера: {"clients": [...], "after": "http_403"}.
    fallback: dict[str, Any] | None = None

    @property
    def mime_type(self) -> str:
        return mime_type(self.container, self.codec)

    def to_json(self) -> dict[str, Any]:
        return {
            "videoId": self.video_id,
            "file": self.path.name,
            "container": self.container,
            "mimeType": self.mime_type,
            "codec": self.codec,
            "originalContainer": self.original_container,
            "itag": self.format_id,
            "abrKbps": round(self.abr, 1) if self.abr else None,
            "sampleRate": self.asr,
            "channels": self.channels,
            "sizeBytes": self.size_bytes,
            "originalSizeBytes": self.original_size_bytes,
            "durationMs": round(self.duration_s * 1000) if self.duration_s else None,
            "client": self.client,
            "poToken": self.po_token,
            "title": self.title,
            "channel": self.channel,
            "timingsMs": self.timings_ms,
            "fallback": self.fallback,
            "warnings": self.warnings[-10:],
        }


_AD_WAIT = re.compile(r"Sleeping ([\d.]+) seconds as required by the site")


class _Collector:
    """Логгер для yt-dlp: копит предупреждения, чтобы классифицировать сбои."""

    def __init__(self) -> None:
        self.warnings: list[str] = []
        self.errors: list[str] = []
        # Перед потоком YouTube может требовать «досмотреть» рекламу: yt-dlp ждёт.
        self.ad_wait_s = 0.0

    def debug(self, msg: str) -> None:
        if m := _AD_WAIT.search(msg):
            self.ad_wait_s += float(m.group(1))

    def info(self, msg: str) -> None:
        pass

    def warning(self, msg: str) -> None:
        self.warnings.append(msg)

    def error(self, msg: str) -> None:
        self.errors.append(msg)


def ffprobe_duration(path: Path) -> float | None:
    out = subprocess.run(
        [
            tools.exe("ffprobe"),
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    try:
        return float(out.stdout.strip())
    except ValueError:
        return None


# Кодек → (расширение, контейнер для сервера, формат ffmpeg, параметры формата).
# Muxer задаётся явно: по расширению .m4a ffmpeg выбрал бы ipod, а тот не знает Opus.
_TARGETS: dict[str, tuple[str, str, str, list[str]]] = {
    "opus": (".m4a", "mp4", "mp4", ["-movflags", "+faststart"]),
    "aac": (".m4a", "mp4", "mp4", ["-movflags", "+faststart"]),
    "mp3": (".mp3", "mp3", "mp3", []),
    "flac": (".flac", "flac", "flac", []),
    "vorbis": (".ogg", "ogg", "ogg", []),
}

# Кодек для параметра codecs в MIME: по нему браузер решает, сможет ли сыграть файл.
_MIME_CODECS = {"opus": "opus", "aac": "mp4a.40.2", "vorbis": "vorbis", "flac": "flac"}
# В MP4 кодек называется кодом своей записи (RFC 6381): Safari понимает «Opus», а на «opus» отвечает «не умею»
_MP4_CODECS = {"opus": "Opus", "flac": "fLaC"}


def mime_type(container: str, codec: str) -> str:
    """`audio/mp4; codecs="Opus"` — для Content-Type в хранилище и плеера."""
    base = {
        "mp4": "audio/mp4",
        "ogg": "audio/ogg",
        "webm": "audio/webm",
        "mp3": "audio/mpeg",
        "flac": "audio/flac",
    }.get(container, "application/octet-stream")
    codecs = (_MP4_CODECS.get(codec) if container == "mp4" else None) or _MIME_CODECS.get(codec)
    if codecs and container in ("mp4", "ogg", "webm"):
        return f'{base}; codecs="{codecs}"'
    return base


def source_url(video_id: str) -> str:
    """Ролик YouTube по id или ссылка, вставленная в «Спорных» (SoundCloud, Bandcamp…), как есть."""
    if video_id.startswith(("http://", "https://")):
        return video_id
    return f"https://www.youtube.com/watch?v={video_id}"


def safe_basename(video_id: str) -> str:
    """Имя файла: id видео или, для ссылки, её короткий хеш — в ссылке есть `/` и `?`."""
    if re.fullmatch(r"[A-Za-z0-9_-]{1,64}", video_id):
        return video_id
    return "url-" + hashlib.sha256(video_id.encode()).hexdigest()[:16]


def probe_codec(path: Path) -> str:
    out = subprocess.run(
        [
            tools.exe("ffprobe"),
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "default=nw=1:nk=1",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    return out.stdout.strip()


def remux(src: Path, codec: str) -> tuple[Path, str]:
    """Перекладывает аудио в контейнер для плеера без перекодирования: Opus и AAC — в MP4."""
    target = _TARGETS.get(codec)
    if target is None:
        raise DownloadError("unsupported_codec", f"кодек {codec or 'неизвестен'} не поддерживается")
    suffix, container, fmt, extra = target
    dst = src.with_suffix(suffix)
    if dst == src:
        dst = src.with_name(f"{src.stem}.remux{src.suffix}")
    cmd = [
        tools.exe("ffmpeg"),
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        str(src),
        "-map",
        "0:a:0",
        "-vn",
        "-c:a",
        "copy",
        "-map_metadata",
        "-1",
        *extra,
        "-f",
        fmt,
        str(dst),
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        raise DownloadError("remux_failed", proc.stderr.strip()[-500:] or "ffmpeg упал")
    return dst, container


class Downloader:
    """Одно скачивание за раз и пауза между ними: YouTube не любит частые запросы."""

    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self._lock = threading.Lock()
        self._last_finished = 0.0

    def _cookie_copy(self) -> Path | None:
        src = self.settings.cookies_file
        if src is None:
            return None
        if not src.is_file():
            raise DownloadError("cookies_missing", f"нет файла cookies: {src}")
        # yt-dlp дописывает cookies при выходе — работаем с копией, оригинал не трогаем.
        dst = self.settings.data_dir / "cookies.txt"
        dst.parent.mkdir(parents=True, exist_ok=True)
        if not dst.exists() or dst.stat().st_mtime < src.stat().st_mtime:
            shutil.copyfile(src, dst)
            dst.chmod(0o600)
        return dst

    def deno(self) -> str | None:
        """Путь к Deno для yt-dlp; None — пусть ищет в PATH сам."""
        deno = self.settings.deno_path or tools.exe("deno")
        return deno if os.sep in deno else None

    def fallback_clients(self) -> tuple[str, ...]:
        """Клиенты для второй попытки того же ролика после 403 и похожих сбоев."""
        s = self.settings
        if s.fallback_clients is not None:
            return s.fallback_clients
        if s.player_clients:
            return ("default",)
        return ("mweb",) if s.pot_url else ()

    def options(
        self, out_dir: Path, basename: str, clients: tuple[str, ...] | None = None
    ) -> dict[str, Any]:
        s = self.settings
        youtube_args: dict[str, list[str]] = {}
        extractor_args: dict[str, dict[str, list[str]]] = {"youtube": youtube_args}
        clients = clients if clients is not None else s.player_clients
        if clients:
            youtube_args["player_client"] = list(clients)
        if s.pot_url:
            extractor_args["youtubepot-bgutilhttp"] = {"base_url": [s.pot_url]}
        else:
            # Без адреса плагин всё равно стучится на 127.0.0.1:4416 — выключаем явно.
            youtube_args["fetch_pot"] = ["never"]
        opts: dict[str, Any] = {
            "format": FORMAT,
            "outtmpl": {"default": str(out_dir / f"{basename}.%(ext)s")},
            "noplaylist": True,
            "quiet": True,
            "no_warnings": False,
            "noprogress": True,
            "overwrites": True,
            "retries": 3,
            "fragment_retries": 3,
            "socket_timeout": 20,
            "cachedir": str(s.data_dir / "yt-dlp-cache"),
            "extractor_args": extractor_args,
            "js_runtimes": {"deno": {"path": deno} if (deno := self.deno()) else {}},
        }
        ffmpeg = tools.exe("ffmpeg")
        if os.sep in ffmpeg:
            # Свой ffmpeg рядом с загрузчиком: ffprobe yt-dlp найдёт по тому же имени в той же папке
            opts["ffmpeg_location"] = ffmpeg
        if s.proxy:
            opts["proxy"] = s.proxy
        if s.source_address:
            opts["source_address"] = s.source_address
        cookies = self._cookie_copy()
        if cookies:
            opts["cookiefile"] = str(cookies)
        return opts

    def download(self, video_id: str, out_dir: Path, basename: str) -> DownloadResult:
        with self._lock:
            wait = self._last_finished + self.settings.min_download_interval_s - time.monotonic()
            if wait > 0:
                time.sleep(wait)
            try:
                try:
                    return self._download(video_id, out_dir, basename)
                except DownloadError as e:
                    # Поток одного клиента плеера бывает закрыт (403), а у другого — открыт.
                    fallback = self.fallback_clients()
                    if e.code not in RETRY_WITH_FALLBACK or not fallback:
                        raise
                    result = self._download(video_id, out_dir, basename, fallback)
                    result.fallback = {
                        "clients": list(fallback),
                        "after": e.code,
                        "message": e.message[:300],
                        "warnings": e.warnings[-3:],
                    }
                    return result
            finally:
                self._last_finished = time.monotonic()

    def _download(
        self,
        video_id: str,
        out_dir: Path,
        basename: str,
        clients: tuple[str, ...] | None = None,
    ) -> DownloadResult:
        out_dir.mkdir(parents=True, exist_ok=True)
        collector = _Collector()
        marks: dict[str, float] = {}

        def hook(d: dict[str, Any]) -> None:
            if d.get("status") == "downloading":
                marks.setdefault("transfer", time.monotonic())
                total = d.get("total_bytes") or d.get("total_bytes_estimate")
                done = d.get("downloaded_bytes") or 0
                # Отсюда же приходит и остановка: исключение прерывает скачивание yt-dlp
                progress.report(progress.DOWNLOADING, done / total if total else 0.0)
            elif d.get("status") == "finished":
                marks["finished"] = time.monotonic()

        opts = self.options(out_dir, basename, clients)
        opts["logger"] = collector
        opts["progress_hooks"] = [hook]
        started = time.monotonic()
        try:
            with yt_dlp.YoutubeDL(opts) as ydl:
                info = ydl.extract_info(source_url(video_id), download=True)
        except YoutubeDLError as e:
            message = str(e).removeprefix("ERROR: ")
            text = " ".join([message, *collector.warnings])
            raise DownloadError(classify_error(text), message, collector.warnings) from e
        if not info:
            raise DownloadError("unknown", "yt-dlp ничего не вернул", collector.warnings)
        done = time.monotonic()

        requested = (info.get("requested_downloads") or [info])[0]
        original = Path(requested.get("filepath") or requested.get("_filename") or "")
        if not original.is_file():
            raise DownloadError("unknown", "yt-dlp не сохранил файл", collector.warnings)
        acodec = (requested.get("acodec") or "").split(".")[0]
        if acodec in ("", "none"):
            # Не YouTube: кодек в описании формата бывает пустым — спрашиваем у самого файла
            acodec = probe_codec(original)
        codec = "aac" if acodec == "mp4a" else acodec
        original_size = original.stat().st_size

        remux_started = time.monotonic()
        path, container = remux(original, codec)
        original.unlink(missing_ok=True)
        remux_ms = int((time.monotonic() - remux_started) * 1000)

        query = urllib.parse.parse_qs(urllib.parse.urlparse(requested.get("url") or "").query)
        transfer_started = marks.get("transfer", done)
        return DownloadResult(
            video_id=video_id,
            path=path,
            container=container,
            codec=codec,
            original_container=requested.get("ext") or original.suffix.lstrip("."),
            format_id=str(requested.get("format_id") or ""),
            abr=requested.get("abr"),
            asr=requested.get("asr"),
            channels=requested.get("audio_channels"),
            size_bytes=path.stat().st_size,
            original_size_bytes=original_size,
            duration_s=ffprobe_duration(path),
            client=(query.get("c") or [None])[0],
            po_token="pot" in query,
            title=info.get("title"),
            channel=info.get("channel") or info.get("uploader"),
            timings_ms={
                "extract": int((transfer_started - started - collector.ad_wait_s) * 1000),
                "adWait": int(collector.ad_wait_s * 1000),
                "transfer": int((marks.get("finished", done) - transfer_started) * 1000),
                "remux": remux_ms,
            },
            warnings=collector.warnings,
        )


def tool_versions(settings: Settings) -> dict[str, str | None]:
    def first_line(cmd: list[str]) -> str | None:
        try:
            out = subprocess.run(cmd, capture_output=True, text=True, timeout=10, check=False)
        except (OSError, subprocess.TimeoutExpired):
            return None
        lines = (out.stdout or out.stderr).strip().splitlines()
        return lines[0] if lines else None

    return {
        "ytDlp": YT_DLP_VERSION,
        "deno": first_line([settings.deno_path or tools.exe("deno"), "--version"]),
        "ffmpeg": first_line([tools.exe("ffmpeg"), "-version"]),
        "fpcalc": first_line([tools.exe("fpcalc"), "-version"]),
    }
