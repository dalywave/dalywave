"""Мел-полосы на входе Discogs-EffNet — точная копия Essentia.

Повторяет то, что внутри `TensorflowPredictEffnetDiscogs` делают потоковый `FrameCutter`
и `TensorflowInputMusiCNN` (Essentia 2.1b6, исходники `framecutter.cpp`, `windowing.cpp`,
`melbands.cpp`, `triangularbands.cpp`, `unaryoperator.cpp`):

- 16 кГц моно; кадр 512, шаг 256; первый кадр центрирован на нулевом отсчёте (слева 256
  нулей), последний — первый, чей центр дошёл до конца сигнала: кадров `ceil(N/256) + 1`;
- окно Ханна симметричное, `0.5 - 0.5·cos(2πi/(N-1))`, без нормировки (`normalized=false`);
  сдвиг фазы (`zeroPhase`) на модуль спектра не влияет;
- модуль БПФ → мощность (`type=power`) → 96 треугольных полос по шкале Слэни от 0 до
  8000 Гц, веса линейные, нормировка `unit_tri` (делим на теоретическую площадь
  треугольника, как librosa `norm="slaney"`);
- `log10(1 + 10000·x)`;
- патчи по 128 кадров с шагом 62 (≈1 патч в секунду), неполный последний — отбрасывается.

Essentia добавляет к полностью тихим кадрам шум −100 дБ; на выходе это ~3e-5, поэтому
здесь тишина остаётся тишиной.
"""

from __future__ import annotations

import math

import numpy as np

SAMPLE_RATE = 16000
FRAME_SIZE = 512
HOP_SIZE = 256
N_BANDS = 96
PATCH_SIZE = 128
PATCH_HOP = 62
FRAME_SECONDS = HOP_SIZE / SAMPLE_RATE


def _hz_to_mel_slaney(hz: float) -> float:
    if hz < 1000.0:
        return hz * 3.0 / 200.0
    return 15.0 + math.log(hz / 1000.0) / (math.log(6.4) / 27.0)


def _mel_to_hz_slaney(mel: float) -> float:
    if mel < 15.0:
        return mel * 200.0 / 3.0
    return 1000.0 * math.exp((mel - 15.0) * (math.log(6.4) / 27.0))


def mel_filterbank() -> np.ndarray:
    """Веса полос [96, 257], как `TriangularBands::createFilters` с `unit_tri`."""
    n_bins = FRAME_SIZE // 2 + 1
    low = _hz_to_mel_slaney(0.0)
    high = _hz_to_mel_slaney(SAMPLE_RATE / 2)
    step = (high - low) / (N_BANDS + 1)
    # В Essentia частота копится сложением шага; погрешность float32 здесь ниже 1e-4 Гц.
    edges = [_mel_to_hz_slaney(low + i * step) for i in range(N_BANDS + 2)]
    scale = (SAMPLE_RATE / 2) / (n_bins - 1)
    weights = np.zeros((N_BANDS, n_bins), dtype=np.float64)
    for i in range(N_BANDS):
        f0, f1, f2 = edges[i], edges[i + 1], edges[i + 2]
        for j in range(math.ceil(f0 / scale), math.floor(f2 / scale) + 1):
            f = j * scale
            weights[i, j] = (f - f0) / (f1 - f0) if f < f1 else (f2 - f) / (f2 - f1)
        weights[i] /= (f2 - f0) / 2.0
    return weights.astype(np.float32)


_WINDOW = (0.5 - 0.5 * np.cos(2.0 * np.pi * np.arange(FRAME_SIZE) / (FRAME_SIZE - 1))).astype(
    np.float32
)
_FILTERS_T = np.ascontiguousarray(mel_filterbank().T)


def frame_count(n_samples: int) -> int:
    return math.ceil(n_samples / HOP_SIZE) + 1 if n_samples > 0 else 0


def bands(signal: np.ndarray) -> np.ndarray:
    """Лог-мел полосы [кадры, 96] для сигнала 16 кГц."""
    signal = np.asarray(signal, dtype=np.float32)
    n_frames = frame_count(len(signal))
    if n_frames == 0:
        return np.zeros((0, N_BANDS), dtype=np.float32)
    half = FRAME_SIZE // 2
    padded = np.zeros(half + (n_frames - 1) * HOP_SIZE + half, dtype=np.float32)
    padded[half : half + len(signal)] = signal
    frames = np.lib.stride_tricks.sliding_window_view(padded, FRAME_SIZE)[::HOP_SIZE]
    out = np.empty((n_frames, N_BANDS), dtype=np.float32)
    # Кусками: на 10-минутном треке кадров ~37 тысяч, спектр целиком занял бы 150 МБ.
    for start in range(0, n_frames, 4096):
        chunk = frames[start : start + 4096] * _WINDOW
        power = np.abs(np.fft.rfft(chunk, axis=1)).astype(np.float32) ** 2
        out[start : start + len(chunk)] = power @ _FILTERS_T
    return np.log10(out * 10000.0 + 1.0)


def patch_starts(n_frames: int) -> np.ndarray:
    """Первые кадры патчей; неполный последний патч отбрасывается, как в Essentia."""
    if n_frames < PATCH_SIZE:
        return np.zeros(0, dtype=np.int64)
    return np.arange(0, n_frames - PATCH_SIZE + 1, PATCH_HOP, dtype=np.int64)


def patches(mel: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Патчи [n, 128, 96] и время центра каждого патча в секундах.

    Сигнал короче патча (≈2 с) дополняется повтором кадров — Essentia в этом случае
    вообще не даёт ответа.
    """
    n_frames = len(mel)
    if n_frames == 0:
        return np.zeros((0, PATCH_SIZE, N_BANDS), dtype=np.float32), np.zeros(0)
    if n_frames < PATCH_SIZE:
        reps = math.ceil(PATCH_SIZE / n_frames)
        mel = np.tile(mel, (reps, 1))[:PATCH_SIZE]
        return mel[None].astype(np.float32), np.array([n_frames / 2 * FRAME_SECONDS])
    starts = patch_starts(n_frames)
    view = np.lib.stride_tricks.sliding_window_view(mel, (PATCH_SIZE, N_BANDS))[:, 0]
    # Кадр k центрирован на отсчёте k·256, середина патча — между кадрами 63 и 64.
    centers = (starts + (PATCH_SIZE - 1) / 2) * FRAME_SECONDS
    return np.ascontiguousarray(view[starts]), centers
