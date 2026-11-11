"""Синтетический «полный трек» для бенчмарка и проверки вступления/концовки.

Своей полной музыки без авторских прав под рукой нет, поэтому трек собирается из 30-секундных
превью Deezer (design/fixtures/audio): тихое вступление без верхов (фильтр 300 Гц, −12 дБ),
«тело» из превью встык с кроссфейдами 2 с и угасающая концовка. Кодируется в Opus внутри
MP4 (.m4a) — как файлы, которые сервис получает в режиме full. Аудио не коммитится.

    uv run python scripts/make_full_track.py ../../design/fixtures/audio/*.mp3 \
        --seconds 240 --out bench/out/full-240s.m4a
"""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
import wave
from pathlib import Path

import numpy as np

from dalywave_ml.audio import decode

SR = 48000


def lowpass(x: np.ndarray, cutoff_hz: float) -> np.ndarray:
    spec = np.fft.rfft(x)
    freqs = np.fft.rfftfreq(len(x), 1 / SR)
    spec[freqs > cutoff_hz] = 0
    return np.fft.irfft(spec, n=len(x)).astype(np.float32)


def crossfade(a: np.ndarray, b: np.ndarray, seconds: float) -> np.ndarray:
    n = int(seconds * SR)
    ramp = np.linspace(0, 1, n, dtype=np.float32)
    mid = a[-n:] * (1 - ramp) + b[:n] * ramp
    return np.concatenate([a[:-n], mid, b[n:]])


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--seconds", type=float, default=240)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    clips = [decode(p).at(SR) for p in args.files]
    intro_s, outro_s = 16.0, 20.0
    intro = lowpass(clips[0][: int(intro_s * SR)], 300) * 10 ** (-12 / 20)
    body = clips[0]
    i = 1
    while len(body) / SR < args.seconds - intro_s - outro_s:
        body = crossfade(body, clips[i % len(clips)], 2.0)
        i += 1
    body = body[: int((args.seconds - intro_s - outro_s) * SR)]
    tail = clips[i % len(clips)][: int(outro_s * SR)].copy()
    tail = lowpass(tail, 1200) * np.linspace(10 ** (-6 / 20), 0.0, len(tail), dtype=np.float32)
    track = crossfade(crossfade(intro, body, 0.5), tail, 1.0)
    track = np.clip(track, -1, 1)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        wav = Path(tmp) / "track.wav"
        with wave.open(str(wav), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes((track * 32767).astype("<i2").tobytes())
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-loglevel",
                "error",
                "-i",
                str(wav),
                "-ac",
                "2",
                "-c:a",
                "libopus",
                "-b:a",
                "160k",
                "-f",
                "mp4",
                str(args.out),
            ],
            check=True,
        )
    truth = {
        "file": str(args.out),
        "durationS": round(len(track) / SR, 2),
        "introEndS": intro_s - 0.25,
        "outroStartS": round(len(track) / SR - outro_s + 0.5, 2),
    }
    args.out.with_suffix(".json").write_text(json.dumps(truth, indent=2) + "\n")
    print(json.dumps(truth))


if __name__ == "__main__":
    main()
