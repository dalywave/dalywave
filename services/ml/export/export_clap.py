"""Экспорт CLAP «larger_clap_music» в ONNX из исходного чекпойнта LAION.

Зачем свой экспорт. Веса `laion/larger_clap_music` на Hugging Face сконвертированы с ошибкой:
любые тексты дают почти один и тот же вектор (косинус между «calm piano» и «aggressive
metal» — 0.999), а аудиовектор не совпадает с исходной моделью (косинус ≈ 0). Значит, поиск
по описанию на них не работает. Исходный чекпойнт LAION `music_audioset_epoch_15_esc_90.14.pt`
(CC0, lukewys/laion_clap) — та же модель «larger CLAP, music», и он корректен. Скрипт:

1. качает чекпойнт и проверяет sha256 (или берёт локальный `--checkpoint`);
2. читает его безопасно (`weights_only=True`, разрешены только скаляры и dtype numpy);
3. переименовывает ключи в формат `transformers.ClapModel` (как convert_clap_original_pytorch_to_hf,
   но со строгой проверкой: ни одного пропущенного или лишнего веса);
4. экспортирует аудио- и текстовую башни в ONNX (вход — лог-мел [n, 1, 1001, 64] и id токенов,
   выход — `audio_embeds` / `text_embeds` до нормировки), сверяет ONNX с PyTorch;
5. перепаковывает веса в float16 (считается по-прежнему в float32) и записывает файлы и их
   суммы в `<out>/clap` и `<out>/manifest.lock.json`.

Окружение — не сервисное: нужны torch (CPU) и transformers 4.x (export/requirements.txt).
Скрипт не импортирует код сервиса, чтобы слой сборки с ним не пересобирался (и не качал
2,35 ГБ заново) от правок сервиса; адрес и sha256 чекпойнта продублированы в
`dalywave_ml.manifest.CLAP_CHECKPOINT`, совпадение проверяет тест.

    uv run --with torch==2.11.0 --with transformers==4.57.6 \
        python export/export_clap.py --out models
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
import tempfile
import time
from pathlib import Path

import httpx
import numpy as np
import torch
from transformers import ClapConfig, ClapModel

sys.path.insert(0, str(Path(__file__).resolve().parent))
from fp16 import to_fp16_storage

CHECKPOINT_URL = (
    "https://huggingface.co/lukewys/laion_clap/resolve/"
    "b3708341862f581175dba5c356a4ebf74a9b6651/music_audioset_epoch_15_esc_90.14.pt"
)
CHECKPOINT_SHA256 = "fae3e9c087f2909c28a09dc31c8dfcdacbc42ba44c70e972b58c1bd1caf6dedd"
CHECKPOINT_SIZE = 2352471003
LOCK_NAME = "manifest.lock.json"

RENAMES = [
    ("text_branch", "text_model"),
    ("audio_branch", "audio_model.audio_encoder"),
    ("attn", "attention.self"),
    ("self.proj", "output.dense"),
    ("attention.self_mask", "attn_mask"),
    ("mlp.fc1", "intermediate.dense"),
    ("mlp.fc2", "output.dense"),
    ("norm1", "layernorm_before"),
    ("norm2", "layernorm_after"),
    ("bn0", "batch_norm"),
]
# Веса, которых нет в ClapModel: STFT и мел-фильтры (признаки считаются снаружи), голова
# классификации AudioSet у HTSAT (эмбеддинг CLAP берётся до неё), маски сдвинутых окон
# (transformers строит их сам) и MLP, которые в LAION нужны только для функции потерь.
SKIP = (
    "audio_branch.spectrogram_extractor.",
    "audio_branch.logmel_extractor.",
    "audio_branch.tscam_conv.",
    "audio_branch.head.",
    "text_transform.",
    "audio_transform.",
    "text_branch.embeddings.position_ids",
)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def fetch(url: str, dest: Path, size: int, attempts: int = 6) -> None:
    """Скачивание с докачкой по Range: 2,35 ГБ с CDN иногда обрываются."""
    for attempt in range(1, attempts + 1):
        have = dest.stat().st_size if dest.exists() else 0
        if have >= size:
            return
        headers = {"Range": f"bytes={have}-"} if have else {}
        try:
            with httpx.stream("GET", url, headers=headers, follow_redirects=True, timeout=60) as r:
                if r.status_code not in (200, 206):
                    raise httpx.HTTPStatusError(
                        f"HTTP {r.status_code}", request=r.request, response=r
                    )
                with dest.open("ab" if r.status_code == 206 else "wb") as f:
                    for chunk in r.iter_bytes(1 << 20):
                        f.write(chunk)
        except httpx.HTTPError as e:
            if attempt == attempts:
                raise SystemExit(f"не удалось скачать {url}: {e}") from e
            print(f"скачивание прервалось ({e}), повтор", flush=True)
            time.sleep(min(60, 2**attempt))


def load_original(path: Path) -> dict[str, torch.Tensor]:
    allowed = [
        (np._core.multiarray.scalar, "numpy.core.multiarray.scalar"),
        (np.dtype, "numpy.dtype"),
        np.dtypes.Float64DType,
        np.dtypes.Float32DType,
        np.dtypes.Int64DType,
    ]
    with torch.serialization.safe_globals(allowed):
        checkpoint = torch.load(path, map_location="cpu", weights_only=True)
    return {k.removeprefix("module."): v for k, v in checkpoint["state_dict"].items()}


def rename(state: dict[str, torch.Tensor]) -> dict[str, torch.Tensor]:
    out: dict[str, torch.Tensor] = {}
    for key, value in state.items():
        if key.startswith(SKIP) or key.endswith(".attn_mask"):
            continue
        for old, new in RENAMES:
            key = key.replace(old, new)
        m = re.match(r"(text|audio)_projection\.(\d+)\.(.*)", key)
        if m:
            key = f"{m[1]}_projection.linear{1 if m[2] == '0' else 2}.{m[3]}"
        if "qkv" in key:
            third = value.size(0) // 3
            for i, name in enumerate(("query", "key", "value")):
                out[key.replace("qkv", name)] = value[i * third : (i + 1) * third]
            continue
        out[key] = value
    return out


def build(state: dict[str, torch.Tensor]) -> ClapModel:
    config = ClapConfig(
        text_config={"hidden_size": 768, "layer_norm_eps": 1e-5},
        audio_config={
            "patch_embeds_hidden_size": 128,
            "depths": [2, 2, 12, 2],
            "hidden_size": 1024,
            "enable_fusion": False,
        },
        projection_dim=512,
        projection_hidden_act="relu",
    )
    model = ClapModel(config).eval()
    result = model.load_state_dict(rename(state), strict=False)
    missing = [k for k in result.missing_keys if not k.endswith(("position_ids", "token_type_ids"))]
    if missing or result.unexpected_keys:
        raise SystemExit(f"веса не сошлись: нет {missing}, лишние {result.unexpected_keys}")
    return model


class AudioTower(torch.nn.Module):
    def __init__(self, model: ClapModel) -> None:
        super().__init__()
        self.model = model

    def forward(self, input_features: torch.Tensor) -> torch.Tensor:
        pooled = self.model.audio_model(input_features=input_features).pooler_output
        return self.model.audio_projection(pooled)


class TextTower(torch.nn.Module):
    def __init__(self, model: ClapModel) -> None:
        super().__init__()
        self.model = model

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        pooled = self.model.text_model(
            input_ids=input_ids, attention_mask=attention_mask
        ).pooler_output
        return self.model.text_projection(pooled)


def export(model: ClapModel, tmp: Path) -> tuple[Path, Path]:
    audio_path, text_path = tmp / "audio_model.onnx", tmp / "text_model.onnx"
    feats = torch.randn(2, 1, 1001, 64) * 10 - 40
    # Обёртки — в режиме eval: экспорт по выходу восстанавливает режим корневого модуля
    # рекурсивно, и обёртка в режиме train включила бы dropout во всей модели.
    torch.onnx.export(
        AudioTower(model).eval(),
        (feats,),
        str(audio_path),
        input_names=["input_features"],
        output_names=["audio_embeds"],
        dynamic_axes={"input_features": {0: "batch"}, "audio_embeds": {0: "batch"}},
        opset_version=17,
        dynamo=False,
    )
    ids = torch.tensor([[0, 22045, 2490, 2]], dtype=torch.long)
    torch.onnx.export(
        TextTower(model).eval(),
        (ids, torch.ones_like(ids)),
        str(text_path),
        input_names=["input_ids", "attention_mask"],
        output_names=["text_embeds"],
        dynamic_axes={
            "input_ids": {0: "batch", 1: "sequence"},
            "attention_mask": {0: "batch", 1: "sequence"},
            "text_embeds": {0: "batch"},
        },
        opset_version=17,
        dynamo=False,
    )
    return audio_path, text_path


def check(model: ClapModel, audio_path: Path, text_path: Path) -> dict[str, float]:
    import onnxruntime as ort

    def cos(a: np.ndarray, b: np.ndarray) -> float:
        a = a / np.linalg.norm(a, axis=1, keepdims=True)
        b = b / np.linalg.norm(b, axis=1, keepdims=True)
        return float((a * b).sum(1).min())

    rng = np.random.default_rng(0)
    feats = (rng.normal(size=(3, 1, 1001, 64)) * 12 - 35).astype(np.float32)
    model.eval()
    with torch.no_grad():
        ref_a = AudioTower(model).eval()(torch.from_numpy(feats)).numpy()
    got_a = ort.InferenceSession(str(audio_path)).run(None, {"input_features": feats})[0]
    # Второй пример дополнен паддингом: маска внимания должна его гасить.
    ids = np.array([[0, 13700, 2730, 3280, 2], [0, 22045, 2490, 2, 1]], dtype=np.int64)
    mask = np.array([[1, 1, 1, 1, 1], [1, 1, 1, 1, 0]], dtype=np.int64)
    with torch.no_grad():
        ref_t = TextTower(model).eval()(torch.from_numpy(ids), torch.from_numpy(mask)).numpy()
    got_t = ort.InferenceSession(str(text_path)).run(
        None, {"input_ids": ids, "attention_mask": mask}
    )[0]
    result = {"audioCos": cos(ref_a, got_a), "textCos": cos(ref_t, got_t)}
    if min(result.values()) < 0.9999:
        raise SystemExit(f"ONNX расходится с PyTorch: {result}")
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True, help="папка моделей (внутри — clap/)")
    parser.add_argument("--checkpoint", type=Path, help="локальный чекпойнт вместо скачивания")
    args = parser.parse_args()

    url, sha256, size = CHECKPOINT_URL, CHECKPOINT_SHA256, CHECKPOINT_SIZE
    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp = Path(tmp_dir)
        checkpoint = args.checkpoint
        if checkpoint is None:
            checkpoint = tmp / "checkpoint.pt"
            print(f"качаю {url}", flush=True)
            fetch(url, checkpoint, size)
        actual = sha256_file(checkpoint)
        if actual != sha256:
            raise SystemExit(f"sha256 чекпойнта {actual}, ожидалась {sha256}")
        model = build(load_original(checkpoint))
        if args.checkpoint is None:
            checkpoint.unlink()
        audio_path, text_path = export(model, tmp)
        print(json.dumps({"onnxVsTorch": check(model, audio_path, text_path)}), flush=True)

        dest = args.out / "clap"
        dest.mkdir(parents=True, exist_ok=True)
        lock_path = args.out / LOCK_NAME
        lock = json.loads(lock_path.read_text()) if lock_path.exists() else {}
        for src in (audio_path, text_path):
            target = dest / src.name
            to_fp16_storage(src, tmp / f"{src.stem}.fp16.onnx")
            shutil.move(tmp / f"{src.stem}.fp16.onnx", target)
            lock[f"clap/{src.name}"] = {
                "url": url,
                "sourceSha256": sha256,
                "sha256": sha256_file(target),
                "size": target.stat().st_size,
                "fp16Storage": True,
                "exportedWith": {"torch": torch.__version__, "transformers": _tf_version()},
            }
        lock_path.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n")
    print(
        json.dumps({k: v["sha256"] for k, v in lock.items() if k.startswith("clap/")}), flush=True
    )


def _tf_version() -> str:
    import transformers

    return transformers.__version__


if __name__ == "__main__":
    main()
