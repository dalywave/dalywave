"""Клип для ленты открытий: официальное видео трека на YouTube, 10–15 секунд вокруг припева,
480p без звука, H.264 в MP4 (`+faststart`) ~1–2 МБ.

Путь: поиск yt-dlp («артист название official video») → отбор официального клипа (`clip_pick`:
в приоритете канал автора; фанатское, текст, лайв и перезаливы отсеиваются; нет официального —
клипа нет) → место припева в клипе (у клипа бывает
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
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import httpx
import yt_dlp
from yt_dlp.utils import YoutubeDLError

from . import clip_sync, tools
from .clip_pick import (
    KEEP_CANDIDATES,
    ClipTrack,
    Pick,
    assess,
    better,
    choose,
    entry,
    unknown_channel,
)
from .download import (
    RETRY_WITH_FALLBACK,
    Downloader,
    DownloadError,
    _Collector,
    classify_error,
)
from .normalize import has_cyrillic

# Поток видео до нужной высоты (по умолчанию 480p): H.264, затем VP9. AV1 не берём — ffmpeg
# приложения его не декодирует. Сначала прямая ссылка (https), затем HLS; DASH по кусочкам ffmpeg
# не читает.
_CODECS = ("avc1", "vp09", "vp9")


def video_format(height: int = 480) -> str:
    return "/".join(
        [f"bv*[height<={height}][vcodec^={c}][protocol=https]" for c in _CODECS]
        + [f"bv*[height<={height}][vcodec^={c}][protocol^=m3u8]" for c in _CODECS]
    )


VIDEO_FORMAT = video_format(480)
# Звук ролика для сопоставления с треком: любой, лишь бы ffmpeg прочёл по ссылке
AUDIO_FORMAT = "ba[protocol=https]/ba"
# Клип целиком — не длиннее (полнометражные «фильмы» не режем целиком)
MAX_WHOLE_S = 600.0
SEARCH_LIMIT = 8
# Ролик оказался неподвижной картинкой — пробуем следующий годный, но не больше стольких
MAX_CUT_TRIES = 2
# Столько роликов без канала в выдаче спрашиваем отдельно (YouTube иногда отдаёт выдачу без каналов)
ENRICH_MAX = 4
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
    height: int = 480,
) -> None:
    """Точная нарезка и сжатие: без звука, до `height` (480p), не больше 30 кадров/с, H.264,
    faststart."""
    info = probe(src)
    filters = [f"scale=-2:'min({height},ih)'"]
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
    src: Path,
    dst: Path,
    offset_s: float,
    length_s: float,
    encoder: str = "libx264",
    height: int = 480,
) -> str:
    """Сжатие с обычными настройками; больше ~2 МБ на 12 секунд — ещё раз, заметнее. Возвращает,
    с какими настройками сжато: `crf27`, `800k`…"""
    encode(src, dst, offset_s, length_s, encoder, height=height)
    # Предел растёт с длиной и высотой кадра: клип целиком в 720p и не должен влезать в 2 МБ
    target = TARGET_BYTES * max(1.0, length_s / 12) * max(1.0, (height / 480) ** 2)
    smaller = dst.stat().st_size > target
    if smaller:
        encode(src, dst, offset_s, length_s, encoder, smaller=True, height=height)
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

    def fetch_range(
        self, video_id: str, start_s: float, end_s: float, out_dir: Path, height: int = 480
    ) -> Path:
        """Только кусок видео [start_s, end_s] без звука; при 403 — ещё раз другим клиентом."""
        try:
            return self._fetch_range(video_id, start_s, end_s, out_dir, None, height)
        except DownloadError as e:
            fallback = self.downloader.fallback_clients()
            if e.code not in RETRY_WITH_FALLBACK or not fallback:
                raise
            return self._fetch_range(video_id, start_s, end_s, out_dir, fallback, height)

    def video_audio(self, video_id: str) -> Any:
        """Звук ролика (моно, 8 кГц) — для сопоставления клипа с треком. Поток YouTube отвечает 403,
        пока ссылка не ожила (`available_at`) или если клиент не тот: ждём и пробуем другим клиентом,
        как при скачивании видео."""
        try:
            return self._video_audio(video_id, None)
        except (DownloadError, RuntimeError) as e:
            fallback = self.downloader.fallback_clients()
            code = e.code if isinstance(e, DownloadError) else classify_error(str(e))
            if code not in RETRY_WITH_FALLBACK or not fallback:
                raise
            return self._video_audio(video_id, fallback)

    def _video_audio(self, video_id: str, clients: tuple[str, ...] | None) -> Any:
        """Звук ролика качает yt-dlp (кусками, как аудио трека: поток звука YouTube не отдаёт ffmpeg
        целиком одним запросом — 403), затем ffmpeg сводит его к моно 8 кГц."""
        self.work_root.mkdir(parents=True, exist_ok=True)
        work = Path(tempfile.mkdtemp(prefix="sync-", dir=self.work_root))
        try:
            opts = self.downloader.options(work, "audio", clients)
            collector = _Collector()
            opts.update({"format": AUDIO_FORMAT, "logger": collector, "http_chunk_size": 1 << 20})
            with self.downloader.paced():
                try:
                    with yt_dlp.YoutubeDL(opts) as ydl:
                        ydl.extract_info(
                            f"https://www.youtube.com/watch?v={video_id}", download=True
                        )
                except YoutubeDLError as e:
                    message = str(e).removeprefix("ERROR: ")
                    text = " ".join([message, *collector.warnings])
                    raise DownloadError(classify_error(text), message, collector.warnings) from e
            files = [f for f in work.iterdir() if f.is_file() and f.stat().st_size > 0]
            if not files:
                raise DownloadError("format_unavailable", "yt-dlp не скачал звук ролика")
            return clip_sync.decode_audio(str(files[0]))
        finally:
            shutil.rmtree(work, ignore_errors=True)

    def _fetch_range(
        self,
        video_id: str,
        start_s: float,
        end_s: float,
        out_dir: Path,
        clients: tuple[str, ...] | None,
        height: int = 480,
    ) -> Path:
        """Ссылку на поток даёт yt-dlp, кусок качает ffmpeg сам: так его ошибка (stderr) видна
        в задаче, а 403 потока узнаётся и пробуется другим клиентом."""
        opts = self.downloader.options(out_dir, "raw", clients)
        collector = _Collector()
        opts.update({"format": video_format(height), "logger": collector, "skip_download": True})
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

    def video_info(self, video_id: str) -> Pick:
        """Сведения об одном ролике (название, канал, длина) — для закреплённого администратором
        и для перепроверки уже нарезанного клипа."""
        opts = self.downloader.options(self.work_root, "search")
        collector = _Collector()
        opts.update({"logger": collector, "skip_download": True})
        try:
            with self.downloader.paced(), yt_dlp.YoutubeDL(opts) as ydl:
                info = ydl.extract_info(
                    f"https://www.youtube.com/watch?v={video_id}", download=False, process=False
                )
        except YoutubeDLError as e:
            message = str(e).removeprefix("ERROR: ")
            text = " ".join([message, *collector.warnings])
            raise DownloadError(classify_error(text), message, collector.warnings) from e
        pick = entry({**(info or {}), "id": video_id, "ie_key": None})
        assert pick is not None
        return pick

    def candidates(
        self, track: ClipTrack, exclude: set[str], details: dict[str, Any], *, thorough: bool
    ) -> tuple[Pick | None, list[Pick]]:
        """Поиск по обоим запросам; без `thorough` второй — только если клипа с канала автора
        не нашлось. Кандидаты из обоих запросов — вместе, без повторов."""
        items: list[dict[str, Any]] = []
        best: Pick | None = None
        picks: list[Pick] = []
        for query in search_queries(track.title, track.artists):
            found = self.search(query)
            items.extend(found)
            best, picks = choose(items, track, exclude)
            details.setdefault("searches", []).append({"query": query, "found": len(found)})
            # Выдача бывает без каналов (YouTube отдаёт другую разметку): канал узнаём у ролика
            if (best is None or best.source != "artist") and self._fill_channels(
                items, picks, details
            ):
                best, picks = choose(items, track, exclude)
            if best is not None and best.source == "artist" and not thorough:
                break
        return best, picks

    def _fill_channels(
        self, items: list[dict[str, Any]], picks: list[Pick], details: dict[str, Any]
    ) -> bool:
        """Сведения о роликах без канала, которые иначе годились бы (не больше `ENRICH_MAX`)."""
        wanted = [p.video_id for p in picks if unknown_channel(p)][:ENRICH_MAX]
        filled = False
        for video_id in wanted:
            try:
                info = self.video_info(video_id)
            except DownloadError:
                continue
            for item in items:
                if item.get("id") == video_id:
                    item.update(
                        channel=info.channel,
                        view_count=info.views or item.get("view_count"),
                        channel_is_verified=info.verified,
                        duration=item.get("duration") or info.duration_s,
                    )
            filled = filled or bool(info.channel)
        if wanted:
            details.setdefault("filledChannels", []).extend(wanted)
        return filled

    def make(
        self,
        *,
        title: str,
        artists: list[str],
        duration_ms: int,
        hook_start_ms: int,
        length_ms: int,
        exclude_video_ids: list[str] | None = None,
        label: str | None = None,
        mode: str = "auto",
        video_id: str | None = None,
        current_video_id: str | None = None,
        upload: Any = None,
        keep: Path | None = None,
        height: int = 480,
        whole: bool = False,
        sync_audio_url: str | None = None,
    ) -> dict[str, Any]:
        """Клип целиком. Возвращает `ClipResult` сервера: status ready / kept / none / error.

        `mode`: `auto` — найти лучший официальный клип и нарезать; `pinned` — нарезать ролик
        `video_id`, выбранный администратором (без отбора); `search` — только кандидаты, без нарезки;
        `review` — перепроверить нарезанный `current_video_id` новым отбором: тот же лучший — `kept`,
        нашёлся с канала выше рангом — нарезать его, текущий не годится и замены нет — `none`.

        `upload` — `UploadSpec` (PUT по подписанной ссылке); `keep` — куда скопировать готовый MP4
        (живая проверка без сервера). `height` — высота кадра; `whole` — клип целиком, а не кусок;
        `sync_audio_url` — звук трека: клип целиком сопоставляется с ним (не сошлось — кусок)."""
        timings: dict[str, int] = {}
        details: dict[str, Any] = {"timingsMs": timings, "mode": mode, "height": height}
        if whole:
            details["whole"] = True
        support = self.support()
        details["encoder"] = support.encoder
        if support.encoder is None and mode != "search":
            # Не тратим запросы к YouTube: сделать клип всё равно нечем
            return {
                "status": "error",
                "error": f"{support.code}: {support.problem}"[:300],
                "details": details,
            }
        started = time.monotonic()
        track = ClipTrack(title, artists, duration_ms / 1000, label)
        exclude = set(exclude_video_ids or [])
        picks: list[Pick] = []
        unsure = False
        try:
            if mode == "pinned" and video_id:
                chosen = [assess(self.video_info(video_id), track)]
            else:
                best, picks = self.candidates(
                    track, exclude, details, thorough=mode in ("search", "review")
                )
                chosen = [p for p in picks if p.ok][:MAX_CUT_TRIES]
                if mode == "review" and current_video_id:
                    current = next((p for p in picks if p.video_id == current_video_id), None)
                    if current is None:
                        try:
                            current = assess(self.video_info(current_video_id), track)
                        except DownloadError as e:
                            # Ролик удалён или закрыт — перевыбираем как без него
                            details["current"] = {"videoId": current_video_id, "error": e.code}
                    if current is not None:
                        details["current"] = current.to_json()
                        unsure = unknown_channel(current)
                    if best is not None and not better(best, current):
                        assert current is not None
                        timings["search"] = _ms(started)
                        return {
                            "status": "kept",
                            **_about(current),
                            "candidates": _top(picks),
                            "details": details,
                        }
        except DownloadError as e:
            return {**self._error(e, details), "candidates": _top(picks)}
        timings["search"] = _ms(started)
        if mode != "pinned" and not chosen and (unsure or any(unknown_channel(p) for p in picks)):
            # Решить «клипа нет» по выдаче без каналов нельзя: так убрался бы и хороший клип.
            # Сбой — задание повторится позже
            return {
                "status": "error",
                "error": "search_degraded: в выдаче YouTube нет каналов, официальность не проверить",
                "candidates": _top(picks),
                "details": details,
            }
        if mode == "search" or not chosen:
            return {"status": "none", "candidates": _top(picks), "details": details}

        result: dict[str, Any] = {"status": "none"}
        for pick in chosen:
            result = self._cut(
                pick,
                track,
                duration_ms,
                hook_start_ms,
                length_ms,
                details,
                upload,
                keep,
                pinned=mode == "pinned",
                height=height,
                whole=whole,
                sync_audio_url=sync_audio_url,
            )
            if result["status"] != "none":
                break
        timings["total"] = _ms(started)
        return {**result, "candidates": _top(picks), "details": details}

    def _cut(
        self,
        pick: Pick,
        track: ClipTrack,
        duration_ms: int,
        hook_start_ms: int,
        length_ms: int,
        details: dict[str, Any],
        upload: Any,
        keep: Path | None,
        *,
        pinned: bool,
        height: int = 480,
        whole: bool = False,
        sync_audio_url: str | None = None,
    ) -> dict[str, Any]:
        """Скачать кусок ролика вокруг припева (или ролик целиком), сжать и положить. Неподвижная
        картинка — `none` (кроме выбранного администратором: ему виднее). Целиком со звуком трека:
        сначала сопоставление, не сошлось — кусок с припевом, как обычно."""
        timings: dict[str, int] = details["timingsMs"]
        video_ms = round(pick.duration_s * 1000)
        sync: clip_sync.Alignment | None = None
        if whole and pick.duration_s > MAX_WHOLE_S:
            details["wholeSkipped"] = "too_long"
            whole = False
        if whole and sync_audio_url:
            t = time.monotonic()
            try:
                track_audio = clip_sync.decode_audio(sync_audio_url)
                diag: dict[str, Any] = {}
                details["syncDiag"] = diag
                sync = clip_sync.align(track_audio, self.video_audio(pick.video_id), diag)
            except (DownloadError, RuntimeError, OSError, subprocess.SubprocessError) as e:
                details["syncError"] = str(e)[:200]
            timings["sync"] = _ms(t)
            details["sync"] = sync.to_json() if sync else None
            if sync is None:
                # Не сопоставлен — кусок с припевом по кругу
                whole = False
        if whole:
            start_ms, length_ms = 0, video_ms
        else:
            start_ms = clip_start_ms(hook_start_ms, duration_ms, video_ms, length_ms)
        details["chosen"] = pick.to_json()
        self.work_root.mkdir(parents=True, exist_ok=True)
        work = Path(tempfile.mkdtemp(prefix="clip-", dir=self.work_root))
        encoder = self.support().encoder or "libx264"
        try:
            t = time.monotonic()
            section_start = max(0.0, start_ms / 1000 - MARGIN_S)
            section_end = (start_ms + length_ms) / 1000 + MARGIN_S
            try:
                raw = self.fetch_range(pick.video_id, section_start, section_end, work, height)
            except DownloadError as e:
                return self._error(e, details, pick.video_id)
            timings["download"] = _ms(t)
            # Копированием поток режется по ключевому кадру до начала: лишнее — в начале файла
            got = probe(raw).get("duration") or 0.0
            offset = max(0.0, got - (section_end - start_ms / 1000))
            out = work / "clip.mp4"
            t = time.monotonic()
            try:
                quality = encode_small(raw, out, offset, length_ms / 1000, encoder, height)
            except DownloadError as e:
                return self._error(e, details, pick.video_id)
            timings["encode"] = _ms(t)
            info = probe(out)
            size = out.stat().st_size
            details["quality"] = quality
            if not pinned and (
                is_still(size, length_ms) or is_frozen(raw, offset, length_ms / 1000)
            ):
                # Неподвижная картинка под звук (обложка на канале артиста) — не клип
                details.setdefault("still", []).append(
                    {"videoId": pick.video_id, "sizeBytes": size}
                )
                return {"status": "none"}
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
                        "videoId": pick.video_id,
                        "error": f"upload_failed: {uploaded.get('error') or uploaded.get('status')}",
                    }
            return {
                "status": "ready",
                **_about(pick),
                "videoDurationMs": video_ms,
                "clipStartMs": start_ms,
                "durationMs": round((info.get("duration") or length_ms / 1000) * 1000),
                "sizeBytes": size,
                "width": info.get("width"),
                "height": info.get("height"),
                "whole": whole,
                "sync": sync.to_json() if sync and whole else None,
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


def _about(pick: Pick) -> dict[str, Any]:
    """Что за ролик выбран: сервер показывает это администратору в «Клип»."""
    return {
        "videoId": pick.video_id,
        "videoTitle": pick.title,
        "channel": pick.channel,
        "source": pick.source,
        "score": round(pick.score, 3),
    }


def _top(picks: list[Pick]) -> list[dict[str, Any]]:
    return [p.to_json() for p in picks[:KEEP_CANDIDATES]]
