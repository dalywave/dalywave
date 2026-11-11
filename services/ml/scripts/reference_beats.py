"""Эталон Beat This! (пакет beat_this, PyTorch) и S-KEY (пакет skey) для сверки ONNX.

Запускается в окружении с torch, torchaudio, beat_this и skey:

    /tmp/ref-torch/bin/python scripts/reference_beats.py --input bench/out/rhythm/input.npz \
        --out bench/out/rhythm/reference.npz

`input.npz` готовит `scripts/validate_rhythm.py prepare`: сигналы 22 050 Гц.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
import torch
from beat_this.inference import Audio2Frames
from beat_this.model.postprocessor import Postprocessor


def skey_probs(signal: np.ndarray, ckpt: Path | None) -> np.ndarray:
    """Выход ChromaNet (уже softmax) так, как его считает `skey.key_detection.infer_key`."""
    from skey.key_detection import load_checkpoint, load_model_components

    hcqt, chromanet, crop_fn = load_model_components(load_checkpoint(ckpt), torch.device("cpu"))
    x = torch.tensor(signal, dtype=torch.float32)[None, None]
    x = x / x.abs().max()
    with torch.no_grad():
        out = chromanet(crop_fn(hcqt(x), torch.zeros(1)))
    return out.mean(dim=0).numpy()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--skey-ckpt", type=Path, default=None, help="по умолчанию — из пакета")
    args = parser.parse_args()
    torch.set_num_threads(8)

    data = np.load(args.input)
    frames = Audio2Frames("final0", device="cpu")
    post = Postprocessor(type="minimal")
    out: dict[str, np.ndarray] = {}
    for i in range(int(data["n"])):
        signal = data[f"signal_{i}"]
        spect = frames.signal2spect(signal, 22050)
        beat, down = frames.spect2frames(spect)
        beats, downbeats = post(beat, down)
        out[f"spect_{i}"] = spect.numpy()
        out[f"beat_{i}"] = beat.numpy()
        out[f"down_{i}"] = down.numpy()
        out[f"beats_{i}"] = np.asarray(beats)
        out[f"downbeats_{i}"] = np.asarray(downbeats)
        out[f"skey_{i}"] = skey_probs(signal, args.skey_ckpt)
        print(json.dumps({"i": i, "frames": len(spect), "beats": len(beats)}))
    ckpt = Path(torch.hub.get_dir()) / "checkpoints" / "beat_this-final0.ckpt"
    out["ckpt_sha256"] = np.array(hashlib.sha256(ckpt.read_bytes()).hexdigest())
    np.savez_compressed(args.out, **out)


if __name__ == "__main__":
    main()
