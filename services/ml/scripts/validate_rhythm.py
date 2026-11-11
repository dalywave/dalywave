"""Сверка Beat This! и S-KEY (ONNX + наш код вокруг) с эталонными пакетами на PyTorch.

    uv run python scripts/validate_rhythm.py prepare \
        --audio-dir ../../design/fixtures/audio --out bench/out/rhythm
    <ref-torch>/bin/python scripts/reference_beats.py \
        --input bench/out/rhythm/input.npz --out bench/out/rhythm/reference.npz
    uv run python scripts/validate_rhythm.py compare --out bench/out/rhythm

Кроме превью (30 с — два куска сети) проверяются склейка всех превью (150 с — шесть кусков
и сдвинутый последний) и короткий отрывок 12 с (один неполный кусок).
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import onnxruntime as ort

from dalywave_ml.audio import decode
from dalywave_ml.beats import BeatThis, log_mel
from dalywave_ml.key import SKey


def prepare(audio_dir: Path, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    signals = [decode(p).at(22050) for p in sorted(audio_dir.glob("*.mp3"))]
    names = [p.stem for p in sorted(audio_dir.glob("*.mp3"))]
    signals.append(np.concatenate(signals))
    names.append("concat")
    signals.append(signals[0][: 12 * 22050])
    names.append("short12s")
    arrays = {f"signal_{i}": s for i, s in enumerate(signals)}
    arrays.update({f"name_{i}": np.array(n) for i, n in enumerate(names)})
    arrays["n"] = np.array(len(signals))
    np.savez(out / "input.npz", **arrays)
    print(json.dumps({"signals": names}))


def f_measure(ref: np.ndarray, est: np.ndarray, tol: float = 0.07) -> float:
    if len(ref) == 0 and len(est) == 0:
        return 1.0
    if len(ref) == 0 or len(est) == 0:
        return 0.0
    used = np.zeros(len(est), dtype=bool)
    hits = 0
    for r in ref:
        d = np.abs(est - r)
        d[used] = np.inf
        j = int(np.argmin(d))
        if d[j] <= tol:
            used[j] = True
            hits += 1
    p, r = hits / len(est), hits / len(ref)
    return 0.0 if hits == 0 else 2 * p * r / (p + r)


def compare(out: Path, models: Path) -> None:
    data = np.load(out / "input.npz")
    ref = np.load(out / "reference.npz")
    opts = ort.SessionOptions()
    beat_model = BeatThis(models / "beat_this", opts)
    skey = SKey(models / "skey", opts)
    rows = []
    for i in range(int(data["n"])):
        signal = data[f"signal_{i}"]
        spect = log_mel(signal, beat_model.filterbank)
        grid = beat_model.track(spect)
        rb, rd = ref[f"beats_{i}"], ref[f"downbeats_{i}"]
        p = skey.probabilities(signal)
        assert p is not None
        rp = ref[f"skey_{i}"]
        row = {
            "clip": str(data[f"name_{i}"]),
            "frames": f"{len(spect)}/{len(ref[f'spect_{i}'])}",
            "melMaxAbs": float(np.abs(spect - ref[f"spect_{i}"]).max()),
            "beatLogitMaxAbs": float(np.abs(grid.beat_logits - ref[f"beat_{i}"]).max()),
            "downLogitMaxAbs": float(np.abs(grid.downbeat_logits - ref[f"down_{i}"]).max()),
            "beats": f"{len(grid.beats)}/{len(rb)}",
            "downbeats": f"{len(grid.downbeats)}/{len(rd)}",
            "beatMaxDiffS": float(np.abs(grid.beats - rb).max())
            if len(rb) == len(grid.beats) and len(rb)
            else None,
            "beatF": f_measure(rb, grid.beats),
            "downF": f_measure(rd, grid.downbeats),
            "key": f"{skey.key_map[int(p.argmax())]}/{skey.key_map[int(rp.argmax())]}",
            "keyProbMaxAbs": float(np.abs(p - rp).max()),
            "keyConfidence": float(p.max()),
        }
        rows.append(row)
        print(json.dumps(row, ensure_ascii=False))
    print(
        json.dumps(
            {
                "summary": {
                    "ckptSha256": str(ref["ckpt_sha256"]),
                    "beatF": min(r["beatF"] for r in rows),
                    "downF": min(r["downF"] for r in rows),
                    "logitMaxAbs": max(r["beatLogitMaxAbs"] for r in rows),
                    "keysEqual": all(
                        r["key"].split("/")[0] == r["key"].split("/")[1] for r in rows
                    ),
                    "keyProbMaxAbs": max(r["keyProbMaxAbs"] for r in rows),
                }
            }
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("prepare")
    p.add_argument("--audio-dir", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    c = sub.add_parser("compare")
    c.add_argument("--out", type=Path, required=True)
    c.add_argument("--models", type=Path, default=Path("models"))
    args = parser.parse_args()
    if args.cmd == "prepare":
        prepare(args.audio_dir, args.out)
    else:
        compare(args.out, args.models)


if __name__ == "__main__":
    main()
