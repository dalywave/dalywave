"""Эталон CLAP от исходного кода LAION (`laion_clap`) для сверки нашего ONNX.

Запускается в отдельном окружении с torch, transformers 4.x и laion-clap (без зависимостей
обучения; нужен torchvision для импорта):

    /tmp/ref-laion/bin/python scripts/reference_clap.py \
        --checkpoint music_audioset_epoch_15_esc_90.14.pt \
        --input bench/out/clap/input.npz --out bench/out/clap/reference.npz

Веса читаются безопасно (`weights_only=True`), а не через `CLAP_Module.load_ckpt`, который
распаковывает pickle целиком. `input.npz` готовит `scripts/validate_clap.py prepare`.
"""

from __future__ import annotations

import argparse
import json

import laion_clap
import numpy as np
import torch
from transformers import RobertaTokenizer


def load_original(path: str) -> dict[str, torch.Tensor]:
    allowed = [
        (np._core.multiarray.scalar, "numpy.core.multiarray.scalar"),
        (np.dtype, "numpy.dtype"),
        np.dtypes.Float64DType,
        np.dtypes.Float32DType,
        np.dtypes.Int64DType,
    ]
    with torch.serialization.safe_globals(allowed):
        checkpoint = torch.load(path, map_location="cpu", weights_only=True)
    state = {k.removeprefix("module."): v for k, v in checkpoint["state_dict"].items()}
    state.pop("text_branch.embeddings.position_ids", None)
    return state


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--checkpoint", required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    torch.set_num_threads(8)

    clap = laion_clap.CLAP_Module(enable_fusion=False, amodel="HTSAT-base", device="cpu")
    result = clap.model.load_state_dict(load_original(args.checkpoint), strict=False)
    assert not result.unexpected_keys, result.unexpected_keys
    assert all("position_ids" in k for k in result.missing_keys), result.missing_keys
    clap.model.eval()
    branch = clap.model.audio_branch

    data = np.load(args.input)
    texts = [str(t) for t in data["texts"]]
    feats, embeds = [], []
    for i in range(int(data["n"])):
        signal = data[f"signal_{i}"]
        windows = np.stack([signal[int(s) : int(s) + 480000] for s in data[f"starts_{i}"]])
        # Как get_audio_embedding_from_data: int16 туда и обратно, затем модель.
        embeds.append(clap.get_audio_embedding_from_data(windows.astype(np.float32)))
        quant = torch.from_numpy(
            (np.clip(windows, -1, 1) * 32767.0).astype(np.int16).astype(np.float32) / 32767.0
        )
        with torch.no_grad():
            mel = branch.logmel_extractor(branch.spectrogram_extractor(quant))
        feats.append(mel[:, 0].numpy())

    with torch.no_grad():
        text_emb = clap.get_text_embedding(texts)
    tokenizer = RobertaTokenizer.from_pretrained("roberta-base")
    ids = [tokenizer(t)["input_ids"] for t in texts]
    np.savez_compressed(
        args.out,
        features=np.concatenate(feats),
        audio_embeds=np.concatenate(embeds),
        text_embeds=np.asarray(text_emb),
        text_ids=np.array(json.dumps(ids)),
    )
    print(json.dumps({"windows": sum(len(e) for e in embeds), "texts": len(texts)}))


if __name__ == "__main__":
    main()
