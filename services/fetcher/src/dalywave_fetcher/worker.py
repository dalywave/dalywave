"""«Домашний загрузчик»: тот же пакет на домашнем Mac забирает задания с сервера,
скачивает со своего адреса, кладёт файл по подписанной ссылке в R2 и отчитывается.

Протокол (все запросы с `Authorization: Bearer <токен загрузчика dwk_…>`):

    POST {server}/api/fetcher/jobs/claim
        {"workerId": "...", "version": "0.2.0", "capabilities": {...}}
        ← 200 {"job": Job} (форма — `models.Job`); 204 — заданий нет: сервер уже подождал
          до ~25 секунд, `Retry-After` — через сколько секунд спросить снова
    POST {server}/api/fetcher/jobs/{id}/lease
        ← 200 {"leaseExpiresAt": ...} — аренда продлена; 409 — задание уже не наше
    POST {server}/api/fetcher/jobs/{id}/result
        {"workerId": "...", "result": <ответ POST /v1/acquire>}
        ← 204; 409 — задание уже не наше (аренда кончилась, его отменили)
    POST {server}/api/fetcher/clips/claim
        {"workerId": "..."}
        ← 200 {"job": ClipJob} (`models.ClipJob`); 204 — клипов нет (сразу, без ожидания);
          404 — сервер клипов не знает: не спрашиваем час
    POST {server}/api/fetcher/clips/{id}/result
        {"workerId": "...", "result": <ClipResult, см. `clip.ClipMaker.make`>}

Клипы ленты открытий — работа второго плана: о них спрашиваем, только когда сервер ответил,
что скачивать нечего (с `clip` в `capabilities.kinds` он отвечает 204 сразу, если клипы ждут).

Задание выдаётся в аренду (`leaseExpiresAt`): пока идёт скачивание, загрузчик продлевает её
раз в минуту. Не отчитался к сроку — сервер возвращает задание в очередь.

Цикл умеет паузу и остановку (их просит приложение, см. `sidecar.py`): прерванное задание
сразу возвращается серверу — `POST …/jobs/{id}/release`, а пока у сервера такого адреса нет,
отчётом о сбое исполнителя (`network`), который сервер откладывает, не тратя попытку.
"""

from __future__ import annotations

import random
import socket
import sys
import threading
import time
from collections.abc import Callable
from typing import Any, Self

import httpx

from . import __version__, progress
from .acquire import SYSTEMIC_ERRORS, Fetcher
from .clip import ClipMaker, error_code
from .download import YT_DLP_VERSION, Downloader
from .models import ClipJob, Job

# YouTube заподозрил бота — долгая пауза лучше, чем бан адреса.
SYSTEMIC_PAUSE_S = 15 * 60
# Как часто продлевать аренду, пока идёт скачивание.
LEASE_RENEW_S = 60.0
# Код сбоя исполнителя, с которым сервер откладывает задачу, не тратя попытку.
GIVE_BACK_CODE = "network"
# Сервер не знает клипов (старая версия) — не спрашиваем столько.
CLIPS_OFF_S = 3600.0

# Коды выхода `run`.
EXIT_OK = 0
EXIT_TOKEN_REJECTED = 2


def _log(msg: str) -> None:
    print(time.strftime("%H:%M:%S"), msg, file=sys.stderr, flush=True)


def _retry_after(response: httpx.Response) -> float | None:
    """`Retry-After` в секундах: так сервер говорит, когда спросить снова."""
    value = response.headers.get("Retry-After", "").strip()
    try:
        return max(0.0, float(value)) if value else None
    except ValueError:
        return None


class Control:
    """Пауза и остановка цикла из другого потока; ожидания цикла прерываются ими сразу.

    `sleep` — подмена ожидания для тестов и CLI: тогда ожидания не прерываются.
    """

    def __init__(self, sleep: Callable[[float], None] | None = None) -> None:
        self._sleep = sleep
        self._cond = threading.Condition()
        self.paused = False
        self.stopped = False

    def wait(self, seconds: float) -> None:
        if self._sleep is not None:
            self._sleep(seconds)
            return
        with self._cond:
            self._cond.wait(timeout=max(0.0, seconds))

    def wait_while_paused(self) -> None:
        with self._cond:
            while self.paused and not self.stopped:
                self._cond.wait()

    def set_paused(self, paused: bool) -> None:
        with self._cond:
            self.paused = paused
            self._cond.notify_all()

    def stop(self) -> None:
        with self._cond:
            self.stopped = True
            self._cond.notify_all()

    def wake(self) -> None:
        with self._cond:
            self._cond.notify_all()


class WorkerEvents:
    """Что цикл сообщает наружу. По умолчанию — ничего: CLI пишет только в лог."""

    def idle(self) -> None:
        """Сервер ответил, заданий нет."""

    def waiting(self) -> None:
        """Запрос задания отправлен, сервер держит его у себя (long-poll): мы на связи."""

    def offline(self, message: str, retry_in_s: float) -> None:
        """Сервер недоступен или ответил ошибкой; следующая попытка — через `retry_in_s`."""

    def rejected(self) -> None:
        """Сервер не принял токен."""

    def paused(self) -> None:
        """Цикл стоит на паузе."""

    def cooling(self, code: str, message: str, until: float) -> None:
        """YouTube недоволен нами: отдых до `until` (time.time())."""

    def job_started(self, job: Job) -> None:
        """Задание взято."""

    def progress(self, job: Job, stage: str, fraction: float, info: dict[str, Any]) -> None:
        """Ход задания: этап, доля 0..1, название и артист, когда они известны."""

    def job_finished(self, job: Job, outcome: str, result: dict[str, Any] | None) -> None:
        """Задание закончилось: `done`, `failed`, `released` (возвращено серверу) или `lost`
        (аренда кончилась, отчёт сервер не принял)."""


class ActiveJob:
    """Задание в работе. Последнее слово серверу — отчёт или возврат — говорится один раз:
    его забирает тот, кто первым вызвал `close`: поток задания (`worker`), остановка приложения
    (`control`) или продление аренды, узнавшее, что задание уже не наше (`lease`)."""

    def __init__(self, job: Job) -> None:
        self.job = job
        self.cancel = threading.Event()
        # Последнее слово сказано: отчёт отправлен, задание возвращено или отдано другому
        self.settled = threading.Event()
        self.closed_by: str | None = None
        self._lock = threading.Lock()

    def close(self, by: str) -> bool:
        with self._lock:
            if self.closed_by is not None:
                return False
            self.closed_by = by
            return True


class _LeaseKeeper:
    """Продлевает аренду задания в фоне, пока идёт скачивание: оно длится до нескольких минут."""

    def __init__(
        self,
        http: httpx.Client,
        active: ActiveJob,
        worker_id: str,
        every_s: float,
        log: Callable[[str], None],
    ) -> None:
        self._http = http
        self._active = active
        self._worker_id = worker_id
        self._every_s = every_s
        self._log = log
        self._stop = threading.Event()
        job_id = active.job.id
        self._thread = threading.Thread(target=self._run, name=f"lease-{job_id}", daemon=True)
        self.renewals = 0
        self.lost = False

    def __enter__(self) -> Self:
        self._thread.start()
        return self

    def __exit__(self, *_: object) -> None:
        self._stop.set()
        self._thread.join(timeout=5)

    def _run(self) -> None:
        job = self._active.job
        while not self._stop.wait(self._every_s):
            if self._active.closed_by is not None:
                return
            try:
                response = self._http.post(
                    f"/api/fetcher/jobs/{job.id}/lease", json={"workerId": self._worker_id}
                )
            except httpx.HTTPError as e:
                self._log(f"аренда задания {job.id} не продлена ({type(e).__name__})")
                continue
            if response.status_code == 409:
                # Задание отдали другому: дальше работать незачем, отчёт сервер не примет
                self.lost = True
                self._log(f"задание {job.id} уже не наше: аренда кончилась")
                if self._active.close("lease"):
                    self._active.cancel.set()
                    self._active.settled.set()
                return
            if response.is_success:
                self.renewals += 1


class Worker:
    def __init__(
        self,
        fetcher: Fetcher,
        server: str,
        token: str,
        *,
        worker_id: str | None = None,
        poll_s: float = 30.0,
        cooldown_s: float = 5.0,
        retry_base_s: float | None = None,
        retry_max_s: float = 600.0,
        lease_renew_s: float = LEASE_RENEW_S,
        log: Callable[[str], None] = _log,
        transport: httpx.BaseTransport | None = None,
        control: Control | None = None,
        events: WorkerEvents | None = None,
        cooldown_until: float = 0.0,
        clips: ClipMaker | None | bool = True,
    ) -> None:
        self.fetcher = fetcher
        # Клипы ленты открытий: по умолчанию — через загрузчик аудио того же Fetcher
        self.clips: ClipMaker | None = None
        if isinstance(clips, ClipMaker):
            self.clips = clips
        elif clips is True:
            downloader = getattr(fetcher, "downloader", None)
            if isinstance(downloader, Downloader):
                self.clips = ClipMaker(downloader)
        # До этого момента (time.time()) о клипах не спрашиваем: сервер их не знает
        self.clips_off_until = 0.0
        self._clips_warned = False
        self.worker_id = worker_id or socket.gethostname()
        self.poll_s = poll_s
        self.cooldown_s = cooldown_s
        self.retry_base_s = retry_base_s if retry_base_s is not None else 2 * poll_s
        self.retry_max_s = retry_max_s
        self.lease_renew_s = lease_renew_s
        self.log = log
        self.control = control or Control()
        self.events = events or WorkerEvents()
        # До этого момента (time.time()) задания не берутся: YouTube заподозрил бота
        self.cooldown_until = cooldown_until
        self.cooldown_code: str | None = None
        self.http = httpx.Client(
            base_url=server.rstrip("/"),
            headers={
                "Authorization": f"Bearer {token}",
                "User-Agent": f"DalyWave-fetcher/{__version__}",
            },
            # Сервер держит запрос задания до ~25 секунд, пока заданий нет
            timeout=30.0,
            transport=transport,
        )
        self._active: ActiveJob | None = None

    def hello(self) -> dict[str, Any]:
        s = self.fetcher.settings
        return {
            "workerId": self.worker_id,
            "version": __version__,
            "capabilities": {
                "kinds": ["acquire", "clip"] if self._clips_ready() else ["acquire"],
                "potProvider": s.pot_url is not None,
                "cookies": s.cookies_file is not None,
                "playerClients": list(s.player_clients),
                "ytDlp": YT_DLP_VERSION,
            },
        }

    def run(self, once: bool = False) -> int:
        failures = 0
        control = self.control
        with self.http:
            while not control.stopped:
                if control.paused:
                    self.events.paused()
                    control.wait_while_paused()
                    continue
                rest = self.cooldown_until - time.time()
                if rest > 0:
                    self.events.cooling(
                        self.cooldown_code or "systemic",
                        "YouTube недоволен частыми запросами — пауза",
                        self.cooldown_until,
                    )
                    control.wait(rest)
                    continue
                try:
                    response = self.http.post(
                        "/api/fetcher/jobs/claim",
                        json=self.hello(),
                        extensions={"trace": self._trace},
                    )
                except httpx.HTTPError as e:
                    failures += 1
                    delay = self._backoff(failures)
                    self.log(f"сервер недоступен ({type(e).__name__}), жду")
                    self.events.offline(f"сервер недоступен ({type(e).__name__})", delay)
                    control.wait(delay)
                    continue
                if response.status_code in (401, 403):
                    self.log("сервер не принял токен загрузчика")
                    self.events.rejected()
                    return EXIT_TOKEN_REJECTED
                if response.status_code == 204:
                    failures = 0
                    clip = self._clip_turn()
                    if clip is not None:
                        if once:
                            return EXIT_OK
                        if clip in SYSTEMIC_ERRORS:
                            self.cooldown_until = time.time() + SYSTEMIC_PAUSE_S
                            self.cooldown_code = clip
                        continue
                    self.events.idle()
                    if once:
                        return EXIT_OK
                    # Сервер с long-poll уже подождал и сам говорит, когда спросить снова
                    retry = _retry_after(response)
                    control.wait(
                        retry
                        if retry is not None
                        else self.poll_s + random.uniform(0, self.poll_s / 3)
                    )
                    continue
                if not response.is_success:
                    failures += 1
                    delay = self._backoff(failures)
                    self.log(f"сервер ответил {response.status_code}, жду")
                    self.events.offline(f"сервер ответил {response.status_code}", delay)
                    control.wait(delay)
                    continue
                failures = 0
                job = Job.model_validate(response.json()["job"])
                code = self._process(job)
                if once:
                    return EXIT_OK
                if code in SYSTEMIC_ERRORS:
                    self.cooldown_until = time.time() + SYSTEMIC_PAUSE_S
                    self.cooldown_code = code
                elif code != "interrupted":
                    control.wait(self.cooldown_s)
        return EXIT_OK

    # ───────────── клипы ─────────────

    def _clips_ready(self) -> bool:
        """Клипы включены и ffmpeg умеет видео (https и рабочий кодировщик H.264). Не умеет —
        клипы не берём вовсе: их сделает загрузчик с новым приложением."""
        if self.clips is None:
            return False
        support = self.clips.support()
        if support.encoder is None and not self._clips_warned:
            self._clips_warned = True
            self.log(f"клипы выключены: {support.problem}")
        return support.encoder is not None

    def _clip_turn(self) -> str | None:
        """Взять и сделать один клип, если сервер его даёт. None — клипа не было; иначе код сбоя
        результата или пустая строка."""
        if self.control.stopped or time.time() < self.clips_off_until or not self._clips_ready():
            return None
        try:
            response = self.http.post(
                "/api/fetcher/clips/claim", json={"workerId": self.worker_id}, timeout=10.0
            )
        except httpx.HTTPError as e:
            self.log(f"клипы: сервер недоступен ({type(e).__name__})")
            return None
        if response.status_code in (404, 405):
            self.clips_off_until = time.time() + CLIPS_OFF_S
            self.log("клипы: сервер их не знает — не спрашиваю час")
            return None
        if response.status_code != 200:
            return None
        try:
            job = ClipJob.model_validate(response.json()["job"])
        except (ValueError, KeyError) as e:
            self.log(f"клипы: задание не разобрать ({type(e).__name__})")
            return None
        return self._run_clip(job)

    def _run_clip(self, job: ClipJob) -> str:
        assert self.clips is not None
        who = ", ".join(job.artists[:2])
        self.log(f"клип {job.id}: {who} — {job.title}")
        try:
            result = self.clips.make(
                title=job.title,
                artists=job.artists,
                duration_ms=job.duration_ms,
                hook_start_ms=job.hook_start_ms,
                length_ms=job.length_ms,
                exclude_video_ids=job.exclude_video_ids,
                upload=job.upload,
            )
        except Exception as e:  # noqa: BLE001 — сбой одного клипа не роняет цикл загрузчика
            result = {"status": "error", "error": f"unknown: {type(e).__name__}: {e}"[:300]}
        self.log(f"клип {job.id}: {result.get('status')}")
        self._report_clip(job, result)
        return error_code(result) or ""

    def _report_clip(self, job: ClipJob, result: dict[str, Any]) -> int | None:
        payload = {"workerId": self.worker_id, "result": result}
        for attempt in range(3):
            try:
                response = self.http.post(f"/api/fetcher/clips/{job.id}/result", json=payload)
            except httpx.HTTPError as e:
                self.log(f"клип {job.id}: отчёт не ушёл ({type(e).__name__}), повтор")
                self.control.wait(2**attempt)
                continue
            if response.is_success or (
                400 <= response.status_code < 500 and response.status_code != 429
            ):
                if not response.is_success:
                    self.log(f"клип {job.id}: сервер не принял отчёт ({response.status_code})")
                return response.status_code
            self.control.wait(2**attempt)
        return None

    def _trace(self, name: str, _: dict[str, Any]) -> None:
        # httpcore: тело запроса ушло — дальше сервер держит запрос до ~25 секунд
        if name.endswith("send_request_body.complete"):
            self.events.waiting()

    def _backoff(self, failures: int) -> float:
        return min(self.retry_base_s * 2 ** (failures - 1), self.retry_max_s)

    def _process(self, job: Job) -> str | None:
        """Задание целиком: скачивание и отчёт. Возвращает код ошибки результата
        или `interrupted`, если задание прервали."""
        active = ActiveJob(job)
        self._active = active
        try:
            return self._run_job(active)
        finally:
            self._active = None

    def _run_job(self, active: ActiveJob) -> str | None:
        job = active.job
        target = job.direct_url or job.deezer_track_id
        self.log(f"задание {job.id}: трек Deezer {job.deezer_track_id} ({target})")
        self.events.job_started(job)

        def listen(stage: str, fraction: float, info: dict[str, Any]) -> None:
            if active.cancel.is_set():
                raise progress.Interrupted
            self.events.progress(job, stage, fraction, info)

        result = None
        try:
            if self.control.stopped:
                # Задание пришло, когда приложение уже закрывалось
                raise progress.Interrupted
            with (
                _LeaseKeeper(self.http, active, self.worker_id, self.lease_renew_s, self.log),
                progress.observe(listen),
            ):
                result = self.fetcher.acquire(
                    job.deezer_track_id,
                    upload=job.upload.target() if job.upload else None,
                    max_attempts=job.max_attempts,
                    prefer_video_ids=job.prefer_video_ids,
                    exclude_video_ids=job.exclude_video_ids,
                    direct_url=job.direct_url,
                )
        except progress.Interrupted:
            self.log(f"задание {job.id}: прервано")
        if result is None and active.close("worker"):
            # Прервали до того, как остановка успела вернуть задание сама
            self._give_back(job, "задание прервано")
            active.settled.set()
            self.events.job_finished(job, "released", None)
            return "interrupted"
        if result is None or not active.close("worker"):
            # Остановка уже вернула задание (`control`) или аренду отдали другому (`lease`)
            if active.closed_by == "lease":
                self.events.job_finished(job, "lost", None)
            return "interrupted"
        self.log(f"задание {job.id}: {result.status}")
        data = result.to_json()
        try:
            status = self._report(job, data)
        finally:
            active.settled.set()
        uploaded = bool((data.get("upload") or {}).get("ok"))
        if status is None or status == 409:
            outcome = "lost"
        elif data.get("status") in ("verified", "unverified") and uploaded and status < 300:
            outcome = "done"
        else:
            outcome = "failed"
        self.events.job_finished(job, outcome, data)
        return (result.error or {}).get("code")

    def interrupt(self, reason: str, wait_report_s: float = 10.0) -> None:
        """Прервать текущее задание и сразу вернуть его серверу (зовётся из другого потока).

        Если отчёт о готовом задании уже отправляется, ждём его — работа не пропадёт.
        """
        active = self._active
        if active is None:
            return
        active.cancel.set()
        if active.close("control"):
            self._give_back(active.job, reason)
            active.settled.set()
            self.events.job_finished(active.job, "released", None)
        else:
            active.settled.wait(timeout=wait_report_s)

    def _give_back(self, job: Job, reason: str) -> None:
        """Вернуть задание в очередь, не тратя попытку."""
        try:
            response = self.http.post(
                f"/api/fetcher/jobs/{job.id}/release",
                json={"workerId": self.worker_id, "reason": reason},
                timeout=5.0,
            )
            if response.is_success:
                self.log(f"задание {job.id} возвращено в очередь")
                return
            if response.status_code == 409:
                return
        except httpx.HTTPError:
            pass
        # Сервер без `release`: отчёт о сбое исполнителя тоже возвращает задачу, через 2 минуты
        result = {
            "status": "error",
            "deezerTrackId": job.deezer_track_id,
            "track": None,
            "chosen": None,
            "file": None,
            "upload": None,
            "attempts": [],
            "candidates": [],
            "error": {
                "code": GIVE_BACK_CODE,
                "message": f"загрузчик остановлен: {reason}",
                "interrupted": True,
            },
            "timingsMs": {},
            "versions": {"fetcher": __version__, "ytDlp": YT_DLP_VERSION},
        }
        try:
            response = self.http.post(
                f"/api/fetcher/jobs/{job.id}/result",
                json={"workerId": self.worker_id, "result": result},
                timeout=5.0,
            )
        except httpx.HTTPError as e:
            self.log(f"задание {job.id} не вернуть ({type(e).__name__}): сервер заберёт по аренде")
            return
        if response.is_success:
            self.log(f"задание {job.id} возвращено в очередь (отчётом о сбое исполнителя)")
        else:
            self.log(f"задание {job.id} не вернуть: сервер ответил {response.status_code}")

    def _report(self, job: Job, result: dict[str, Any]) -> int | None:
        """Отчёт о задании. Возвращает статус ответа сервера или None, если не достучались."""
        payload = {"workerId": self.worker_id, "result": result}
        for attempt in range(5):
            try:
                response = self.http.post(f"/api/fetcher/jobs/{job.id}/result", json=payload)
            except httpx.HTTPError as e:
                self.log(f"не удалось отправить результат ({type(e).__name__}), повтор")
                self.control.wait(2**attempt)
                continue
            if response.is_success:
                return response.status_code
            if 400 <= response.status_code < 500 and response.status_code != 429:
                # Повтор не поможет: задание уже не наше или сервер отверг отчёт
                self.log(f"сервер не принял результат задания {job.id}: {response.status_code}")
                return response.status_code
            self.log(f"сервер ответил {response.status_code} на результат, повтор")
            self.control.wait(2**attempt)
        self.log(f"результат задания {job.id} потерян — сервер вернёт его в очередь по аренде")
        return None


def run_worker(
    fetcher: Fetcher,
    server: str,
    token: str,
    *,
    worker_id: str | None = None,
    once: bool = False,
    poll_s: float = 30.0,
    cooldown_s: float = 5.0,
    lease_renew_s: float = LEASE_RENEW_S,
    log: Callable[[str], None] = _log,
    sleep: Callable[[float], None] = time.sleep,
    transport: httpx.BaseTransport | None = None,
) -> int:
    """`fetcher worker`: цикл без паузы и событий — для launchd и `make downloader`."""
    return Worker(
        fetcher,
        server,
        token,
        worker_id=worker_id,
        poll_s=poll_s,
        cooldown_s=cooldown_s,
        lease_renew_s=lease_renew_s,
        log=log,
        transport=transport,
        control=Control(sleep),
    ).run(once=once)
