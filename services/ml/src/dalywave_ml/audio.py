"""Декодирование через ffmpeg и пересэмплирование под частоты моделей."""

from __future__ import annotations

import functools
import shutil
import struct
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import soxr


class AudioError(Exception):
    """Файл не читается или в нём нет звука."""


@dataclass
class Audio:
    """Моно-сигнал в исходной частоте и кэш пересэмплированных копий."""

    samples: np.ndarray
    sample_rate: int
    channels: int
    codec: str | None = None
    truncated: bool = False
    _cache: dict[int, np.ndarray] = field(default_factory=dict, repr=False)

    @property
    def duration_s(self) -> float:
        return len(self.samples) / self.sample_rate

    def at(self, rate: int) -> np.ndarray:
        """Сигнал в нужной частоте. soxr HQ — тот же пересэмплер, что у Beat This!."""
        if rate == self.sample_rate:
            return self.samples
        if rate not in self._cache:
            out = soxr.resample(self.samples, self.sample_rate, rate, quality="HQ")
            self._cache[rate] = np.ascontiguousarray(out, dtype=np.float32)
        return self._cache[rate]


def ffmpeg_path() -> str:
    path = shutil.which("ffmpeg")
    if path is None:
        raise AudioError("ffmpeg не найден в PATH")
    return path


@functools.cache
def ffmpeg_version() -> str | None:
    try:
        out = subprocess.run(
            [ffmpeg_path(), "-hide_banner", "-version"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        ).stdout
    except (AudioError, OSError, subprocess.TimeoutExpired):
        return None
    first = out.splitlines()[0] if out else ""
    parts = first.split()
    return parts[2] if len(parts) > 2 else first or None


def _parse_wav(data: bytes) -> tuple[np.ndarray, int, int]:
    """WAV из ffmpeg в канал: размер блока данных там не известен заранее, читаем до конца."""
    if len(data) < 12 or data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise AudioError("ffmpeg вернул не WAV")
    pos = 12
    channels = rate = bits = fmt = None
    while pos + 8 <= len(data):
        tag = data[pos : pos + 4]
        size = struct.unpack_from("<I", data, pos + 4)[0]
        body = pos + 8
        if tag == b"fmt ":
            fmt, channels, rate = struct.unpack_from("<HHI", data, body)
            bits = struct.unpack_from("<H", data, body + 14)[0]
            if fmt == 0xFFFE:  # WAVE_FORMAT_EXTENSIBLE: настоящий формат — в GUID
                fmt = struct.unpack_from("<H", data, body + 24)[0]
        elif tag == b"data":
            if channels is None or rate is None:
                raise AudioError("в WAV нет описания формата")
            if fmt != 3 or bits != 32:
                raise AudioError(f"неожиданный формат WAV: {fmt}/{bits}")
            available = len(data) - body
            if size not in (0, 0xFFFFFFFF) and size <= available:
                available = size
            count = available // (4 * channels) * channels
            # Без копирования: на 4 минутах стерео 48 кГц это ~90 МБ.
            pcm = np.frombuffer(data, dtype="<f4", count=count, offset=body).reshape(-1, channels)
            return pcm, rate, channels
        pos = body + size + (size & 1)
    raise AudioError("в WAV нет данных")


def _codec_name(stderr: str) -> str | None:
    for line in stderr.splitlines():
        if "Audio:" in line and "Stream #0" in line:
            return line.split("Audio:", 1)[1].strip().split()[0].rstrip(",")
    return None


def decode(path: Path, *, max_seconds: float | None = None, timeout_s: float = 120.0) -> Audio:
    """Первая аудиодорожка файла → моно float32 в исходной частоте.

    Моно — среднее каналов: так сводят Beat This! и Essentia (`ffmpeg -ac 1` громче на √2,
    а признаки Beat This! от громкости зависят нелинейно).
    """
    cmd = [ffmpeg_path(), "-nostdin", "-hide_banner", "-nostats", "-loglevel", "info"]
    cmd += ["-i", str(path)]
    if max_seconds:
        cmd += ["-t", f"{max_seconds:.3f}"]
    cmd += ["-map", "0:a:0", "-vn", "-sn", "-dn", "-c:a", "pcm_f32le", "-f", "wav", "pipe:1"]
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=timeout_s, check=False)
    except subprocess.TimeoutExpired as e:
        raise AudioError(f"ffmpeg не уложился в {timeout_s:.0f} с") from e
    stderr = proc.stderr.decode("utf-8", errors="replace")
    if proc.returncode != 0:
        tail = " ".join(stderr.strip().splitlines()[-2:])[-300:]
        raise AudioError(f"ffmpeg не смог прочитать файл: {tail}")
    pcm, rate, channels = _parse_wav(proc.stdout)
    if len(pcm) == 0:
        raise AudioError("в файле нет звука")
    if channels == 2:
        mono = pcm[:, 0] + pcm[:, 1]
        mono *= np.float32(0.5)
    elif channels > 2:
        mono = pcm.mean(axis=1, dtype=np.float32)
    else:
        mono = pcm[:, 0].copy()
    if not np.isfinite(mono).all():
        mono = np.nan_to_num(mono, nan=0.0, posinf=0.0, neginf=0.0)
    truncated = bool(max_seconds) and len(mono) >= int(max_seconds * rate) - rate // 100
    return Audio(
        samples=mono,
        sample_rate=rate,
        channels=channels,
        codec=_codec_name(stderr),
        truncated=truncated,
    )
