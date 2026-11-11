"""CLAP «larger_clap_music» (LAION): общее пространство звука и текста (512).

ONNX экспортирован из исходного чекпойнта LAION (`export/export_clap.py`). Признаки — копия
`laion_clap.get_audio_embedding_from_data` и `ClapFeatureExtractor` (`truncation="rand_trunc"`,
`padding="repeatpad"`): 48 кГц, int16 туда и обратно, окно 10 с; периодический Ханн 1024,
шаг 480, отражение краёв; мощность; 64 мел-полосы Слэни (норма Слэни) от 50 до 14 000 Гц;
`10·log10(max(x, 1e-10))`; результат [1001, 64]. Вместо случайного куска трека берутся
несколько окон по всей длине: эмбеддинги окон нормируются, усредняются и нормируются снова.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

SAMPLE_RATE = 48000
WINDOW_SAMPLES = 480000
N_FFT = 1024
HOP = 480
N_MELS = 64
F_MIN = 50.0
F_MAX = 14000.0
MAX_TOKENS = 77


def _hz_to_mel(f: np.ndarray) -> np.ndarray:
    f = np.asarray(f, dtype=np.float64)
    mel = 3.0 * f / 200.0
    log = f >= 1000.0
    mel[log] = 15.0 + np.log(f[log] / 1000.0) * (27.0 / np.log(6.4))
    return mel


def _mel_to_hz(m: np.ndarray) -> np.ndarray:
    m = np.asarray(m, dtype=np.float64)
    f = 200.0 * m / 3.0
    log = m >= 15.0
    f[log] = 1000.0 * np.exp(np.log(6.4) / 27.0 * (m[log] - 15.0))
    return f


def mel_filters() -> np.ndarray:
    """[513, 64] — как `mel_filter_bank(norm="slaney", mel_scale="slaney")` в transformers."""
    n_bins = N_FFT // 2 + 1
    mels = np.linspace(
        _hz_to_mel(np.array([F_MIN]))[0], _hz_to_mel(np.array([F_MAX]))[0], N_MELS + 2
    )
    edges = _mel_to_hz(mels)
    fft_freqs = np.linspace(0, SAMPLE_RATE // 2, n_bins)
    diff = np.diff(edges)
    slopes = edges[None, :] - fft_freqs[:, None]
    down = -slopes[:, :-2] / diff[:-1]
    up = slopes[:, 2:] / diff[1:]
    filters = np.maximum(0.0, np.minimum(down, up))
    filters *= 2.0 / (edges[2 : N_MELS + 2] - edges[:N_MELS])
    return filters


_WINDOW = np.hanning(N_FFT + 1)[:-1]
_FILTERS = mel_filters()


def features(window: np.ndarray) -> np.ndarray:
    """Лог-мел [1001, 64] для окна до 10 с (короче — повтор и тишина, как repeatpad).

    Сигнал сначала проходит через int16 и обратно, как в `laion_clap` перед моделью: у MP3 и
    Opus верх спектра пуст, и шум квантования там заметно двигает эмбеддинг (косинус до 0.96).
    """
    x = np.clip(np.asarray(window, dtype=np.float32), -1.0, 1.0)
    x = (x * 32767.0).astype(np.int16).astype(np.float32) / np.float32(32767.0)
    x = x.astype(np.float64)
    if len(x) == 0:
        x = np.zeros(WINDOW_SAMPLES)
    if len(x) > WINDOW_SAMPLES:
        x = x[:WINDOW_SAMPLES]
    elif len(x) < WINDOW_SAMPLES:
        x = np.tile(x, WINDOW_SAMPLES // len(x))
        x = np.pad(x, (0, WINDOW_SAMPLES - len(x)))
    x = np.pad(x, (N_FFT // 2, N_FFT // 2), mode="reflect")
    frames = np.lib.stride_tricks.sliding_window_view(x, N_FFT)[::HOP]
    # Спектр в complex64, как в ClapFeatureExtractor; от мела laion_clap (torchlibrosa)
    # отличие не больше 0,003 дБ.
    spec = np.fft.rfft(frames * _WINDOW, axis=1).astype(np.complex64)
    power = np.abs(spec, dtype=np.float64) ** 2
    mel = np.maximum(1e-10, power @ _FILTERS)
    return (10.0 * np.log10(np.maximum(mel, 1e-10))).astype(np.float32)


def window_starts(n_samples: int, max_windows: int) -> list[int]:
    """Начала окон по 10 с: центры равномерно по треку, на превью — три окна встык."""
    if n_samples <= WINDOW_SAMPLES:
        return [0]
    count = max(1, min(max_windows, round(n_samples / WINDOW_SAMPLES)))
    span = n_samples - WINDOW_SAMPLES
    return [
        min(span, max(0, round((i + 0.5) * n_samples / count - WINDOW_SAMPLES / 2)))
        for i in range(count)
    ]


def _normalize(v: np.ndarray) -> np.ndarray:
    n = np.linalg.norm(v, axis=-1, keepdims=True)
    return v / np.maximum(n, 1e-12)


class Clap:
    def __init__(self, model_dir: Path, options: ort.SessionOptions) -> None:
        providers = ["CPUExecutionProvider"]
        self.audio = ort.InferenceSession(str(model_dir / "audio_model.onnx"), options, providers)
        self.text = ort.InferenceSession(str(model_dir / "text_model.onnx"), options, providers)
        self.tokenizer = Tokenizer.from_file(str(model_dir / "tokenizer.json"))
        self.tokenizer.no_padding()
        self.tokenizer.enable_truncation(MAX_TOKENS)

    def embed_windows(self, signal48k: np.ndarray, starts: list[int], batch: int = 2) -> np.ndarray:
        """Нормированные эмбеддинги окон [n, 512]."""
        feats = np.stack([features(signal48k[s : s + WINDOW_SAMPLES]) for s in starts])[:, None]
        out = [
            self.audio.run(["audio_embeds"], {"input_features": feats[i : i + batch]})[0]
            for i in range(0, len(feats), batch)
        ]
        return _normalize(np.concatenate(out).astype(np.float32))

    def embed_audio(self, signal48k: np.ndarray, max_windows: int) -> tuple[np.ndarray, int]:
        starts = window_starts(len(signal48k), max_windows)
        windows = self.embed_windows(signal48k, starts)
        return _normalize(windows.mean(axis=0)), len(starts)

    def embed_text(self, text: str) -> np.ndarray:
        enc = self.tokenizer.encode(text)
        ids = np.asarray([enc.ids], dtype=np.int64)
        mask = np.asarray([enc.attention_mask], dtype=np.int64)
        out = self.text.run(["text_embeds"], {"input_ids": ids, "attention_mask": mask})[0]
        return _normalize(out[0].astype(np.float32))
