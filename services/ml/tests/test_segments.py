import numpy as np
import pytest

from dalywave_ml.features import intro_outro, vocal_segments

STEP = 62 * 0.016  # шаг патчей EffNet, с


def centers(n: int) -> np.ndarray:
    return 1.016 + STEP * np.arange(n)


def test_hysteresis_and_edges():
    voice = np.array([0.1] * 10 + [0.9] * 20 + [0.5] * 3 + [0.9] * 10 + [0.1] * 10)
    segs = vocal_segments(voice, centers(len(voice)), 60.0)
    # 0.5 ниже порога входа (0.6), но выше порога выхода (0.4): сегмент не рвётся.
    assert len(segs) == 1
    start, end = segs[0]
    assert start == pytest.approx(centers(53)[10] - STEP / 2)
    assert end == pytest.approx(centers(53)[42] + STEP / 2)


def test_short_gap_is_merged_long_gap_splits():
    voice = np.array([0.9] * 10 + [0.1] * 2 + [0.9] * 10 + [0.1] * 6 + [0.9] * 10)
    segs = vocal_segments(voice, centers(len(voice)), 60.0)
    assert len(segs) == 2


def test_short_blips_are_dropped():
    voice = np.array([0.1] * 10 + [0.95] + [0.1] * 10)
    # Медианный фильтр убирает одиночный всплеск, а сегмент < 2 с всё равно отбросился бы.
    assert vocal_segments(voice, centers(len(voice)), 25.0) == []


def test_voice_through_whole_track_reaches_edges():
    voice = np.full(29, 0.99)
    segs = vocal_segments(voice, centers(29), 29.99)
    assert segs == [(0.0, 29.99)]


def test_empty():
    assert vocal_segments(np.array([]), np.array([]), 10.0) == []


def curve_with_sections(intro: int, body: int, outro: int) -> np.ndarray:
    return np.concatenate([np.full(intro, 0.3), np.full(body, 0.85), np.linspace(0.5, 0.0, outro)])


def test_intro_outro_from_energy():
    curve = curve_with_sections(16, 180, 20)
    downbeats = np.arange(0.4, 216, 2.0)
    intro, outro = intro_outro(curve, [], downbeats, 216.0)
    # Тело начинается на 16 с и кончается на 196 с; сглаживание сдвигает порог на ~1 с,
    # граница притягивается к сильной доле.
    assert intro == pytest.approx(16.4)
    assert outro == pytest.approx(196.4)


def test_vocals_start_before_body():
    curve = curve_with_sections(30, 160, 20)
    intro, _ = intro_outro(curve, [(12.0, 150.0)], np.array([]), 210.0)
    assert intro == pytest.approx(12.0)


def test_phrase_on_the_fall_is_not_cut_but_fadeout_chorus_is():
    curve = curve_with_sections(10, 150, 30)
    _, outro = intro_outro(curve, [(140.0, 165.0)], np.array([]), 190.0)
    assert outro == pytest.approx(165.0)
    _, outro = intro_outro(curve, [(140.0, 190.0)], np.array([]), 190.0)
    assert outro == pytest.approx(160.0, abs=1.5)


def test_flat_track_has_no_intro_or_outro():
    intro, outro = intro_outro(np.full(120, 0.8), [], np.array([]), 120.0)
    assert intro == 0.0
    assert outro == 120.0


def test_limits():
    # Тело только во второй половине — вступление не длиннее 40 % трека.
    curve = curve_with_sections(100, 100, 0)
    intro, outro = intro_outro(curve, [], np.array([]), 200.0)
    assert intro == pytest.approx(80.0)
    assert outro >= 0.6 * 200
