from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

from dalywave_fetcher.config import Settings
from dalywave_fetcher.download import (
    Downloader,
    DownloadError,
    classify_error,
    ffprobe_duration,
    mime_type,
    remux,
    safe_basename,
    source_url,
)


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


def _probe(path: Path) -> list[str]:
    return subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=format_name:stream=codec_name",
            "-of",
            "default=nw=1:nk=1",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()


def _encode(dst: Path, codec: str, *extra: str) -> Path:
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
            codec,
            "-b:a",
            "96k",
            *extra,
            str(dst),
        ],
        check=True,
    )
    return dst


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нужен ffmpeg")
def test_remux_webm_opus_to_mp4_without_reencoding(tmp_path: Path) -> None:
    src = _encode(tmp_path / "a.webm", "libopus")
    dst, container = remux(src, "opus")
    assert container == "mp4"
    assert dst.suffix == ".m4a"
    assert _probe(dst) == ["opus", "mov,mp4,m4a,3gp,3g2,mj2"]
    assert ffprobe_duration(dst) == pytest.approx(3.0, abs=0.1)
    # faststart: оглавление (moov) — в начале файла, до данных (mdat)
    head = dst.read_bytes()[:4096]
    assert head[4:8] == b"ftyp"
    assert 0 <= head.find(b"moov") < head.find(b"mdat") or b"mdat" not in head


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="нужен ffmpeg")
def test_remux_aac_keeps_m4a(tmp_path: Path) -> None:
    src = _encode(tmp_path / "a.m4a", "aac")
    dst, container = remux(src, "aac")
    assert (container, dst.suffix) == ("mp4", ".m4a")
    assert dst != src
    assert _probe(dst)[0] == "aac"


def test_unknown_codec_is_not_remuxed(tmp_path: Path) -> None:
    with pytest.raises(DownloadError) as e:
        remux(tmp_path / "a.bin", "speex")
    assert e.value.code == "unsupported_codec"


def test_mime_types_name_the_codec() -> None:
    assert mime_type("mp4", "opus") == 'audio/mp4; codecs="Opus"'
    assert mime_type("mp4", "aac") == 'audio/mp4; codecs="mp4a.40.2"'
    assert mime_type("ogg", "opus") == 'audio/ogg; codecs="opus"'
    assert mime_type("mp3", "mp3") == "audio/mpeg"


def test_direct_links_and_safe_names() -> None:
    assert source_url("afkL3uLNeXA") == "https://www.youtube.com/watch?v=afkL3uLNeXA"
    url = "https://soundcloud.com/lowborn/light-me-up?si=1"
    assert source_url(url) == url
    assert safe_basename("afkL3uLNeXA") == "afkL3uLNeXA"
    name = safe_basename(url)
    assert name.startswith("url-") and "/" not in name and len(name) == 20
