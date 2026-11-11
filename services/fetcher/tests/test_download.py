from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

from dalywave_fetcher.config import Settings
from dalywave_fetcher.download import Downloader, classify_error, ffprobe_duration, remux


@pytest.mark.parametrize(
    ("message", "code"),
    [
        ("Sign in to confirm you’re not a bot. Use --cookies", "bot_check"),
        ("HTTP Error 403: Forbidden", "http_403"),
        ("HTTP Error 429: Too Many Requests", "rate_limited"),
        ("Requested format is not available. Use --list-formats", "format_unavailable"),
        ("mweb client https formats require a GVS PO Token which was not provided", "po_token"),
        ("Video unavailable. This video has been removed by the uploader", "unavailable"),
        ("The uploader has not made this video available in your country", "geo_blocked"),
        ("This video is not available in your country", "geo_blocked"),
        ("Sign in to confirm your age", "age_restricted"),
        ("Read timed out", "network"),
        ("something new", "unknown"),
    ],
)
def test_classify_error(message: str, code: str) -> None:
    assert classify_error(message) == code


def test_options_without_pot_disable_token_fetching(tmp_path: Path) -> None:
    opts = Downloader(Settings(data_dir=tmp_path)).options(tmp_path, "x")
    assert opts["extractor_args"]["youtube"] == {"fetch_pot": ["never"]}
    assert "youtubepot-bgutilhttp" not in opts["extractor_args"]
    assert opts["js_runtimes"] == {"deno": {}}
    assert "cookiefile" not in opts


def test_options_with_pot_clients_and_cookies(tmp_path: Path) -> None:
    cookies = tmp_path / "cookies-src.txt"
    cookies.write_text("# Netscape HTTP Cookie File\n")
    settings = Settings(
        data_dir=tmp_path / "data",
        pot_url="http://pot:4416",
        player_clients=("mweb",),
        cookies_file=cookies,
        deno_path="/usr/local/bin/deno",
    )
    opts = Downloader(settings).options(tmp_path, "x")
    assert opts["extractor_args"]["youtube"] == {"player_client": ["mweb"]}
    assert opts["extractor_args"]["youtubepot-bgutilhttp"] == {"base_url": ["http://pot:4416"]}
    assert opts["js_runtimes"] == {"deno": {"path": "/usr/local/bin/deno"}}
    # yt-dlp пишет в cookies при выходе — ему достаётся копия.
    assert Path(opts["cookiefile"]) == tmp_path / "data" / "cookies.txt"
    assert Path(opts["cookiefile"]).read_text() == cookies.read_text()


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нужен ffmpeg")
def test_remux_webm_opus_to_ogg_without_reencoding(tmp_path: Path) -> None:
    src = tmp_path / "a.webm"
    subprocess.run(
        [
            "ffmpeg",
            "-nostdin",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=3",
            "-c:a",
            "libopus",
            "-b:a",
            "96k",
            str(src),
        ],
        check=True,
    )
    dst, container = remux(src, "opus")
    assert container == "ogg"
    assert dst.suffix == ".opus"
    probe = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=format_name:stream=codec_name",
            "-of",
            "default=nw=1:nk=1",
            str(dst),
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()
    assert probe == ["opus", "ogg"]
    assert ffprobe_duration(dst) == pytest.approx(3.0, abs=0.1)
