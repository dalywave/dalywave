"""Сетка долей и сильных долей: Beat This! (CPJKU, final0) в ONNX.

Всё вокруг сети — копия эталона `beat_this` 1.1 (`preprocessing.LogMelSpect`,
`inference.split_piece`/`aggregate_prediction`, `Postprocessor(type="minimal")`):

- 22 050 Гц, моно — среднее каналов, пересэмплер soxr;
- STFT: периодический Ханн 1024, шаг 441 (50 кадров/с), отражение краёв, модуль делится на
  √1024 (`normalized="frame_length"`); фильтры — `mel-filterbank.bin` (torchaudio, Слэни,
  30–11 000 Гц, 128 полос); `log1p(1000·x)`;
- куски по 1500 кадров (30 с) с полями по 6 кадров, последний сдвинут к концу трека, в
  перекрытиях побеждает более ранний кусок; сеть получает один кусок за вызов;
- пики: максимум в окне ±3 кадра и логит > 0, соседние пики сливаются в среднее, сильные
  доли притягиваются к ближайшей доле.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import numpy as np
import onnxruntime as ort

SAMPLE_RATE = 22050
N_FFT = 1024
HOP = 441
FPS = 50.0
CHUNK = 1500
BORDER = 6

WINDOW = np.hanning(N_FFT + 1)[:-1].astype(np.float32)


@dataclass
class BeatGrid:
    beats: np.ndarray  # секунды
    downbeats: np.ndarray  # секунды
    beat_logits: np.ndarray  # по кадрам 50 Гц
    downbeat_logits: np.ndarray


def stft_magnitude(signal: np.ndarray) -> np.ndarray:
    """|STFT| [кадры, 513] без нормировки; кадров `1 + len // 441`, как у torch.stft(center)."""
    x = np.asarray(signal, dtype=np.float32)
    if len(x) < N_FFT // 2 + 1:
        x = np.pad(x, (0, N_FFT // 2 + 1 - len(x)))
    x = np.pad(x, (N_FFT // 2, N_FFT // 2), mode="reflect")
    frames = np.lib.stride_tricks.sliding_window_view(x, N_FFT)[::HOP]
    out = np.empty((len(frames), N_FFT // 2 + 1), dtype=np.float32)
    for start in range(0, len(frames), 4096):
        chunk = frames[start : start + 4096] * WINDOW
        out[start : start + len(chunk)] = np.abs(np.fft.rfft(chunk, axis=1))
    return out


def log_mel_from_magnitude(magnitude: np.ndarray, filterbank: np.ndarray) -> np.ndarray:
    mel = (magnitude / np.float32(np.sqrt(N_FFT))) @ filterbank
    return np.log1p(1000.0 * mel)


def log_mel(signal: np.ndarray, filterbank: np.ndarray) -> np.ndarray:
    """Вход сети [кадры, 128]."""
    return log_mel_from_magnitude(stft_magnitude(signal), filterbank)


def split_piece(n_frames: int) -> list[tuple[int, int, int]]:
    """Куски как `split_piece(avoid_short_end=True)`: (начало, поле слева, поле справа)."""
    starts = list(range(-BORDER, n_frames - BORDER, CHUNK - 2 * BORDER))
    if n_frames > CHUNK - 2 * BORDER:
        starts[-1] = n_frames - (CHUNK - BORDER)
    return [(s, max(0, -s), max(0, min(BORDER, s + CHUNK - n_frames))) for s in starts]


def peaks(logits: np.ndarray) -> np.ndarray:
    """Кадры пиков: локальный максимум ±3 и логит > 0, соседние кадры сливаются в среднее."""
    if len(logits) == 0:
        return np.zeros(0)
    padded = np.pad(logits, 3, constant_values=-np.inf)
    local_max = np.lib.stride_tricks.sliding_window_view(padded, 7).max(axis=1)
    frames = np.nonzero((logits == local_max) & (logits > 0))[0]
    if len(frames) == 0:
        return np.zeros(0)
    result: list[float] = []
    p = float(frames[0])
    c = 1
    for f in frames[1:]:
        if f - p <= 1:
            c += 1
            p += (f - p) / c
        else:
            result.append(p)
            p = float(f)
            c = 1
    result.append(p)
    return np.asarray(result)


class BeatThis:
    def __init__(self, model_dir: Path, options: ort.SessionOptions) -> None:
        self.session = ort.InferenceSession(
            str(model_dir / "beat_this.onnx"), options, ["CPUExecutionProvider"]
        )
        fb = np.fromfile(model_dir / "mel-filterbank.bin", dtype="<f4")
        self.filterbank = np.ascontiguousarray(fb.reshape(N_FFT // 2 + 1, -1))

    def logits(self, spect: np.ndarray, first_chunk_only: bool = False) -> tuple[np.ndarray, ...]:
        """Логиты долей и сильных долей по кадрам.

        `first_chunk_only` — для превью: 30 с превью — это 1500 кадров, эталон режет их на два
        куска, второй добавил бы лишь последние 0,12 с. Для темпа хватает первого.
        """
        n = len(spect)
        beat = np.full(n, -1000.0, dtype=np.float32)
        down = np.full(n, -1000.0, dtype=np.float32)
        pieces = split_piece(n)
        if first_chunk_only:
            pieces = pieces[:1]
        # Сначала поздние куски, затем ранние: в перекрытии остаётся ранний (keep_first).
        for start, pad_left, pad_right in reversed(pieces):
            chunk = spect[max(start, 0) : min(start + CHUNK, n)]
            if pad_left or pad_right:
                chunk = np.pad(chunk, ((pad_left, pad_right), (0, 0)))
            b, d = self.session.run(None, {"spect": chunk[None].astype(np.float32)})
            b, d = b[0][BORDER:-BORDER], d[0][BORDER:-BORDER]
            lo = start + BORDER
            hi = min(start + CHUNK - BORDER, lo + len(b), n)
            beat[lo:hi] = b[: hi - lo]
            down[lo:hi] = d[: hi - lo]
        return beat, down

    def track(self, spect: np.ndarray, first_chunk_only: bool = False) -> BeatGrid:
        beat_logits, down_logits = self.logits(spect, first_chunk_only)
        beats = peaks(beat_logits) / FPS
        downbeats = peaks(down_logits) / FPS
        if len(beats) and len(downbeats):
            idx = np.abs(beats[None, :] - downbeats[:, None]).argmin(axis=1)
            downbeats = np.unique(beats[idx])
        return BeatGrid(beats, downbeats, beat_logits, down_logits)
