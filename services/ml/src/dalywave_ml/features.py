"""Производные признаки: темп, громкость и энергия по секундам, вокал, вступление и концовка.

Всё здесь — правила поверх выходов моделей, без своих весов, поэтому покрыто юнит-тестами.
"""

from __future__ import annotations

import itertools
from dataclasses import dataclass

import numpy as np

from .beats import FPS, N_FFT, SAMPLE_RATE, WINDOW

# --- Темп ---

MIN_BPM = 60.0
MAX_BPM = 200.0


@dataclass
class Tempo:
    bpm: float | None
    confidence: float
    beats: np.ndarray
    downbeats: np.ndarray
    fold: str | None = None  # "half" — взяли каждую вторую долю, "double" — вставили середины


def beats_per_bar(beats: np.ndarray, downbeats: np.ndarray) -> float | None:
    """Медиана числа долей между соседними сильными долями."""
    if len(downbeats) < 3:
        return None
    counts = [
        int(((beats >= a - 1e-6) & (beats < b - 1e-6)).sum())
        for a, b in itertools.pairwise(downbeats)
    ]
    counts = [c for c in counts if c > 0]
    return float(np.median(counts)) if counts else None


def _halve(beats: np.ndarray, downbeats: np.ndarray) -> np.ndarray:
    """Каждая вторая доля; чётность выбирается так, чтобы сохранить больше сильных долей."""
    best = beats[0::2]
    if len(downbeats):
        down = set(np.round(downbeats, 4))
        hits = [sum(round(b, 4) in down for b in beats[p::2]) for p in (0, 1)]
        best = beats[int(np.argmax(hits)) :: 2]
    return best


def _double(beats: np.ndarray, period: float) -> np.ndarray:
    mids = [(a + b) / 2 for a, b in itertools.pairwise(beats) if b - a < 1.5 * period]
    return np.sort(np.concatenate([beats, np.asarray(mids)]))


def estimate_tempo(
    beats: np.ndarray,
    downbeats: np.ndarray,
    duration_s: float,
    beat_strength: float | None = None,
) -> Tempo:
    """BPM по сетке долей и уверенность 0..1.

    BPM = 60 / среднее межударных интервалов, близких к медиане (±7 %): средний интервал
    по длинной ровной сетке точнее, чем шаг кадров сети (20 мс). Защита от ошибки в два раза:
    вне 60–200 BPM или при 7+ долях в такте темп складывается вдвое, и сетка долей
    прореживается или достраивается вместе с ним, чтобы BPM и сетка не расходились.
    """
    beats = np.asarray(beats, dtype=np.float64)
    downbeats = np.asarray(downbeats, dtype=np.float64)
    if len(beats) < 8:
        return Tempo(None, 0.0, beats, downbeats)
    ibi = np.diff(beats)
    period = float(np.median(ibi))
    bpm = 60.0 / period
    fold = None
    bpb = beats_per_bar(beats, downbeats)
    if bpm > MAX_BPM or (bpb is not None and bpb >= 7 and bpm > 2 * MIN_BPM):
        beats, fold = _halve(beats, downbeats), "half"
    elif bpm < MIN_BPM:
        beats, fold = _double(beats, period), "double"
    if fold:
        ibi = np.diff(beats)
        period = float(np.median(ibi))
        downbeats = downbeats[np.isin(np.round(downbeats, 4), np.round(beats, 4))]

    inliers = ibi[np.abs(ibi - period) <= 0.07 * period]
    regularity = len(inliers) / len(ibi)
    mean = float(inliers.mean()) if len(inliers) else period
    bpm = 60.0 / mean
    # Разброс интервалов сверх шага кадров (20 мс дают ±10 мс сами по себе).
    jitter = max(0.0, float(inliers.std()) - 0.5 / FPS) if len(inliers) > 1 else period
    stability = float(np.clip(1.0 - jitter / (0.05 * mean), 0.0, 1.0))
    coverage = float(np.clip((beats[-1] - beats[0]) / max(duration_s, 1e-6) / 0.6, 0.0, 1.0))
    strength = 1.0 if beat_strength is None else float(np.clip((beat_strength - 0.5) / 0.4, 0, 1))
    confidence = (
        regularity * (0.6 + 0.4 * stability) * (0.7 + 0.3 * coverage) * (0.6 + 0.4 * strength)
    )
    return Tempo(round(bpm, 2), round(float(confidence), 3), beats, downbeats, fold)


# --- Громкость и энергия ---

# K-фильтр ITU-R BS.1770 (полка +4 дБ и срез низа ~38 Гц), коэффициенты для 48 кГц.
_K_STAGES = (
    (
        (1.53512485958697, -2.69169618940638, 1.19839281085285),
        (1.0, -1.69065929318241, 0.73248077421585),
    ),
    ((1.0, -2.0, 1.0), (1.0, -1.99004745483398, 0.99007225036621)),
)


def k_weighting(freqs_hz: np.ndarray) -> np.ndarray:
    """|H(f)|² K-фильтра; частоты до 11 кГц лежат далеко от Найквиста 48 кГц."""
    z = np.exp(-2j * np.pi * np.asarray(freqs_hz) / 48000.0)
    h = np.ones_like(z)
    for b, a in _K_STAGES:
        h *= (b[0] + b[1] * z + b[2] * z**2) / (a[0] + a[1] * z + a[2] * z**2)
    return np.abs(h) ** 2


_FREQS = np.arange(N_FFT // 2 + 1) * SAMPLE_RATE / N_FFT
_ONE_SIDED = np.full(N_FFT // 2 + 1, 2.0)
_ONE_SIDED[[0, -1]] = 1.0
# Парсеваль: средний квадрат сигнала в кадре = Σ c·|X|² / (N · Σw²).
_K_GAIN = (
    k_weighting(_FREQS) * _ONE_SIDED / (N_FFT * float((WINDOW.astype(np.float64) ** 2).sum()))
).astype(np.float32)

LOUD_FLOOR_LUFS = -40.0
LOUD_TOP_LUFS = -5.0
FLUX_REF = 0.35


def per_second(values: np.ndarray, fps: float = FPS) -> np.ndarray:
    """Среднее по целым секундам; хвост короче полсекунды отбрасывается."""
    step = int(fps)
    n = len(values) // step
    if len(values) - n * step >= step // 2:
        n += 1
    out = np.empty(n, dtype=np.float64)
    for i in range(n):
        out[i] = float(np.mean(values[i * step : (i + 1) * step]))
    return out


def loudness_per_second(magnitude: np.ndarray) -> np.ndarray:
    """Громкость по K-фильтру, LUFS по секундам (моно-сумма, без стробирования)."""
    power = (magnitude.astype(np.float32) ** 2) @ _K_GAIN
    return -0.691 + 10.0 * np.log10(per_second(power) + 1e-12)


def flux_per_second(log_mel: np.ndarray) -> np.ndarray:
    """Спектральный поток (рост лог-мел полос) по секундам — насколько «много всего происходит»."""
    if len(log_mel) < 2:
        return np.zeros(max(1, len(log_mel) // int(FPS)))
    diff = np.maximum(0.0, np.diff(log_mel, axis=0)).mean(axis=1)
    return per_second(np.concatenate([[0.0], diff]))


def energy_components(loud_lufs: np.ndarray, log_mel: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Громкость и активность по секундам, обе в 0..1."""
    loud01 = np.clip((loud_lufs - LOUD_FLOOR_LUFS) / (LOUD_TOP_LUFS - LOUD_FLOOR_LUFS), 0.0, 1.0)
    flux01 = np.clip(flux_per_second(log_mel) / FLUX_REF, 0.0, 1.0)
    n = min(len(loud01), len(flux01))
    return loud01[:n], flux01[:n]


def energy_curve(loud01: np.ndarray, flux01: np.ndarray) -> np.ndarray:
    """Кривая энергии по секундам 0..1: в основном громкость, четверть — активность."""
    return np.clip(0.75 * loud01 + 0.25 * flux01, 0.0, 1.0)


def track_energy(curve: np.ndarray, moods: dict[str, float]) -> float:
    """Энергия трека 0..1: «тело» кривой (верхняя половина секунд) и настроение по моделям.

    Верхняя половина, а не среднее: тихое вступление не делает трек спокойным, а превью
    (обычно припев) и полный трек дают близкие значения.
    """
    if len(curve):
        top = np.sort(curve)[len(curve) // 2 :]
        body = float(top.mean())
    else:
        body = 0.0
    mood = (
        0.4 * moods.get("aggressive", 0.0)
        + 0.3 * moods.get("party", 0.0)
        + 0.3 * (1.0 - moods.get("relaxed", 0.0))
    )
    return round(float(np.clip(0.6 * body + 0.4 * mood, 0.0, 1.0)), 3)


def valence(moods: dict[str, float]) -> float:
    """Позитивность 0..1 из голов настроения: весёлое против грустного, вечеринка против агрессии.

    Своих голов валентности у Discogs-EffNet нет (DEAM/emoMusic есть только для других
    эмбеддингов), поэтому это правило, а не модель.
    """
    v = (
        0.5
        + 0.45 * (moods.get("happy", 0.0) - moods.get("sad", 0.0))
        + 0.15 * (moods.get("party", 0.0) - moods.get("aggressive", 0.0))
    )
    return round(float(np.clip(v, 0.0, 1.0)), 3)


# --- Вокал ---


def _median3(x: np.ndarray) -> np.ndarray:
    if len(x) < 3:
        return x.copy()
    padded = np.concatenate([[x[0]], x, [x[-1]]])
    return np.median(np.lib.stride_tricks.sliding_window_view(padded, 3), axis=1)


def vocal_segments(
    voice: np.ndarray,
    centers: np.ndarray,
    duration_s: float,
    *,
    enter: float = 0.6,
    leave: float = 0.4,
    min_len_s: float = 2.0,
    max_gap_s: float = 2.5,
) -> list[tuple[float, float]]:
    """Участки с голосом [(начало, конец)] в секундах по вероятности голоса в патчах (~1 с).

    Гистерезис (вход ≥ 0.6, выход < 0.4) не даёт сегментам дрожать; паузы короче 2,5 с
    склеиваются (вдох между фразами), участки короче 2 с выбрасываются.
    """
    if len(voice) == 0:
        return []
    p = _median3(np.asarray(voice, dtype=np.float64))
    half = (centers[1] - centers[0]) / 2 if len(centers) > 1 else 0.5
    segments: list[list[float]] = []
    on = False
    for i, (prob, c) in enumerate(zip(p, centers, strict=True)):
        on = prob >= leave if on else prob >= enter
        if not on:
            continue
        # Крайние патчи покрывают и края трека: первый начинается с нуля, последний — у конца.
        start = 0.0 if i == 0 else max(0.0, c - half)
        end = duration_s if i == len(p) - 1 else min(duration_s, c + half)
        if segments and start - segments[-1][1] <= max_gap_s:
            segments[-1][1] = end
        else:
            segments.append([start, end])
    return [(a, b) for a, b in segments if b - a >= min_len_s]


# --- Вступление и концовка ---


def _smooth(x: np.ndarray, width: int) -> np.ndarray:
    if len(x) < width:
        return x.copy()
    kernel = np.ones(width) / width
    padded = np.pad(x, (width // 2, width - 1 - width // 2), mode="edge")
    return np.convolve(padded, kernel, mode="valid")


def _snap(t: float, grid: np.ndarray, direction: int, tolerance: float = 4.0) -> float:
    """К ближайшей сильной доле не позже (direction=-1) или не раньше (+1) момента t."""
    if len(grid) == 0:
        return t
    cand = grid[grid <= t + 0.05] if direction < 0 else grid[grid >= t - 0.05]
    if len(cand) == 0:
        return t
    best = float(cand[-1] if direction < 0 else cand[0])
    return best if abs(best - t) <= tolerance else t


def intro_outro(
    curve: np.ndarray,
    vocals: list[tuple[float, float]],
    downbeats: np.ndarray,
    duration_s: float,
    *,
    sustain_s: int = 8,
    outro_grace_s: float = 8.0,
) -> tuple[float, float]:
    """Конец вступления и начало концовки в секундах.

    «Тело» трека — где сглаженная энергия держится не ниже 85 % от типичного уровня
    (70-й процентиль) хотя бы 8 секунд подряд. Вступление кончается, где тело начинается
    или раньше, если раньше вступает голос; концовка начинается, где тело кончается
    (если на этом месте звучит фраза, которая кончается в пределах 8 с, — после неё).
    Обе границы притягиваются к сильной доле — так переход ляжет на начало такта.
    """
    n = len(curve)
    if n == 0 or duration_s <= 0:
        return 0.0, duration_s
    e = _smooth(np.asarray(curve, dtype=np.float64), 5)
    threshold = 0.85 * float(np.percentile(e, 70))
    above = e >= threshold
    runs: list[tuple[int, int]] = []
    start = None
    for i, a in enumerate(above):
        if a and start is None:
            start = i
        if not a and start is not None:
            runs.append((start, i))
            start = None
    if start is not None:
        runs.append((start, n))
    long_runs = [r for r in runs if r[1] - r[0] >= min(sustain_s, max(1, n // 4))] or runs
    body_start = float(long_runs[0][0]) if long_runs else 0.0
    body_end = float(long_runs[-1][1]) if long_runs else float(n)

    intro = body_start
    if vocals:
        intro = min(intro, vocals[0][0])
    intro = min(intro, 0.4 * duration_s)
    outro = body_end
    # Фразу, которая ещё звучит на спаде, не обрываем; но если голос тянется до самого
    # конца (угасающий припев), концовкой остаётся спад энергии.
    for start_v, end_v in vocals:
        if start_v < outro < end_v <= outro + outro_grace_s:
            outro = end_v
    outro = max(outro, 0.6 * duration_s)

    intro = _snap(intro, downbeats, -1)
    outro = _snap(outro, downbeats, +1)
    intro = float(np.clip(intro, 0.0, duration_s))
    outro = float(np.clip(outro, intro, duration_s))
    return intro, outro
