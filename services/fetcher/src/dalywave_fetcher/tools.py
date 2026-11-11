"""Внешние программы: ffmpeg, ffprobe, fpcalc, deno.

В приложении они лежат рядом с загрузчиком (Tauri кладёт sidecar-программы в
`Contents/MacOS/` без суффикса платформы, в `dist/sidecar/` они с суффиксом
`-aarch64-apple-darwin`), в разработке и в Docker — в `PATH`. Найденные пути задаются
один раз на процесс (`configure`); без них — переменная `FETCHER_<ИМЯ>` или имя из `PATH`.
"""

from __future__ import annotations

import os
import platform
import shutil
import sys
from pathlib import Path

NAMES = ("ffmpeg", "ffprobe", "fpcalc", "deno")

_configured: dict[str, str] = {}


def target_triple() -> str:
    """Суффикс sidecar-программ Tauri для этой машины: `aarch64-apple-darwin` и т. п."""
    machine = platform.machine().lower()
    arch = {"arm64": "aarch64", "aarch64": "aarch64", "amd64": "x86_64", "x86_64": "x86_64"}.get(
        machine, machine
    )
    if sys.platform == "darwin":
        return f"{arch}-apple-darwin"
    if sys.platform == "win32":
        return f"{arch}-pc-windows-msvc"
    return f"{arch}-unknown-linux-gnu"


def executable_dir() -> Path | None:
    """Папка собранного загрузчика; при запуске из исходников (`uv run`) — None."""
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return None


def find(name: str, dirs: list[Path]) -> str | None:
    """Ищет программу в папках: сначала `name`, затем `name-<triple>`."""
    suffix = ".exe" if sys.platform == "win32" else ""
    for directory in dirs:
        for candidate in (f"{name}{suffix}", f"{name}-{target_triple()}{suffix}"):
            path = directory / candidate
            if path.is_file() and os.access(path, os.X_OK):
                return str(path)
    return None


def configure(paths: dict[str, str]) -> None:
    _configured.update(paths)


def exe(name: str) -> str:
    """Путь к программе: заданный `configure`, `FETCHER_<ИМЯ>` или имя для поиска в `PATH`."""
    return _configured.get(name) or os.environ.get(f"FETCHER_{name.upper()}", "").strip() or name


def resolve(dirs: list[Path]) -> dict[str, str | None]:
    """Полные пути всех программ: переменная окружения, папки `dirs`, затем `PATH`."""
    found: dict[str, str | None] = {}
    for name in NAMES:
        explicit = os.environ.get(f"FETCHER_{name.upper()}", "").strip()
        found[name] = explicit or find(name, dirs) or shutil.which(name)
    return found
