import numpy as np

from dalywave_fetcher import clip_sync
from dalywave_fetcher.clip_sync import FPS, align, align_envelopes, envelope


def spikes(seconds: float, seed: int) -> np.ndarray:
    """Огибающая «музыки»: редкие удары разной силы на фоне шума."""
    rng = np.random.default_rng(seed)
    n = int(seconds * FPS)
    env = rng.normal(0, 0.2, n)
    hits = rng.choice(n, size=n // 12, replace=False)
    env[hits] += rng.uniform(2, 6, len(hits))
    return env.astype(np.float32)


def stretched(src: np.ndarray, start_s: float, rate: float, seconds: float) -> np.ndarray:
    """Кусок `src` с `start_s`, проигранный так, что секунда клипа = `rate` секунд трека."""
    x = np.arange(int(seconds * FPS))
    y = start_s * FPS + x * rate
    return np.interp(y, np.arange(len(src)), src).astype(np.float32)


def test_intro_and_stretch_are_found() -> None:
    track = spikes(200, 1)
    intro = spikes(15, 2)  # сценка перед песней: в треке её нет
    body = stretched(track, 3.0, 1.02, 150)
    video = np.concatenate([intro, body])
    a = align_envelopes(track, video)
    assert a is not None
    # Кадр клипа x (после вступления) — кадр трека 3 с + (x − 15 с) × 1,02
    expected_offset = 3000 - 15000 * 1.02
    assert abs(a.offset_ms - expected_offset) < 120
    assert abs(a.rate - 1.02) < 0.005
    assert a.windows >= clip_sync.MIN_WINDOWS


def test_exact_copy_starts_at_zero() -> None:
    track = spikes(180, 3)
    a = align_envelopes(track, track[: 170 * FPS].copy())
    assert a is not None
    assert abs(a.offset_ms) < 30
    assert abs(a.rate - 1.0) < 0.002


def test_other_song_is_not_aligned() -> None:
    assert align_envelopes(spikes(180, 4), spikes(170, 5)) is None


def test_envelope_from_audio_finds_offset() -> None:
    """От сырого звука: щелчки в треке и тот же звук в клипе после 4 с тишины."""
    rng = np.random.default_rng(7)
    sr = clip_sync.SAMPLE_RATE
    track = rng.normal(0, 0.01, sr * 90).astype(np.float32)
    for at in rng.choice(len(track) - 400, size=500, replace=False):
        track[at : at + 200] += rng.uniform(0.3, 0.9)
    video = np.concatenate([np.zeros(sr * 4, np.float32), track[sr * 10 :]])
    a = align(track, video)
    assert a is not None
    # Клип в 0 — трек на 10 с минус 4 с тишины
    assert abs(a.offset_ms - 6000) < 60
    assert envelope(np.zeros(10, np.float32)).size == 0
