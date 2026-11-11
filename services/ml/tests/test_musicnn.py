from pathlib import Path

import numpy as np
import pytest

from dalywave_ml import musicnn

DATA = Path(__file__).parent / "data"


def test_bands_match_essentia_reference():
    # Эталон: настоящая Essentia 2.1b6 (FrameCutter + TensorflowInputMusiCNN) на синтетическом
    # сигнале — scripts/reference_essentia.py --synthetic.
    ref = np.load(DATA / "musicnn_reference.npz")
    mel = musicnn.bands(ref["signal"])
    assert mel.shape == ref["bands"].shape
    # log10(1 + 10⁴·x) усиливает погрешность float32 БПФ у тихих полос до ~2e-4.
    assert np.abs(mel - ref["bands"]).max() < 1e-3
    assert np.abs(mel - ref["bands"]).mean() < 1e-5


@pytest.mark.parametrize(("n", "frames"), [(1, 2), (256, 2), (257, 3), (512, 3), (479818, 1876)])
def test_frame_count_like_framecutter(n, frames):
    # Первый кадр центрирован на нуле, последний — первый, чей центр дошёл до конца.
    assert musicnn.frame_count(n) == frames
    assert len(musicnn.bands(np.zeros(n, dtype=np.float32))) == frames


def test_patches_discard_incomplete_tail():
    mel = np.random.default_rng(0).random((1876, 96), dtype=np.float32)
    patches, centers = musicnn.patches(mel)
    # Превью 30 с: 1 + floor((1876 - 128) / 62) = 29 патчей, как у Essentia.
    assert patches.shape == (29, 128, 96)
    np.testing.assert_array_equal(patches[1], mel[62:190])
    np.testing.assert_allclose(centers[0], 63.5 * 0.016)
    np.testing.assert_allclose(np.diff(centers), 62 * 0.016)


def test_short_signal_repeats_frames():
    mel = np.arange(50 * 96, dtype=np.float32).reshape(50, 96)
    patches, centers = musicnn.patches(mel)
    assert patches.shape == (1, 128, 96)
    np.testing.assert_array_equal(patches[0, 50:100], mel)
    assert len(centers) == 1


def test_filterbank_unit_tri():
    fb = musicnn.mel_filterbank()
    assert fb.shape == (96, 257)
    # unit_tri: площадь треугольника (в герцах) равна 1; у узких полос около 1 кГц
    # сумма по двум-трём бинам отличается от теоретической площади.
    areas = fb.sum(axis=1) * (8000 / 256)
    assert np.median(areas) == pytest.approx(1.0, abs=1e-3)
    np.testing.assert_allclose(areas[10:], 1.0, rtol=0.1)
    assert (fb >= 0).all()


def test_silence_is_zero():
    mel = musicnn.bands(np.zeros(16000, dtype=np.float32))
    assert np.abs(mel).max() == 0.0
