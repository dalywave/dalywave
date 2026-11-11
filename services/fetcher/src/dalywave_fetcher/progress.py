"""Ход задания для приложения: этап, доля и прерывание.

Конвейер (`acquire`, yt-dlp, загрузка в хранилище) сообщает об этапах через `report`, не зная,
кто слушает. Слушатель задаётся на время задания (`observe`) в том же потоке; он же может
прервать работу — тогда `report` поднимает `Interrupted`, и конвейер сворачивается на ближайшей
точке: между этапами, на очередном куске скачивания или загрузки.
"""

from __future__ import annotations

from collections.abc import Callable, Iterator
from contextlib import contextmanager
from contextvars import ContextVar
from typing import Any

# Этапы, которые видит приложение.
MATCHING = "matching"
DOWNLOADING = "downloading"
VERIFYING = "verifying"
UPLOADING = "uploading"

Listener = Callable[[str, float, dict[str, Any]], None]

_listener: ContextVar[Listener | None] = ContextVar("dalywave_progress", default=None)


class Interrupted(Exception):
    """Задание прервано: приложение закрывается или загрузчик поставлен на паузу."""


def report(stage: str, fraction: float, **info: Any) -> None:
    listener = _listener.get()
    if listener is not None:
        listener(stage, min(max(fraction, 0.0), 1.0), info)


@contextmanager
def observe(listener: Listener) -> Iterator[None]:
    token = _listener.set(listener)
    try:
        yield
    finally:
        _listener.reset(token)
