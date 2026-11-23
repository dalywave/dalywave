"""Какие файлы моделей нужны сервису: откуда качать, контрольные суммы, версии и лицензии.

Адреса закреплены на ревизиях (Hugging Face) или на неизменяемых файлах (essentia.upf.edu),
sha256 проверяется после скачивания.

CLAP не качается готовым: ONNX-файлы делает `export/export_clap.py` из исходного чекпойнта
LAION (веса `laion/larger_clap_music` на Hugging Face сконвертированы с ошибкой — см. экспортёр).
Такие файлы помечены `exported`: загрузчик их не качает, а проверяет запись экспортёра в
`manifest.lock.json`. Веса CLAP хранятся в float16, считаются в float32 — onnxruntime
сворачивает Cast при загрузке, а файлы и образ вдвое меньше.

Модели MTG (EffNet и головы) — CC BY-NC-SA 4.0, только некоммерческое использование: публичный
образ их не содержит. Они помечены `on_start` и качаются при запуске сервиса (`ml serve`) в свою
папку (`ML_MTG_MODELS_DIR`, в образе — том `/models`), рядом пишется ATTRIBUTION.md.
"""

from __future__ import annotations

from dataclasses import dataclass

ESSENTIA = "https://essentia.upf.edu/models"
HF = "https://huggingface.co"
LAION_CLAP_REV = "b3708341862f581175dba5c356a4ebf74a9b6651"
CLAP_REV = "a0b4534a14f58e20944452dff00a22a06ce629d1"
BEAT_THIS_REV = "4e971bd43753023e1bf961c34a0cb74985cfcb88"
SKEY_REV = "9d90d2a9ff6679df1d64000f4fa750643f247643"

MTG_LICENSE = "CC BY-NC-SA 4.0"
MTG_LICENSE_URL = "https://creativecommons.org/licenses/by-nc-sa/4.0/"
MTG_AUTHORS = "Music Technology Group, Universitat Pompeu Fabra"
MTG_PAPER = (
    'P. Alonso-Jiménez, X. Serra, D. Bogdanov. "Music Representation Learning Based on '
    'Editorial Metadata from Discogs". ISMIR 2022'
)

# Исходный чекпойнт «larger CLAP, music» (HTSAT-base + RoBERTa), CC0.
CLAP_CHECKPOINT = {
    "url": f"{HF}/lukewys/laion_clap/resolve/{LAION_CLAP_REV}/music_audioset_epoch_15_esc_90.14.pt",
    "sha256": "fae3e9c087f2909c28a09dc31c8dfcdacbc42ba44c70e972b58c1bd1caf6dedd",
    "size": 2352471003,
}


@dataclass(frozen=True)
class ModelFile:
    path: str  # относительно папки моделей
    url: str
    sha256: str  # исходного файла
    size: int
    fp16_storage: bool = False
    # Файл делает экспортёр, а не загрузчик (url и sha256 — у исходного чекпойнта).
    exported: bool = False


@dataclass(frozen=True)
class Model:
    key: str
    name: str
    version: str
    license: str
    source: str
    files: tuple[ModelFile, ...]
    # Качается при запуске сервиса, а не при сборке (модели MTG: в публичном образе их нет)
    on_start: bool = False


def _head(
    name: str, folder: str, onnx_sha: str, onnx_size: int, json_sha: str, json_size: int
) -> Model:
    stem = f"{name}-discogs-effnet-1"
    base = f"{ESSENTIA}/classification-heads/{folder}/{stem}"
    return Model(
        key=f"head:{name}",
        name=stem,
        version="1",
        license=MTG_LICENSE,
        source="https://essentia.upf.edu/models.html",
        files=(
            ModelFile(f"heads/{stem}.onnx", f"{base}.onnx", onnx_sha, onnx_size),
            ModelFile(f"heads/{stem}.json", f"{base}.json", json_sha, json_size),
        ),
        on_start=True,
    )


MODELS: tuple[Model, ...] = (
    Model(
        key="effnet",
        name="discogs-effnet-bsdynamic-1",
        version="1",
        license=MTG_LICENSE,
        source="https://essentia.upf.edu/models.html#discogs-effnet",
        files=(
            ModelFile(
                "effnet/discogs-effnet-bsdynamic-1.onnx",
                f"{ESSENTIA}/feature-extractors/discogs-effnet/discogs-effnet-bsdynamic-1.onnx",
                "a280825b334797cf677939db8cd5762c0392aedd0ca6415dbc1cd083f045e43c",
                18027718,
            ),
            ModelFile(
                "effnet/discogs-effnet-bsdynamic-1.json",
                f"{ESSENTIA}/feature-extractors/discogs-effnet/discogs-effnet-bsdynamic-1.json",
                "a2e85b2e7372d5f8e0f35bdd6aeae1139f101087d183d0b2fb60b0ea0f01a0ff",
                14986,
            ),
        ),
        on_start=True,
    ),
    _head(
        "mood_happy",
        "mood_happy",
        "0ca322819ef137b4b87e9866bffe7370a630e6f1165184ec106326cef6f81e06",
        514097,
        "ed4601bc396b23367d29cf45aa327366cdfdbd3e2baeb7a3f7893237f1ffd9e6",
        2350,
    ),
    _head(
        "mood_sad",
        "mood_sad",
        "1a50d11c23181bdfdeabc7d6032a2dad829dfc19c3776d2ee57ef1724cb08805",
        514093,
        "7f2c00099ad8255af33e43ab77d1ac2c86a1dd9f46d3e5ea2a9a52246f5033d9",
        2334,
    ),
    _head(
        "mood_aggressive",
        "mood_aggressive",
        "de36550b5d1660791ad732ed6de6ebfdc3e65dcf50b928b2578ddf103dbfb400",
        514107,
        "81773e95d78db1b93283d73b2d06344d1ff79685b57d9428a40d47fdfcf537b8",
        2386,
    ),
    _head(
        "mood_relaxed",
        "mood_relaxed",
        "8ba6515a1e5943a72b3b475e3a25fc7a2ff04142c3eaa6aa0716fca371efdfff",
        514101,
        "86c0fe1c2c6d49bf08537bc2d3a602204feaede011ed119c1fc6c36270f60e6a",
        2364,
    ),
    _head(
        "mood_party",
        "mood_party",
        "c50ac2106ec2f209dd04ad48756582df0e3f3512235310d1a4a3fcc453745f04",
        514097,
        "a7f15c6e05db7c3e9226646a87162ebb9cc00aea8bdbbc910aedafa7f4a8fdfb",
        2350,
    ),
    _head(
        "danceability",
        "danceability",
        "9ce9b8c44f1dd5df5ffc124e5d41d67acf254232c1b90c7e057e079ab7cead73",
        514101,
        "589e61dc05f0d7935be4b5fa7ab4341e5dc7cfecc6898f83503558564790cd75",
        1989,
    ),
    _head(
        "voice_instrumental",
        "voice_instrumental",
        "20155e4c439714b0c45c08644b73c8e12d9dccb173bd4ab9934bf1e5aee837ca",
        514114,
        "43ac2c3b055dfaed20f6232e0f10636c287f1c5a6e5bd02c5585860031964c8f",
        2013,
    ),
    _head(
        "approachability_regression",
        "approachability",
        "f783b17ee994394f30f27d05ccd3fb845e589d107802e55e0b9cf1ea041ec894",
        513637,
        "39c033e4af6d4f543be420b92e17c8aa3feffe70a88caf2b152d91731320b73f",
        2013,
    ),
    _head(
        "engagement_regression",
        "engagement",
        "9551a6c162c9803aef5ce4e4d29a2e4fae99b5bfdc95dae9dd8fe5335e0fb567",
        513626,
        "6dc6f93a5c023f74bcb4ba38297bf9a4fd9e106d2dd1f913b880ed87fcfbd958",
        1988,
    ),
    Model(
        key="clap",
        name="larger_clap_music",
        version="music_audioset_epoch_15_esc_90.14",
        license="CC0-1.0 (веса LAION), Apache-2.0 (токенизатор из laion/larger_clap_music)",
        source=f"{HF}/lukewys/laion_clap → export/export_clap.py",
        files=(
            ModelFile(
                "clap/audio_model.onnx",
                CLAP_CHECKPOINT["url"],
                CLAP_CHECKPOINT["sha256"],
                CLAP_CHECKPOINT["size"],
                fp16_storage=True,
                exported=True,
            ),
            ModelFile(
                "clap/text_model.onnx",
                CLAP_CHECKPOINT["url"],
                CLAP_CHECKPOINT["sha256"],
                CLAP_CHECKPOINT["size"],
                fp16_storage=True,
                exported=True,
            ),
            ModelFile(
                "clap/tokenizer.json",
                f"{HF}/laion/larger_clap_music/resolve/{CLAP_REV}/tokenizer.json",
                "dc239041d98de27ffc3975473a1a23e3db4c937b23c138c38bbc66588bd247e5",
                2108774,
            ),
        ),
    ),
    Model(
        key="beat_this",
        name="beat-this-final0",
        version=BEAT_THIS_REV[:7],
        license="MIT",
        source=f"https://github.com/CPJKU/beat_this (ONNX: {HF}/musetric/beat-this-onnx)",
        files=(
            ModelFile(
                "beat_this/beat_this.onnx",
                f"{HF}/musetric/beat-this-onnx/resolve/{BEAT_THIS_REV}/beat_this.onnx",
                "078572af6ca47741e06a82d09525d13c793eaa8e311a8cf15e831dcd7e73f218",
                83143431,
            ),
            ModelFile(
                "beat_this/mel-filterbank.bin",
                f"{HF}/musetric/beat-this-onnx/resolve/{BEAT_THIS_REV}/mel-filterbank.bin",
                "1ee975d96f44ccf2c3bfe37825c1c1f0b089f5703c7a12a84b1f0a3bce004533",
                262656,
            ),
        ),
    ),
    Model(
        key="skey",
        name="skey",
        version=SKEY_REV[:7],
        license="MIT",
        source=f"https://github.com/deezer/skey (ONNX: {HF}/musetric/skey-onnx)",
        files=(
            ModelFile(
                "skey/skey.onnx",
                f"{HF}/musetric/skey-onnx/resolve/{SKEY_REV}/skey.onnx",
                "5113c1378c1007c8559fcb767593366ba9794397b060535eb80a113db50530fc",
                338482,
            ),
            ModelFile(
                "skey/config.json",
                f"{HF}/musetric/skey-onnx/resolve/{SKEY_REV}/config.json",
                "20be1e139e1b05dea4bae2e2dde717d593c10c30bb38b300aeedc6693be88a52",
                712,
            ),
        ),
    ),
)


def all_files() -> list[ModelFile]:
    return [f for m in MODELS for f in m.files]


def image_files() -> list[ModelFile]:
    """Файлы, которые кладёт в образ сборка."""
    return [f for m in MODELS if not m.on_start for f in m.files]


def start_files() -> list[ModelFile]:
    """Файлы моделей MTG: качаются при запуске сервиса."""
    return [f for m in MODELS if m.on_start for f in m.files]


def versions() -> dict[str, str]:
    """Версии моделей для поля `models` ответа: по ним воркер решает, что пересчитать."""
    return {
        "sound": "discogs-effnet-bsdynamic-1",
        "heads": "discogs-effnet-1",
        "semantic": "larger_clap_music@laion-music_audioset_epoch_15",
        "beats": f"beat-this-final0@{BEAT_THIS_REV[:7]}",
        "key": f"skey@{SKEY_REV[:7]}",
    }
