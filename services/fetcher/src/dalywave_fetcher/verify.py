"""Проверка записи акустическим отпечатком: 30-секундное превью Deezer ↔ скачанный файл.

chromaprint (`fpcalc -raw`) отдаёт по 32-битному числу на каждые ~0,124 с звука.
Превью «прикладывается» к треку со всеми сдвигами; на каждом сдвиге считается доля
несовпавших бит (BER). Сходство = 1 − BER на лучшем сдвиге. У разных записей BER
около 0,5 на любом сдвиге, у одной и той же — заметно ниже.
"""

from __future__ import annotations

import json
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
from numpy.lib.stride_tricks import sliding_window_view

from . import tools

# Шаг отпечатка: окно 4096 отсчётов с перекрытием 2/3 на частоте 11025 Гц.
ITEM_S = 4096 / 3 / 11025
# Порог по калибровке (bench/REPORT.md): верные записи — от 0,92, чужие треки и другие
# записи той же песни (концерт, sped up, перезапись) — до 0,62. Выше порога бывают
# только версии на тех же студийных дорожках (ремикс с куплетом гостя, концерт под
# фонограмму) — их отсекают метки версий в поиске, ещё до скачивания.
DEFAULT_THRESHOLD = 0.80
# Края превью у Deezer бывают с затуханием — их не сравниваем.
TRIM_ITEMS = 8
MIN_OVERLAP = 0.9


class VerifyError(Exception):
    pass


@dataclass(frozen=True)
class Fingerprint:
    items: np.ndarray
    duration_s: float


def fingerprint(path: Path, length_s: int = 0) -> Fingerprint:
    """Отпечаток файла целиком (`length_s=0`) или его начала."""
    try:
        proc = subprocess.run(
            [tools.exe("fpcalc"), "-raw", "-json", "-length", str(length_s), str(path)],
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as e:
        raise VerifyError(f"fpcalc не запустился: {e}") from e
    if proc.returncode != 0:
        raise VerifyError(f"fpcalc: {proc.stderr.strip()[-300:]}")
    data = json.loads(proc.stdout)
    items = (np.asarray(data["fingerprint"], dtype=np.int64) & 0xFFFFFFFF).astype(np.uint32)
    return Fingerprint(items=items, duration_s=float(data.get("duration") or 0.0))


@dataclass(frozen=True)
class Alignment:
    # Индекс элемента трека, на который ложится начало превью (с учётом обрезки краёв).
    offset_items: int
    ber: float
    overlap: int
    # Медиана сходства по всем сдвигам: у любой пары около 0,5, это «шум».
    baseline: float

    @property
    def similarity(self) -> float:
        return 1.0 - self.ber

    @property
    def offset_s(self) -> float:
        return self.offset_items * ITEM_S


def align(
    preview: np.ndarray,
    track: np.ndarray,
    *,
    trim: int = TRIM_ITEMS,
    min_overlap: float = MIN_OVERLAP,
) -> Alignment | None:
    """Ищет сдвиг превью внутри трека с наименьшей долей несовпавших бит."""
    if len(preview) > 4 * trim:
        p = preview[trim : len(preview) - trim]
    else:
        trim, p = 0, preview
    m, n = len(p), len(track)
    if m == 0 or n == 0:
        return None
    need = max(1, int(m * min_overlap))
    if n < need:
        return None
    # Превью может чуть выступать за края трека (у YouTube обрезана тишина) —
    # дополняем трек нулями и считаем только реально перекрытые элементы.
    pad = m - need
    padded = np.concatenate(
        [np.zeros(pad, np.uint32), track.astype(np.uint32), np.zeros(pad, np.uint32)]
    )
    valid = np.concatenate([np.zeros(pad, bool), np.ones(n, bool), np.zeros(pad, bool)])
    windows = sliding_window_view(padded, m)
    valid_windows = sliding_window_view(valid, m)
    errors = np.bitwise_count(windows ^ p[np.newaxis, :]).astype(np.int32)
    errors = np.where(valid_windows, errors, 0).sum(axis=1)
    counts = valid_windows.sum(axis=1)
    ok = counts >= need
    ber = np.full(len(counts), np.inf)
    ber[ok] = errors[ok] / (32.0 * counts[ok])
    best = int(np.argmin(ber))
    return Alignment(
        offset_items=best - pad - trim,
        ber=float(ber[best]),
        overlap=int(counts[best]),
        baseline=float(1.0 - np.median(ber[ok])),
    )


@dataclass
class VerifyResult:
    verified: bool | None
    similarity: float | None
    offset_s: float | None
    threshold: float
    baseline: float | None = None
    preview_items: int = 0
    track_items: int = 0
    error: str | None = None
    elapsed_ms: int = 0

    def to_json(self) -> dict[str, Any]:
        return {
            "verified": self.verified,
            "similarity": None if self.similarity is None else round(self.similarity, 4),
            "offsetS": None if self.offset_s is None else round(self.offset_s, 2),
            "threshold": self.threshold,
            "baseline": None if self.baseline is None else round(self.baseline, 4),
            "previewItems": self.preview_items,
            "trackItems": self.track_items,
            "error": self.error,
            "elapsedMs": self.elapsed_ms,
        }


def compare(
    preview: Fingerprint, track: Fingerprint, threshold: float = DEFAULT_THRESHOLD
) -> VerifyResult:
    result = align(preview.items, track.items)
    if result is None:
        return VerifyResult(
            verified=False,
            similarity=None,
            offset_s=None,
            threshold=threshold,
            preview_items=len(preview.items),
            track_items=len(track.items),
            error="слишком короткий файл для сравнения",
        )
    return VerifyResult(
        verified=result.similarity >= threshold,
        similarity=result.similarity,
        offset_s=result.offset_s,
        threshold=threshold,
        baseline=result.baseline,
        preview_items=len(preview.items),
        track_items=len(track.items),
    )


def verify(
    preview: Fingerprint, track_path: Path, threshold: float = DEFAULT_THRESHOLD
) -> VerifyResult:
    started = time.monotonic()
    try:
        track = fingerprint(track_path)
    except VerifyError as e:
        return VerifyResult(
            verified=None, similarity=None, offset_s=None, threshold=threshold, error=str(e)
        )
    result = compare(preview, track, threshold)
    result.elapsed_ms = int((time.monotonic() - started) * 1000)
    return result
