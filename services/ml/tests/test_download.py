import ast
import hashlib
import importlib.util
import json
from pathlib import Path

import numpy as np
import onnxruntime as ort
import pytest

onnx = pytest.importorskip("onnx")
from onnx import TensorProto, helper, numpy_helper  # noqa: E402

from dalywave_ml import download  # noqa: E402
from dalywave_ml.manifest import CLAP_CHECKPOINT, ModelFile  # noqa: E402

EXPORT = Path(__file__).resolve().parents[1] / "export"


def load_fp16():
    spec = importlib.util.spec_from_file_location("fp16", EXPORT / "fp16.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def tiny_model(path):
    rng = np.random.default_rng(0)
    w = numpy_helper.from_array(rng.normal(0, 0.1, (64, 128)).astype(np.float32), "w")
    b = numpy_helper.from_array(rng.normal(0, 0.1, (128,)).astype(np.float32), "b")
    graph = helper.make_graph(
        [helper.make_node("MatMul", ["x", "w"], ["h"]), helper.make_node("Add", ["h", "b"], ["y"])],
        "tiny",
        [helper.make_tensor_value_info("x", TensorProto.FLOAT, [None, 64])],
        [helper.make_tensor_value_info("y", TensorProto.FLOAT, [None, 128])],
        [w, b],
    )
    model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)])
    model.ir_version = 8  # как у экспортов моделей; новые IR onnxruntime может ещё не знать
    onnx.save(model, str(path))


def test_fp16_storage_keeps_outputs(tmp_path):
    src, dst = tmp_path / "m.onnx", tmp_path / "m16.onnx"
    tiny_model(src)
    load_fp16().to_fp16_storage(src, dst, min_elements=1000)
    x = np.random.default_rng(1).normal(size=(4, 64)).astype(np.float32)
    a = ort.InferenceSession(str(src)).run(None, {"x": x})[0]
    b = ort.InferenceSession(str(dst)).run(None, {"x": x})[0]
    np.testing.assert_allclose(a, b, atol=2e-3)
    stored = {i.name: i.data_type for i in onnx.load(str(dst)).graph.initializer}
    # Большой вес хранится в float16, маленький (меньше порога) остаётся float32.
    assert stored == {"w__fp16": TensorProto.FLOAT16, "b": TensorProto.FLOAT}


def test_ensure_adopts_verified_file_and_rejects_bad_sum(tmp_path, monkeypatch):
    data = b"model bytes"
    good = ModelFile(
        "x/a.bin", "https://example.invalid/a.bin", hashlib.sha256(data).hexdigest(), len(data)
    )
    (tmp_path / "x").mkdir()
    (tmp_path / "x/a.bin").write_bytes(data)
    lock: dict = {}
    monkeypatch.setattr(download, "_fetch", lambda *a, **k: pytest.fail("не должен качать"))
    download._ensure(good, tmp_path, lock, force=False)
    assert lock["x/a.bin"]["sha256"] == good.sha256

    bad = ModelFile("x/b.bin", "https://example.invalid/b.bin", "0" * 64, len(data))

    def fake_fetch(url, dest, size, **_):
        dest.write_bytes(data)

    monkeypatch.setattr(download, "_fetch", fake_fetch)
    with pytest.raises(download.DownloadError):
        download._ensure(bad, tmp_path, lock, force=False)
    assert not (tmp_path / "x/b.bin").exists()


def test_verify_reports_changes(tmp_path, monkeypatch):
    data = b"abc"
    f = ModelFile("a.bin", "u", hashlib.sha256(data).hexdigest(), 3)
    monkeypatch.setattr(download, "all_files", lambda: [f])
    (tmp_path / "a.bin").write_bytes(data)
    (tmp_path / download.LOCK_NAME).write_text(
        json.dumps({"a.bin": {"sourceSha256": f.sha256, "sha256": f.sha256}})
    )
    assert download.verify(tmp_path) == []
    (tmp_path / "a.bin").write_bytes(b"abd")
    assert download.verify(tmp_path) == ["a.bin: файл изменён после скачивания"]


def test_exporter_pins_same_checkpoint_as_manifest():
    # Экспортёр не импортирует код сервиса (ради кэша сборки) — константы сверяются здесь.
    tree = ast.parse((EXPORT / "export_clap.py").read_text())
    consts = {
        t.id: ast.literal_eval(node.value)
        for node in tree.body
        if isinstance(node, ast.Assign)
        for t in node.targets
        if isinstance(t, ast.Name) and t.id.startswith("CHECKPOINT_")
    }
    assert consts["CHECKPOINT_URL"] == CLAP_CHECKPOINT["url"]
    assert consts["CHECKPOINT_SHA256"] == CLAP_CHECKPOINT["sha256"]
    assert consts["CHECKPOINT_SIZE"] == CLAP_CHECKPOINT["size"]
