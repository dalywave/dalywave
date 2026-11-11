"""Громкость, истинный пик и огибающая для плеера — одним проходом ffmpeg по готовому файлу.

Интегральная громкость (EBU R128) и истинный пик нужны выравниванию громкости, огибающая
из 512 точек (0..1000) — полосе прокрутки. Всё это сервер хранит в файле записи и отдаёт
плееру вместе со ссылкой, без лишних запросов.
"""

from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

WAVEFORM_POINTS = 512
# Для огибающей хватает 8 кГц моно: ей нужна громкость по времени, а не спектр.
WAVEFORM_RATE = 8000

_LOUDNESS = re.compile(r"I:\s+(-?[\d.]+|-inf)\s+LUFS")
_PEAK = re.compile(r"Peak:\s+(-?[\d.]+|-inf)\s+dBFS")


class AnalyzeError(Exception):
    pass


@dataclass(frozen=True)
class Analysis:
    loudness_lufs: float | None
    true_peak_db: float | None
    waveform: list[int]

    def to_json(self) -> dict[str, Any]:
        return {
            "loudnessLufs": self.loudness_lufs,
            "truePeakDb": self.true_peak_db,
            "waveform": self.waveform,
        }


def _number(pattern: re.Pattern[str], text: str) -> float | None:
    match = pattern.search(text)
    if not match or match.group(1) == "-inf":
        return None
    return float(match.group(1))


def envelope(samples: np.ndarray, points: int = WAVEFORM_POINTS) -> list[int]:
    """RMS по равным кускам, нормированный на самый громкий кусок: 0..1000.

    RMS, а не пик: у современных громких записей пиковая огибающая — сплошной брусок.
    """
    if samples.size == 0:
        return [0] * points
    x = samples.astype(np.float64) / 32768.0
    edges = np.linspace(0, x.size, points + 1).astype(np.int64)
    cumulative = np.concatenate(([0.0], np.cumsum(x * x)))
    counts = np.diff(edges)
    sums = cumulative[edges[1:]] - cumulative[edges[:-1]]
    rms = np.sqrt(np.divide(sums, counts, out=np.zeros(points), where=counts > 0))
    loudest = float(rms.max())
    if loudest <= 0:
        return [0] * points
    return [int(v) for v in np.round(rms / loudest * 1000).clip(0, 1000)]


def analyze(path: Path, points: int = WAVEFORM_POINTS) -> Analysis:
    """Один декодинг файла: ветка ebur128 (громкость и истинный пик) и ветка PCM для огибающей."""
    graph = (
        "[0:a:0]asplit=2[m][w];"
        "[m]ebur128=peak=true:framelog=quiet[mo];"
        f"[w]aresample={WAVEFORM_RATE},aformat=sample_fmts=s16:channel_layouts=mono[wo]"
    )
    cmd = [
        "ffmpeg",
        "-nostdin",
        "-hide_banner",
        "-nostats",
        "-i",
        str(path),
        "-filter_complex",
        graph,
        "-map",
        "[mo]",
        "-f",
        "null",
        "-",
        "-map",
        "[wo]",
        "-f",
        "s16le",
        "pipe:1",
    ]
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=300, check=False)
    except (OSError, subprocess.TimeoutExpired) as e:
        raise AnalyzeError(f"ffmpeg не запустился: {e}") from e
    log = proc.stderr.decode("utf-8", errors="replace")
    if proc.returncode != 0:
        raise AnalyzeError(f"ffmpeg: {log.strip()[-300:]}")
    summary = log[log.rfind("Summary:") :] if "Summary:" in log else log
    return Analysis(
        loudness_lufs=_number(_LOUDNESS, summary),
        true_peak_db=_number(_PEAK, summary),
        waveform=envelope(np.frombuffer(proc.stdout, dtype="<i2"), points),
    )
