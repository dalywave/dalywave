"""Перепаковка весов ONNX в float16 без смены точности вычислений.

Отдельный модуль без torch: его импортируют и экспортёр, и тесты.
"""

from __future__ import annotations

from pathlib import Path


def to_fp16_storage(src: Path, dst: Path, min_elements: int = 4096) -> None:
    """Веса float32 → float16 + Cast в float32 перед использованием.

    onnxruntime сворачивает эти Cast при загрузке сессии (ConstantFolding), так что считается
    всё по-прежнему в float32 и с той же скоростью; меняется только округление весов
    (косинус с исходной моделью ≥ 0.9999999).
    """
    import numpy as np
    import onnx
    from onnx import TensorProto, helper, numpy_helper

    model = onnx.load(str(src))
    graph = model.graph
    inputs = {i.name for i in graph.input}
    initializers, casts = [], []
    for init in graph.initializer:
        count = int(np.prod(init.dims)) if init.dims else 1
        if (
            init.data_type == TensorProto.FLOAT
            and count >= min_elements
            and init.name not in inputs
        ):
            arr = numpy_helper.to_array(init)
            if np.isfinite(arr).all() and float(np.abs(arr).max()) < 60000.0:
                half = numpy_helper.from_array(arr.astype(np.float16), f"{init.name}__fp16")
                initializers.append(half)
                casts.append(
                    helper.make_node(
                        "Cast",
                        [half.name],
                        [init.name],
                        to=TensorProto.FLOAT,
                        name=f"{init.name}__cast",
                    )
                )
                continue
        initializers.append(init)
    del graph.initializer[:]
    graph.initializer.extend(initializers)
    nodes = list(graph.node)
    del graph.node[:]
    graph.node.extend(casts + nodes)
    onnx.save(model, str(dst))
