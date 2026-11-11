"""Эталон Discogs-EffNet от настоящей Essentia (essentia-tensorflow).

Запускается в отдельном окружении с essentia-tensorflow (колёса есть для macOS arm64 и
linux x86_64, но не для linux aarch64 — поэтому сервис Essentia не использует):

    uv venv --python 3.14 /tmp/ref-ess
    VIRTUAL_ENV=/tmp/ref-ess uv pip install essentia-tensorflow
    /tmp/ref-ess/bin/python scripts/reference_essentia.py --pb-dir <папка с .pb> \
        --out bench/out/essentia design/fixtures/audio/*.mp3

Для каждого файла пишет `<имя>.npz`: сигнал 16 кГц из MonoLoader, мел-полосы
TensorflowInputMusiCNN (потоковый FrameCutter, как внутри TensorflowPredictEffnetDiscogs),
эмбеддинги и активации EffNet (bs64 и bs1) и выходы голов. С `--synthetic` дополнительно
пишет эталон мел-полос для синтетического сигнала — его хранят тесты.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import essentia
import essentia.standard as es
import essentia.streaming as ess
import numpy as np

HEADS = {
    "mood_happy": "model/Softmax",
    "mood_sad": "model/Softmax",
    "mood_aggressive": "model/Softmax",
    "mood_relaxed": "model/Softmax",
    "mood_party": "model/Softmax",
    "danceability": "model/Softmax",
    "voice_instrumental": "model/Softmax",
    "approachability_regression": "model/Identity",
    "engagement_regression": "model/Identity",
}


def musicnn_bands(signal: np.ndarray) -> np.ndarray:
    """Мел-полосы так же, как их считает TensorflowPredictEffnetDiscogs внутри себя."""
    vector = ess.VectorInput(essentia.array(signal))
    cutter = ess.FrameCutter(frameSize=512, hopSize=256)
    bands = ess.TensorflowInputMusiCNN()
    pool = essentia.Pool()
    vector.data >> cutter.signal
    cutter.frame >> bands.frame
    bands.bands >> (pool, "bands")
    essentia.run(vector)
    return np.asarray(pool["bands"], dtype=np.float32)


def synthetic_signal() -> np.ndarray:
    """3 с: свип, аккорд, шумовой всплеск и тишина — детерминированно, без RNG numpy."""
    sr = 16000
    t = np.arange(int(sr * 0.75)) / sr
    sweep = 0.5 * np.sin(2 * np.pi * (50 * t + (7950 - 50) / (2 * 0.75) * t**2))
    chord = sum(0.2 * np.sin(2 * np.pi * f * t) for f in (220.0, 277.18, 329.63, 440.0))
    # Детерминированный «шум»: логистическое отображение, а не генератор случайных чисел.
    x = np.empty(len(t))
    v = 0.37
    for i in range(len(t)):
        v = 3.99 * v * (1 - v)
        x[i] = v - 0.5
    burst = 0.6 * x * np.hanning(len(t))
    silence = np.zeros(len(t))
    return np.concatenate([sweep, chord, burst, silence]).astype(np.float32)


def run_file(path: Path, pb_dir: Path, out: Path) -> None:
    audio = es.MonoLoader(filename=str(path), sampleRate=16000, resampleQuality=4)()
    result: dict[str, np.ndarray] = {"audio16k": audio, "bands": musicnn_bands(audio)}
    for bs in ("bs64",):
        graph = str(pb_dir / f"discogs-effnet-{bs}-1.pb")
        emb = es.TensorflowPredictEffnetDiscogs(graphFilename=graph, output="PartitionedCall:1")(
            audio
        )
        act = es.TensorflowPredictEffnetDiscogs(graphFilename=graph, output="PartitionedCall:0")(
            audio
        )
        result[f"emb_{bs}"] = np.asarray(emb, dtype=np.float32)
        result[f"act_{bs}"] = np.asarray(act, dtype=np.float32)
    emb = result["emb_bs64"]
    for name, output in HEADS.items():
        head = es.TensorflowPredict2D(
            graphFilename=str(pb_dir / f"{name}-discogs-effnet-1.pb"),
            input="model/Placeholder",
            output=output,
        )
        result[f"head_{name}"] = np.asarray(head(emb), dtype=np.float32)
    np.savez_compressed(out / f"{path.stem}.npz", **result)
    print(
        json.dumps(
            {
                "file": path.name,
                "samples": len(audio),
                "frames": result["bands"].shape[0],
                "patches": emb.shape[0],
            }
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("files", nargs="*", type=Path)
    parser.add_argument("--pb-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--synthetic", type=Path, help="куда записать эталон для тестов (.npz)")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    print(json.dumps({"essentia": essentia.__version__}))
    for path in args.files:
        run_file(path, args.pb_dir, args.out)
    if args.synthetic:
        signal = synthetic_signal()
        np.savez_compressed(
            args.synthetic,
            signal=signal,
            bands=musicnn_bands(signal),
            essentia=essentia.__version__,
        )
        print(json.dumps({"synthetic": str(args.synthetic), "samples": len(signal)}))


if __name__ == "__main__":
    main()
