"""Настройки сервиса из переменных окружения `ML_*`."""

from __future__ import annotations

import math
import os
from dataclasses import dataclass, field
from pathlib import Path


def _env(name: str) -> str | None:
    value = os.environ.get(name, "").strip()
    return value or None


def _int_env(name: str, default: int) -> int:
    value = _env(name)
    return int(value) if value else default


def _float_env(name: str, default: float) -> float:
    value = _env(name)
    return float(value) if value else default


def available_cpus() -> int:
    """Сколько ядер реально доступно: квота cgroup (docker --cpus) или маска процесса."""
    count = os.process_cpu_count() or os.cpu_count() or 1
    try:
        quota, period = Path("/sys/fs/cgroup/cpu.max").read_text().split()
        if quota != "max":
            count = min(count, max(1, math.ceil(int(quota) / int(period))))
    except (OSError, ValueError):
        pass
    return count


@dataclass(frozen=True)
class Settings:
    models_dir: Path = field(default_factory=lambda: Path("models"))
    # Потоки onnxruntime на одну модель; модели идут по очереди, так что это ≈ ядра на запрос.
    threads: int = field(default_factory=available_cpus)
    # Дольше — анализируется только начало (длинные миксы и подкасты).
    max_duration_s: float = 900.0
    download_timeout_s: float = 60.0
    max_download_mb: int = 300
    # Папка, внутри которой разрешены запросы с `path`. Пусто — только `url`.
    files_root: Path | None = None
    # Окна CLAP по 10 с на полном треке; превью — три окна встык.
    clap_windows: int = 6
    work_dir: Path | None = None

    @classmethod
    def from_env(cls) -> Settings:
        root = _env("ML_FILES_ROOT")
        work = _env("ML_WORK_DIR")
        return cls(
            models_dir=Path(_env("ML_MODELS_DIR") or "models"),
            threads=_int_env("ML_THREADS", available_cpus()),
            max_duration_s=_float_env("ML_MAX_DURATION_S", 900.0),
            download_timeout_s=_float_env("ML_DOWNLOAD_TIMEOUT_S", 60.0),
            max_download_mb=_int_env("ML_MAX_DOWNLOAD_MB", 300),
            files_root=Path(root).resolve() if root else None,
            clap_windows=max(1, _int_env("ML_CLAP_WINDOWS", 6)),
            work_dir=Path(work) if work else None,
        )

    def describe(self) -> dict[str, object]:
        return {
            "modelsDir": str(self.models_dir),
            "threads": self.threads,
            "maxDurationS": self.max_duration_s,
            "filesRoot": str(self.files_root) if self.files_root else None,
            "clapWindows": self.clap_windows,
        }
