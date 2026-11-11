"""Тональность: S-KEY (Deezer) в ONNX и перевод в нотацию Камелота."""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import onnxruntime as ort

SAMPLE_RATE = 22050
# Короче ~2,95 с свёртки ChromaNet по времени не на что опереться — граф падает.
MIN_SAMPLES = 3 * SAMPLE_RATE

_PITCH = {
    "C": 0,
    "C#": 1,
    "Db": 1,
    "D": 2,
    "D#": 3,
    "Eb": 3,
    "E": 4,
    "F": 5,
    "F#": 6,
    "Gb": 6,
    "G": 7,
    "G#": 8,
    "Ab": 8,
    "A": 9,
    "A#": 10,
    "Bb": 10,
    "B": 11,
}


def camelot(name: str) -> str:
    """«A Major» → «11B», «A minor» → «8A». Минор считается от параллельного мажора (+3)."""
    root, mode = name.split()
    pc = _PITCH[root]
    minor = mode.lower() == "minor"
    major_pc = (pc + 3) % 12 if minor else pc
    number = ((7 * major_pc) % 12 + 7) % 12 + 1
    return f"{number}{'A' if minor else 'B'}"


def undo_outer_softmax(probs: np.ndarray) -> np.ndarray:
    """Граф S-KEY применяет softmax к выходу ChromaNet, который уже вероятности.

    Второй softmax прижимает все 24 класса к 1/24, и уверенность теряет смысл. Он обратим:
    log(q) = p − LSE(p) и Σp = 1, поэтому p = log(q) + (1 − Σ log q) / 24.
    """
    logq = np.log(np.maximum(probs.astype(np.float64), 1e-30))
    p = logq + (1.0 - logq.sum()) / len(logq)
    p = np.clip(p, 0.0, None)
    return p / p.sum()


class SKey:
    def __init__(self, model_dir: Path, options: ort.SessionOptions) -> None:
        self.session = ort.InferenceSession(
            str(model_dir / "skey.onnx"), options, ["CPUExecutionProvider"]
        )
        config = json.loads((model_dir / "config.json").read_text())
        self.key_map: list[str] = config["keyMap"]

    def probabilities(self, signal22k: np.ndarray) -> np.ndarray | None:
        """Вероятности 24 тональностей; None — сигнал короче 3 с или тишина."""
        x = np.asarray(signal22k, dtype=np.float32)
        peak = float(np.abs(x).max()) if len(x) else 0.0
        if len(x) < MIN_SAMPLES or peak <= 1e-4:
            return None
        q = self.session.run(["probs"], {"audio": (x / peak)[None]})[0]
        return undo_outer_softmax(q)

    def detect(self, signal22k: np.ndarray) -> tuple[str, str, float] | tuple[None, None, None]:
        """(название, Камелот, уверенность 0..1) или три None."""
        p = self.probabilities(signal22k)
        if p is None:
            return None, None, None
        i = int(np.argmax(p))
        name = self.key_map[i]
        return name, camelot(name), float(p[i])
