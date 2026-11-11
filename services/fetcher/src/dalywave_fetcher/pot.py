"""Сервис PO-токенов без Docker: HTTP-сервер bgutil-ytdlp-pot-provider на своём Deno.

Сервер слушает только 127.0.0.1 на случайном свободном порту; его запускает и останавливает
загрузчик. Запуск идёт через обёртку `serve.ts` (кладётся рядом с исходниками сервера при
сборке): она завершает сервер, как только закрылся stdin, — то есть и тогда, когда загрузчик
убит без предупреждения. Сирот на машине не остаётся.

Почему сервер, а не одноразовый скрипт bgutil: сервер держит BotGuard «тёплым» (токен
за доли секунды вместо нескольких секунд на запуск Deno, jsdom и BotGuard), а yt-dlp передаёт
ему задачу BotGuard прямо со страницы видео. Тот же режим — у сервиса в Docker, на котором
сделан замер `bench/REPORT.md`.
"""

from __future__ import annotations

import os
import socket
import subprocess
import threading
import time
from collections.abc import Callable
from pathlib import Path

import httpx

# Обёртка над src/main.ts сервера bgutil (services/fetcher/sidecar/pot/serve.ts).
SERVER_ENTRY = "serve.ts"


class PotError(Exception):
    pass


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


def deno_env(data_dir: Path) -> dict[str, str]:
    """Окружение Deno: свой кэш в папке загрузчика, без проверок обновлений и вопросов."""
    return {
        "DENO_DIR": str(data_dir / "deno"),
        "DENO_NO_UPDATE_CHECK": "1",
        "DENO_NO_PROMPT": "1",
        "NO_COLOR": "1",
    }


class PotServer:
    def __init__(
        self,
        deno: str,
        home: Path,
        data_dir: Path,
        log: Callable[[str], None],
    ) -> None:
        self.deno = deno
        self.home = home
        self.data_dir = data_dir
        self.log = log
        self.port: int | None = None
        self.version: str | None = None
        self._proc: subprocess.Popen[bytes] | None = None
        self._lock = threading.Lock()
        self._stopping = threading.Event()

    @property
    def url(self) -> str | None:
        return f"http://127.0.0.1:{self.port}" if self.port else None

    def command(self, port: int) -> list[str]:
        modules = self.home / "node_modules"
        return [
            self.deno,
            "run",
            "--allow-env",
            "--allow-net",
            f"--allow-ffi={modules}",
            f"--allow-read={modules}",
            str(self.home / SERVER_ENTRY),
            "--port",
            str(port),
            "--host",
            "127.0.0.1",
        ]

    def start(self, timeout_s: float = 90.0) -> str:
        """Запускает сервер и ждёт ответа `/ping`. Возвращает его адрес."""
        if not (self.home / SERVER_ENTRY).is_file():
            raise PotError(f"нет сервера PO-токенов: {self.home / SERVER_ENTRY}")
        with self._lock:
            last: Exception | None = None
            # Порт, на котором сервер уже жил, пробуем первым: yt-dlp знает этот адрес
            ports = [self.port] if self.port else []
            for port in [*ports, free_port(), free_port()]:
                if self._stopping.is_set():
                    break
                try:
                    self._spawn(port)
                    self.version = self._wait_ready(port, timeout_s)
                    self.port = port
                    return f"http://127.0.0.1:{port}"
                except PotError as e:
                    last = e
                    self._kill()
            raise PotError(str(last or "сервер PO-токенов остановлен"))

    def _spawn(self, port: int) -> None:
        env = {**os.environ, **deno_env(self.data_dir)}
        self._proc = subprocess.Popen(
            self.command(port),
            cwd=self.home,
            env=env,
            # stdin держим открытым: его закрытие — сигнал серверу выйти
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        threading.Thread(target=self._pipe_log, args=(self._proc,), daemon=True).start()

    def _pipe_log(self, proc: subprocess.Popen[bytes]) -> None:
        assert proc.stdout is not None
        for raw in proc.stdout:
            line = raw.decode("utf-8", errors="replace").rstrip()
            if line:
                self.log(f"pot: {line}")

    def _wait_ready(self, port: int, timeout_s: float) -> str:
        deadline = time.monotonic() + timeout_s
        while time.monotonic() < deadline:
            if self._stopping.is_set():
                raise PotError("сервер PO-токенов остановлен")
            proc = self._proc
            if proc is None or proc.poll() is not None:
                code = None if proc is None else proc.returncode
                raise PotError(f"сервер PO-токенов завершился при запуске (код {code})")
            try:
                response = httpx.get(f"http://127.0.0.1:{port}/ping", timeout=2.0)
                if response.is_success:
                    return str(response.json().get("version") or "")
            except (httpx.HTTPError, ValueError):
                pass
            time.sleep(0.25)
        raise PotError(f"сервер PO-токенов не ответил за {timeout_s:.0f} с")

    def alive(self) -> bool:
        return self._proc is not None and self._proc.poll() is None

    def stop(self, timeout_s: float = 5.0) -> None:
        """Останавливает сервер; не ждёт, если тот ещё запускается в другом потоке."""
        self._stopping.set()
        proc = self._proc
        if proc is None:
            return
        if proc.stdin is not None:
            try:
                proc.stdin.close()
            except OSError:
                pass
        try:
            proc.wait(timeout=timeout_s)
        except subprocess.TimeoutExpired:
            proc.kill()
        self._proc = None

    def _kill(self) -> None:
        proc = self._proc
        if proc is None:
            return
        proc.kill()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass
        self._proc = None


def generate_token(url: str, content_binding: str, timeout_s: float = 60.0) -> str:
    """Один PO-токен напрямую у сервера — для самопроверки (`fetcher pot-check`)."""
    response = httpx.post(
        f"{url}/get_pot",
        json={"content_binding": content_binding, "bypass_cache": True},
        timeout=timeout_s,
    )
    data = response.json()
    if not response.is_success or "poToken" not in data:
        raise PotError(str(data.get("error") or f"HTTP {response.status_code}"))
    return str(data["poToken"])
