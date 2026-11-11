import base64

import numpy as np
import onnxruntime as ort
import pytest
from fastapi.testclient import TestClient

from dalywave_ml.config import Settings
from dalywave_ml.server import create_app
from dalywave_ml.taste_model import (
    HEADS,
    TooLittleDataError,
    TrainData,
    auc,
    calibrate,
    forward,
    logloss,
    train,
)

SOUND, SEMANTIC, SCALARS = 64, 16, 6


def synthetic(rows: int, seed: int = 1) -> TrainData:
    """Вкус задан осью звучания: близкое к ней дослушивают и лайкают, далёкое пропускают.
    Правила знают лишь половину правды — шумную копию той же близости."""
    rng = np.random.default_rng(seed)
    tracks = max(40, rows // 4)
    vectors = rng.standard_normal((tracks, SOUND + SEMANTIC)).astype(np.float32)
    taste = vectors[:, 0] + 0.5 * vectors[:, SOUND]
    row_vector = rng.integers(0, tracks, rows).astype(np.int32)
    hour = rng.uniform(0, 24, rows)
    scalars = np.stack(
        [
            np.sin(hour / 24 * 2 * np.pi),
            np.cos(hour / 24 * 2 * np.pi),
            rng.uniform(0, 1, rows),
            rng.integers(0, 2, rows),
            rng.uniform(0, 1, rows),
            rng.uniform(0, 1, rows),
        ],
        axis=1,
    ).astype(np.float32)
    # Вечером любят больше: этого правила не знают
    logit = 1.6 * taste[row_vector] + 1.0 * scalars[:, 1]
    p = 1 / (1 + np.exp(-logit))
    complete = (rng.uniform(size=rows) < p).astype(np.float32)
    skip = ((1 - complete) * (rng.uniform(size=rows) < 0.8)).astype(np.float32)
    like = (complete * (rng.uniform(size=rows) < p * 0.6)).astype(np.float32)
    labels = np.stack([skip, complete, like], axis=1)
    baseline = (taste[row_vector] + rng.standard_normal(rows) * 1.5).astype(np.float32)
    return TrainData(
        vectors=vectors,
        row_vector=row_vector,
        scalars=scalars,
        labels=labels,
        baseline=baseline,
        holdout_from=int(rows * 0.8),
        sound_dim=SOUND,
        semantic_dim=SEMANTIC,
    )


def test_metrics_helpers():
    y = np.array([0, 0, 1, 1], dtype=np.float32)
    assert auc(y, np.array([0.1, 0.2, 0.8, 0.9])) == 1.0
    assert auc(y, np.array([0.9, 0.8, 0.2, 0.1])) == 0.0
    assert auc(y, np.array([0.5, 0.5, 0.5, 0.5])) == 0.5
    assert auc(np.ones(3), np.array([0.1, 0.2, 0.3])) is None
    assert logloss(y, np.array([0.0, 0.0, 1.0, 1.0])) < 1e-5
    # Убывающая оценка калибруется с отрицательным наклоном
    x = np.linspace(-2, 2, 400)
    a, _, _, _ = calibrate(x, (x < 0).astype(np.float64))
    assert a < 0


def test_too_little_data_is_refused():
    data = synthetic(120)
    with pytest.raises(TooLittleDataError):
        train(data)


def test_head_without_positives_still_trains():
    data = synthetic(600)
    data.labels[:, 2] = 0
    m = train(data, max_epochs=5).metrics
    assert m["heads"]["like"]["auc"] is None
    assert m["auc"] is not None, "сравнение — по головам с обоими исходами"
    data.labels[:, :] = 0
    with pytest.raises(TooLittleDataError):
        train(data)


def test_learns_more_than_rules_and_onnx_matches_numpy():
    data = synthetic(3000)
    result = train(data, max_epochs=40)
    m = result.metrics
    assert m["holdout"] == 600
    assert set(m["heads"]) == set(HEADS)
    # Правила видят только шумную близость, сеть — ещё и вечер
    assert m["auc"] > m["baselineAuc"] + 0.02
    assert m["logloss"] < m["baselineLogloss"]
    assert 100_000 < result.params.trainable < 200_000

    features = np.concatenate([data.vectors[data.row_vector[:256]], data.scalars[:256]], axis=1)
    sess = ort.InferenceSession(result.onnx, providers=["CPUExecutionProvider"])
    (probs,) = sess.run(["probs"], {"features": features})
    assert probs.shape == (256, 3)
    np.testing.assert_allclose(probs, forward(result.params, features), atol=1e-4)
    assert ((probs > 0) & (probs < 1)).all()


def encode(arr: np.ndarray, dtype: str) -> str:
    return base64.b64encode(np.ascontiguousarray(arr, dtype=np.dtype(dtype)).tobytes()).decode()


def request(data: TrainData) -> dict:
    return {
        "vectors": encode(data.vectors, "<f2"),
        "vectorRows": data.vectors.shape[0],
        "soundDim": SOUND,
        "semanticDim": SEMANTIC,
        "rowVector": encode(data.row_vector, "<i4"),
        "scalars": encode(data.scalars, "<f4"),
        "scalarDim": SCALARS,
        "rows": data.rows,
        "labels": encode(data.labels, "<f4"),
        "baseline": encode(data.baseline, "<f4"),
        "holdoutFrom": data.holdout_from,
        "featureNames": [f"x{i}" for i in range(SCALARS)],
    }


@pytest.fixture
def client(tmp_path):
    settings = Settings(models_dir=tmp_path / "models", threads=1)
    # Модели анализа для обучения не нужны: сервис без них всё равно учит сеть вкуса
    with TestClient(create_app(settings, analyzer=object())) as c:  # type: ignore[arg-type]
        yield c


def test_train_endpoint(client):
    data = synthetic(1200)
    r = client.post("/v1/taste/train", json=request(data))
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["inputDim"] == SOUND + SEMANTIC + SCALARS
    assert body["heads"] == ["skip", "complete", "like"]
    assert body["metrics"]["holdout"] == 240
    onnx_bytes = base64.b64decode(body["onnx"])
    sess = ort.InferenceSession(onnx_bytes, providers=["CPUExecutionProvider"])
    assert sess.get_inputs()[0].name == "features"


def test_train_endpoint_refuses_small_and_broken_sets(client):
    r = client.post("/v1/taste/train", json=request(synthetic(100)))
    assert r.status_code == 422
    assert r.json()["error"]["code"] == "too_little_data"
    bad = request(synthetic(400))
    bad["rows"] = 401
    r = client.post("/v1/taste/train", json=bad)
    assert r.status_code == 422
    assert r.json()["error"]["code"] == "invalid_dataset"
