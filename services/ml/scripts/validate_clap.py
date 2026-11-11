"""Сверка CLAP (наш ONNX + признаки на numpy + tokenizers) с исходным `laion_clap`.

    uv run python scripts/validate_clap.py prepare \
        --audio-dir ../../design/fixtures/audio --out bench/out/clap
    <ref-laion>/bin/python scripts/reference_clap.py \
        --checkpoint <music_audioset_epoch_15_esc_90.14.pt> \
        --input bench/out/clap/input.npz --out bench/out/clap/reference.npz
    uv run python scripts/validate_clap.py compare --out bench/out/clap

Кроме совпадения векторов печатает матрицу «звук × текст» — видно, что запросы различаются.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import onnxruntime as ort

from dalywave_ml import clap
from dalywave_ml.audio import decode

TEXTS = [
    "calm instrumental music for work",
    "aggressive phonk with heavy bass",
    "sad indie song for a rainy day",
    "energetic party dance music",
    "russian hip hop",
    "lo-fi hip hop beats to relax/study to",
    "spokoinyi instrumental",
    "Ёлка — праздник",
]


def cos(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    a = np.atleast_2d(a).astype(np.float64)
    b = np.atleast_2d(b).astype(np.float64)
    return (a * b).sum(1) / (np.linalg.norm(a, axis=1) * np.linalg.norm(b, axis=1))


def prepare(audio_dir: Path, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    arrays: dict[str, np.ndarray] = {"texts": np.array(TEXTS)}
    files = sorted(audio_dir.glob("*.mp3"))
    for i, path in enumerate(files):
        signal = decode(path).at(clap.SAMPLE_RATE)
        arrays[f"signal_{i}"] = signal
        arrays[f"starts_{i}"] = np.array(clap.window_starts(len(signal), 8))
        arrays[f"name_{i}"] = np.array(path.stem)
    arrays["n"] = np.array(len(files))
    np.savez(out / "input.npz", **arrays)
    print(json.dumps({"files": len(files), "out": str(out / "input.npz")}))


def compare(out: Path, models: Path) -> None:
    data = np.load(out / "input.npz")
    ref = np.load(out / "reference.npz")
    model = clap.Clap(models / "clap", ort.SessionOptions())
    feats, embeds, track_cos = [], [], []
    k = 0
    for i in range(int(data["n"])):
        signal = data[f"signal_{i}"]
        starts = [int(s) for s in data[f"starts_{i}"]]
        f = np.stack([clap.features(signal[s : s + clap.WINDOW_SAMPLES]) for s in starts])
        feats.append(f)
        e = model.embed_windows(signal, starts)
        embeds.append(e)
        r = ref["audio_embeds"][k : k + len(starts)]
        r = r / np.linalg.norm(r, axis=1, keepdims=True)
        track_cos.append(float(cos(e.mean(0), r.mean(0))[0]))
        k += len(starts)
    feats_all = np.concatenate(feats)
    emb_all = np.concatenate(embeds)
    ids_ref = json.loads(str(ref["text_ids"]))
    ids_ok = all(model.tokenizer.encode(t).ids == ids_ref[j] for j, t in enumerate(TEXTS))
    text = np.stack([model.embed_text(t) for t in TEXTS])
    names = [str(data[f"name_{i}"]) for i in range(int(data["n"]))]
    tracks = np.stack([e.mean(0) / np.linalg.norm(e.mean(0)) for e in embeds])
    sims = tracks @ text.T
    for j, t in enumerate(TEXTS):
        print(f"{t:40} " + " ".join(f"{s:+.3f}" for s in sims[:, j]))
    print(f"{'':40} " + " ".join(n[-6:] for n in names))
    text_cos = text @ text.T
    result = {
        "windows": len(emb_all),
        "featuresMaxAbsDb": float(np.abs(feats_all - ref["features"]).max()),
        "windowCosMin": float(cos(emb_all, ref["audio_embeds"]).min()),
        "trackCosMin": min(track_cos),
        "tokenIdsEqual": ids_ok,
        "textCosMin": float(cos(text, ref["text_embeds"]).min()),
        # Тексты должны различаться: у сломанной конвертации HF все пары давали 0.999.
        "textPairCosMax": float(text_cos[~np.eye(len(TEXTS), dtype=bool)].max()),
    }
    print(json.dumps(result))


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
