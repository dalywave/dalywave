import shutil
import struct
import wave
from pathlib import Path

import numpy as np
import pytest

from dalywave_ml.audio import AudioError, _parse_wav, decode

needs_ffmpeg = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нет ffmpeg")


def write_wav(path: Path, data: np.ndarray, rate: int) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(data.shape[1])
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes((data * 32767).astype("<i2").tobytes())


def float_wav(pcm: np.ndarray, rate: int, data_size: int | None = None) -> bytes:
    channels = pcm.shape[1]
    payload = pcm.astype("<f4").tobytes()
    fmt = struct.pack("<HHIIHH", 3, channels, rate, rate * 4 * channels, 4 * channels, 32)
    size = len(payload) if data_size is None else data_size
    body = b"WAVE" + b"fmt " + struct.pack("<I", len(fmt)) + fmt
    body += b"LIST" + struct.pack("<I", 4) + b"INFO"
    body += b"data" + struct.pack("<I", size) + payload
    return b"RIFF" + struct.pack("<I", 0xFFFFFFFF) + body


def test_parse_wav_streamed_size():
    pcm = np.random.default_rng(0).uniform(-1, 1, (1000, 2)).astype(np.float32)
    # ffmpeg в канал пишет размер блока данных как 0xFFFFFFFF — читаем до конца.
    out, rate, channels = _parse_wav(float_wav(pcm, 44100, data_size=0xFFFFFFFF))
    assert (rate, channels) == (44100, 2)
    np.testing.assert_array_equal(out, pcm)


def test_parse_wav_rejects_garbage():
    with pytest.raises(AudioError):
        _parse_wav(b"not a wav at all")


@needs_ffmpeg
def test_decode_mean_downmix_and_rate(tmp_path):
    t = np.arange(44100) / 44100
    left = 0.5 * np.sin(2 * np.pi * 440 * t)
    right = np.zeros_like(left)
    path = tmp_path / "a.wav"
    write_wav(path, np.stack([left, right], axis=1), 44100)
    audio = decode(path)
    assert audio.sample_rate == 44100
    assert audio.channels == 2
    assert audio.codec == "pcm_s16le"
    # Среднее каналов, а не сумма и не «энергосберегающее» сведение ffmpeg -ac 1.
    np.testing.assert_allclose(audio.samples, left / 2, atol=1e-4)
    assert len(audio.at(16000)) == pytest.approx(16000, abs=2)
    assert len(audio.at(48000)) == pytest.approx(48000, abs=2)


@needs_ffmpeg
def test_decode_truncates_long_input(tmp_path):
    path = tmp_path / "long.wav"
    write_wav(path, np.zeros((44100 * 5, 1)), 44100)
    audio = decode(path, max_seconds=2)
    assert audio.duration_s == pytest.approx(2.0, abs=0.01)
    assert audio.truncated


@needs_ffmpeg
def test_decode_garbage_raises(tmp_path):
    path = tmp_path / "x.mp3"
    path.write_bytes(b"\x00" * 1000)
    with pytest.raises(AudioError):
        decode(path)
