import numpy as np
import pytest

from dalywave_ml.features import beats_per_bar, estimate_tempo


def grid(bpm: float, seconds: float, start: float = 0.5, jitter: float = 0.0, seed: int = 0):
    period = 60.0 / bpm
    beats = np.arange(start, seconds, period)
    if jitter:
        beats = beats + np.random.default_rng(seed).uniform(-jitter, jitter, len(beats))
    # Сеть выдаёт доли с шагом кадра 20 мс.
    return np.round(beats * 50) / 50


def test_steady_grid_gives_precise_bpm():
    beats = grid(128.0, 200)
    tempo = estimate_tempo(beats, beats[::4], 200)
    assert tempo.bpm == pytest.approx(128.0, abs=0.05)
    assert tempo.confidence > 0.9
    assert tempo.fold is None


def test_missing_beat_does_not_shift_bpm():
    beats = np.delete(grid(100.0, 120), [40, 41, 90])
    tempo = estimate_tempo(beats, beats[::4], 120)
    assert tempo.bpm == pytest.approx(100.0, abs=0.1)
    assert tempo.confidence > 0.8


def test_double_time_grid_is_halved_with_downbeats_kept():
    beats = grid(240.0, 60, start=0.25)
    downbeats = beats[1::8]  # 8 долей в такте — сеть поймала восьмые
    tempo = estimate_tempo(beats, downbeats, 60)
    assert tempo.fold == "half"
    assert tempo.bpm == pytest.approx(120.0, abs=0.2)
    assert set(np.round(downbeats, 4)) <= set(np.round(tempo.beats, 4))
    assert beats_per_bar(tempo.beats, tempo.downbeats) == 4


def test_seven_beats_per_bar_at_moderate_tempo_is_halved():
    beats = grid(176.0, 60)
    tempo = estimate_tempo(beats, beats[::8], 60)
    assert tempo.fold == "half"
    assert tempo.bpm == pytest.approx(88.0, abs=0.2)


def test_half_time_grid_is_doubled():
    beats = grid(50.0, 120)
    tempo = estimate_tempo(beats, beats[::2], 120)
    assert tempo.fold == "double"
    assert tempo.bpm == pytest.approx(100.0, abs=0.2)
    assert len(tempo.beats) == 2 * len(beats) - 1


def test_irregular_beats_have_low_confidence():
    rng = np.random.default_rng(3)
    beats = np.cumsum(rng.uniform(0.3, 0.9, 80))
    tempo = estimate_tempo(beats, beats[::4], float(beats[-1]))
    assert tempo.confidence < 0.4


def test_beats_only_in_part_of_track_lower_confidence():
    beats = grid(120.0, 40)
    full = estimate_tempo(beats, beats[::4], 40)
    partial = estimate_tempo(beats, beats[::4], 200)
    assert partial.confidence < full.confidence


def test_too_few_beats():
    tempo = estimate_tempo(np.array([0.5, 1.0, 1.5]), np.array([0.5]), 10)
    assert tempo.bpm is None
    assert tempo.confidence == 0.0


def test_weak_activations_lower_confidence():
    beats = grid(120.0, 60)
    strong = estimate_tempo(beats, beats[::4], 60, beat_strength=0.95)
    weak = estimate_tempo(beats, beats[::4], 60, beat_strength=0.55)
    assert weak.confidence < strong.confidence
