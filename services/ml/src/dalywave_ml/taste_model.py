"""Маленькая сеть вкуса: по признакам трека, контексту и граням вкуса предсказывает три
вероятности — пропустит, дослушает, лайкнет.

Обучение — numpy на CPU (torch в образе не нужен): ~150 тысяч параметров, Adam, ранняя остановка
по последним по времени строкам обучающей части. Векторы трека (Discogs-EffNet 1280 и CLAP 512)
сжимаются PCA с выбеливанием, посчитанным по трекам обучающей части; сжатие и нормировка числовых
признаков вшиты в ONNX, поэтому сервер подаёт сырые признаки и ничего не пересчитывает.

Проверка — на отложенной по времени части (самые поздние строки): AUC и logloss по каждой голове
у сети и у правил. Оценка правил калибруется логистической регрессией на обучающей части — так
сравнение logloss честное. Включать ли сеть, решает сервер.
"""

from __future__ import annotations

import base64
import itertools
from dataclasses import dataclass, field
from typing import Any

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

HEADS = ("skip", "complete", "like")
# Меньше — сеть не обучается: правила лучше переобученной сети
MIN_ROWS = 300
MIN_HOLDOUT = 60
OPSET = 17


class TooLittleDataError(ValueError):
    """Данных мало для обучения и честной проверки."""


@dataclass
class TrainData:
    # Векторы треков: строка — звучание (sound_dim) и «смысл» (semantic_dim) подряд
    vectors: np.ndarray
    # Строка набора → строка `vectors`
    row_vector: np.ndarray
    scalars: np.ndarray
    # 0/1 по головам HEADS
    labels: np.ndarray
    # Оценка правил для той же строки
    baseline: np.ndarray
    # Строки упорядочены по времени; с этой начинается отложенная часть
    holdout_from: int
    sound_dim: int
    semantic_dim: int
    feature_names: list[str] = field(default_factory=list)

    @property
    def rows(self) -> int:
        return int(self.row_vector.shape[0])

    def validate(self) -> None:
        n = self.rows
        if self.vectors.ndim != 2 or self.vectors.shape[1] != self.sound_dim + self.semantic_dim:
            raise ValueError("ширина векторов не совпадает с soundDim + semanticDim")
        if self.scalars.shape[0] != n or self.labels.shape != (n, len(HEADS)):
            raise ValueError("число строк признаков и меток не совпадает")
        if self.baseline.shape != (n,):
            raise ValueError("оценка правил нужна для каждой строки")
        if n and (self.row_vector.min() < 0 or self.row_vector.max() >= self.vectors.shape[0]):
            raise ValueError("ссылка на несуществующий вектор")
        if not np.isfinite(self.scalars).all() or not np.isfinite(self.vectors).all():
            raise ValueError("в признаках есть NaN или бесконечность")
        train, hold = self.holdout_from, n - self.holdout_from
        if n < MIN_ROWS or train < MIN_ROWS - MIN_HOLDOUT or hold < MIN_HOLDOUT:
            raise TooLittleDataError(
                f"мало данных: {n} строк ({train} на обучение, {hold} на проверку), "
                f"нужно не меньше {MIN_ROWS} и {MIN_HOLDOUT} на проверку"
            )
        # Лайков может не быть вовсе: голова с одним исходом учится, но в сравнение не идёт
        if all(len(np.unique(self.labels[:train, h])) < 2 for h in range(len(HEADS))):
            raise TooLittleDataError("в обучающей части у всех голов только один исход")


@dataclass
class Params:
    """Всё, что вшивается в ONNX: сжатие, нормировка и веса слоёв."""

    sound_mean: np.ndarray
    sound_proj: np.ndarray
    semantic_mean: np.ndarray
    semantic_proj: np.ndarray
    scalar_mean: np.ndarray
    scalar_scale: np.ndarray
    weights: list[np.ndarray]
    biases: list[np.ndarray]
    sound_dim: int
    semantic_dim: int

    @property
    def trainable(self) -> int:
        return int(sum(w.size for w in self.weights) + sum(b.size for b in self.biases))

    @property
    def input_dim(self) -> int:
        return self.sound_dim + self.semantic_dim + int(self.scalar_mean.shape[0])


def _pca(block: np.ndarray, k: int) -> tuple[np.ndarray, np.ndarray]:
    """Среднее и матрица сжатия с выбеливанием: выходы — примерно единичной дисперсии."""
    mean = block.mean(axis=0)
    centered = block - mean
    k = max(1, min(k, block.shape[0] - 1, block.shape[1]))
    _, s, vt = np.linalg.svd(centered, full_matrices=False)
    var = (s[:k] ** 2) / max(1, block.shape[0] - 1)
    proj = vt[:k].T / np.sqrt(np.maximum(var, 1e-6))
    return mean.astype(np.float32), proj.astype(np.float32)


def reduce(p: Params, vectors: np.ndarray, scalars: np.ndarray) -> np.ndarray:
    sd = p.sound_dim
    zs = (vectors[:, :sd] - p.sound_mean) @ p.sound_proj
    zt = (vectors[:, sd : sd + p.semantic_dim] - p.semantic_mean) @ p.semantic_proj
    zc = (scalars - p.scalar_mean) * p.scalar_scale
    return np.concatenate([zs, zt, zc], axis=1).astype(np.float32)


def _sigmoid(x: np.ndarray) -> np.ndarray:
    return 1.0 / (1.0 + np.exp(-np.clip(x, -40, 40)))


def _mlp(p: Params, z: np.ndarray) -> tuple[list[np.ndarray], np.ndarray]:
    acts = [z]
    h = z
    for i, (w, b) in enumerate(zip(p.weights, p.biases, strict=True)):
        h = h @ w + b
        if i < len(p.weights) - 1:
            h = np.maximum(h, 0.0)
        acts.append(h)
    return acts, h


def forward(p: Params, features: np.ndarray) -> np.ndarray:
    """То же, что ONNX: сырые признаки [N, D] → вероятности [N, 3]."""
    sd, md = p.sound_dim, p.semantic_dim
    z = reduce(p, features[:, : sd + md], features[:, sd + md :])
    return _sigmoid(_mlp(p, z)[1]).astype(np.float32)


def auc(y: np.ndarray, score: np.ndarray) -> float | None:
    """AUC по рангам (Манн — Уитни), равные оценки — средним рангом."""
    pos = y > 0.5
    npos, nneg = int(pos.sum()), int((~pos).sum())
    if npos == 0 or nneg == 0:
        return None
    _, inv, counts = np.unique(score, return_inverse=True, return_counts=True)
    ends = np.cumsum(counts)
    ranks = ((ends - counts + 1 + ends) / 2.0)[inv]
    return float((ranks[pos].sum() - npos * (npos + 1) / 2.0) / (npos * nneg))


def logloss(y: np.ndarray, p: np.ndarray) -> float:
    p = np.clip(p, 1e-6, 1 - 1e-6)
    return float(-np.mean(y * np.log(p) + (1 - y) * np.log(1 - p)))


def calibrate(x: np.ndarray, y: np.ndarray, iters: int = 30) -> tuple[float, float, float, float]:
    """Логистическая регрессия по одному признаку (Ньютон, L2): оценка правил → вероятность."""
    mu, sd = float(x.mean()), float(x.std()) or 1.0
    xs = (x - mu) / sd
    a, b = 0.0, float(np.log((y.mean() + 1e-3) / (1 - y.mean() + 1e-3)))
    for _ in range(iters):
        p = _sigmoid(a * xs + b)
        g_a = float(((p - y) * xs).sum() + 1e-3 * a)
        g_b = float((p - y).sum())
        w = p * (1 - p)
        h_aa = float((w * xs * xs).sum() + 1e-3)
        h_ab = float((w * xs).sum())
        h_bb = float(w.sum() + 1e-9)
        det = h_aa * h_bb - h_ab * h_ab
        if abs(det) < 1e-12:
            break
        a -= (h_bb * g_a - h_ab * g_b) / det
        b -= (h_aa * g_b - h_ab * g_a) / det
    return a, b, mu, sd


@dataclass
class TrainResult:
    params: Params
    onnx: bytes
    metrics: dict[str, Any]
    epochs: int


def _init(rng: np.random.Generator, sizes: list[int]) -> tuple[list[np.ndarray], list[np.ndarray]]:
    weights, biases = [], []
    for fan_in, fan_out in itertools.pairwise(sizes):
        w = rng.standard_normal((fan_in, fan_out)) * np.sqrt(2.0 / fan_in)
        weights.append(w.astype(np.float32))
        biases.append(np.zeros(fan_out, dtype=np.float32))
    return weights, biases


def _loss(p: Params, z: np.ndarray, y: np.ndarray) -> float:
    return logloss(y, _sigmoid(_mlp(p, z)[1]))


def train(
    data: TrainData,
    *,
    seed: int = 0,
    hidden: tuple[int, ...] = (384, 256, 64),
    sound_k: int = 32,
    semantic_k: int = 16,
    max_epochs: int = 60,
    patience: int = 6,
    batch: int = 256,
    lr: float = 1e-3,
    weight_decay: float = 1e-4,
) -> TrainResult:
    data.validate()
    rng = np.random.default_rng(seed)
    n_train = data.holdout_from
    # Ранняя остановка — по самым поздним строкам обучающей части: так же, как будет в жизни
    n_fit = max(1, int(n_train * 0.85))
    sd, md = data.sound_dim, data.semantic_dim

    train_vecs = data.vectors[np.unique(data.row_vector[:n_train])]
    sound_mean, sound_proj = _pca(train_vecs[:, :sd], sound_k)
    sem_mean, sem_proj = _pca(train_vecs[:, sd : sd + md], semantic_k)
    sc_train = data.scalars[:n_train]
    sc_mean = sc_train.mean(axis=0).astype(np.float32)
    sc_scale = (1.0 / np.maximum(sc_train.std(axis=0), 1e-3)).astype(np.float32)

    in_dim = sound_proj.shape[1] + sem_proj.shape[1] + data.scalars.shape[1]
    weights, biases = _init(rng, [in_dim, *hidden, len(HEADS)])
    p = Params(
        sound_mean=sound_mean,
        sound_proj=sound_proj,
        semantic_mean=sem_mean,
        semantic_proj=sem_proj,
        scalar_mean=sc_mean,
        scalar_scale=sc_scale,
        weights=weights,
        biases=biases,
        sound_dim=sd,
        semantic_dim=md,
    )
    z_all = reduce(p, data.vectors[data.row_vector], data.scalars)
    y_all = data.labels.astype(np.float32)
    z_fit, y_fit = z_all[:n_fit], y_all[:n_fit]
    z_val, y_val = z_all[n_fit:n_train], y_all[n_fit:n_train]

    params = [*p.weights, *p.biases]
    m = [np.zeros_like(x) for x in params]
    v = [np.zeros_like(x) for x in params]
    beta1, beta2, eps = 0.9, 0.999, 1e-8
    step = 0
    best = (float("inf"), [x.copy() for x in params], 0)
    stale = 0
    epochs = 0
    layers = len(p.weights)
    for epoch in range(max_epochs):
        epochs = epoch + 1
        order = rng.permutation(n_fit)
        for start in range(0, n_fit, batch):
            idx = order[start : start + batch]
            acts, logits = _mlp(p, z_fit[idx])
            grad = (_sigmoid(logits) - y_fit[idx]) / (len(idx) * len(HEADS))
            gw: list[np.ndarray] = [np.empty(0)] * layers
            gb: list[np.ndarray] = [np.empty(0)] * layers
            for i in reversed(range(layers)):
                gw[i] = acts[i].T @ grad + weight_decay * p.weights[i]
                gb[i] = grad.sum(axis=0)
                if i > 0:
                    grad = (grad @ p.weights[i].T) * (acts[i] > 0)
            step += 1
            for j, g in enumerate([*gw, *gb]):
                m[j] = beta1 * m[j] + (1 - beta1) * g
                v[j] = beta2 * v[j] + (1 - beta2) * g * g
                mh = m[j] / (1 - beta1**step)
                vh = v[j] / (1 - beta2**step)
                params[j] -= (lr * mh / (np.sqrt(vh) + eps)).astype(np.float32)
        val = _loss(p, z_val, y_val) if len(y_val) else _loss(p, z_fit, y_fit)
        if val < best[0] - 1e-5:
            best = (val, [x.copy() for x in params], epochs)
            stale = 0
        else:
            stale += 1
            if stale >= patience:
                break
    for j, x in enumerate(best[1]):
        params[j][...] = x

    metrics = evaluate(p, data, z_all)
    metrics["params"] = p.trainable
    metrics["epochs"] = best[2]
    return TrainResult(params=p, onnx=export(p), metrics=metrics, epochs=best[2])


def evaluate(p: Params, data: TrainData, z_all: np.ndarray) -> dict[str, Any]:
    n_train = data.holdout_from
    y = data.labels.astype(np.float32)
    probs = _sigmoid(_mlp(p, z_all[n_train:])[1])
    heads: dict[str, Any] = {}
    aucs, base_aucs, losses, base_losses = [], [], [], []
    for h, name in enumerate(HEADS):
        yh = y[n_train:, h]
        a, b, mu, sd = calibrate(data.baseline[:n_train], y[:n_train, h])
        base = _sigmoid(a * (data.baseline[n_train:] - mu) / sd + b)
        m_auc, b_auc = auc(yh, probs[:, h]), auc(yh, base)
        m_loss, b_loss = logloss(yh, probs[:, h]), logloss(yh, base)
        heads[name] = {
            "positives": int(yh.sum()),
            "auc": m_auc,
            "logloss": m_loss,
            "baselineAuc": b_auc,
            "baselineLogloss": b_loss,
        }
        if m_auc is not None and b_auc is not None:
            aucs.append(m_auc)
            base_aucs.append(b_auc)
            losses.append(m_loss)
            base_losses.append(b_loss)

    def mean(xs: list[float]) -> float | None:
        return float(np.mean(xs)) if xs else None

    return {
        "train": n_train,
        "holdout": data.rows - n_train,
        "heads": heads,
        "auc": mean(aucs),
        "logloss": mean(losses),
        "baselineAuc": mean(base_aucs),
        "baselineLogloss": mean(base_losses),
    }


def export(p: Params) -> bytes:
    """ONNX: features [N, D] → probs [N, 3]. Сжатие векторов и нормировка — внутри графа."""
    sd, md = p.sound_dim, p.semantic_dim
    d = p.input_dim
    inits = [
        numpy_helper.from_array(np.array(x, dtype=np.int64), name)
        for name, x in (
            ("s_start", [0]),
            ("s_end", [sd]),
            ("t_start", [sd]),
            ("t_end", [sd + md]),
            ("c_start", [sd + md]),
            ("c_end", [d]),
            ("axis1", [1]),
        )
    ]
    for name, x in (
        ("sound_mean", p.sound_mean),
        ("sound_proj", p.sound_proj),
        ("semantic_mean", p.semantic_mean),
        ("semantic_proj", p.semantic_proj),
        ("scalar_mean", p.scalar_mean),
        ("scalar_scale", p.scalar_scale),
    ):
        inits.append(numpy_helper.from_array(np.ascontiguousarray(x, dtype=np.float32), name))
    nodes = [
        helper.make_node("Slice", ["features", "s_start", "s_end", "axis1"], ["snd"]),
        helper.make_node("Sub", ["snd", "sound_mean"], ["snd_c"]),
        helper.make_node("MatMul", ["snd_c", "sound_proj"], ["z_s"]),
        helper.make_node("Slice", ["features", "t_start", "t_end", "axis1"], ["sem"]),
        helper.make_node("Sub", ["sem", "semantic_mean"], ["sem_c"]),
        helper.make_node("MatMul", ["sem_c", "semantic_proj"], ["z_t"]),
        helper.make_node("Slice", ["features", "c_start", "c_end", "axis1"], ["sc"]),
        helper.make_node("Sub", ["sc", "scalar_mean"], ["sc_c"]),
        helper.make_node("Mul", ["sc_c", "scalar_scale"], ["z_c"]),
        helper.make_node("Concat", ["z_s", "z_t", "z_c"], ["h0"], axis=1),
    ]
    prev = "h0"
    for i, (w, b) in enumerate(zip(p.weights, p.biases, strict=True)):
        inits.append(numpy_helper.from_array(np.ascontiguousarray(w, dtype=np.float32), f"w{i}"))
        inits.append(numpy_helper.from_array(np.ascontiguousarray(b, dtype=np.float32), f"b{i}"))
        out = f"g{i}"
        nodes.append(helper.make_node("Gemm", [prev, f"w{i}", f"b{i}"], [out]))
        if i < len(p.weights) - 1:
            nodes.append(helper.make_node("Relu", [out], [f"h{i + 1}"]))
            prev = f"h{i + 1}"
        else:
            nodes.append(helper.make_node("Sigmoid", [out], ["probs"]))
    graph = helper.make_graph(
        nodes,
        "dalywave_taste",
        [helper.make_tensor_value_info("features", TensorProto.FLOAT, ["N", d])],
        [helper.make_tensor_value_info("probs", TensorProto.FLOAT, ["N", len(HEADS)])],
        initializer=inits,
    )
    model = helper.make_model(
        graph,
        opset_imports=[helper.make_opsetid("", OPSET)],
        producer_name="dalywave-ml",
        doc_string="skip, complete, like",
    )
    # IR 9 понимает и onnxruntime сервиса, и ort на сервере
    model.ir_version = 9
    onnx.checker.check_model(model)
    return model.SerializeToString()


def b64_array(raw: str, dtype: str, count: int) -> np.ndarray:
    data = base64.b64decode(raw, validate=True)
    arr = np.frombuffer(data, dtype=np.dtype(dtype))
    if arr.size != count:
        raise ValueError(f"ожидалось {count} чисел, пришло {arr.size}")
    return arr
