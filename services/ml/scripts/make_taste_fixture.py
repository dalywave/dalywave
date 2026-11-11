"""Крошечная сеть вкуса для тестов сервера (crates/server/tests/fixtures/taste/tiny.onnx).

Та же форма входа, что у настоящей (`features [N, 1280 + 512 + число признаков]` → `probs [N, 3]`),
но сжатие до 2 + 2 измерений и один скрытый слой: файл — десятки килобайт. Веса заданы так, что
сеть «любит» первую ось звучания: чем больше `sound[0]`, тем выше дослушивание и лайк.

    uv run python scripts/make_taste_fixture.py \
        ../../crates/server/tests/fixtures/taste/tiny.onnx 36
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

from dalywave_ml.taste_model import Params, export

SOUND, SEMANTIC = 1280, 512


def main() -> None:
    out = Path(sys.argv[1])
    scalars = int(sys.argv[2])
    sound_proj = np.zeros((SOUND, 2), dtype=np.float32)
    sound_proj[0, 0] = 1.0
    sound_proj[1, 1] = 1.0
    hidden = 4
    w0 = np.zeros((2 + 2 + scalars, hidden), dtype=np.float32)
    w0[0, 0] = 3.0  # первая ось звучания → «нравится»
    w0[0, 1] = -3.0  # → «не нравится»
    w1 = np.zeros((hidden, 3), dtype=np.float32)
    w1[1, 0] = 2.0  # skip
    w1[0, 1] = 2.0  # complete
    w1[0, 2] = 1.5  # like
    p = Params(
        sound_mean=np.zeros(SOUND, dtype=np.float32),
        sound_proj=sound_proj,
        semantic_mean=np.zeros(SEMANTIC, dtype=np.float32),
        semantic_proj=np.zeros((SEMANTIC, 2), dtype=np.float32),
        scalar_mean=np.zeros(scalars, dtype=np.float32),
        scalar_scale=np.ones(scalars, dtype=np.float32),
        weights=[w0, w1],
        biases=[np.zeros(hidden, dtype=np.float32), np.array([0.0, -1.0, -2.0], dtype=np.float32)],
        sound_dim=SOUND,
        semantic_dim=SEMANTIC,
    )
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(export(p))
    print(f"{out}: {out.stat().st_size} байт, вход {p.input_dim}")


if __name__ == "__main__":
    main()
