"""Загрузчик внутри приложения (sidecar Tauri): `dalywave-fetcher sidecar`.

Связь с приложением — строки JSON: события в stdout, команды из stdin (договор —
`services/fetcher/SIDECAR.md`). Почему не HTTP-порт: stdin/stdout у sidecar Tauri есть из
коробки, не нужны ни порт, ни секрет, а закрытый stdin — надёжный знак, что приложения больше
нет (даже если оно упало): загрузчик возвращает задание серверу и выходит. Пока приложение
работает — загрузчик качает, закрыто — не качает ничего.

Лог — в файл (`--log-file`) и в stderr; stdout занят только событиями.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import logging
import logging.handlers
import os
import queue
import signal
import socket
import subprocess
import sys
import threading
import time
from datetime import UTC, date, datetime
from pathlib import Path
from typing import IO, Any

from . import __version__, tools
from .config import Settings
from .download import YT_DLP_VERSION
from .models import Job
from .pot import PotError, PotServer, deno_env
from .worker import EXIT_TOKEN_REJECTED, Control, Worker, WorkerEvents

PROTOCOL = 1
APP_ID = "rest.daly.wave"
TOKEN_ENV = ("DALYWAVE_DOWNLOADER_TOKEN", "FETCHER_WORKER_TOKEN")

# Коды выхода sidecar.
EXIT_OK = 0
EXIT_CRASH = 1
EXIT_USAGE = 2
EXIT_TOKEN = 3
EXIT_TOOLS = 4

# Ход задания приложению — не чаще, чем раз в столько секунд (смена этапа — сразу).
PROGRESS_EVERY_S = 0.25
# Сколько ждать отправки отчёта о готовом задании при остановке.
STOP_REPORT_WAIT_S = 10.0

log = logging.getLogger("dalywave_fetcher.sidecar")


def _now() -> str:
    return datetime.now(UTC).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def _iso(epoch: float | None) -> str | None:
    if epoch is None:
        return None
    return datetime.fromtimestamp(epoch, UTC).isoformat(timespec="seconds").replace("+00:00", "Z")


def default_dirs() -> tuple[Path, Path]:
    """Папки данных и логов — те же, что у приложения Tauri (`app_local_data_dir`,
    `app_log_dir`), чтобы его файлы лежали вместе."""
    home = Path.home()
    if sys.platform == "darwin":
        return (
            home / "Library" / "Application Support" / APP_ID / "fetcher",
            home / "Library" / "Logs" / APP_ID,
        )
    if sys.platform == "win32":
        local = Path(os.environ.get("LOCALAPPDATA") or home / "AppData" / "Local") / APP_ID
        return local / "fetcher", local / "logs"
    base = Path(os.environ.get("XDG_DATA_HOME") or home / ".local" / "share") / APP_ID
    return base / "fetcher", base / "logs"


def default_pot_home() -> Path | None:
    """Сервер PO-токенов среди ресурсов: в .app — `Contents/Resources/bgutil-server`,
    в `dist/sidecar/` — `resources/bgutil-server`."""
    here = tools.executable_dir()
    if here is None:
        return None
    for path in (
        here.parent / "Resources" / "bgutil-server",
        here / "resources" / "bgutil-server",
        here / "bgutil-server",
    ):
        if path.is_dir():
            return path
    return None


class Emitter:
    """События приложению: одна строка JSON на событие."""

    def __init__(self, stream: IO[str]) -> None:
        self._stream = stream
        self._lock = threading.Lock()
        self.broken = False

    def emit(self, event: str, **data: Any) -> None:
        line = json.dumps(
            {"event": event, "ts": _now(), **data}, ensure_ascii=False, separators=(",", ":")
        )
        with self._lock:
            if self.broken:
                return
            try:
                self._stream.write(line + "\n")
                self._stream.flush()
            except (BrokenPipeError, OSError, ValueError):
                # Приложение закрыло канал — дальше говорить некому
                self.broken = True


class Counters:
    """Сделано и не удалось за сегодня (по часам Mac) и отдых после бот-проверки — в файле,
    чтобы перезапуск приложения их не обнулял."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.day = date.today().isoformat()
        self.done = 0
        self.failed = 0
        self.cooldown_until = 0.0
        self.cooldown_code: str | None = None
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            data = {}
        if data.get("date") == self.day:
            self.done = int(data.get("done") or 0)
            self.failed = int(data.get("failed") or 0)
        self.cooldown_until = float(data.get("cooldownUntil") or 0.0)
        self.cooldown_code = data.get("cooldownCode")

    def rollover(self) -> bool:
        today = date.today().isoformat()
        if today == self.day:
            return False
        self.day, self.done, self.failed = today, 0, 0
        self.save()
        return True

    def add(self, outcome: str) -> None:
        self.rollover()
        if outcome == "done":
            self.done += 1
        elif outcome == "failed":
            self.failed += 1
        self.save()

    def save(self) -> None:
        data = {
            "date": self.day,
            "done": self.done,
            "failed": self.failed,
            "cooldownUntil": self.cooldown_until,
            "cooldownCode": self.cooldown_code,
        }
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            tmp = self.path.with_suffix(".tmp")
            tmp.write_text(json.dumps(data), encoding="utf-8")
            tmp.replace(self.path)
        except OSError as e:
            log.warning("не удалось сохранить счётчики: %s", e)

    def to_json(self) -> dict[str, Any]:
        return {"date": self.day, "doneToday": self.done, "failedToday": self.failed}


class Sidecar(WorkerEvents):
    """Состояние загрузчика для приложения и обработка его команд."""

    def __init__(
        self,
        emitter: Emitter,
        *,
        server: str,
        worker_name: str,
        counters: Counters,
        control: Control,
    ) -> None:
        self.emitter = emitter
        self.server = server
        self.worker_name = worker_name
        self.counters = counters
        self.control = control
        self.worker: Worker | None = None
        self.pot_server: PotServer | None = None
        self.state = "starting"
        self.error: dict[str, str] | None = None
        self.retry_at: float | None = None
        self.job: dict[str, Any] | None = None
        self.pot: dict[str, Any] = {"state": "off"}
        # Не 0.0: monotonic считает от загрузки системы, и в первую минуту после неё перезапуск
        # считался бы «недавним»
        self._pot_restarted = float("-inf")
        self._job_started = 0.0
        self._last_status: str | None = None
        self._last_progress = 0.0
        self._lock = threading.RLock()

    # ───────────── состояние ─────────────

    def status(self) -> dict[str, Any]:
        return {
            "state": self.state,
            "paused": self.control.paused,
            "server": self.server,
            "workerName": self.worker_name,
            "job": self.job,
            "error": self.error,
            "retryAt": _iso(self.retry_at),
            "counters": self.counters.to_json(),
            "pot": self.pot,
        }

    def publish(self, force: bool = False) -> None:
        with self._lock:
            status = self.status()
            key = json.dumps(status, sort_keys=True, ensure_ascii=False)
            if not force and key == self._last_status:
                return
            self._last_status = key
            self.emitter.emit("status", **status)

    def set_state(
        self,
        state: str,
        *,
        error: dict[str, str] | None = None,
        retry_at: float | None = None,
    ) -> None:
        with self._lock:
            if self.state == "stopping":
                return
            self.state = state
            self.error = error
            self.retry_at = retry_at
            if state != "working":
                self.job = None
            self.counters.rollover()
            self.publish()

    # ───────────── события цикла (WorkerEvents) ─────────────

    def idle(self) -> None:
        self.set_state("paused" if self.control.paused else "idle")

    def waiting(self) -> None:
        # Сервер принял запрос задания и держит его: на связи, не дожидаясь ответа (до 25 с)
        if self.state in ("starting", "offline", "error"):
            self.set_state("paused" if self.control.paused else "idle")

    def offline(self, message: str, retry_in_s: float) -> None:
        code = "server_error" if message.startswith("сервер ответил") else "server_unreachable"
        self.set_state(
            "offline",
            error={"code": code, "message": message},
            retry_at=time.time() + retry_in_s,
        )

    def rejected(self) -> None:
        self.set_state(
            "error",
            error={
                "code": "token_rejected",
                "message": "сервер не принял токен загрузчика — нужен новый",
            },
        )

    def paused(self) -> None:
        self.set_state("paused")

    def cooling(self, code: str, message: str, until: float) -> None:
        with self._lock:
            if self.counters.cooldown_until != until:
                self.counters.cooldown_until = until
                self.counters.cooldown_code = code
                self.counters.save()
        self.set_state(
            "error",
            error={"code": code, "message": f"{message} до {_iso(until)}"},
            retry_at=until,
        )

    def job_started(self, job: Job) -> None:
        with self._lock:
            self._job_started = time.monotonic()
            self.job = {
                "id": job.id,
                "deezerTrackId": job.deezer_track_id,
                "track": None,
                "artist": None,
                "stage": "matching",
                "progress": 0.0,
                "attempt": None,
            }
            self.set_state("working")

    def progress(self, job: Job, stage: str, fraction: float, info: dict[str, Any]) -> None:
        with self._lock:
            if self.job is None or self.job["id"] != job.id:
                return
            changed = stage != self.job["stage"]
            self.job["stage"] = stage
            self.job["progress"] = round(fraction, 3)
            for key in ("track", "artist", "attempt"):
                if info.get(key) is not None:
                    self.job[key] = info[key]
                    changed = True
            now = time.monotonic()
            if changed or now - self._last_progress >= PROGRESS_EVERY_S or fraction >= 1.0:
                self._last_progress = now
                self.publish()

    def job_finished(self, job: Job, outcome: str, result: dict[str, Any] | None) -> None:
        with self._lock:
            current = self.job if self.job and self.job["id"] == job.id else {}
            track = (result or {}).get("track") or {}
            self.counters.add(outcome)
            self.emitter.emit(
                "job",
                id=job.id,
                deezerTrackId=job.deezer_track_id,
                track=track.get("title") or current.get("track"),
                artist=track.get("artist") or current.get("artist"),
                outcome=outcome,
                status=(result or {}).get("status"),
                error=(result or {}).get("error"),
                elapsedMs=int((time.monotonic() - self._job_started) * 1000),
            )
            self.job = None
            self.set_state("paused" if self.control.paused else "idle")

    # ───────────── команды приложения ─────────────

    def handle(self, command: dict[str, Any]) -> bool:
        """Выполняет команду: сначала `ack`, затем `status`, если состояние поменялось
        (на `status` — всегда). False — пора останавливаться."""
        name = command.get("cmd")
        if command.get("internal"):
            # Остановку попросил не приложение (сигнал, закрытый stdin, конец цикла) — без ack
            return name != "stop"
        reply: dict[str, Any] = {"cmd": name, "ok": True}
        if "id" in command:
            reply["id"] = command["id"]
        if name not in ("stop", "pause", "resume", "status"):
            message = "строка не JSON" if "raw" in command else f"неизвестная команда: {name!r}"
            reply.update(ok=False, message=message)
            self.emitter.emit("ack", **reply)
            return True
        self.emitter.emit("ack", **reply)
        if name == "stop":
            return False
        if name == "pause":
            self.control.set_paused(True)
            if command.get("now") and self.worker is not None:
                self.worker.interrupt("загрузчик поставлен на паузу")
            if self.state not in ("working", "stopping"):
                self.set_state("paused")
        elif name == "resume":
            self.control.set_paused(False)
            if self.state == "paused":
                self.set_state("idle")
        self.publish(force=name == "status")
        return True

    def check_pot(self) -> None:
        """Сервис PO-токенов, который уже работал, остановился — поднять снова, не чаще раза
        в минуту. Порт тот же, если свободен; иначе новый адрес получит yt-dlp."""
        server = self.pot_server
        if server is None or server.port is None or server.alive():
            return
        if self.state == "stopping" or time.monotonic() - self._pot_restarted < 60:
            return
        self._pot_restarted = time.monotonic()
        log.warning("сервис PO-токенов остановился — перезапускаю")
        self.pot = {"state": "starting"}
        self.publish()
        threading.Thread(target=self._restart_pot, args=(server,), name="pot", daemon=True).start()

    def _restart_pot(self, server: PotServer) -> None:
        try:
            url = server.start()
        except PotError as e:
            log.warning("сервис PO-токенов не перезапустился: %s", e)
            self.pot = {"state": "failed", "message": str(e)}
        else:
            self.pot = {"state": "ready", "version": server.version}
            worker = self.worker
            if worker is not None and worker.fetcher.settings.pot_url != url:
                settings = dataclasses.replace(worker.fetcher.settings, pot_url=url)
                worker.fetcher.settings = settings
                worker.fetcher.downloader.settings = settings
        self.publish()

    def shutdown(self, reason: str) -> None:
        """Остановка: текущее задание — серверу, сервер PO-токенов — выключить."""
        with self._lock:
            self.state = "stopping"
            self.publish()
        self.control.stop()
        if self.worker is not None:
            self.worker.interrupt(f"приложение закрыто ({reason})", STOP_REPORT_WAIT_S)
        if self.pot_server is not None:
            self.pot_server.stop()


# ───────────── запуск ─────────────


def _setup_logging(path: Path) -> None:
    root = logging.getLogger()
    root.setLevel(logging.INFO)
    fmt = logging.Formatter("%(asctime)s %(levelname)s %(message)s")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        file = logging.handlers.RotatingFileHandler(
            path, maxBytes=5 * 1024 * 1024, backupCount=3, encoding="utf-8"
        )
        file.setFormatter(fmt)
        root.addHandler(file)
    except OSError as e:
        print(f"лог в файл недоступен ({e}), пишу только в stderr", file=sys.stderr)
    console = logging.StreamHandler(sys.stderr)
    console.setFormatter(fmt)
    root.addHandler(console)
    # Строка на каждый запрос задания (раз в ~25 секунд) логу не нужна; ошибки останутся
    logging.getLogger("httpx").setLevel(logging.WARNING)


def _take_stdout() -> IO[str]:
    """Отдаёт stdout событиям, а всё остальное (print, библиотеки, дочерние процессы)
    отправляет в stderr: случайная строка не должна сломать протокол."""
    fd = os.dup(sys.stdout.fileno())
    os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
    sys.stdout = sys.stderr
    return os.fdopen(fd, "w", encoding="utf-8", buffering=1)


def _version_line(path: str, flag: str) -> str | None:
    try:
        out = subprocess.run([path, flag], capture_output=True, text=True, timeout=15, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if out.returncode != 0:
        return None
    lines = (out.stdout or out.stderr).strip().splitlines()
    return lines[0] if lines else ""


def _read_commands(commands: queue.SimpleQueue[dict[str, Any]], stream: IO[str]) -> None:
    for line in stream:
        line = line.strip()
        if not line:
            continue
        try:
            command = json.loads(line)
        except ValueError:
            command = {"cmd": None, "raw": line[:200]}
        commands.put(command if isinstance(command, dict) else {"cmd": None})
    # stdin закрыт: приложения больше нет
    commands.put({"cmd": "stop", "reason": "stdin closed", "internal": True})


def token_from_env() -> str | None:
    """Токен из окружения. Переменные убираются: дочерним процессам (Deno, ffmpeg) он ни к чему."""
    token = None
    for name in TOKEN_ENV:
        value = os.environ.pop(name, "").strip()
        token = token or value or None
    return token


def add_arguments(p: argparse.ArgumentParser) -> None:
    data_dir, log_dir = default_dirs()
    p.add_argument("--server", default=os.environ.get("DALYWAVE_SERVER", "https://wave.daly.rest"))
    p.add_argument(
        "--worker-name", help="имя в админке («Источники»); по умолчанию — имя компьютера"
    )
    p.add_argument("--data-dir", type=Path, default=data_dir, help=f"по умолчанию {data_dir}")
    p.add_argument(
        "--log-file", type=Path, default=log_dir / "fetcher.log", help="по умолчанию %(default)s"
    )
    p.add_argument("--paused", action="store_true", help="запуститься на паузе")
    p.add_argument(
        "--tools-dir",
        type=Path,
        action="append",
        default=[],
        help="где искать deno, ffmpeg, ffprobe, fpcalc (по умолчанию — рядом с программой)",
    )
    p.add_argument("--pot-server-dir", type=Path, help="папка bgutil-server (сервис PO-токенов)")
    p.add_argument("--no-pot", action="store_true", help="качать без PO-токенов")


def add_pot_check_arguments(p: argparse.ArgumentParser) -> None:
    data_dir, _ = default_dirs()
    p.add_argument("--data-dir", type=Path, default=data_dir)
    p.add_argument("--tools-dir", type=Path, action="append", default=[])
    p.add_argument("--pot-server-dir", type=Path)
    p.add_argument(
        "--video",
        default="afkL3uLNeXA",
        help="ролик для проверки yt-dlp клиентом mweb (токен GVS); пусто — без yt-dlp",
    )


def pot_check(args: argparse.Namespace) -> dict[str, Any]:
    """Свой сервер PO-токенов: запуск, два токена подряд и, по желанию, ссылки на поток yt-dlp
    клиентом mweb, которому без токена потоки не отдают."""
    import urllib.parse

    import yt_dlp

    from .download import Downloader, source_url
    from .pot import generate_token

    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s", stream=sys.stderr)
    data_dir: Path = args.data_dir
    data_dir.mkdir(parents=True, exist_ok=True)
    found = tools.resolve([*args.tools_dir, *([d] if (d := tools.executable_dir()) else [])])
    home = args.pot_server_dir or default_pot_home()
    if not found["deno"] or home is None:
        return {"ok": False, "error": "нет deno или папки bgutil-server", "deno": found["deno"]}
    tools.configure({name: path for name, path in found.items() if path})
    for key, value in deno_env(data_dir).items():
        os.environ.setdefault(key, value)
    server = PotServer(found["deno"], home, data_dir, log.info)
    report: dict[str, Any] = {"ok": False, "deno": found["deno"], "serverDir": str(home)}
    started = time.monotonic()
    try:
        url = server.start()
        report["server"] = {
            "url": url,
            "version": server.version,
            "startMs": int((time.monotonic() - started) * 1000),
        }
        tokens = []
        for binding in (args.video or "dQw4w9WgXcQ", "jNQXAC9IVRw"):
            t = time.monotonic()
            token = generate_token(url, binding)
            tokens.append(
                {"binding": binding, "length": len(token), "ms": int((time.monotonic() - t) * 1000)}
            )
        report["tokens"] = tokens
        if args.video:
            settings = Settings(data_dir=data_dir, deno_path=found["deno"], pot_url=url)
            opts = Downloader(settings).options(data_dir, "pot-check", ("mweb",))
            opts.update(skip_download=True, quiet=True, no_warnings=True)
            t = time.monotonic()
            with yt_dlp.YoutubeDL(opts) as ydl:
                info = ydl.extract_info(source_url(args.video), download=False) or {}
            audio = [f for f in info.get("formats") or [] if f.get("vcodec") == "none"]

            def query(f: dict[str, Any]) -> dict[str, list[str]]:
                return urllib.parse.parse_qs(urllib.parse.urlparse(f.get("url") or "").query)

            with_pot = [f for f in audio if "pot" in query(f)]
            report["ytDlp"] = {
                "video": args.video,
                "client": "mweb",
                "audioFormats": len(audio),
                "withPoToken": len(with_pot),
                "clients": sorted({(query(f).get("c") or ["?"])[0] for f in audio}),
                "ms": int((time.monotonic() - t) * 1000),
            }
        report["ok"] = bool(tokens) and (not args.video or bool(report["ytDlp"]["withPoToken"]))
    except (PotError, yt_dlp.utils.YoutubeDLError, OSError) as e:
        report["error"] = str(e)
    finally:
        server.stop()
    return report


def run(args: argparse.Namespace) -> int:
    protocol_out = _take_stdout()
    emitter = Emitter(protocol_out)
    data_dir: Path = args.data_dir
    data_dir.mkdir(parents=True, exist_ok=True)
    _setup_logging(args.log_file)
    worker_name = (args.worker_name or "").strip() or socket.gethostname()
    counters = Counters(data_dir / "state.json")
    control = Control()
    control.paused = bool(args.paused)
    sidecar = Sidecar(
        emitter, server=args.server, worker_name=worker_name, counters=counters, control=control
    )

    def finish(code: int, reason: str, message: str | None = None) -> int:
        emitter.emit("exit", code=code, reason=reason, message=message)
        # После `exit` — ни строчки: фоновые потоки ещё могут что-то сообщить
        emitter.broken = True
        log.info("загрузчик остановлен: %s (код %s)", reason, code)
        logging.shutdown()
        return code

    token = token_from_env()
    dirs = [*args.tools_dir, *([d] if (d := tools.executable_dir()) else [])]
    found = tools.resolve(dirs)
    versions = {
        name: _version_line(path, "--version" if name == "deno" else "-version") if path else None
        for name, path in found.items()
    }
    emitter.emit(
        "hello",
        protocol=PROTOCOL,
        version=__version__,
        ytDlp=YT_DLP_VERSION,
        pid=os.getpid(),
        server=args.server,
        workerName=worker_name,
        dataDir=str(data_dir),
        logFile=str(args.log_file),
        tools={name: {"path": found[name], "version": versions[name]} for name in found},
    )
    log.info("загрузчик %s → %s как «%s»", __version__, args.server, worker_name)
    if not token:
        sidecar.set_state(
            "error", error={"code": "no_token", "message": "нет токена загрузчика (dwk_…)"}
        )
        return finish(EXIT_USAGE, "no_token", f"задайте {TOKEN_ENV[0]}")
    broken = [name for name, version in versions.items() if version is None]
    if broken:
        message = "не запускаются: " + ", ".join(broken)
        sidecar.set_state("error", error={"code": "tools_missing", "message": message})
        return finish(EXIT_TOOLS, "tools_missing", message)
    tools.configure({name: path for name, path in found.items() if path})
    for key, value in deno_env(data_dir).items():
        os.environ.setdefault(key, value)

    commands: queue.SimpleQueue[dict[str, Any]] = queue.SimpleQueue()

    def on_signal(signum: int, _: object) -> None:
        # SimpleQueue.put можно звать из обработчика сигнала
        commands.put({"cmd": "stop", "reason": f"signal {signum}", "internal": True})

    for sig in (signal.SIGTERM, signal.SIGINT, getattr(signal, "SIGHUP", None)):
        if sig is not None:
            signal.signal(sig, on_signal)
    threading.Thread(
        target=_read_commands, args=(commands, sys.stdin), name="stdin", daemon=True
    ).start()

    settings = dataclasses.replace(
        Settings.from_env(), data_dir=data_dir, deno_path=found["deno"], pot_url=None
    )
    pot_home = args.pot_server_dir or default_pot_home()
    if args.no_pot:
        sidecar.pot = {"state": "off"}
    elif pot_home is None:
        sidecar.pot = {"state": "failed", "message": "нет сервера PO-токенов среди ресурсов"}
    else:
        sidecar.pot = {"state": "starting"}
        sidecar.pot_server = PotServer(str(found["deno"]), pot_home, data_dir, log.info)
    exit_code: list[int] = []

    def work() -> None:
        nonlocal settings
        try:
            if sidecar.pot_server is not None:
                try:
                    url = sidecar.pot_server.start()
                    settings = dataclasses.replace(settings, pot_url=url)
                    sidecar.pot = {"state": "ready", "version": sidecar.pot_server.version}
                    log.info("сервис PO-токенов %s на %s", sidecar.pot_server.version, url)
                except PotError as e:
                    log.warning("сервис PO-токенов не запустился: %s — качаю без токенов", e)
                    sidecar.pot = {"state": "failed", "message": str(e)}
                sidecar.publish()
            from .acquire import Fetcher

            worker = Worker(
                Fetcher(settings),
                args.server,
                token,
                worker_id=worker_name,
                retry_base_s=5.0,
                retry_max_s=300.0,
                log=log.info,
                control=control,
                events=sidecar,
                cooldown_until=counters.cooldown_until,
            )
            worker.cooldown_code = counters.cooldown_code
            sidecar.worker = worker
            exit_code.append(worker.run())
        except Exception:
            log.exception("цикл загрузчика упал")
            exit_code.append(EXIT_CRASH)
        commands.put({"cmd": "stop", "reason": "worker exited", "internal": True})

    sidecar.publish(force=True)
    threading.Thread(target=work, name="worker", daemon=True).start()

    reason = "stop"
    while True:
        try:
            command = commands.get(timeout=30.0)
        except queue.Empty:
            if counters.rollover():
                sidecar.publish()
            sidecar.check_pot()
            continue
        if command.get("reason"):
            reason = str(command["reason"])
        if not sidecar.handle(command) or emitter.broken:
            break
    sidecar.shutdown(reason)
    code = exit_code[0] if exit_code else EXIT_OK
    if code == EXIT_TOKEN_REJECTED:
        return finish(EXIT_TOKEN, "token_rejected", "сервер не принял токен загрузчика")
    if code == EXIT_CRASH:
        return finish(EXIT_CRASH, "crashed", "цикл загрузчика упал — подробности в логе")
    return finish(EXIT_OK, reason)
