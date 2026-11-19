"""Клип целиком в такт с треком: где в треке идёт каждый момент клипа.

Звук клипа и звук трека сводятся к огибающей начала нот (onset envelope, 100 кадров в секунду):
она одинакова у одной записи при разной громкости, сжатии и эквализации. Клип режется на окна по
20 секунд, каждое ищется в треке взаимной корреляцией. Окна вступления, сценок и тишины в треке не
находятся — они отбрасываются, по остальным строится прямая «время клипа → время трека»:
`track_ms = offset_ms + video_ms * rate`. Наклон прямой — растяжение (клип чуть быстрее или
медленнее трека). Сценка посреди клипа или вырезанный куплет сдвигают остаток: тогда прямых
несколько — по куску на каждую (`segments`). Мало согласных окон — клип с треком не сопоставлен.
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from typing import Any

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
# Сколько мест трека рассматривать для одного окна клипа (припев повторяется) и насколько они
# должны отстоять друг от друга
PEAKS = 3
PEAK_GAP_S = 2.0
# Кусков (прямых) не больше стольких; у дополнительного — хотя бы столько согласных окон
MAX_SEGMENTS = 4
SEGMENT_WINDOWS = 3
# Нахлёст кусков по времени клипа или трека, который ещё допустим (окно на стыке)
OVERLAP_S = 4.0
MAX_AUDIO_S = 900
DECODE_TIMEOUT_S = 240


@dataclass(frozen=True)
class Segment:
    """Кусок клипа [video_start_ms, video_end_ms) на своей прямой."""

    video_start_ms: int
    video_end_ms: int
    offset_ms: int
    rate: float

    def to_json(self) -> dict[str, float | int]:
        return {
            "videoStartMs": self.video_start_ms,
            "videoEndMs": self.video_end_ms,
            "offsetMs": self.offset_ms,
            "rate": round(self.rate, 5),
        }


@dataclass(frozen=True)
class Alignment:
    # Главная прямая — кусок с большинством окон
    offset_ms: int
    rate: float
    # Доля окон клипа, согласных с прямыми, и их число
    share: float
    windows: int
    segments: tuple[Segment, ...] = ()

    def to_json(self) -> dict[str, Any]:
        out: dict[str, Any] = {"offsetMs": self.offset_ms, "rate": round(self.rate, 5)}
        if len(self.segments) > 1:
            out["segments"] = [x.to_json() for x in self.segments]
        return out


def decode_audio(
    source: str,
    headers: dict[str, str] | None = None,
    cookies: str | None = None,
    env: dict[str, str] | None = None,
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
    proc = subprocess.run(cmd, capture_output=True, timeout=DECODE_TIMEOUT_S, env=env, check=False)
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


def _locate(track: np.ndarray, piece: np.ndarray) -> list[tuple[int, float]]:
    """Где кусок клипа может лежать в треке: до `PEAKS` лучших мест (кадр, пик над фоном в σ).
    Припев повторяется — одно и то же окно клипа похоже на несколько мест трека, и самое сильное
    совпадение не обязательно верное: выбирает прямая по всем окнам."""
    if len(piece) < FPS * 5 or len(track) < len(piece) or piece.std() == 0:
        return []
    corr = _xcorr(track, piece - piece.mean())
    std = corr.std() if len(corr) else 0.0
    if std == 0:
        return []
    z = (corr - corr.mean()) / std
    found: list[tuple[int, float]] = []
    gap = int(PEAK_GAP_S * FPS)
    for at in np.argsort(z)[::-1]:
        if z[at] < MIN_PEAK or len(found) >= PEAKS:
            break
        if all(abs(int(at) - f) >= gap for f, _ in found):
            found.append((int(at), float(z[at])))
    return found


def _smooth(env: np.ndarray) -> np.ndarray:
    k = np.exp(-0.5 * (np.arange(-3 * SMOOTH, 3 * SMOOTH + 1) / SMOOTH) ** 2)
    out = np.convolve(env.astype(np.float64), k / k.sum(), mode="same")
    return out - out.mean()


def align_envelopes(
    track: np.ndarray, video: np.ndarray, diag: dict[str, Any] | None = None
) -> Alignment | None:
    """Прямая «кадр клипа → кадр трека» по окнам клипа; `None` — не сопоставлено. В `diag` —
    что нашлось по окнам (для журнала задачи): где окно клипа легло в трек и насколько уверенно."""
    if diag is None:
        diag = {}
    track = _smooth(track)
    video = _smooth(video)
    win = int(WINDOW_S * FPS)
    step = int(STEP_S * FPS)
    if len(video) < win:
        win = len(video)
        step = win
    # Кандидаты: (номер окна, кадр клипа, кадр трека, пик)
    points: list[tuple[int, float, float, float]] = []
    total = 0
    for n, start in enumerate(range(0, max(1, len(video) - win + 1), step)):
        total += 1
        found = _locate(track, video[start : start + win])
        if found:
            at, peak = found[0]
            diag.setdefault("windows", []).append(
                [round((start + win / 2) / FPS, 1), round((at + win / 2) / FPS, 1), round(peak, 1)]
            )
        for at, peak in found:
            # Середина окна в клипе → середина найденного места в треке
            points.append((n, start + win / 2, at + win / 2, peak))
    windows_found = len({p[0] for p in points})
    diag["total"] = total
    diag["matched"] = windows_found
    if windows_found < MIN_WINDOWS or total == 0:
        return None
    ids = np.array([p[0] for p in points])
    xs = np.array([p[1] for p in points])
    ys = np.array([p[2] for p in points])
    tol = TOLERANCE_S * FPS

    def agreeing(offset: float, rate: float, allowed: np.ndarray) -> np.ndarray:
        """Согласные с прямой кандидаты — не больше одного на окно (ближайший к прямой)."""
        err = np.where(allowed, np.abs(ys - (offset + rate * xs)), np.inf)
        keep = np.zeros(len(points), dtype=bool)
        for w in np.unique(ids[err <= tol]):
            idx = np.where((ids == w) & (err <= tol))[0]
            keep[idx[np.argmin(err[idx])]] = True
        return keep

    def fit(allowed: np.ndarray) -> tuple[float, float, np.ndarray] | None:
        """Прямая по кандидатам `allowed`, с которой согласно больше всего окон."""
        best: tuple[int, float, float] | None = None
        cand = np.where(allowed)[0]
        # Перебор пар кандидатов (их немного)
        for a, i in enumerate(cand):
            for j in cand[a:]:
                if i == j:
                    rate = 1.0
                elif xs[j] == xs[i]:
                    continue
                else:
                    rate = float((ys[j] - ys[i]) / (xs[j] - xs[i]))
                if abs(rate - 1.0) > MAX_STRETCH:
                    continue
                offset = float(ys[i] - rate * xs[i])
                err = np.abs(ys[cand] - (offset + rate * xs[cand]))
                inliers = len(np.unique(ids[cand][err <= tol]))
                if best is None or inliers > best[0]:
                    best = (inliers, offset, rate)
        if best is None:
            return None
        _, offset, rate = best
        mask = agreeing(offset, rate, allowed)
        # Уточнение по согласным окнам (наименьшие квадраты)
        if mask.sum() >= 2 and np.ptp(xs[mask]) > 0:
            r, o = (float(v) for v in np.polyfit(xs[mask], ys[mask], 1))
            if abs(r - 1.0) <= MAX_STRETCH:
                rate, offset = r, o
                mask = agreeing(offset, rate, allowed)
        return offset, rate, mask

    def longest_run(mask: np.ndarray) -> np.ndarray:
        """Самый длинный подряд идущий ряд окон прямой (пропуск в одно окно не рвёт ряд)."""
        wins = sorted(set(ids[mask].tolist()))
        best: list[int] = []
        run: list[int] = []
        for w in wins:
            if run and w - run[-1] > 2:
                run = []
            run.append(w)
            if len(run) > len(best):
                best = list(run)
        return mask & np.isin(ids, best)

    def spans(mask: np.ndarray) -> tuple[float, float, float, float]:
        return (
            float(xs[mask].min()),
            float(xs[mask].max()),
            float(ys[mask].min()),
            float(ys[mask].max()),
        )

    def overlaps(
        a: tuple[float, float, float, float], b: tuple[float, float, float, float]
    ) -> bool:
        """Куски не делят ни время клипа, ни время трека: иначе позиция трека двусмысленна
        (повтор припева в клипе, которого нет в треке, — не кусок, а совпадение). Окно на стыке
        ложится на обе стороны — нахлёст до `OVERLAP_S` не в счёт."""
        allow = OVERLAP_S * FPS
        video = min(a[1], b[1]) - max(a[0], b[0])
        track = min(a[3], b[3]) - max(a[2], b[2])
        return video > allow or track > allow

    lines: list[tuple[float, float, np.ndarray]] = []
    allowed = np.ones(len(points), dtype=bool)
    for _ in range(MAX_SEGMENTS * 3):
        if len(lines) >= MAX_SEGMENTS:
            break
        found = fit(allowed)
        if found is None:
            break
        offset, rate, mask = found
        if lines:
            mask = longest_run(mask)
        need = MIN_WINDOWS if not lines else SEGMENT_WINDOWS
        if len(set(ids[mask].tolist())) < need:
            break
        # Окна этой прямой больше не участвуют — и когда она принята, и когда отвергнута
        allowed &= ~np.isin(ids, ids[mask])
        if lines and any(overlaps(spans(mask), spans(m)) for _, _, m in lines):
            continue
        lines.append((offset, rate, mask))
    used = sum(int(m.sum()) for _, _, m in lines)
    diag["inliers"] = used
    diag["lines"] = [[round(o * 1000 / FPS), round(r, 5), int(m.sum())] for o, r, m in lines]
    if not lines or used < MIN_WINDOWS or used / total < MIN_SHARE:
        return None

    # Куски клипа: от первого до последнего окна каждой прямой; на стыке — посередине между окнами
    spans = sorted(
        [(float(xs[m].min()), float(xs[m].max()), o, r) for o, r, m in lines], key=lambda x: x[0]
    )
    segments: list[Segment] = []
    for k, (first, last, offset, rate) in enumerate(spans):
        # Края клипа — к первому и последнему куску: до первого окна и после последнего видео то же
        start = 0.0 if k == 0 else (spans[k - 1][1] + first) / 2
        end = float(len(video)) if k == len(spans) - 1 else (last + spans[k + 1][0]) / 2
        segments.append(
            Segment(
                video_start_ms=max(0, round(start * 1000 / FPS)),
                video_end_ms=round(end * 1000 / FPS),
                offset_ms=round(offset * 1000 / FPS),
                rate=rate,
            )
        )
    main_offset, main_rate, _ = max(lines, key=lambda line: int(line[2].sum()))
    return Alignment(
        offset_ms=round(main_offset * 1000 / FPS),
        rate=main_rate,
        share=float(used / total),
        windows=used,
        segments=tuple(segments),
    )


def align(
    track_audio: np.ndarray, video_audio: np.ndarray, diag: dict[str, Any] | None = None
) -> Alignment | None:
    return align_envelopes(envelope(track_audio), envelope(video_audio), diag)
