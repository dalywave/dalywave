#!/usr/bin/env python3
"""Генерирует тестовое аудио для спайка только средствами ffmpeg (никакой чужой музыки).

Наборы:
  gapless/<fmt>/partN.*  — один непрерывный лог-свип, разрезанный на 4 трека по точным
                           номерам сэмплов; на первом сэмпле каждого трека щелчок +0.5,
                           на последнем −0.5. При идеальной склейке на стыке стоит дублет.
  ref/ref_<rate>.wav     — эталон (float32) того же свипа целиком, для анализа стыков.
  demo/*                 — тональные «пьесы» для проверки кроссфейда и нормализации громкости.
  covers/*.png           — обложки (градиенты в цветах бренда).
  manifest.json          — альбомы для тестовой страницы: файлы, длительности, LUFS, пики.

Запуск: python3 tools/gen_audio.py [--out testdata]
"""

import argparse
import json
import math
import re
import subprocess
import sys
from pathlib import Path

FFMPEG = "ffmpeg"

# Границы треков в секундах — нарочно «некруглые», чтобы стыки не совпадали с кадрами кодеков.
BOUNDS_S = [0.0, 5.98623, 12.25901, 18.08277, 24.25]
SWEEP_F0, SWEEP_F1 = 110.0, 1760.0
SWEEP_AMP = 0.4
CLICK_AMP = 0.5

# (имя, частота эталона, расширение, аргументы кодека)
FORMATS = [
    ("opus_ogg", 48000, "opus", ["-c:a", "libopus", "-b:a", "160k"]),
    ("opus_mp4", 48000, "mp4", ["-c:a", "libopus", "-b:a", "160k", "-movflags", "+faststart"]),
    ("opus_webm", 48000, "webm", ["-c:a", "libopus", "-b:a", "160k"]),
    ("flac16_44", 44100, "flac", ["-c:a", "flac", "-sample_fmt", "s16"]),
    ("flac24_96", 96000, "flac", ["-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"]),
    ("mp3_320", 44100, "mp3", ["-c:a", "libmp3lame", "-b:a", "320k"]),
    ("aac_256", 44100, "m4a", ["-c:a", "aac", "-b:a", "256k", "-movflags", "+faststart"]),
    ("vorbis_q6", 44100, "ogg", ["-c:a", "vorbis", "-strict", "-2", "-q:a", "6"]),
]

COVERS = {
    "dusk": ("0x4B21FF", "0xFF3D7A"),
    "sunset": ("0xFF3D7A", "0xFF9A3C"),
    "night": ("0x10123A", "0x4B21FF"),
    "violet": ("0xB01CF0", "0x4B21FF"),
}


def run(args, capture=False):
    res = subprocess.run(args, capture_output=True, text=True)
    if res.returncode != 0:
        sys.stderr.write(res.stderr)
        raise SystemExit(f"ffmpeg failed: {' '.join(args)}")
    return res.stderr if capture else None


def bounds_samples(rate):
    return [round(b * rate) for b in BOUNDS_S]


def sweep_expr(rate):
    """Выражение aevalsrc: лог-свип + щелчки на первом и последнем сэмпле каждого трека."""
    total = BOUNDS_S[-1]
    k = math.log(SWEEP_F1 / SWEEP_F0)
    phase = f"2*PI*{SWEEP_F0}*{total}/{k}*(exp(t*{k}/{total})-1)"
    expr = f"{SWEEP_AMP}*sin({phase})"
    b = bounds_samples(rate)
    clicks = []
    for i in range(len(b) - 1):
        clicks.append(f"{CLICK_AMP}*eq(n,{b[i]})")
        clicks.append(f"-{CLICK_AMP}*eq(n,{b[i + 1] - 1})")
    return expr + "+" + "+".join(clicks)


def make_reference(out: Path, rate: int) -> Path:
    path = out / "ref" / f"ref_{rate}.wav"
    if path.exists():
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    total_samples = bounds_samples(rate)[-1]
    expr = sweep_expr(rate)
    # Стерео: правый канал — тот же сигнал со знаком минус, чтобы ловить перепутанные каналы.
    run([FFMPEG, "-y", "-hide_banner", "-loglevel", "error",
         "-f", "lavfi", "-i", f"aevalsrc=exprs='{expr}|-({expr})':s={rate}:d={BOUNDS_S[-1] + 0.01}",
         "-af", f"atrim=end_sample={total_samples}",
         "-c:a", "pcm_f32le", str(path)])
    return path


def loudness(path: Path):
    """Интегральная громкость (LUFS) и истинный пик (dBTP) через фильтр ebur128."""
    err = run([FFMPEG, "-hide_banner", "-nostats", "-i", str(path),
               "-af", "ebur128=peak=true", "-f", "null", "-"], capture=True)
    i = re.findall(r"I:\s+(-?[\d.]+) LUFS", err)
    p = re.findall(r"Peak:\s+(-?[\d.inf]+) dBFS", err)
    lufs = float(i[-1]) if i else None
    peak = float(p[-1]) if p and p[-1] not in ("-inf",) else None
    return lufs, peak


def make_gapless(out: Path, manifest: dict):
    for name, rate, ext, codec in FORMATS:
        ref = make_reference(out, rate)
        b = bounds_samples(rate)
        album = {
            "id": f"gapless-{name}",
            "title": f"Gapless · {name}",
            "artist": "DalyWave Test Signals",
            "cover": f"covers/{list(COVERS)[len(manifest['albums']) % len(COVERS)]}.png",
            "kind": "gapless",
            "format": name,
            "sourceRate": rate,
            "bounds": b,
            "tracks": [],
        }
        for i in range(len(b) - 1):
            dst = out / "gapless" / name / f"part{i + 1}.{ext}"
            dst.parent.mkdir(parents=True, exist_ok=True)
            if not dst.exists():
                run([FFMPEG, "-y", "-hide_banner", "-loglevel", "error", "-i", str(ref),
                     "-af", f"atrim=start_sample={b[i]}:end_sample={b[i + 1]},asetpts=PTS-STARTPTS",
                     "-ar", str(rate), *codec, "-map_metadata", "-1", str(dst)])
            lufs, peak = loudness(dst)
            album["tracks"].append({
                "file": str(dst.relative_to(out)),
                "title": f"Part {i + 1}",
                "frames": b[i + 1] - b[i],
                "durationMs": round((b[i + 1] - b[i]) * 1000 / rate),
                "lufs": lufs,
                "peakDb": peak,
            })
        manifest["albums"].append(album)
        print(f"gapless {name}: ok")


def note_expr(seq, bpm, wave="sin"):
    """Мелодия из синусов с затуханием: seq — частоты нот по долям."""
    beat = 60.0 / bpm
    n = len(seq)
    # Выбор ноты через сумму произведений индикаторов: без ветвлений в выражении.
    idx = f"mod(floor(t/{beat}),{n})"
    freq = "+".join(f"{f}*eq({idx},{i})" for i, f in enumerate(seq))
    env = f"exp(-3*mod(t,{beat})/{beat})"
    return f"({env})*(0.6*sin(2*PI*({freq})*t)+0.25*sin(4*PI*({freq})*t))"


def make_demo(out: Path, manifest: dict):
    pieces = [
        ("a", "Morning Lines", [261.63, 329.63, 392.0, 523.25, 392.0, 329.63], 112, 0.0, "opus"),
        ("b", "Quiet Hour", [220.0, 261.63, 329.63, 440.0, 329.63, 261.63], 84, -9.0, "opus"),
        ("c", "Bright Steps", [293.66, 369.99, 440.0, 587.33, 440.0, 369.99], 128, -3.0, "flac"),
    ]
    album = {"id": "demo", "title": "Demo pieces", "artist": "DalyWave Test Signals",
             "cover": "covers/sunset.png", "kind": "demo", "tracks": []}
    for key, title, seq, bpm, gain_db, ext in pieces:
        dst = out / "demo" / f"{key}.{ext}"
        dst.parent.mkdir(parents=True, exist_ok=True)
        if not dst.exists():
            expr = note_expr(seq, bpm)
            codec = ["-c:a", "libopus", "-b:a", "160k"] if ext == "opus" else ["-c:a", "flac", "-sample_fmt", "s16"]
            rate = 48000 if ext == "opus" else 44100
            run([FFMPEG, "-y", "-hide_banner", "-loglevel", "error",
                 "-f", "lavfi", "-i", f"aevalsrc=exprs='{expr}|{expr}':s={rate}:d=30",
                 "-af", f"volume={gain_db}dB,afade=t=in:d=0.02,afade=t=out:st=29.5:d=0.5",
                 *codec, "-map_metadata", "-1", str(dst)])
        lufs, peak = loudness(dst)
        album["tracks"].append({"file": str(dst.relative_to(out)), "title": title,
                                "durationMs": 30000, "lufs": lufs, "peakDb": peak})
    manifest["albums"].append(album)
    print("demo: ok")


def make_covers(out: Path):
    d = out / "covers"
    d.mkdir(parents=True, exist_ok=True)
    for name, (c0, c1) in COVERS.items():
        dst = d / f"{name}.png"
        if not dst.exists():
            run([FFMPEG, "-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi",
                 "-i", f"gradients=s=600x600:c0={c0}:c1={c1}:x0=0:y0=0:x1=600:y1=600:nb_colors=2",
                 "-frames:v", "1", str(dst)])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(Path(__file__).resolve().parent.parent / "testdata"))
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    make_covers(out)
    manifest = {"albums": []}
    make_gapless(out, manifest)
    make_demo(out, manifest)
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False))
    print(f"manifest: {out / 'manifest.json'}")


if __name__ == "__main__":
    main()
