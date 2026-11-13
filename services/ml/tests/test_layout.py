import numpy as np
import pytest
from fastapi.testclient import TestClient

from dalywave_ml import layout
from dalywave_ml.config import Settings
from dalywave_ml.server import create_app


def clusters(n_per: int = 40, dim: int = 64, seed: int = 1) -> tuple[np.ndarray, np.ndarray]:
    """Три облака векторов: карта должна держать их врозь."""
    rng = np.random.default_rng(seed)
    centers = rng.normal(size=(3, dim)) * 3
    x = np.vstack([c + rng.normal(size=(n_per, dim)) * 0.3 for c in centers])
    labels = np.repeat(np.arange(3), n_per)
    return x.astype(np.float32), labels


def separated(points: np.ndarray, labels: np.ndarray) -> bool:
    """Ближайший сосед каждой точки — из её же облака."""
    d = ((points[:, None, :] - points[None, :, :]) ** 2).sum(-1)
    np.fill_diagonal(d, np.inf)
    return bool((labels[d.argmin(1)] == labels).mean() > 0.95)


def test_small_index_falls_back_to_pca():
    x, labels = clusters(n_per=5)
    p = layout.layout(x)
    assert p.shape == (15, 2)
    assert separated(p, labels)


def test_umap_keeps_clusters_apart_and_starts_from_previous_map():
    x, labels = clusters()
    first = layout.layout(x)
    assert first.shape == (120, 2)
    assert separated(first, labels)
    # Новые точки: прошлых координат нет — начало у них из соседей
    prev = first.copy()
    prev[::10] = np.nan
    second = layout.layout(x, prev)
    assert separated(second, labels)


def test_fill_from_neighbours():
    z = np.array([[0.0, 0.0], [1.0, 0.0], [10.0, 10.0], [0.1, 0.0]], dtype=np.float32)
    prev = np.array([[0.0, 0.0], [2.0, 0.0], [50.0, 50.0], [np.nan, np.nan]], dtype=np.float32)
    out = layout.fill_from_neighbours(z, prev, k=2)
    assert np.allclose(out[3], [1.0, 0.0])
    assert np.allclose(out[:3], prev[:3])


def test_decode_body_checks_sizes():
    x = np.ones((3, 4), dtype="<f2")
    v, prev = layout.decode_body(x.tobytes(), 3, 4)
    assert v.shape == (3, 4)
    assert prev is None
    with pytest.raises(ValueError):
        layout.decode_body(x.tobytes()[:-2], 3, 4)


def test_layout_endpoint(tmp_path):
    settings = Settings(models_dir=tmp_path / "models", files_root=tmp_path.resolve(), threads=1)
    x, labels = clusters(n_per=5, dim=16)
    prev = np.full((15, 2), np.nan, dtype="<f4")
    body = x.astype("<f2").tobytes() + prev.tobytes()
    with TestClient(create_app(settings, object())) as client:
        res = client.post(
            "/v1/layout?rows=15&dim=16",
            content=body,
            headers={"content-type": "application/octet-stream"},
        )
        assert res.status_code == 200, res.text
        points = np.array(res.json()["points"])
        assert points.shape == (15, 2)
        assert separated(points, labels)
        bad = client.post("/v1/layout?rows=15&dim=16", content=body[:10])
        assert bad.status_code == 422
