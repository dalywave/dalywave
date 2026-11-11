"""Проверки на настоящих моделях и превью Deezer: `uv run pytest -m models`.

Нужны файлы моделей (`ml models download` + `export/export_clap.py`) и превью в
design/fixtures/audio — их нет в git, поэтому тесты помечены и по умолчанию не запускаются.
"""

from __future__ import annotations

import os
import wave
from pathlib import Path

import numpy as np
import pytest

from dalywave_ml.analysis import Analyzer, decode_f16
from dalywave_ml.audio import decode
from dalywave_ml.config import Settings
from dalywave_ml.download import verify

pytestmark = pytest.mark.models

SERVICE = Path(__file__).resolve().parents[1]
FIXTURES = SERVICE.parents[1] / "design/fixtures/audio"
MODELS = Path(os.environ.get("ML_MODELS_DIR", SERVICE / "models"))

# Темп и тональность превью. Сетка долей и тональность совпали с эталонными beat_this и
# skey на этих же файлах (scripts/validate_rhythm.py), BPM — наше правило поверх сетки.
EXPECTED = {
    "3107728361": (90.0, "2A"),
    "3580793301": (81.0, "1B"),
    "3661025372": (110.0, "10A"),
    "3898084851": (141.2, "6A"),
    "4059700721": (104.8, "10A"),
}

TRACK_FIELDS = {
    "source", "models", "durationMs", "sound", "semantic", "bpm", "bpmConfidence",
    "keyCamelot", "key", "keyConfidence", "energy", "danceability", "valence", "moods",
    "voice", "language", "styles", "introEndMs", "outroStartMs", "beatsMs", "downbeatsMs",
    "vocalSegments", "energyCurve", "extra", "timingsMs",
}  # fmt: skip


@pytest.fixture(scope="module")
def analyzer() -> Analyzer:
    problems = verify(MODELS)
    if problems:
        pytest.skip(f"нет моделей в {MODELS}: {problems[0]}")
    return Analyzer(Settings(models_dir=MODELS))


@pytest.fixture(scope="module")
def previews() -> list[Path]:
    files = sorted(FIXTURES.glob("*.mp3"))
    if not files:
        pytest.skip("нет превью в design/fixtures/audio")
    return files


@pytest.fixture(scope="module")
def preview_results(analyzer, previews) -> dict[str, dict]:
    return {p.stem: analyzer.analyze(decode(p), "preview") for p in previews}


def test_preview_shape(preview_results):
    for result in preview_results.values():
        assert set(result) == TRACK_FIELDS
        assert result["source"] == "preview"
        sound, semantic = decode_f16(result["sound"]), decode_f16(result["semantic"])
        assert sound.shape == (1280,) and semantic.shape == (512,)
        assert np.linalg.norm(sound) == pytest.approx(1.0, abs=2e-3)
        assert np.linalg.norm(semantic) == pytest.approx(1.0, abs=2e-3)
        assert set(result["moods"]) == {"happy", "sad", "aggressive", "relaxed", "party"}
        for value in [*result["moods"].values(), result["energy"], result["valence"]]:
            assert 0.0 <= value <= 1.0
        assert len(result["styles"]) == 10
        assert result["styles"][0]["p"] >= result["styles"][-1]["p"]
        # Привязанные ко времени поля у превью пустые: время не ложится на полный файл.
        assert result["beatsMs"] is None and result["energyCurve"] is None


@pytest.mark.parametrize(("stem", "expected"), EXPECTED.items())
def test_preview_tempo_and_key(preview_results, stem, expected):
    bpm, camelot = expected
    result = preview_results[stem]
    assert result["bpm"] == pytest.approx(bpm, abs=0.5)
    assert result["bpmConfidence"] > 0.7
    assert result["keyCamelot"] == camelot


def test_similar_tracks_are_closer_in_sound_space(preview_results):
    vectors = {k: decode_f16(v["sound"]) for k, v in preview_results.items()}
    # Два «замедленных» фонк-трека ближе друг к другу, чем к металкору.
    slowed = vectors["3898084851"] @ vectors["4059700721"]
    rock = vectors["3898084851"] @ vectors["3580793301"]
    assert slowed > rock


def test_text_search(analyzer, preview_results):
    tracks = {k: decode_f16(v["semantic"]) for k, v in preview_results.items()}

    def best(query: str) -> str:
        text = decode_f16(analyzer.embed_text(query)["semantic"])
        return max(tracks, key=lambda k: float(tracks[k] @ text))

    # Металкор Mytch — «тяжёлый рок с гитарами», хаус Sayfalse — «хаус».
    assert best("агрессивный тяжёлый рок с гитарами") == "3580793301"
    assert best("house music") == "4059700721"


def test_russian_query_lands_near_english(analyzer):
    ru = decode_f16(analyzer.embed_text("спокойная расслабляющая музыка")["semantic"])
    calm = decode_f16(analyzer.embed_text("calm relaxing music")["semantic"])
    metal = decode_f16(analyzer.embed_text("aggressive heavy metal")["semantic"])
    assert ru @ calm > 0.95
    assert ru @ metal < ru @ calm


def synthetic_full_track(previews: list[Path], path: Path) -> tuple[float, float, float]:
    """Тихое вступление 16 с, тело из превью, угасающая концовка 20 с. Возвращает границы."""
    sr = 44100
    clips = [decode(p).at(sr) for p in previews]
    intro = clips[0][: 16 * sr] * 0.1
    spec = np.fft.rfft(intro)
    spec[np.fft.rfftfreq(len(intro), 1 / sr) > 300] = 0
    intro = np.fft.irfft(spec, n=len(intro)).astype(np.float32)
    body = np.concatenate(clips[:4])
    outro = clips[4][: 20 * sr] * np.linspace(0.5, 0.0, 20 * sr, dtype=np.float32)
    track = np.clip(np.concatenate([intro, body, outro]), -1, 1)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes((track * 32767).astype("<i2").tobytes())
    return 16.0, 16.0 + len(body) / sr, len(track) / sr


def test_full_track(analyzer, previews, tmp_path):
    intro_end, outro_start, duration = synthetic_full_track(previews, tmp_path / "full.wav")
    result = analyzer.analyze(decode(tmp_path / "full.wav"), "full")
    assert result["source"] == "full"
    assert result["introEndMs"] / 1000 == pytest.approx(intro_end, abs=3.0)
    assert result["outroStartMs"] / 1000 == pytest.approx(outro_start, abs=6.0)
    beats = np.array(result["beatsMs"])
    assert len(beats) > 200 and (np.diff(beats) > 0).all()
    assert set(result["downbeatsMs"]) <= set(result["beatsMs"])
    assert abs(len(result["energyCurve"]) - duration) <= 1
    assert all(0 <= v <= 1000 for v in result["energyCurve"])
    # Вступление тихое: энергия там заметно ниже, чем в теле трека.
    curve = np.array(result["energyCurve"])
    assert curve[:12].mean() < curve[30:120].mean() - 200
    segments = result["vocalSegments"]
    assert segments and all(a < b for a, b in segments)
    assert segments[0][0] >= 10_000  # в отфильтрованном вступлении голоса не слышно
