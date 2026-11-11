"""Полный путь трека: Deezer → поиск → скачивание → проверка отпечатком → громкость
и огибающая → загрузка."""

from __future__ import annotations

import hashlib
import shutil
import tempfile
import time
from collections.abc import Iterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import httpx

from . import __version__
from .analyze import Analysis, AnalyzeError, analyze
from .config import Settings
from .deezer import DeezerClient, DeezerError, DeezerNotFoundError, DeezerTrack
from .download import YT_DLP_VERSION, Downloader, DownloadError, DownloadResult, safe_basename
from .match import MIN_TRY_SCORE, Candidate, Matcher, MatchResult, VideosMode
from .verify import DEFAULT_THRESHOLD, Fingerprint, VerifyError, VerifyResult, fingerprint, verify

# Эти сбои не про конкретное видео, а про отношение YouTube к нам: следующий кандидат
# упадёт так же, а лишние попытки только ухудшат дело.
STOP_NOW = frozenset({"bot_check", "rate_limited", "cookies_missing"})
# Бывает и у отдельного ролика, поэтому останавливаемся только на втором таком сбое.
STOP_ON_REPEAT = frozenset({"http_403", "po_token", "network"})
SYSTEMIC_ERRORS = STOP_NOW | STOP_ON_REPEAT
# Без превью проверить нечем: берём лучшего кандидата, только если он очень хорош.
UNVERIFIED_ACCEPT_SCORE = 0.9
# Сколько видео пробовать, когда аудиоверсии не прошли проверку.
VIDEO_ATTEMPTS = 2
# Папки заданий без загрузки во внешнее хранилище живут сутки.
JOB_DIR_TTL_S = 24 * 3600


@dataclass
class UploadTarget:
    url: str
    method: str = "PUT"
    headers: dict[str, str] = field(default_factory=dict)


@dataclass
class Attempt:
    candidate: Candidate
    download: DownloadResult | None = None
    verify: VerifyResult | None = None
    error: DownloadError | None = None

    def to_json(self) -> dict[str, Any]:
        return {
            "videoId": self.candidate.video_id,
            "score": round(self.candidate.score, 4),
            "source": self.candidate.source,
            "download": self.download.to_json() if self.download else None,
            "verify": self.verify.to_json() if self.verify else None,
            "error": self.error.to_json() if self.error else None,
        }


@dataclass
class AcquireResult:
    status: str
    track_id: int
    track: DeezerTrack | None = None
    match: MatchResult | None = None
    attempts: list[Attempt] = field(default_factory=list)
    chosen: Attempt | None = None
    file_sha256: str | None = None
    # Громкость и огибающая выбранного файла; не вышло — файл всё равно годится
    analysis: Analysis | None = None
    analysis_error: str | None = None
    upload: dict[str, Any] | None = None
    error: dict[str, Any] | None = None
    timings_ms: dict[str, int] = field(default_factory=dict)
    setup: dict[str, Any] = field(default_factory=dict)

    @property
    def path(self) -> Path | None:
        return self.chosen.download.path if self.chosen and self.chosen.download else None

    def to_json(self, max_candidates: int = 10) -> dict[str, Any]:
        chosen = None
        if self.chosen and self.chosen.download:
            chosen = {
                "candidate": self.chosen.candidate.to_json(),
                "download": self.chosen.download.to_json(),
                "verify": self.chosen.verify.to_json() if self.chosen.verify else None,
            }
        file = None
        if self.chosen and self.chosen.download:
            d = self.chosen.download
            file = {
                "path": str(d.path),
                "sha256": self.file_sha256,
                "sizeBytes": d.size_bytes,
                "container": d.container,
                "mimeType": d.mime_type,
                "codec": d.codec,
                "durationMs": round(d.duration_s * 1000) if d.duration_s else None,
                "bitrateKbps": round(d.abr) if d.abr else None,
                "sampleRate": d.asr,
                "channels": d.channels,
                **(
                    self.analysis.to_json()
                    if self.analysis
                    else {"loudnessLufs": None, "truePeakDb": None, "waveform": None}
                ),
                "analysisError": self.analysis_error,
            }
        candidates = self.match.candidates[:max_candidates] if self.match else []
        return {
            "status": self.status,
            "deezerTrackId": self.track_id,
            "track": self.track.to_json() if self.track else None,
            "chosen": chosen,
            "file": file,
            "upload": self.upload,
            "attempts": [a.to_json() for a in self.attempts],
            "candidates": [c.to_json() for c in candidates],
            "steps": self.match.steps if self.match else [],
            "error": self.error,
            "timingsMs": self.timings_ms,
            "setup": self.setup,
            "versions": {"fetcher": __version__, "ytDlp": YT_DLP_VERSION},
        }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


class _Timer:
    def __init__(self) -> None:
        self.ms: dict[str, int] = {}

    def add(self, name: str, started: float) -> None:
        self.ms[name] = self.ms.get(name, 0) + int((time.monotonic() - started) * 1000)


class Fetcher:
    """Клиенты Deezer, YouTube Music и yt-dlp на весь процесс."""

    def __init__(
        self,
        settings: Settings,
        deezer: DeezerClient | None = None,
        matcher: Matcher | None = None,
        downloader: Downloader | None = None,
    ) -> None:
        self.settings = settings
        self.deezer = deezer or DeezerClient()
        self.matcher = matcher or Matcher(self.deezer)
        self.downloader = downloader or Downloader(settings)

    def match(self, track_id: int, videos: VideosMode = "auto") -> MatchResult:
        return self.matcher.match(self.deezer.track(track_id), videos=videos)

    def acquire(
        self,
        track_id: int,
        out_dir: Path | None = None,
        *,
        max_attempts: int = 3,
        upload: UploadTarget | None = None,
        prefer_video_ids: list[str] | None = None,
        exclude_video_ids: list[str] | None = None,
        direct_url: str | None = None,
        keep_rejected: bool = False,
        threshold: float = DEFAULT_THRESHOLD,
    ) -> AcquireResult:
        """`direct_url` — ссылка из «Спорных» (SoundCloud, Bandcamp, любой сайт yt-dlp): качается
        она как выбор человека, а поиск даёт кандидатов только для отчёта."""
        started = time.monotonic()
        timer = _Timer()
        result = AcquireResult(status="error", track_id=track_id, setup=self.settings.describe())
        own_dir = out_dir is None
        work = out_dir or Path(tempfile.mkdtemp(prefix=f"{track_id}-", dir=self.jobs_dir()))
        work.mkdir(parents=True, exist_ok=True)
        try:
            self._acquire(
                result,
                work,
                timer,
                max_attempts=max_attempts,
                prefer=([direct_url] if direct_url else []) + (prefer_video_ids or []),
                exclude=set(exclude_video_ids or []),
                keep_rejected=keep_rejected,
                threshold=threshold,
            )
            if upload and result.path:
                t = time.monotonic()
                result.upload = self._upload(result, upload)
                timer.add("upload", t)
        finally:
            timer.ms["total"] = int((time.monotonic() - started) * 1000)
            result.timings_ms = timer.ms
            # Своя временная папка нужна, только пока файл никуда не отдан.
            uploaded = bool(upload and result.upload and result.upload.get("ok"))
            if own_dir and (uploaded or result.path is None):
                shutil.rmtree(work, ignore_errors=True)
        return result

    def jobs_dir(self) -> Path:
        path = self.settings.data_dir / "jobs"
        path.mkdir(parents=True, exist_ok=True)
        cutoff = time.time() - JOB_DIR_TTL_S
        for old in path.iterdir():
            if old.is_dir() and old.stat().st_mtime < cutoff:
                shutil.rmtree(old, ignore_errors=True)
        return path

    def _acquire(
        self,
        result: AcquireResult,
        work: Path,
        timer: _Timer,
        *,
        max_attempts: int,
        prefer: list[str],
        exclude: set[str],
        keep_rejected: bool,
        threshold: float,
    ) -> None:
        t = time.monotonic()
        try:
            track = self.deezer.track(result.track_id)
        except DeezerNotFoundError:
            result.error = {"code": "deezer_not_found", "message": "трека нет в Deezer"}
            return
        except (DeezerError, httpx.HTTPError) as e:
            result.error = {"code": "deezer_error", "message": str(e)}
            return
        timer.add("deezer", t)
        result.track = track

        t = time.monotonic()
        match = self.matcher.match(track)
        timer.add("match", t)
        result.match = match
        searches = [st for st in match.steps if st["name"] in ("album", "isrc", "song", "video")]
        if not match.candidates and searches and all("error" in st for st in searches):
            # YouTube Music не ответил ни на один поиск — это сбой, а не «записи нет».
            result.error = {"code": "search_failed", "message": searches[-1]["error"]}
            return

        # Превью качаем сразу: подписанная ссылка из свежего ответа Deezer живёт ~15 минут.
        t = time.monotonic()
        preview_fp, preview_error = self._preview(track, work)
        timer.add("preview", t)

        tried: set[str] = set(exclude)
        queue = self._queue(match, prefer, tried)
        stop = self._try(result, queue[:max_attempts], work, timer, preview_fp, threshold, tried)
        if not stop and not result.chosen and preview_fp is not None:
            # Аудиоверсии не подошли или их нет — пробуем видео, если их ещё не искали.
            t = time.monotonic()
            new = {c.video_id for c in self.matcher.more_videos(match)}
            timer.add("match", t)
            queue = [c for c in self._queue(match, [], tried) if c.video_id in new]
            stop = self._try(
                result, queue[:VIDEO_ATTEMPTS], work, timer, preview_fp, threshold, tried
            )
        if stop:
            return
        if not keep_rejected and preview_fp is not None:
            for attempt in result.attempts:
                if attempt is not result.chosen and attempt.download:
                    attempt.download.path.unlink(missing_ok=True)

        self._settle(result, preview_fp is not None, preview_error)
        if result.chosen and result.chosen.download:
            path = result.chosen.download.path
            t = time.monotonic()
            try:
                result.analysis = analyze(path)
            except AnalyzeError as e:
                result.analysis_error = str(e)
            timer.add("analyze", t)
            result.file_sha256 = sha256_file(path)

    @staticmethod
    def _settle(result: AcquireResult, has_preview: bool, preview_error: str | None) -> None:
        """Итоговый статус по сделанным попыткам."""
        downloaded = [a for a in result.attempts if a.download]
        if not has_preview:
            result.error = {"code": "no_preview", "message": preview_error or "нет превью"}
            # Проверить нечем: берём скачанное, если оценка очень высокая или видео
            # выбрано вручную.
            if not result.chosen and downloaded:
                first = downloaded[0]
                if (
                    first.candidate.source == "manual"
                    or first.candidate.score >= UNVERIFIED_ACCEPT_SCORE
                ):
                    result.chosen = first
        if result.chosen:
            verify = result.chosen.verify
            result.status = "verified" if verify is not None and verify.verified else "unverified"
        elif downloaded:
            result.status = "disputed"
        elif result.attempts:
            # Ни один кандидат не скачался — это сбой загрузки, а не «нет записи».
            result.status = "error"
            last = result.attempts[-1].error
            if last:
                result.error = {"code": last.code, "message": last.message}
        else:
            result.status = "not_found"

    def _try(
        self,
        result: AcquireResult,
        queue: list[Candidate],
        work: Path,
        timer: _Timer,
        preview_fp: Fingerprint | None,
        threshold: float,
        tried: set[str],
    ) -> bool:
        """Качает и проверяет кандидатов по очереди. True — дальше пробовать бессмысленно."""
        for cand in queue:
            tried.add(cand.video_id)
            attempt = self._attempt(cand, work, timer, preview_fp, threshold)
            result.attempts.append(attempt)
            code = attempt.error.code if attempt.error else None
            repeated = sum(1 for a in result.attempts if a.error and a.error.code in STOP_ON_REPEAT)
            if code in STOP_NOW or (code in STOP_ON_REPEAT and repeated >= 2):
                result.status = "error"
                result.error = {"code": code, "message": attempt.error.message}
                return True
            # Видео, выбранное вручную в «Спорных», берём даже без совпадения отпечатка.
            manual = cand.source == "manual" and attempt.download is not None
            if manual or (attempt.verify and attempt.verify.verified):
                result.chosen = attempt
                return False
            if preview_fp is None and attempt.download:
                # Проверить нечем — следующие кандидаты ничего не добавят.
                return False
        return False

    def _queue(self, match: MatchResult, prefer: list[str], tried: set[str]) -> list[Candidate]:
        by_id = {c.video_id: c for c in match.candidates}
        queue: list[Candidate] = []
        for video_id in prefer:
            cand = by_id.get(video_id) or Candidate(
                video_id=video_id,
                title="",
                artists=[],
                album=None,
                duration_s=None,
                video_type=None,
                source="manual",
                rank=0,
            )
            if video_id not in tried:
                queue.append(cand)
        queue += [c for c in match.viable(MIN_TRY_SCORE) if c.video_id not in tried]
        return list({c.video_id: c for c in queue}.values())

    def _preview(self, track: DeezerTrack, work: Path) -> tuple[Fingerprint | None, str | None]:
        if not track.preview_url:
            return None, "у трека нет превью в Deezer"
        path = work / f"{track.id}.preview.mp3"
        try:
            self.deezer.download_preview(track, path)
            return fingerprint(path), None
        except (DeezerError, httpx.HTTPError, VerifyError) as e:
            return None, f"превью: {e}"

    def _attempt(
        self,
        cand: Candidate,
        work: Path,
        timer: _Timer,
        preview_fp: Fingerprint | None,
        threshold: float,
    ) -> Attempt:
        attempt = Attempt(candidate=cand)
        t = time.monotonic()
        try:
            attempt.download = self.downloader.download(
                cand.video_id, work, safe_basename(cand.video_id)
            )
        except DownloadError as e:
            attempt.error = e
            return attempt
        finally:
            timer.add("download", t)
        if preview_fp is not None:
            t = time.monotonic()
            attempt.verify = verify(preview_fp, attempt.download.path, threshold)
            timer.add("verify", t)
        return attempt

    def _upload(self, result: AcquireResult, target: UploadTarget) -> dict[str, Any]:
        chosen = result.chosen
        if chosen is None or chosen.download is None:
            return {"ok": False, "status": None, "error": "нечего загружать"}
        path = chosen.download.path
        headers = dict(target.headers)
        headers.setdefault("Content-Type", chosen.download.mime_type)

        def chunks() -> Iterator[bytes]:
            with path.open("rb") as f:
                yield from iter(lambda: f.read(1 << 20), b"")

        try:
            # Content-Length обязателен: S3 и R2 не принимают PUT частями без него.
            response = httpx.request(
                target.method,
                target.url,
                content=chunks(),
                headers={**headers, "Content-Length": str(path.stat().st_size)},
                timeout=httpx.Timeout(30.0, write=300.0),
            )
        except httpx.HTTPError as e:
            return {"ok": False, "status": None, "error": str(e)}
        return {
            "ok": response.is_success,
            "status": response.status_code,
            "etag": response.headers.get("ETag"),
            "error": None if response.is_success else response.text[:300],
        }
