from __future__ import annotations

import numpy as np
import pytest

from dalywave_fetcher.verify import (
    ITEM_S,
    TRIM_ITEMS,
    Fingerprint,
    align,
    compare,
)

RNG = np.random.default_rng(42)


def random_print(n: int) -> np.ndarray:
    return RNG.integers(0, 2**32, size=n, dtype=np.uint64).astype(np.uint32)


def flip_bits(items: np.ndarray, rate: float) -> np.ndarray:
    """Портит долю `rate` бит — как перекодирование превью в MP3 и другой мастер."""
    noise = np.zeros(len(items), dtype=np.uint32)
    for bit in range(32):
        noise |= (RNG.random(len(items)) < rate).astype(np.uint32) << np.uint32(bit)
    return items ^ noise


def test_finds_offset_of_noisy_preview() -> None:
    track = random_print(1600)
    start = 700
    preview = flip_bits(track[start : start + 240].copy(), 0.12)
    result = align(preview, track)
    assert result is not None
    assert result.offset_items == start
    assert result.offset_s == pytest.approx(start * ITEM_S)
    assert result.similarity == pytest.approx(0.88, abs=0.03)
    assert result.baseline == pytest.approx(0.5, abs=0.02)


def test_unrelated_audio_stays_near_half() -> None:
    result = align(random_print(240), random_print(1600))
    assert result is not None
    assert 0.5 < result.similarity < 0.6


def test_preview_overhanging_the_end_still_aligns() -> None:
    track = random_print(1000)
    preview = np.concatenate([track[-230:], random_print(10)])
    result = align(preview, track)
    assert result is not None
    assert result.offset_items == 1000 - 230
    assert result.overlap >= int((240 - 2 * TRIM_ITEMS) * 0.9)


def test_too_short_track() -> None:
    assert align(random_print(240), random_print(50)) is None
    assert align(np.array([], dtype=np.uint32), random_print(50)) is None


def test_compare_applies_threshold() -> None:
    track = random_print(1200)
    good = Fingerprint(items=flip_bits(track[100:340].copy(), 0.1), duration_s=30.0)
    bad = Fingerprint(items=random_print(240), duration_s=30.0)
    full = Fingerprint(items=track, duration_s=150.0)
    assert compare(good, full).verified is True
    assert compare(bad, full).verified is False
    assert compare(good, full).to_json()["offsetS"] == pytest.approx(100 * ITEM_S, abs=0.01)
