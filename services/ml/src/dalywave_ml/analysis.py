"""Анализ трека целиком: декодированный сигнал → строка `track_analysis` в JSON."""

from __future__ import annotations

import base64
import time
from collections.abc import Iterator
from contextlib import contextmanager
from typing import Any, Literal

import numpy as np
import onnxruntime as ort

from . import __version__, features
from .audio import Audio
from .beats import FPS, BeatThis, log_mel_from_magnitude, stft_magnitude
from .clap import SAMPLE_RATE as CLAP_RATE
from .clap import Clap
from .config import Settings
from .effnet import Effnet
from .key import SKey
from .manifest import versions
from .musicnn import SAMPLE_RATE as EFFNET_RATE
from .query import translate

Mode = Literal["preview", "full"]
MOODS = ("happy", "sad", "aggressive", "relaxed", "party")
MIN_DURATION_S = 2.0
PREVIEW_BEAT_FRAMES = 1000  # 20 с при 50 кадрах в секунду


class TooShortError(Exception):
    pass


def encode_f16(vector: np.ndarray) -> str:
    """Вектор → base64 от float16 little-endian: так его без потерь кладут в halfvec."""
    return base64.b64encode(np.asarray(vector, dtype="<f2").tobytes()).decode("ascii")


def decode_f16(data: str) -> np.ndarray:
    return np.frombuffer(base64.b64decode(data), dtype="<f2").astype(np.float32)


def session_options(threads: int) -> ort.SessionOptions:
    opts = ort.SessionOptions()
    opts.intra_op_num_threads = max(1, threads)
    opts.inter_op_num_threads = 1
    opts.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
    opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    opts.log_severity_level = 3
    # Модели идут по очереди, и у каждой сессии свой пул потоков: без этого потоки
    # закончившей модели крутятся вхолостую и отнимают ядра у следующей.
    opts.add_session_config_entry("session.intra_op.allow_spinning", "0")
    # Арена onnxruntime держит пик каждой модели навсегда (Beat This! — матрицы внимания на
    # 1500 кадров): с ней процесс после полного трека занимал 3,3 ГБ, без неё — 1,3 ГБ,
    # и анализ даже на ~5 % быстрее.
    opts.enable_cpu_mem_arena = False
    return opts


def _ms(seconds: float) -> int:
    return round(seconds * 1000)


class Timer:
    def __init__(self) -> None:
        self.ms: dict[str, int] = {}

    @contextmanager
    def __call__(self, name: str) -> Iterator[None]:
        start = time.perf_counter()
        try:
            yield
        finally:
            self.ms[name] = self.ms.get(name, 0) + _ms(time.perf_counter() - start)


class Analyzer:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        root = settings.models_dir
        opts = session_options(settings.threads)
        # Головы — крошечные MLP на [патчи, 1280]: им хватает одного потока.
        self.effnet = Effnet(root / "effnet", root / "heads", opts, session_options(1))
        self.clap = Clap(root / "clap", opts)
        self.beat_this = BeatThis(root / "beat_this", opts)
        self.skey = SKey(root / "skey", opts)

    def embed_text(self, text: str) -> dict[str, Any]:
        query = translate(text)
        vector = self.clap.embed_text(query.text)
        return {
            "semantic": encode_f16(vector),
            "dim": int(vector.shape[0]),
            "query": query.text,
            "translated": query.translated,
            "unknownWords": query.unknown,
            "model": versions()["semantic"],
        }

    def analyze(self, audio: Audio, mode: Mode) -> dict[str, Any]:
        duration = audio.duration_s
        if duration < MIN_DURATION_S:
            raise TooShortError(f"звука {duration:.1f} с, нужно хотя бы {MIN_DURATION_S:.0f}")
        t = Timer()
        full = mode == "full"

        with t("resample"):
            x16 = audio.at(EFFNET_RATE)
            x22 = audio.at(22050)
            x48 = audio.at(CLAP_RATE)

        with t("effnet"):
            eff = self.effnet.run(x16)
        heads = {k: float(v.mean()) for k, v in eff.heads.items()}
        moods = {k: round(heads[k], 4) for k in MOODS}

        with t("clap"):
            windows = 3 if not full else self.settings.clap_windows
            semantic, n_windows = self.clap.embed_audio(x48, windows)

        with t("beats"):
            magnitude = stft_magnitude(x22)
            spect = log_mel_from_magnitude(magnitude, self.beat_this.filterbank)
            # Превью нужен только темп: 20 с дают тот же BPM (±0.2), а сеть считает их
            # вдвое быстрее полного куска в 30 с.
            beat_spect = spect if full else spect[:PREVIEW_BEAT_FRAMES]
            grid = self.beat_this.track(beat_spect, first_chunk_only=not full)
            frames = np.clip(np.round(grid.beats * FPS).astype(int), 0, len(beat_spect) - 1)
            logits = np.clip(grid.beat_logits[frames], -30.0, 30.0)
            strength = float((1 / (1 + np.exp(-logits))).mean()) if len(frames) else None
            span = len(beat_spect) / FPS
            tempo = features.estimate_tempo(grid.beats, grid.downbeats, span, strength)

        with t("key"):
            key_name, camelot, key_confidence = self.skey.detect(x22)

        with t("features"):
            loudness = features.loudness_per_second(magnitude)
            loud01, flux01 = features.energy_components(loudness, spect)
            curve = features.energy_curve(loud01, flux01)
            energy = features.track_energy(curve, moods)

        result: dict[str, Any] = {
            "source": mode,
            "models": {**versions(), "service": f"dalywave-ml {__version__}"},
            "durationMs": _ms(duration),
            "sound": encode_f16(eff.mean_embedding()),
            "semantic": encode_f16(semantic),
            "bpm": tempo.bpm,
            "bpmConfidence": tempo.confidence,
            "keyCamelot": camelot,
            "key": key_name,
            "keyConfidence": None if key_confidence is None else round(key_confidence, 3),
            "energy": energy,
            "danceability": round(heads["danceability"], 4),
            "valence": features.valence(moods),
            "moods": moods,
            "voice": round(heads["voice"], 4),
            "language": None,
            "styles": self.effnet.top_styles(eff),
            "introEndMs": None,
            "outroStartMs": None,
            "beatsMs": None,
            "downbeatsMs": None,
            "vocalSegments": None,
            "energyCurve": None,
            "extra": {
                # Регрессионные головы иногда выходят за 0..1 на пару тысячных.
                "approachability": round(min(1.0, max(0.0, heads["approachability"])), 4),
                "engagement": round(min(1.0, max(0.0, heads["engagement"])), 4),
                "beatsPerBar": features.beats_per_bar(tempo.beats, tempo.downbeats),
                "tempoFold": tempo.fold,
                "loudnessLufs": round(float(np.percentile(loudness, 90)), 2)
                if len(loudness)
                else None,
                "clapWindows": n_windows,
                "patches": len(eff.embeddings),
                "truncated": audio.truncated,
                "codec": audio.codec,
                "sampleRate": audio.sample_rate,
            },
        }
        if full:
            with t("features"):
                vocals = features.vocal_segments(eff.heads["voice"], eff.centers, duration)
                intro, outro = features.intro_outro(curve, vocals, tempo.downbeats, duration)
            result.update(
                {
                    "introEndMs": _ms(intro),
                    "outroStartMs": _ms(outro),
                    "beatsMs": [_ms(b) for b in tempo.beats],
                    "downbeatsMs": [_ms(b) for b in tempo.downbeats],
                    "vocalSegments": [[_ms(a), _ms(b)] for a, b in vocals],
                    "energyCurve": [round(float(v) * 1000) for v in curve],
                }
            )
        result["timingsMs"] = t.ms
        return result
