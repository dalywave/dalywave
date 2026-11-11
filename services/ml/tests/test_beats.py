import numpy as np
import pytest

from dalywave_ml.beats import BORDER, CHUNK, log_mel, peaks, split_piece


def reference_starts(n: int) -> list[int]:
    # beat_this.inference.split_piece(avoid_short_end=True), переписанный дословно.
    starts = np.arange(-BORDER, n - BORDER, CHUNK - 2 * BORDER)
    if n > CHUNK - 2 * BORDER:
        starts[-1] = n - (CHUNK - BORDER)
    return [int(s) for s in starts]


@pytest.mark.parametrize("n", [1, 600, 1488, 1489, 1500, 2976, 2977, 7498, 12000])
def test_split_piece_matches_reference(n):
    pieces = split_piece(n)
    assert [p[0] for p in pieces] == reference_starts(n)
    for start, left, right in pieces:
        length = min(start + CHUNK, n) - max(start, 0) + left + right
        assert length == min(CHUNK, n + 2 * BORDER)


def test_chunks_cover_every_frame_once_after_borders():
    n = 7498
    covered = np.zeros(n, dtype=int)
    for start, _, _ in split_piece(n):
        covered[max(0, start + BORDER) : min(n, start + CHUNK - BORDER)] += 1
    assert (covered >= 1).all()


def test_peaks_local_maximum_and_threshold():
    logits = np.full(100, -5.0)
    logits[[10, 30, 31, 60]] = [2.0, 1.0, 1.0, -0.5]
    # 30 и 31 — плато из двух соседних кадров: сливаются в 30.5; 60 ниже порога.
    np.testing.assert_allclose(peaks(logits), [10.0, 30.5])


def test_peaks_suppressed_within_three_frames():
    logits = np.full(50, -5.0)
    logits[[20, 23]] = [3.0, 2.0]
    np.testing.assert_allclose(peaks(logits), [20.0])
    logits[24] = 2.5  # 24 дальше трёх кадров от 20 и выше 23
    np.testing.assert_allclose(peaks(logits), [20.0, 24.0])


def test_log_mel_frame_count_and_silence():
    fb = np.ones((513, 128), dtype=np.float32) / 513
    spect = log_mel(np.zeros(22050 * 3, dtype=np.float32), fb)
    assert spect.shape == (1 + 22050 * 3 // 441, 128)
    assert np.abs(spect).max() == 0.0
