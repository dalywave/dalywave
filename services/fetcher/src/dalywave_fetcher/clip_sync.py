"""Клип целиком в такт с треком: где в треке идёт каждый момент клипа.

Звук клипа и звук трека сводятся к огибающей начала нот (onset envelope, 100 кадров в секунду):
она одинакова у одной записи при разной громкости, сжатии и эквализации. Клип режется на окна по
20 секунд, каждое ищется в треке взаимной корреляцией. Окна вступления, сценок и тишины в треке не
находятся — они отбрасываются, по остальным строится прямая «время клипа → время трека»:
`track_ms = offset_ms + video_ms * rate`. Наклон прямой — растяжение (клип чуть быстрее или
медленнее трека). Мало согласных окон — клип с треком не сопоставлен.
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass

import numpy as np

from . import tools

SAMPLE_RATE = 8000
FPS = 100
HOP = SAMPLE_RATE // FPS
WINDOW_S = 12.0
STEP_S = 6.0
# Сглаживание огибающей, кадров: растяжение клипа размазывает удары, гладкие — совпадают
SMOOTH = 4
# Окно согласно с прямой, если расходится не больше чем на столько
TOLERANCE_S = 0.15
# Растяжение больше этого — уже не тот же звук (другая версия песни)
MAX_STRETCH = 0.06
MIN_WINDOWS = 3
MIN_SHARE = 0.4
# Пик корреляции окна должен заметно выделяться над фоном
MIN_PEAK = 4.0
MAX_AUDIO_S = 900
DECODE_TIMEOUT_S = 240


@dataclass(frozen=True)
class Alignment:
    offset_ms: int
    rate: float
    # Доля окон клипа, согласных с прямой, и их число
    share: float
    windows: int

    def to_json(self) -> dict[str, float | int]:
        return {"offsetMs": self.offset_ms, "rate": round(self.rate, 5)}


def decode_audio(
    source: str, headers: dict[str, str] | None = None, cookies: str | None = None
) -> np.ndarray:
    """Звук по ссылке или файлу — моно, 8 кГц, float32 (не больше `MAX_AUDIO_S`)."""
    cmd = [tools.exe("ffmpeg"), "-nostdin", "-hide_banner", "-loglevel", "error"]
    if cookies:
        cmd += ["-cookies", cookies]
    if headers:
        cmd += ["-headers", "".join(f"{k}: {v}\r\n" for k, v in headers.items())]
    cmd += [
        "-i",
        source,
        "-t",
        str(MAX_AUDIO_S),
        "-vn",
        "-ac",
        "1",
        "-ar",
        str(SAMPLE_RATE),
        "-f",
        "f32le",
        "-",
    ]
    proc = subprocess.run(cmd, capture_output=True, timeout=DECODE_TIMEOUT_S, check=False)
    if proc.returncode != 0:
        tail = proc.stderr.decode(errors="replace").strip().splitlines()[-1:] or [""]
        raise RuntimeError(f"ffmpeg: {tail[0][:200]}")
    return np.frombuffer(proc.stdout, dtype=np.float32)


def envelope(samples: np.ndarray) -> np.ndarray:
    """Огибающая начала нот: рост логарифма энергии кадра по 10 мс, нормированный."""
    frames = len(samples) // HOP
    if frames < 2:
        return np.zeros(0, dtype=np.float32)
    x = samples[: frames * HOP].reshape(frames, HOP).astype(np.float64)
    energy = np.log1p(1000.0 * np.mean(x * x, axis=1))
    onset = np.maximum(np.diff(energy, prepend=energy[0]), 0.0)
    std = onset.std()
    return (
        ((onset - onset.mean()) / std).astype(np.float32) if std > 0 else onset.astype(np.float32)
    )


def _xcorr(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """Взаимная корреляция через БПФ: значение с индексом k — совпадение `b` со сдвигом k
    внутри `a` (k от 0 до len(a) - len(b))."""
    n = 1 << int(np.ceil(np.log2(len(a) + len(b))))
    fa = np.fft.rfft(a, n)
    fb = np.fft.rfft(b[::-1], n)
    full = np.fft.irfft(fa * fb, n)
    return full[len(b) - 1 : len(a)]


def _locate(track: np.ndarray, piece: np.ndarray) -> tuple[int, float] | None:
    """Где кусок клипа лежит в треке (кадр) и насколько уверенно (пик над фоном в σ)."""
    if len(piece) < FPS * 5 or len(track) < len(piece):
        return None
    if piece.std() == 0:
        return None
    corr = _xcorr(track, piece - piece.mean())
    if len(corr) == 0:
        return None
    best = int(np.argmax(corr))
    std = corr.std()
    if std == 0:
        return None
    return best, float((corr[best] - corr.mean()) / std)


def _smooth(env: np.ndarray) -> np.ndarray:
    k = np.exp(-0.5 * (np.arange(-3 * SMOOTH, 3 * SMOOTH + 1) / SMOOTH) ** 2)
    out = np.convolve(env.astype(np.float64), k / k.sum(), mode="same")
    return out - out.mean()


def align_envelopes(track: np.ndarray, video: np.ndarray) -> Alignment | None:
    """Прямая «кадр клипа → кадр трека» по окнам клипа; `None` — не сопоставлено."""
    track = _smooth(track)
    video = _smooth(video)
    win = int(WINDOW_S * FPS)
    step = int(STEP_S * FPS)
    if len(video) < win:
        win = len(video)
        step = win
    points: list[tuple[float, float, float]] = []
    total = 0
    for start in range(0, max(1, len(video) - win + 1), step):
        piece = video[start : start + win]
        total += 1
        found = _locate(track, piece)
        if found is None:
            continue
        at, peak = found
        if peak < MIN_PEAK:
            continue
        # Середина окна в клипе → середина найденного места в треке
        points.append((start + win / 2, at + win / 2, peak))
    if len(points) < MIN_WINDOWS or total == 0:
        return None
    xs = np.array([p[0] for p in points])
    ys = np.array([p[1] for p in points])
    tol = TOLERANCE_S * FPS
    best: tuple[int, float, float] | None = None
    # Перебор пар точек (их немного): прямая, с которой согласно больше всего окон
    for i in range(len(points)):
        for j in range(i, len(points)):
            if i == j:
                rate = 1.0
            else:
                if xs[j] == xs[i]:
                    continue
                rate = float((ys[j] - ys[i]) / (xs[j] - xs[i]))
            if abs(rate - 1.0) > MAX_STRETCH:
                continue
            offset = float(ys[i] - rate * xs[i])
            inliers = int(np.sum(np.abs(ys - (offset + rate * xs)) <= tol))
            if best is None or inliers > best[0]:
                best = (inliers, offset, rate)
    if best is None:
        return None
    _, offset, rate = best
    mask = np.abs(ys - (offset + rate * xs)) <= tol
    if mask.sum() < MIN_WINDOWS or mask.sum() / total < MIN_SHARE:
        return None
    # Уточнение по всем согласным окнам (наименьшие квадраты)
    if mask.sum() >= 2 and np.ptp(xs[mask]) > 0:
        rate, offset = (float(v) for v in np.polyfit(xs[mask], ys[mask], 1))
        if abs(rate - 1.0) > MAX_STRETCH:
            return None
    return Alignment(
        offset_ms=round(offset * 1000 / FPS),
        rate=rate,
        share=float(mask.sum() / total),
        windows=int(mask.sum()),
    )


def align(track_audio: np.ndarray, video_audio: np.ndarray) -> Alignment | None:
    return align_envelopes(envelope(track_audio), envelope(video_audio))
