"""Discogs-EffNet (MTG): эмбеддинг звучания 1280, 400 стилей Discogs и головы поверх эмбеддинга."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import onnxruntime as ort

from . import musicnn

MODEL = "discogs-effnet-bsdynamic-1"

# Ключ в ответе → (файл головы, положительный класс; None — регрессия с одним выходом).
HEADS: dict[str, tuple[str, str | None]] = {
    "happy": ("mood_happy-discogs-effnet-1", "happy"),
    "sad": ("mood_sad-discogs-effnet-1", "sad"),
    "aggressive": ("mood_aggressive-discogs-effnet-1", "aggressive"),
    "relaxed": ("mood_relaxed-discogs-effnet-1", "relaxed"),
    "party": ("mood_party-discogs-effnet-1", "party"),
    "danceability": ("danceability-discogs-effnet-1", "danceable"),
    "voice": ("voice_instrumental-discogs-effnet-1", "voice"),
    "approachability": ("approachability_regression-discogs-effnet-1", None),
    "engagement": ("engagement_regression-discogs-effnet-1", None),
}

BATCH = 32


@dataclass
class EffnetResult:
    embeddings: np.ndarray  # [патчи, 1280]
    activations: np.ndarray  # [патчи, 400], сигмоиды стилей
    centers: np.ndarray  # середина каждого патча, с
    heads: dict[str, np.ndarray]  # ключ → [патчи]

    def mean_embedding(self) -> np.ndarray:
        """Эмбеддинг трека: среднее по патчам, нормированное по L2."""
        v = self.embeddings.mean(axis=0)
        return v / max(float(np.linalg.norm(v)), 1e-12)


class Effnet:
    def __init__(
        self,
        model_dir: Path,
        heads_dir: Path,
        options: ort.SessionOptions,
        head_options: ort.SessionOptions | None = None,
    ) -> None:
        providers = ["CPUExecutionProvider"]
        self.session = ort.InferenceSession(str(model_dir / f"{MODEL}.onnx"), options, providers)
        meta = json.loads((model_dir / f"{MODEL}.json").read_text())
        self.styles: list[str] = meta["classes"]
        self.heads: dict[str, tuple[ort.InferenceSession, int]] = {}
        for key, (stem, positive) in HEADS.items():
            head_meta = json.loads((heads_dir / f"{stem}.json").read_text())
            index = head_meta["classes"].index(positive) if positive else 0
            sess = ort.InferenceSession(
                str(heads_dir / f"{stem}.onnx"), head_options or options, providers
            )
            self.heads[key] = (sess, index)

    def run(self, signal16k: np.ndarray) -> EffnetResult:
        mel = musicnn.bands(signal16k)
        patches, centers = musicnn.patches(mel)
        acts, embs = [], []
        for i in range(0, len(patches), BATCH):
            a, e = self.session.run(None, {"melspectrogram": patches[i : i + BATCH]})
            acts.append(a)
            embs.append(e)
        activations = np.concatenate(acts).astype(np.float32)
        embeddings = np.concatenate(embs).astype(np.float32)
        heads = {
            key: sess.run(None, {"embeddings": embeddings})[0][:, index].astype(np.float32)
            for key, (sess, index) in self.heads.items()
        }
        return EffnetResult(embeddings, activations, centers, heads)

    def top_styles(self, result: EffnetResult, limit: int = 10) -> list[dict[str, object]]:
        mean = result.activations.mean(axis=0)
        order = np.argsort(-mean)[:limit]
        return [{"name": self.styles[i], "p": round(float(mean[i]), 4)} for i in order]
