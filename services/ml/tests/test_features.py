import numpy as np
import pytest

from dalywave_ml import clap
from dalywave_ml.beats import stft_magnitude
from dalywave_ml.features import (
    energy_curve,
    k_weighting,
    loudness_per_second,
    per_second,
    track_energy,
    valence,
)


def test_k_weighting_shape():
    db = 10 * np.log10(k_weighting(np.array([20.0, 100.0, 1000.0, 4000.0, 10000.0])))
    assert db[0] < -10  # срез низа
    assert abs(db[1]) < 1.5
    assert db[2] == pytest.approx(0.69, abs=0.1)
    assert db[3] == pytest.approx(4.0, abs=0.3)  # полка +4 дБ


def test_loudness_of_sine():
    t = np.arange(22050 * 4) / 22050
    x = (0.5 * np.sin(2 * np.pi * 1000 * t)).astype(np.float32)
    lufs = loudness_per_second(stft_magnitude(x))
    # Средний квадрат 0.125 → −9.72 LUFS, K-фильтр на 1 кГц добавляет +0.69 дБ.
    np.testing.assert_allclose(lufs[1:-1], -9.03, atol=0.05)


def test_per_second_keeps_long_tail():
    assert len(per_second(np.ones(50 * 10 + 30))) == 11
    assert len(per_second(np.ones(50 * 10 + 10))) == 10


def test_energy_curve_and_track_energy_ranges():
    loud = np.linspace(0, 1, 60)
    flux = np.full(60, 0.5)
    curve = energy_curve(loud, flux)
    assert curve.min() >= 0 and curve.max() <= 1
    calm = track_energy(np.full(60, 0.3), {"aggressive": 0.05, "party": 0.1, "relaxed": 0.9})
    loud_track = track_energy(np.full(60, 0.9), {"aggressive": 0.9, "party": 0.8, "relaxed": 0.1})
    assert 0 <= calm < 0.3 < 0.8 < loud_track <= 1


def test_track_energy_ignores_quiet_intro():
    body = np.full(100, 0.8)
    with_intro = np.concatenate([np.full(30, 0.1), body])
    moods = {"aggressive": 0.5, "party": 0.5, "relaxed": 0.5}
    assert track_energy(with_intro, moods) == pytest.approx(track_energy(body, moods), abs=0.01)


def test_valence():
    assert valence({"happy": 0.9, "sad": 0.05, "party": 0.8, "aggressive": 0.1}) > 0.8
    assert valence({"happy": 0.05, "sad": 0.9, "party": 0.1, "aggressive": 0.6}) < 0.2
    assert valence({}) == 0.5


def test_clap_windows():
    sr = clap.SAMPLE_RATE
    # Превью 30 с — три окна почти встык, от начала до самого конца.
    n = int(29.99 * sr)
    starts = clap.window_starts(n, 6)
    assert len(starts) == 3
    assert starts[0] == 0
    assert starts[-1] == n - clap.WINDOW_SAMPLES
    assert abs(starts[1] - (n - clap.WINDOW_SAMPLES) // 2) <= 1
    # Короче окна — одно окно с начала (repeatpad допишет).
    assert clap.window_starts(5 * sr, 6) == [0]
    starts = clap.window_starts(240 * sr, 6)
    assert len(starts) == 6
    assert starts == sorted(starts)
    assert starts[-1] + clap.WINDOW_SAMPLES <= 240 * sr


def test_clap_features_shape_and_floor():
    feats = clap.features(np.zeros(48000 * 3, dtype=np.float32))
    assert feats.shape == (1001, 64)
    assert feats.max() == pytest.approx(-100.0)
