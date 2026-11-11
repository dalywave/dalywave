"""Громкость, истинный пик и огибающая — на сгенерированном звуке."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import numpy as np
import pytest

from dalywave_fetcher.analyze import WAVEFORM_POINTS, analyze, envelope

needs_ffmpeg = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нужен ffmpeg")


def test_envelope_is_rms_normalised_to_the_loudest_part() -> None:
    quiet = np.zeros(4000, dtype=np.int16)
    loud = (np.sin(np.linspace(0, 400 * np.pi, 4000)) * 16000).astype(np.int16)
    wave = envelope(np.concatenate([quiet, loud]), points=8)
    assert len(wave) == 8
    assert wave[:4] == [0, 0, 0, 0]
    assert max(wave) == 1000
    assert all(900 <= v <= 1000 for v in wave[4:])


def test_envelope_of_silence_and_short_input() -> None:
    assert envelope(np.zeros(0, dtype=np.int16)) == [0] * WAVEFORM_POINTS
    assert envelope(np.zeros(100, dtype=np.int16), points=4) == [0, 0, 0, 0]
    # Отсчётов меньше, чем точек: пустые куски — нули, а не ошибка
    wave = envelope(np.array([1000, -1000], dtype=np.int16), points=4)
    assert len(wave) == 4 and max(wave) == 1000


@needs_ffmpeg
def test_analyze_opus_in_mp4(tmp_path: Path) -> None:
    # Секунда тишины, затем секунда синуса 440 Гц с амплитудой 0,5 (−6 dBFS)
    path = tmp_path / "a.m4a"
    subprocess.run(
        [
            "ffmpeg",
            "-nostdin",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc='if(lt(t,1),0,0.5*sin(2*PI*440*t))':s=48000:d=2",
            "-c:a",
            "libopus",
            "-b:a",
            "128k",
            "-f",
            "mp4",
            str(path),
        ],
        check=True,
    )
    result = analyze(path)
    # Тишина отсекается гейтом EBU R128: громкость — у синуса (−0,691 + 10·lg(0,125) ≈ −9,7)
    assert result.loudness_lufs is not None and -12.0 < result.loudness_lufs < -7.5
    assert result.true_peak_db is not None and -7.5 < result.true_peak_db < -4.0
    wave = result.waveform
    assert len(wave) == WAVEFORM_POINTS
    assert max(wave) == 1000
    assert max(wave[: WAVEFORM_POINTS // 2 - 20]) < 50, "первая половина — тишина"
    assert min(wave[WAVEFORM_POINTS // 2 + 20 : -20]) > 800
    assert result.to_json()["waveform"] == wave


@needs_ffmpeg
def test_analyze_reports_broken_files(tmp_path: Path) -> None:
    from dalywave_fetcher.analyze import AnalyzeError

    path = tmp_path / "broken.m4a"
    path.write_bytes(b"not audio at all")
    with pytest.raises(AnalyzeError):
        analyze(path)
