"""Карта звучания для «Как подобран»: векторы Discogs-EffNet всего индекса → 2D (UMAP).

Векторы нормируются (косинус ≈ евклидово расстояние), сжимаются PCA до 50 компонент, затем UMAP.
Если сервер прислал прошлую карту, UMAP начинается с неё: у новых треков — среднее прошлых координат
десяти ближайших соседей. Так соседства переезжают мало, а поворот и масштаб сервер снимает сам
выравниванием (Procrustes) — здесь карта не выравнивается.
"""

from __future__ import annotations

import numpy as np

# Параметры подобраны на прототипе (3 тыс. треков): облака граней читаются, не слипаются
N_NEIGHBORS = 25
MIN_DIST = 0.35
PCA_DIM = 50
SEED = 7
# Меньше точек — UMAP не строит граф соседей: хватает двух главных компонент
MIN_UMAP = 30
# Прошлая карта годится как начало, если в ней хотя бы такая доля точек
MIN_PREV_SHARE = 0.5
KNN = 10


def pca(x: np.ndarray, k: int) -> np.ndarray:
    """Главные компоненты строк `x` (центрированных), `k` штук."""
    centered = x - x.mean(axis=0)
    k = max(1, min(k, centered.shape[0] - 1, centered.shape[1]))
    _, _, vt = np.linalg.svd(centered, full_matrices=False)
    return centered @ vt[:k].T


def fill_from_neighbours(z: np.ndarray, prev: np.ndarray, k: int = KNN) -> np.ndarray:
    """Прошлые координаты; у точек без них — среднее прошлых координат `k` ближайших в `z`."""
    out = prev.copy()
    known = np.isfinite(prev).all(axis=1)
    missing = np.flatnonzero(~known)
    if missing.size == 0:
        return out
    base = z[known]
    base_xy = prev[known]
    k = min(k, base.shape[0])
    base_sq = (base**2).sum(axis=1)
    # Порциями: матрица расстояний целиком на десятках тысяч точек не нужна в памяти
    for start in range(0, missing.size, 1024):
        idx = missing[start : start + 1024]
        q = z[idx]
        d = base_sq[None, :] - 2.0 * q @ base.T
        near = np.argpartition(d, k - 1, axis=1)[:, :k]
        out[idx] = base_xy[near].mean(axis=1)
    return out


def layout(vectors: np.ndarray, prev: np.ndarray | None = None, seed: int = SEED) -> np.ndarray:
    """`vectors` — n × dim, `prev` — n × 2 (NaN — точки не было). Ответ — n × 2."""
    x = np.asarray(vectors, dtype=np.float32)
    n = x.shape[0]
    if n == 0:
        return np.zeros((0, 2), dtype=np.float32)
    norms = np.linalg.norm(x, axis=1, keepdims=True)
    x = x / np.maximum(norms, 1e-12)
    if n < MIN_UMAP:
        two = pca(x, 2)
        if two.shape[1] < 2:
            two = np.hstack([two, np.zeros((n, 1), dtype=two.dtype)])
        return two.astype(np.float32)
    z = pca(x, PCA_DIM).astype(np.float32)
    init: str | np.ndarray = "spectral"
    if prev is not None and np.isfinite(prev).all(axis=1).mean() >= MIN_PREV_SHARE:
        init = fill_from_neighbours(z, np.asarray(prev, dtype=np.float32))
    import umap  # тяжёлый импорт (numba) — только когда нужен

    reducer = umap.UMAP(
        n_neighbors=min(N_NEIGHBORS, n - 1),
        min_dist=MIN_DIST,
        metric="euclidean",
        init=init,
        random_state=seed,
        n_jobs=1,
        # Конвейер 0.5.x: им подобраны параметры, и он принимает прошлую карту как начало
        compatibility_layout=True,
    )
    return np.asarray(reducer.fit_transform(z), dtype=np.float32)


def decode_body(body: bytes, rows: int, dim: int) -> tuple[np.ndarray, np.ndarray | None]:
    """Тело `POST /v1/layout`: векторы float16 little-endian подряд, затем (необязательно) прошлые
    координаты float32 little-endian (x, y; NaN — нет)."""
    vec_bytes = rows * dim * 2
    prev_bytes = rows * 2 * 4
    if len(body) not in (vec_bytes, vec_bytes + prev_bytes):
        raise ValueError(
            f"тело {len(body)} байт, ждали {vec_bytes} или {vec_bytes + prev_bytes} "
            f"для {rows} × {dim}"
        )
    vectors = np.frombuffer(body, dtype="<f2", count=rows * dim).reshape(rows, dim)
    vectors = vectors.astype(np.float32)
    if not np.isfinite(vectors).all():
        raise ValueError("в векторах есть бесконечность или NaN")
    prev = None
    if len(body) > vec_bytes:
        prev = np.frombuffer(body, dtype="<f4", offset=vec_bytes).reshape(rows, 2).copy()
    return vectors, prev
