"""Сверка Discogs-EffNet с настоящей Essentia (эталон — scripts/reference_essentia.py).

    uv run python scripts/validate_effnet.py --ref bench/out/essentia \
        --audio-dir ../../design/fixtures/audio

Три уровня:
1. фронтенд: наши мел-полосы против `TensorflowInputMusiCNN` на одном и том же сигнале 16 кГц;
2. модель: эмбеддинги и активации ONNX (наш фронтенд) против TF-графа bs64 внутри Essentia,
   головы ONNX против голов .pb;
3. целиком: наш декодер (ffmpeg + soxr HQ) против `MonoLoader(resampleQuality=4)` —
   здесь расходится только пересэмплер.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import onnxruntime as ort

from dalywave_ml import musicnn
from dalywave_ml.audio import decode

HEADS = [
    "mood_happy",
    "mood_sad",
    "mood_aggressive",
    "mood_relaxed",
    "mood_party",
    "danceability",
    "voice_instrumental",
    "approachability_regression",
    "engagement_regression",
]


def cos(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    a = np.atleast_2d(a).astype(np.float64)
    b = np.atleast_2d(b).astype(np.float64)
    return (a * b).sum(1) / (np.linalg.norm(a, axis=1) * np.linalg.norm(b, axis=1))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ref", type=Path, required=True)
    parser.add_argument("--audio-dir", type=Path, required=True)
    parser.add_argument("--models", type=Path, default=Path("models"))
    args = parser.parse_args()

    opts = ort.SessionOptions()
    effnet = ort.InferenceSession(
        str(args.models / "effnet/discogs-effnet-bsdynamic-1.onnx"), opts, ["CPUExecutionProvider"]
    )
    heads = {
        h: ort.InferenceSession(
            str(args.models / f"heads/{h}-discogs-effnet-1.onnx"), opts, ["CPUExecutionProvider"]
        )
        for h in HEADS
    }

    def run(signal16k: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
        mel = musicnn.bands(signal16k)
        x, _ = musicnn.patches(mel)
        act, emb = effnet.run(None, {"melspectrogram": x})
        return mel, act, emb

    rows = []
    for ref_path in sorted(args.ref.glob("*.npz")):
        ref = np.load(ref_path)
        mel, act, emb = run(ref["audio16k"])
        row: dict[str, object] = {"clip": ref_path.stem}
        row["frames"] = f"{len(mel)}/{len(ref['bands'])}"
        row["melMaxAbs"] = float(np.abs(mel - ref["bands"]).max())
        row["patches"] = f"{len(emb)}/{len(ref['emb_bs64'])}"
        c = cos(emb, ref["emb_bs64"])
        row["embCosMin"] = float(c.min())
        row["trackCos"] = float(cos(emb.mean(0), ref["emb_bs64"].mean(0))[0])
        row["actMaxAbs"] = float(np.abs(act - ref["act_bs64"]).max())
        head_diff = 0.0
        for h, sess in heads.items():
            out = sess.run(None, {"embeddings": ref["emb_bs64"]})[0]
            head_diff = max(head_diff, float(np.abs(out - ref[f"head_{h}"]).max()))
        row["headsMaxAbs"] = head_diff
        # Целиком: свой декодер и пересэмплер против MonoLoader.
        audio = decode(args.audio_dir / f"{ref_path.stem}.mp3")
        _, act2, emb2 = run(audio.at(16000))
        n = min(len(emb2), len(ref["emb_bs64"]))
        row["e2eTrackCos"] = float(cos(emb2[:n].mean(0), ref["emb_bs64"][:n].mean(0))[0])
        row["e2ePatchCosMin"] = float(cos(emb2[:n], ref["emb_bs64"][:n]).min())
        row["e2eActMaxAbs"] = float(np.abs(act2[:n].mean(0) - ref["act_bs64"][:n].mean(0)).max())
        rows.append(row)
        print(json.dumps(row))

    summary = {
        "melMaxAbs": max(r["melMaxAbs"] for r in rows),
        "embCosMin": min(r["embCosMin"] for r in rows),
        "trackCosMin": min(r["trackCos"] for r in rows),
        "headsMaxAbs": max(r["headsMaxAbs"] for r in rows),
        "e2eTrackCosMin": min(r["e2eTrackCos"] for r in rows),
        "e2ePatchCosMin": min(r["e2ePatchCosMin"] for r in rows),
    }
    print(json.dumps({"summary": summary}))


if __name__ == "__main__":
    main()
